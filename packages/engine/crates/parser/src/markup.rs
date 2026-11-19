//! Markup (02 §6's P7): `parser.yy`'s `full_markup`, `full_markup_list`,
//! `markup_mode_word` and the `markup_*` rules after them, read by recursive
//! descent in the lexer's markup mode, which `\markup` and `\markuplist` push
//! and their rules pop.
//!
//! A markup command is looked up by its name, `name-markup` and then
//! `name-markup-list`, in the innermost scope, as `lookup-markup-command`
//! looks it up in the current module (`lexer.rs`); a part's stub loads its
//! part first (D74). Its parameters are those `markup-command-signature`
//! gives ([`signature`]): a command whose last parameter is `markup?` is a
//! head (`markup_head_1_item`) of the markup or the markup list after it,
//! which `composed-markup-list` composes it with (`ly-syntax-constructors.scm`,
//! in `(lily)`); any other is a markup of its own
//! (`simple_markup_noword`), and a markup list command a list of one.
//! An argument of another predicate is Scheme, a string, music, or LilyPond
//! in braces, checked by its predicate as `check_scheme_arg` checks it:
//! one that fails is `wrong type for argument N`, and leaves the command's
//! arguments an improper list, on which the command's `scm_reverse_x`
//! throws. That Guile error escapes `ly:parse-file` and ends the run, which
//! prints it and no `failed files` (case 20, D64):
//! [`crate::Parse::guile_abort`].
//!
//! A `#` form or a `SCM_IDENTIFIER` is markup or a markup list by its value
//! (`markup_scm`). A word is a string; `\markup` and a word alone is
//! `markup_mode_word`, which an assignment `\markup name = …` defines a
//! markup command by. `\score { … }` and `\score-lines { … }` read a score
//! in notes mode: its music is not handed to the toplevel's handlers.
//!
//! P11 (MS2 step 34): markup cut short by `\etc` after a command's
//! arguments (`markup_partial_function`), whose commands `partial.rs` makes
//! a markup function of.
//!
//! Not yet: a `SCM_FUNCTION`'s call in markup mode; a score's default `\layout`
//! (`get_layout`), which [`crate::output_def::Score`] has no place for.

use crate::bison;
use crate::classify::{Env, Id};
use crate::diag::Diagnostics;
use crate::lexer::{Mode, Tok, Token};
use crate::syntax::{Music, MusicKind};
use crate::{NOTEMODE, Parsed, Parser, SCORE, Span, SyntaxError};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Tag, Value};

const M: Mode = Mode::Markup;
const N: Mode = Mode::Notes;

/// A markup command's parameter, as `push_markup_predicates` pushes its
/// token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expect {
    Markup,
    MarkupList,
    /// Any other predicate: `EXPECT_SCM`.
    Scm(Value),
}

impl Expect {
    /// Its token's name in Bison's messages.
    pub(crate) fn display(self) -> &'static str {
        match self {
            Expect::Markup => "markup?",
            Expect::MarkupList => "markup-list?",
            Expect::Scm(_) => "scheme?",
        }
    }
}

/// `(proc value)` in `env`'s scopes, None if `proc` is unbound or fails.
fn call(env: &mut Env<'_>, proc: &str, value: Value) -> Option<Value> {
    let proc = env.lookup(proc)?;
    env.vm().apply(proc, &[value]).ok()
}

/// The parameters `markup-command-signature` gives `command`; none for a
/// procedure that is no markup command.
pub(crate) fn signature(env: &mut Env<'_>, command: Value) -> Vec<Expect> {
    let Some(mut list) = call(env, "markup-command-signature", command) else {
        return Vec::new();
    };
    let markup = env.lookup("markup?");
    let markup_list = env.lookup("markup-list?");
    let mut params = Vec::new();
    let heap = env.vm_ref().heap();
    while let (Some(pred), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        params.push(match Some(pred) {
            p if p == markup => Expect::Markup,
            p if p == markup_list => Expect::MarkupList,
            _ => Expect::Scm(pred),
        });
        list = rest;
    }
    params
}

/// `markup-function?` and `markup-list-function?`: the token of a markup
/// command, by its value, or None for any other value.
pub(crate) fn command_kind(env: &mut Env<'_>, value: Value) -> Option<Id> {
    if !env.vm_ref().heap().is_procedure(value) {
        return None;
    }
    call(env, "markup-command-signature", value).filter(|&s| s != Value::FALSE)?;
    let list = call(env, "markup-list-function?", value).is_some_and(|l| l != Value::FALSE);
    Some(match list {
        true => Id::MarkupListFunction,
        false => Id::MarkupFunction,
    })
}

/// `lookup_markup_command` and then `lookup_markup_list_command` of
/// `name`: the command's kind and its procedure, kept. `%markup-procedure`
/// loads a part whose stub the name is bound to; an error loading it is
/// Guile's, written to `diags`, and the name is then no command.
pub(crate) fn lookup_command(
    env: &mut Env<'_>,
    name: &str,
    diags: &mut Diagnostics,
) -> Option<(Id, u32)> {
    let find = env.lookup("%markup-procedure");
    for (suffix, kind) in [
        ("-markup", Id::MarkupFunction),
        ("-markup-list", Id::MarkupListFunction),
    ] {
        let full = alloc::format!("{name}{suffix}");
        let found = match find {
            Some(find) => {
                let symbol = env.vm().heap_mut().intern(&full);
                let module = env.innermost();
                match env.vm().apply(find, &[module, symbol]) {
                    Ok(value) => Some(value),
                    Err(e) => {
                        let mut text = e.text(env.vm_ref().heap());
                        text.push('\n');
                        diags.output(&text);
                        diags.flush();
                        return None;
                    }
                }
            }
            None => env.lookup(&full),
        };
        if let Some(proc) = found
            && command_kind(env, proc) == Some(kind)
        {
            return Some((kind, env.keep(proc)));
        }
    }
    None
}

/// Where markup is read, which decides what may stand there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    /// `markup`: a markup command's `markup?` argument.
    Markup,
    /// `markup_list`: its `markup-list?` argument, `\markuplist`.
    List,
    /// `markup_top` and `markup_braced_list_body`: either.
    Either,
}

/// What a markup rule gave.
enum Piece {
    One(Value),
    /// A list of markups, as a Scheme list.
    List(Value),
}

/// What `\markup` gave: `markup_mode_word`, the word's text and its string,
/// or `markup_top`'s markup; or `markup_partial_function`'s commands, cut
/// short by an `\etc`, not taken, markup mode still pushed (`partial.rs`).
pub(crate) enum FullMarkup {
    Word(String, Value),
    Markup(Value),
    Partial(Value),
}

impl Parser<'_, '_> {
    /// `full_markup` after `\markup`: its markup, kept.
    pub(crate) fn full_markup(&mut self) -> Parsed<Value> {
        Ok(match self.markup_mode()? {
            FullMarkup::Word(_, value) | FullMarkup::Markup(value) => value,
            FullMarkup::Partial(_) => unreachable!("markup is cut short only with `etc`"),
        })
    }

    /// `markup_mode markup_top` or `markup_mode_word`, after `\markup`. The
    /// mode is popped by the rule's action, before the next token is lexed.
    pub(crate) fn markup_mode(&mut self) -> Parsed<FullMarkup> {
        self.markup_mode_etc(false)
    }

    /// [`Parser::markup_mode`], or with `etc`, `markup_partial_function`.
    pub(crate) fn markup_mode_etc(&mut self, etc: bool) -> Parsed<FullMarkup> {
        self.pushed += 1;
        let token = self.peek(M);
        let result = match token.kind {
            Tok::String | Tok::Symbol => {
                self.next(M);
                let (text, value) = self.markup_word(&token);
                Ok(FullMarkup::Word(text, value))
            }
            _ => match self.markup_piece_etc(Context::Either, etc) {
                Ok(Err(commands)) => return Ok(FullMarkup::Partial(commands)),
                Ok(Ok(piece)) => Ok(FullMarkup::Markup(match piece {
                    // `markup_top: markup_list`.
                    Piece::List(list) => self.line_markup(list),
                    Piece::One(markup) => markup,
                })),
                Err(e) => Err(e),
            },
        };
        self.leave_markup(result)
    }

    /// `full_markup_list` after `\markuplist`: its list, kept.
    pub(crate) fn full_markup_list(&mut self) -> Parsed<Value> {
        self.pushed += 1;
        let result = self.markup_piece(Context::List).map(|piece| match piece {
            Piece::List(list) | Piece::One(list) => list,
        });
        self.leave_markup(result)
    }

    /// Pops markup mode after `result`. After a syntax error no rule pops
    /// it: the rest of the file is lexed in markup mode
    /// ([`Parser::markup_stuck`]), the error's lookahead included.
    fn leave_markup<T>(&mut self, result: Parsed<T>) -> Parsed<T> {
        match result {
            Ok(value) => {
                self.pushed = self.pushed.saturating_sub(1);
                Ok(value)
            }
            Err(e) => {
                self.markup_stuck = true;
                Err(e)
            }
        }
    }

    /// A syntax error at `token` in markup.
    fn markup_error(&mut self, token: &Token) -> SyntaxError {
        self.syntax_error(token, bison::MANY)
    }

    /// `markup_word`: a string's or a word's text, and its string, kept.
    fn markup_word(&mut self, token: &Token) -> (String, Value) {
        let text = match token.kind {
            Tok::String => String::from(self.lexer.string(token)),
            _ => String::from_utf8_lossy(self.lexer.text_of(token.span)).into_owned(),
        };
        let value = self.env.vm().heap_mut().string(text.as_str());
        self.env.keep(value);
        (text, value)
    }

    /// `(line-markup list)`, kept.
    fn line_markup(&mut self, list: Value) -> Value {
        let line = self.procedure("line-markup").unwrap_or(Value::FALSE);
        self.list_of(&[line, list])
    }

    /// A list of `items`, kept.
    fn list_of(&mut self, items: &[Value]) -> Value {
        let list = self.env.vm().heap_mut().list(items);
        self.env.keep(list);
        list
    }

    /// The heads of a markup (`markup_head_1_list`), if any, and what they
    /// apply to; or a markup or a markup list alone, as `context` lets one
    /// stand.
    fn markup_piece(&mut self, context: Context) -> Parsed<Piece> {
        match self.markup_piece_etc(context, false)? {
            Ok(piece) => Ok(piece),
            Err(_) => unreachable!("markup is cut short only with `etc`"),
        }
    }

    /// [`Parser::markup_piece`], or with `etc`, `markup_partial_function`: a
    /// command whose arguments end at an `\etc`, after the heads read, a
    /// head's markup the argument cut. Its commands, `partial-markup`'s
    /// `((command argument …) head …)`, the last head first.
    fn markup_piece_etc(&mut self, context: Context, etc: bool) -> Parsed<Result<Piece, Value>> {
        // The heads, the first read first.
        let mut heads = Vec::new();
        loop {
            let token = self.peek(M);
            let piece = match token.kind {
                Tok::Identifier(Id::MarkupFunction) => {
                    self.next(M);
                    let command = self.env.value(token.value);
                    let params = signature(&mut self.env, command);
                    if let Some((Expect::Markup, rest)) = params.split_last() {
                        let head = self.markup_command_etc(command, rest, etc)?;
                        if etc && (head.1 || self.peek(M).kind == crate::partial::ETC) {
                            heads.push(head.0);
                            heads.reverse();
                            return Ok(Err(self.list_of(&heads)));
                        }
                        heads.push(head.0);
                        continue;
                    }
                    if context == Context::List {
                        return Err(self.markup_error(&token));
                    }
                    let (markup, partial) = self.markup_command_etc(command, &params, etc)?;
                    if partial {
                        heads.push(markup);
                        heads.reverse();
                        return Ok(Err(self.list_of(&heads)));
                    }
                    Piece::One(markup)
                }
                Tok::String | Tok::Symbol if context != Context::List => {
                    self.next(M);
                    Piece::One(self.markup_word(&token).1)
                }
                SCORE if context != Context::List => {
                    self.next(M);
                    let score = self.markup_score(token)?;
                    let proc = self.procedure("score-markup").unwrap_or(Value::FALSE);
                    Piece::One(self.list_of(&[proc, score]))
                }
                Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                    let piece = self.markup_scm()?;
                    match (&piece, context) {
                        (Piece::One(_), Context::List) | (Piece::List(_), Context::Markup) => {
                            return Err(self.markup_error(&token));
                        }
                        _ => piece,
                    }
                }
                Tok::Char(b'{') if context != Context::Markup => {
                    self.next(M);
                    Piece::List(self.braced_list()?)
                }
                Tok::Identifier(Id::MarkupListFunction) if context != Context::Markup => {
                    self.next(M);
                    let command = self.env.value(token.value);
                    let params = signature(&mut self.env, command);
                    let list = self.markup_command(command, &params)?;
                    Piece::List(self.list_of(&[list]))
                }
                Tok::ScoreLines if context != Context::Markup => {
                    self.next(M);
                    let score = self.markup_score(token)?;
                    let proc = self
                        .procedure("score-lines-markup-list")
                        .unwrap_or(Value::FALSE);
                    let lines = self.list_of(&[proc, score]);
                    Piece::List(self.list_of(&[lines]))
                }
                _ => return Err(self.markup_error(&token)),
            };
            return Ok(Ok(self.composed(heads, piece)));
        }
    }

    /// `composed-markup-list` of the heads read and `piece`: each head
    /// applied to each markup, the last head innermost.
    fn composed(&mut self, heads: Vec<Value>, piece: Piece) -> Piece {
        if heads.is_empty() {
            return piece;
        }
        let mut commands = heads;
        commands.reverse();
        let commands = self.list_of(&commands);
        let markups = match piece {
            Piece::One(markup) => self.list_of(&[markup]),
            Piece::List(list) => list,
        };
        let composed = self
            .constructor("composed-markup-list")
            .and_then(|proc| self.call_scheme(proc, &[commands, markups]))
            .unwrap_or(Value::NULL);
        match piece {
            Piece::One(_) => Piece::One(
                self.env
                    .vm_ref()
                    .heap()
                    .car(composed)
                    .unwrap_or(Value::FALSE),
            ),
            Piece::List(_) => Piece::List(composed),
        }
    }

    /// `markup_braced_list` after `{`: its markups and the markups of its
    /// lists, until `}`.
    fn braced_list(&mut self) -> Parsed<Value> {
        let mut items = Vec::new();
        loop {
            if self.peek(M).kind == Tok::Char(b'}') {
                self.next(M);
                return Ok(self.list_of(&items));
            }
            match self.markup_piece(Context::Either)? {
                Piece::One(markup) => items.push(markup),
                Piece::List(mut list) => {
                    let heap = self.env.vm_ref().heap();
                    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
                        items.push(item);
                        list = rest;
                    }
                }
            }
        }
    }

    /// A markup command's arguments by `params`, in order
    /// (`markup_command_list_arguments`), and the command they make:
    /// `(command arg …)`, kept. When an argument failed its predicate, the
    /// command's `scm_reverse_x` throws (case 20).
    fn markup_command(&mut self, command: Value, params: &[Expect]) -> Parsed<Value> {
        Ok(self.markup_command_etc(command, params, false)?.0)
    }

    /// [`Parser::markup_command`], or with `etc`, `markup_arglist_partial`:
    /// the arguments to an `\etc`, not taken, where one is due. Whether it
    /// was cut short.
    fn markup_command_etc(
        &mut self,
        command: Value,
        params: &[Expect],
        etc: bool,
    ) -> Parsed<(Value, bool)> {
        let mut args = Vec::with_capacity(params.len() + 1);
        let mut failed = false;
        let mut partial = false;
        for &param in params {
            if etc && self.peek(M).kind == crate::partial::ETC {
                partial = true;
                break;
            }
            let value = match param {
                Expect::Markup => match self.markup_piece(Context::Markup)? {
                    Piece::One(markup) | Piece::List(markup) => markup,
                },
                Expect::MarkupList => match self.markup_piece(Context::List)? {
                    Piece::One(list) | Piece::List(list) => list,
                },
                Expect::Scm(pred) => {
                    let (value, good) = self.markup_scheme_arg(pred, args.len() + 1)?;
                    failed |= !good;
                    value
                }
            };
            args.push(value);
        }
        if failed {
            return Err(self.reverse_abort(&args));
        }
        args.insert(0, command);
        Ok((self.list_of(&args), partial))
    }

    /// The Guile error that escapes `ly:parse-file` and ends the run: what
    /// `reverse!` throws for the arguments `args` of a markup command, last
    /// first and ending in `#f` after one failed (`check_scheme_arg`).
    fn reverse_abort(&mut self, args: &[Value]) -> SyntaxError {
        let heap = self.env.vm().heap_mut();
        let mut list = Value::FALSE;
        for &arg in args {
            list = heap.cons(arg, list);
        }
        self.env.keep(list);
        let thrown = self
            .procedure("reverse!")
            .map(|reverse| self.env.vm().apply(reverse, &[list]));
        let text = match thrown {
            Some(Err(e)) => e.text(self.env.vm_ref().heap()),
            _ => {
                let mut written = String::new();
                self.env.vm_ref().heap().write(list, &mut written);
                alloc::format!(
                    "In procedure reverse!: Wrong type argument in position 1: {written}"
                )
            }
        };
        self.guile_abort_in("ly:parse-file", &text);
        SyntaxError
    }

    /// A Guile error that escaped `ly:parse-file` from Scheme the grammar
    /// called unprotected, `text` its message: Guile prints it after
    /// `ERROR: In procedure` of its own procedure, and the run ends there
    /// ([`crate::Parse::guile_abort`]).
    pub(crate) fn guile_abort_with(&mut self, text: &str) {
        let subr = text
            .strip_prefix("In procedure ")
            .and_then(|t| t.split_once(':'))
            .map_or("ly:parse-file", |(subr, _)| subr);
        let subr = String::from(subr);
        self.guile_abort_in(&subr, text);
    }

    /// Guile's report of an error that ends the run, in the frame of `subr`.
    fn guile_abort_in(&mut self, subr: &str, text: &str) {
        self.lexer.diags.flush();
        self.lexer
            .diags
            .raw(alloc::format!("ERROR: In procedure {subr}:\n{text}\n"));
        self.guile_abort = true;
        self.aborted = true;
        self.pushed = 0;
    }

    /// An `EXPECT_SCM` argument, the `n`th: a `#` form or a
    /// `SCM_IDENTIFIER` (`embedded_scm`), a string, music, or LilyPond in
    /// braces; its value, and whether `pred` held of it, else reported.
    fn markup_scheme_arg(&mut self, pred: Value, n: usize) -> Parsed<(Value, bool)> {
        let token = self.peek(M);
        let value = match token.kind {
            Tok::ScmToken => {
                self.next(M);
                self.scheme_value(&token)
            }
            Tok::Identifier(Id::Scm | Id::Music) => {
                self.next(M);
                self.env.value(token.value)
            }
            Tok::String => {
                self.next(M);
                self.markup_word(&token).1
            }
            Tok::Char(b'{') => {
                self.next(M);
                return self.braced_lilypond(pred, n);
            }
            NOTEMODE => {
                // `mode_changed_music`, read in notes mode.
                self.ahead = Some((token, self.lex_mode(N)));
                let music = self.composite_music()?;
                self.music_value(&music)
            }
            _ => return Err(self.markup_error(&token)),
        };
        let at = self.from(&token);
        if self.holds(pred, value) {
            return Ok((value, true));
        }
        self.argument_error(at, n, pred, value);
        Ok((value, false))
    }

    /// `'{' embedded_lilypond '}'` as an `EXPECT_SCM` argument, after `{`,
    /// read in notes mode: nothing, a duration, or music (a pitch alone is
    /// a pitch). A value `pred` rejects is tried as the note it makes
    /// (`make_music_from_simple`), whose duration then becomes the default.
    fn braced_lilypond(&mut self, pred: Value, n: usize) -> Parsed<(Value, bool)> {
        self.pushed += 1;
        let first = self.peek(N);
        let mut musics = Vec::new();
        let mut value = None;
        // A duration alone, which a note made of it makes the default.
        let mut written = None;
        if matches!(first.kind, Tok::Unsigned | Tok::Identifier(Id::Duration)) {
            let duration = self.duration()?;
            let post = self.post_events()?;
            value = Some(match post.is_empty() {
                true => {
                    written = Some(duration);
                    self.duration_value(duration)
                }
                false => {
                    self.default_duration = duration;
                    let music = Music {
                        span: self.span_from(first.span),
                        kind: MusicKind::Rhythm { duration, post },
                    };
                    self.music_value(&music)
                }
            });
        } else {
            while self.peek(N).kind != Tok::Char(b'}') {
                if let Some(music) = self.music_embedded()? {
                    musics.push(music);
                }
            }
        }
        let span = self.span_from(first.span);
        let close = self.peek(N);
        if close.kind != Tok::Char(b'}') {
            return Err(self.syntax_error(&close, bison::MANY));
        }
        let value = match (value, &musics[..]) {
            (Some(value), _) => value,
            (None, []) => self.make_at("Music", span, &[]),
            (None, [one]) => match &one.kind {
                MusicKind::Note(note) if crate::assign::bare_pitch(note) => {
                    self.pitch_value(note.pitch)
                }
                _ => self.music_value(one),
            },
            (None, _) => {
                let music = Music {
                    span,
                    kind: MusicKind::Sequential(musics),
                };
                self.music_value(&music)
            }
        };
        // The conversion to a note needs the lexer still in notes mode.
        let mut good = self.holds(pred, value);
        let mut taken = value;
        if !good {
            let music = self.simple_music(value, span);
            if music != value && self.holds(pred, music) {
                if let Some(duration) = written {
                    self.default_duration = duration;
                }
                (good, taken) = (true, music);
            }
        }
        self.next(N);
        self.pushed = self.pushed.saturating_sub(1);
        if !good {
            self.argument_error(span, n, pred, value);
        }
        Ok((taken, good))
    }

    /// `markup_scm`: a `#` form or a `SCM_IDENTIFIER` as markup or a markup
    /// list by its value, the unspecified as an empty list; anything else
    /// is `not a markup`, and an empty string.
    fn markup_scm(&mut self) -> Parsed<Piece> {
        let token = self.next(M);
        let value = match token.kind {
            Tok::ScmToken => self.scheme_value(&token),
            _ => self.env.value(token.value),
        };
        let at = self.from(&token);
        if self.is_markup(value) {
            return Ok(Piece::One(value));
        }
        if self.is_markup_list(value) {
            return Ok(Piece::List(value));
        }
        if value == Value::UNSPECIFIED {
            return Ok(Piece::List(Value::NULL));
        }
        self.error(at, "not a markup");
        let empty = self.env.vm().heap_mut().string("");
        self.env.keep(empty);
        Ok(Piece::One(empty))
    }

    /// `Text_interface::is_markup`.
    pub(crate) fn is_markup(&mut self, value: Value) -> bool {
        match self.procedure("markup?") {
            Some(markup) => self.holds(markup, value),
            None => value.tag() == Tag::String,
        }
    }

    /// `Text_interface::is_markup_list`.
    fn is_markup_list(&mut self, value: Value) -> bool {
        self.procedure("markup-list?")
            .is_some_and(|list| self.holds(list, value))
    }

    /// The text a text handler is given for `value`, as the grammar's
    /// `embedded_scm_active` items make it: a markup in a list of its own,
    /// a markup list that is a pair as it is, else None.
    pub(crate) fn text_of(&mut self, value: Value) -> Option<Value> {
        if self.is_markup(value) {
            Some(self.one_text(value))
        } else if self.is_markup_list(value) && self.env.vm_ref().heap().car(value).is_some() {
            Some(value)
        } else {
            None
        }
    }

    /// `markup` in a list of its own (`ly_list`), kept, for a text handler.
    pub(crate) fn one_text(&mut self, markup: Value) -> Value {
        let list = self.env.vm().heap_mut().cons(markup, Value::NULL);
        self.env.keep(list);
        list
    }

    /// `\score { … }` or `\score-lines { … }` in markup, after its keyword:
    /// the score, read in notes mode. Its music is not a toplevel score's.
    fn markup_score(&mut self, head: Token) -> Parsed<Value> {
        let (_, score) = self.score_block(head)?;
        Ok(score)
    }

    /// `\markup name = value` after `\markup` and the word: a markup
    /// function defined as the markup command `name`
    /// (`define-markup-command-internal`, into the current module), or
    /// `Not a markup function` at the value.
    pub(crate) fn define_markup_command(&mut self, name: &str, value: Value, at: Span) {
        if !self.env.vm_ref().heap().is_procedure(value)
            || command_kind(&mut self.env, value) != Some(Id::MarkupFunction)
        {
            self.error(at, "Not a markup function");
            return;
        }
        let Some(define) = self.procedure("define-markup-command-internal") else {
            return;
        };
        let symbol = self.env.vm().heap_mut().intern(name);
        let module = self.env.innermost();
        let previous = self.env.vm().set_current_module(module);
        self.call_scheme(define, &[symbol, value, Value::FALSE]);
        self.env.vm().set_current_module(previous);
    }
}
