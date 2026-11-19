//! Recovery from syntax errors where LilyPond's `error` productions put it
//! (02 §3.4, §5.6), and what an error leaves for the toplevel handlers.
//!
//! Bison pops its stack to the innermost state that shifts `error`: the music
//! list being read (`music_list error`), a score's body once its items are
//! reduced (`score_body error`), or toplevel (`lilypond error`). A note, a
//! header, an assignment and a score's item have none, so an error in them
//! recovers at the production around them; that is [`SyntaxError`] returned
//! up to the function of that production, which calls [`Parser::resync`].
//! Bison then discards tokens until one can follow there. Only at toplevel
//! can the end of input follow: anywhere else Bison aborts the parse, and
//! nothing more is reported.

use crate::bison::{self, Expected};
use crate::diag::{At, Severity};
use crate::lexer::{Mode, Tok, Token};
use crate::reuse::Boundary;
use crate::syntax::{Item, Music, MusicKind};
use crate::{Parser, Span, SyntaxError};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::Score;
use lily_engine_scheme::{Exception, Value};

/// A `\score` block's state between its items (`output_def.rs`).
pub(crate) struct ScoreBody {
    pub(crate) head: Token,
    pub(crate) items: Vec<Item>,
    /// The score once there is one, the header before it, and its output
    /// definitions, which it is given at its end.
    pub(crate) score: Option<Value>,
    pub(crate) header: Value,
    pub(crate) defs: Vec<Value>,
    /// Whether it is a toplevel item, whose `scorify-music` may run in steps.
    pub(crate) toplevel: bool,
}

/// A toplevel `\score` stopped in its music's `scorify-music` (B23, D272):
/// the call goes on in the VM's steps, its frames rooted there, and the parse
/// goes on at `at`, where the music ended and its lookahead begins, with the
/// block's state, the music's first token and the music, its item once the
/// call is done. Only the score's first item stops so: nothing it holds
/// between steps but the call's music.
pub(crate) struct InScore {
    pub(crate) body: ScoreBody,
    pub(crate) token: Token,
    pub(crate) music: Music,
    pub(crate) at: Boundary,
}

/// What a call in steps came to: its value, none after an error it wrote,
/// or stopped where the parse goes on at that place.
pub(crate) enum Stepped {
    Done(Option<Value>),
    Paused(Boundary),
}

/// `music-check-error`, which `scorify-music` maps over the music: whether a
/// `music_list error` is anywhere in it.
pub(crate) fn error_found(music: &Music) -> bool {
    match &music.kind {
        MusicKind::Error => true,
        MusicKind::Sequential(elements) | MusicKind::Simultaneous(elements) => {
            elements.iter().any(error_found)
        }
        MusicKind::Context(context) => error_found(&context.music),
        MusicKind::Unrelativable(music) => error_found(music),
        _ => false,
    }
}

impl Parser<'_, '_> {
    /// `syntax error, unexpected X` at `token`, with what Bison lists as
    /// `expected` there, unless Bison is still recovering from the last one
    /// (`yyerrstatus`: three tokens must be shifted first). An error with no
    /// token shifted since the last discards `token`, as Bison does, and at
    /// the end of input aborts.
    pub(crate) fn syntax_error(&mut self, token: &Token, expected: Expected) -> SyntaxError {
        match self.recovering {
            0 if !self.aborted => {
                let message = bison::message(token.kind, expected);
                self.lexer.diags.error(At::Span(token.loc), message);
            }
            3 if token.kind == Tok::Eof => self.aborted = true,
            3 if self.ahead.is_some_and(|(ahead, _)| ahead == *token) => self.discard(),
            _ => {}
        }
        self.recovering = 3;
        SyntaxError
    }

    /// Recovers at an `error` production after a syntax error: discards
    /// tokens until one that `follows` it, and returns true; false when the
    /// end of input comes first and the parse is aborted. The tokens are
    /// read in `mode`.
    pub(crate) fn resync(&mut self, mode: Mode, follows: impl Fn(Tok) -> bool) -> bool {
        while !self.aborted {
            let token = self.peek(mode);
            if follows(token.kind) {
                return true;
            }
            if token.kind == Tok::Eof {
                self.aborted = true;
            } else {
                self.discard();
            }
        }
        false
    }

    /// The lexer's rule for the end of the main file: `Unfinished main
    /// input` when a mode is still pushed, at the end of input's token.
    pub(crate) fn end_of_input(&mut self, token: &Token) {
        if self.pushed > 0 {
            self.pushed = 0;
            self.lexer
                .diags
                .error(At::Span(token.loc), "Unfinished main input");
        }
    }

    /// A diagnostic of the grammar's actions (`parser_error`, a music's or
    /// a score's message), unless the parse was aborted before Bison would
    /// have reduced its rule.
    pub(crate) fn diagnose(&mut self, severity: Severity, at: At, message: impl Into<String>) {
        if self.aborted {
            return;
        }
        match severity {
            Severity::Error | Severity::Fatal => self.lexer.diags.error(at, message),
            Severity::Warning => self.lexer.diags.warning(at, message),
        }
    }

    /// `scorify-music` on `music` as a call in steps (B23, D272), in a parse
    /// that may stop for its step's time and can go on where the music
    /// ended: done within the step, or stopped there. `None` where it cannot
    /// stop, for [`Parser::scorify`].
    pub(crate) fn scorify_in_steps(&mut self, music: &Music) -> Option<Stepped> {
        if !self.returns() {
            return None;
        }
        let at = self.boundary(0)?;
        let scorify = self.procedure("scorify-music")?;
        let value = self.music_value(music);
        let due = lily_engine_music::returns::time_up;
        let result = self.in_scope(|vm| vm.begin_steps_due(scorify, &[value], due));
        let result = match result {
            Ok(None) => return Some(Stepped::Paused(at)),
            Ok(Some(value)) => Ok(value),
            Err(e) => Err(e),
        };
        Some(Stepped::Done(self.called(result)))
    }

    /// The call in steps of [`Parser::scorify_in_steps`] gone on with: its
    /// value, none after an error, or `Err` if it stopped again.
    pub(crate) fn scorify_on(&mut self) -> Result<Option<Value>, ()> {
        let result = self.in_scope(|vm| vm.go_on_steps());
        let result = match result {
            Ok(None) => return Err(()),
            Ok(Some(value)) => Ok(value),
            Err(e) => Err(e),
        };
        Ok(self.called(result))
    }

    /// What `scorify-music` gave, or none after an error: the score, as
    /// [`Parser::scorify`] makes it.
    pub(crate) fn scorified(&mut self, value: Option<Value>, header: Value) -> Value {
        let heap = self.env.vm().heap_mut();
        let score = value
            .filter(|&s| heap.foreign::<Score>(s).is_some())
            .unwrap_or_else(|| {
                let mut score = Score::new(Value::FALSE);
                score.error = true;
                heap.wrap(score)
            });
        if let Some(s) = heap.foreign_mut::<Score>(score)
            && header != Value::FALSE
        {
            s.header = header;
        }
        self.env.keep(score);
        score
    }

    /// `f` run on the VM with the innermost scope current, as
    /// [`Parser::call_scheme`] calls.
    fn in_scope<T>(&mut self, f: impl FnOnce(&mut lily_engine_scheme::Vm) -> T) -> T {
        let scope = self.env.innermost();
        self.publish();
        let previous = self.env.vm().set_current_module(scope);
        let result = f(self.env.vm());
        self.env.vm().set_current_module(previous);
        result
    }

    /// What a call to Scheme ended with, as [`Parser::call_scheme`] takes it.
    pub(crate) fn called(&mut self, result: Result<Value, Exception>) -> Option<Value> {
        self.take_template_tokens();
        self.env.define_taken();
        self.take_note_names();
        let here = self.here();
        crate::scheme::take_scheme_output(self.env.vm(), &mut self.lexer.diags, here);
        match result {
            Ok(value) => {
                self.env.keep(value);
                Some(value)
            }
            Err(e) => {
                let mut text = e.text(self.env.vm().heap());
                text.push('\n');
                self.lexer.diags.output(&text);
                self.lexer.error_level = true;
                None
            }
        }
    }

    /// `scorify-music` on `music`, in `\score`: a score of the music after
    /// `toplevel-music-functions`, with the header `header`. `Score::set_music`'s
    /// `errors found, ignoring music expression` at the music when a syntax
    /// error left an element of it, and the score has no music then.
    pub(crate) fn scorify(&mut self, music: &Music, header: Value) -> Value {
        let mut score = None;
        if error_found(music) {
            self.error(music.span, "errors found, ignoring music expression");
        } else {
            let value = self.music_value(music);
            score = match self.procedure("scorify-music") {
                Some(scorify) => self.call_scheme(scorify, &[value]),
                None => {
                    let mut score = Score::new(Value::FALSE);
                    score.music = value;
                    Some(self.env.vm().heap_mut().wrap(score))
                }
            };
        }
        self.scorified(score, header)
    }

    /// The handler `handler` of `declarations-init.ly` called with the music
    /// `music`, after the book `book` if there is one, as `parser.yy`'s
    /// actions call it (`toplevel-music-handler`, `book-music-handler`, …);
    /// unless a syntax error left an element of it, which is `errors found,
    /// ignoring music expression`, as `scorify-music` would find it.
    pub(crate) fn hand_music(&mut self, handler: &str, book: Option<Value>, music: &Music) {
        if error_found(music) {
            self.error(music.span, "errors found, ignoring music expression");
            return;
        }
        let value = self.music_value(music);
        self.hand(handler, book, value);
    }

    /// The handler `handler` called with `value`, after the book `book` if
    /// there is one. Without the init files there is none, and nothing is.
    pub(crate) fn hand(&mut self, handler: &str, book: Option<Value>, value: Value) {
        let Some(procedure) = self.procedure(handler) else {
            return;
        };
        match book {
            Some(book) => self.call_scheme(procedure, &[book, value]),
            None => self.call_scheme(procedure, &[value]),
        };
    }

    /// An error of the grammar's actions (`parser_error`), at `at`.
    pub(crate) fn error(&mut self, at: Span, message: impl Into<String>) {
        self.diagnose(Severity::Error, At::Span(at), message);
    }
}
