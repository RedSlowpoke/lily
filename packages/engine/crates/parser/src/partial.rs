//! P11, part 2 (02 §6; MS2 step 34): functions and markup cut short by
//! `\etc`, `parser.yy`'s `partial_function`, `partial_function_scriptable`,
//! `function_arglist_partial` and `partial_markup`.
//!
//! An assignment's value (`identifier_init: partial_function ETC`) that
//! starts as a call (a music, event or Scheme function's, `\override` or
//! `\set` to `=`, `\repeat` with its kind and count, or a direction) is
//! read as Bison reads it: its arguments one by one, until an `\etc` stands
//! where an argument is due (the rest of its parameters are then the new
//! function's), or until its last argument, which may itself be such a
//! call. Each call cut short is `(function . arguments)`, its arguments
//! last first, as Bison's argument lists are; a value read in full goes on
//! as the same tokens' ordinary reading. The calls are handed to
//! `ly-syntax-constructors.scm`'s `partial-music-function`, which makes the
//! music function the assignment defines; a direction before them chains
//! `create-script-function`, and before markup cut short,
//! `partial-text-script` (D93).
//!
//! Markup cut short (`partial_markup`): after `\markup`, the heads read and
//! a command whose arguments end at an `\etc`, which is taken; the commands
//! go to `partial-markup`, whose markup function an assignment defines, a
//! function takes as an argument, or `\markup name = …` defines a command
//! by. After a direction, the `\etc` is left to the assignment.

use crate::args::{Context, Got, N};
use crate::classify::Id;
use crate::lexer::{Tok, Token, keyword};
use crate::property::{OVERRIDE, SET};
use crate::repeat::REPEAT;
use crate::syntax::{Direction, Music, PostEvent, PostEventKind};
use crate::{MARKUP, Parsed, Parser, Span};
use alloc::vec::Vec;
use lily_engine_scheme::Value;

pub(crate) const ETC: Tok = Tok::Keyword(keyword("etc"));

/// A call's outcome.
pub(crate) enum Call {
    /// Its value, kept, and its span.
    Done(Value, Span),
    /// Cut short by `\etc`: its calls, outermost first.
    Partial(Vec<Value>),
}

/// Whether `kind` may start `partial_function`.
pub(crate) fn starts_partial(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Identifier(Id::MusicFunction | Id::EventFunction | Id::ScmFunction)
            | OVERRIDE
            | SET
            | REPEAT
            | Tok::Char(b'-' | b'^' | b'_')
    )
}

/// Whether `kind` may start a `partial_function` whose reading in full is
/// music, as `music_embedded` reads it.
fn starts_partial_music(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Identifier(Id::MusicFunction) | OVERRIDE | SET | REPEAT
    )
}

/// What an assignment's value cut short or not gave.
pub(crate) enum Init {
    Music(Music),
    Events(Vec<PostEvent>),
    Scheme(Value, Span),
    /// Cut short: the function made, and its span to the `\etc`.
    Partial(Value, Span),
}

impl Parser<'_, '_> {
    /// Takes the lookahead in the mode it was lexed in: an `\etc` that markup
    /// mode lexed.
    pub(crate) fn take_ahead(&mut self) -> Token {
        let mode = self.ahead.map_or(N, |(_, mode)| mode);
        self.next(mode)
    }

    /// `(function . arguments)` of a call cut short, the arguments `given`
    /// last first; their list ends in `#f`, not `'()`, when one was
    /// rejected (`check_scheme_arg`).
    pub(crate) fn partial_call(&mut self, function: Value, given: &[Value], good: bool) -> Value {
        let heap = self.env.vm().heap_mut();
        let mut args = if good { Value::NULL } else { Value::FALSE };
        for &arg in given {
            args = heap.cons(arg, args);
        }
        let call = heap.cons(function, args);
        self.env.keep(call);
        call
    }

    /// `MAKE_SYNTAX (name, span, args)`: the constructor `name` of `(lily)`
    /// called with `(*location*)` at `span`; its value, kept, or `#f`.
    fn make_syntax(&mut self, name: &str, span: Span, args: &[Value]) -> Value {
        let Some(proc) = self.constructor(name) else {
            return Value::FALSE;
        };
        let location = self.origin(span);
        let previous = self.env.vm().set_location(location);
        self.env.keep(previous);
        // The grammar's action calls it unprotected: an error escapes
        // `ly:parse-file` and ends the run, as `partial-music-function`'s
        // `caar` of a wrong path's `(#f)` does (OPEN F).
        let scope = self.env.innermost();
        self.publish();
        let module = self.env.vm().set_current_module(scope);
        let result = self.env.vm().apply(proc, args);
        self.env.vm().set_current_module(module);
        self.env.vm().set_location(previous);
        match result {
            Ok(value) => self.called(Ok(value)).unwrap_or(Value::FALSE),
            Err(e) => {
                self.called(Ok(Value::FALSE));
                let text = e.text(self.env.vm_ref().heap());
                self.guile_abort_with(&text);
                Value::FALSE
            }
        }
    }

    /// A list of `items`, kept.
    fn kept_list(&mut self, items: &[Value]) -> Value {
        let list = self.env.vm().heap_mut().list(items);
        self.env.keep(list);
        list
    }

    /// `identifier_init` at a token that [`starts_partial`]: the value read
    /// in full, or, cut short, `partial_function ETC`'s function. The
    /// `\etc` is taken.
    pub(crate) fn partial_init(&mut self) -> Parsed<Init> {
        let start = self.peek(N);
        let calls = match start.kind {
            kind if starts_partial_music(kind) => match self.partial_music()? {
                Ok(music) => return Ok(Init::Music(music)),
                Err(calls) => calls,
            },
            Tok::Identifier(Id::EventFunction) => match self.call(true)? {
                Call::Done(value, span) => {
                    let event = PostEvent {
                        span,
                        direction: Direction::Neutral,
                        kind: PostEventKind::Value(self.env.keep(value)),
                    };
                    return Ok(Init::Events(alloc::vec![event]));
                }
                Call::Partial(calls) => calls,
            },
            Tok::Identifier(Id::ScmFunction) => match self.call(true)? {
                Call::Done(value, span) => return Ok(Init::Scheme(value, span)),
                Call::Partial(calls) => calls,
            },
            _ => {
                let direction = self.next(N);
                return self.partial_script_init(direction);
            }
        };
        self.partial_function(calls, start)
    }

    /// [`Parser::partial_init`] after a direction, `start`, was taken.
    pub(crate) fn partial_script_init(&mut self, start: Token) -> Parsed<Init> {
        match self.partial_script(start)? {
            Ok(events) => Ok(Init::Events(events)),
            Err(calls) => self.partial_function(calls, start),
        }
    }

    /// `partial_function ETC`'s action: the `\etc` taken, and
    /// `partial-music-function` of `calls`, which began at `start`.
    fn partial_function(&mut self, calls: Vec<Value>, start: Token) -> Parsed<Init> {
        // Markup cut short after a direction left it lexed in markup mode.
        let etc = match self.ahead {
            Some((token, _)) => token,
            None => self.peek(N),
        };
        if etc.kind != ETC {
            return Err(self.syntax_error(&etc, crate::bison::MANY));
        }
        self.take_ahead();
        let span = self.span_from(start.span);
        // `scm_reverse_x`: the innermost call first.
        let mut calls = calls;
        calls.reverse();
        let list = self.kept_list(&calls);
        let function = self.make_syntax("partial-music-function", span, &[list]);
        Ok(Init::Partial(function, span))
    }

    /// A music function's call, `\override` or `\set` to `=`, or `\repeat`
    /// with its kind and count, cut short; else the music `music_embedded`
    /// reads there.
    fn partial_music(&mut self) -> Parsed<Result<Music, Vec<Value>>> {
        let start = self.peek(N);
        match start.kind {
            Tok::Identifier(Id::MusicFunction) => match self.call(true)? {
                Call::Done(value, span) => {
                    let music = self.music_of_value(value, span);
                    self.new_lyrics(music).map(Ok)
                }
                Call::Partial(calls) => Ok(Err(calls)),
            },
            REPEAT => match self.repeated_music_etc(true)? {
                Ok(music) => self.new_lyrics(music).map(Ok),
                Err(calls) => Ok(Err(calls)),
            },
            _ => match self.property_def_etc(true)? {
                Ok(kind) => {
                    let span = Span {
                        start: start.loc.start,
                        ..self.span_from(start.span)
                    };
                    Ok(Ok(Music { span, kind }))
                }
                Err(calls) => Ok(Err(calls)),
            },
        }
    }

    /// After `\override path =`, `\set path =` or `\repeat kind [count]`,
    /// with `etc`: an `\etc`, not taken, or a `partial_function`, whose
    /// calls are given after `call`'s; None when neither stands there, and
    /// the value is read as ever. A call that is read in full is the
    /// music of `music_embedded`.
    pub(crate) fn partial_after(
        &mut self,
        etc: bool,
        call: Value,
    ) -> Parsed<Option<Result<Music, Vec<Value>>>> {
        if !etc {
            return Ok(None);
        }
        let next = self.peek(N).kind;
        if next == ETC {
            return Ok(Some(Err(alloc::vec![call])));
        }
        if !starts_partial_music(next) {
            return Ok(None);
        }
        Ok(Some(match self.partial_music()? {
            Ok(music) => Ok(music),
            Err(inner) => {
                let mut calls = alloc::vec![call];
                calls.extend(inner);
                Err(calls)
            }
        }))
    }

    /// After a direction, `start`, just taken: what `partial_function`
    /// takes there, cut short, the `\etc` left; else the post-event read
    /// as ever.
    fn partial_script(&mut self, start: Token) -> Parsed<Result<Vec<PostEvent>, Vec<Value>>> {
        let direction = match start.kind {
            Tok::Char(b'^') => 1,
            Tok::Char(b'_') => -1,
            _ => 0,
        };
        let direction = self.env.vm().heap_mut().integer(direction);
        let next = self.peek(N);
        match next.kind {
            ETC => {
                let script = self.script_call(direction);
                Ok(Err(alloc::vec![script]))
            }
            Tok::Identifier(Id::MusicFunction | Id::EventFunction | Id::ScmFunction) => {
                match self.call(true)? {
                    Call::Done(value, span) => self.script_done(start, next, value, span).map(Ok),
                    Call::Partial(inner) => {
                        let mut calls = alloc::vec![self.script_call(direction)];
                        calls.extend(inner);
                        Ok(Err(calls))
                    }
                }
            }
            MARKUP => {
                self.next(N);
                match self.markup_mode_etc(true)? {
                    crate::markup::FullMarkup::Partial(commands) => {
                        // The rule pops markup mode; its `\etc` is lexed.
                        self.pushed = self.pushed.saturating_sub(1);
                        let at = self.span_from(next.span);
                        let markup = self.make_syntax("partial-markup", at, &[commands]);
                        let span = self.span_from(start.span);
                        let script = self.make_syntax("partial-text-script", span, &[markup]);
                        let call = self.kept_list(&[script, markup, direction]);
                        Ok(Err(alloc::vec![call]))
                    }
                    crate::markup::FullMarkup::Word(_, markup)
                    | crate::markup::FullMarkup::Markup(markup) => {
                        Ok(Ok(self.text_event(start, markup)))
                    }
                }
            }
            _ => self.after_direction(start).map(Ok),
        }
    }

    /// `(create-script-function direction)`, a call of a direction cut
    /// short.
    fn script_call(&mut self, direction: Value) -> Value {
        let function = self
            .constructor("create-script-function")
            .unwrap_or(Value::FALSE);
        self.kept_list(&[function, direction])
    }

    /// `gen_text_def`'s markup after a direction, `start`.
    fn text_event(&mut self, start: Token, markup: Value) -> Vec<PostEvent> {
        let event = PostEvent {
            span: self.span_from(start.span),
            direction: match start.kind {
                Tok::Char(b'^') => Direction::Up,
                Tok::Char(b'_') => Direction::Down,
                _ => Direction::Neutral,
            },
            kind: PostEventKind::Text(crate::syntax::Text::Markup(self.env.keep(markup))),
        };
        alloc::vec![event]
    }

    /// `partial_function` as a function's last argument, at a token that
    /// [`starts_partial`]: the argument read in full and checked as
    /// [`Parser::arguments`] checks it, or the calls cut short.
    pub(crate) fn partial_arg(
        &mut self,
        pred: Value,
        context: Context,
        n: usize,
    ) -> Parsed<Result<Got, Vec<Value>>> {
        let token = self.peek(N);
        match token.kind {
            kind if starts_partial_music(kind) => match self.partial_music()? {
                Ok(music) => {
                    let value = self.music_value(&music);
                    Ok(Ok(self.embedded(pred, context, n, value, &token)))
                }
                Err(calls) => Ok(Err(calls)),
            },
            Tok::Identifier(Id::EventFunction) => match self.call(true)? {
                Call::Done(value, span) => {
                    let event = PostEvent {
                        span,
                        direction: Direction::Neutral,
                        kind: PostEventKind::Value(self.env.keep(value)),
                    };
                    Ok(Ok(self.event_arg(pred, context, n, &[event], &token)))
                }
                Call::Partial(calls) => Ok(Err(calls)),
            },
            Tok::Identifier(Id::ScmFunction) => match self.call(true)? {
                Call::Done(value, _) => Ok(Ok(self.embedded(pred, context, n, value, &token))),
                Call::Partial(calls) => Ok(Err(calls)),
            },
            _ => {
                let direction = self.next(N);
                if direction.kind == Tok::Char(b'-')
                    && matches!(
                        self.peek(N).kind,
                        Tok::Unsigned | Tok::Real | Tok::Identifier(Id::Number)
                    )
                {
                    return self.minus_arg_after(direction, pred, context, n).map(Ok);
                }
                match self.partial_script(direction)? {
                    Ok(events) => Ok(Ok(self.event_arg(pred, context, n, &events, &direction))),
                    Err(calls) => Ok(Err(calls)),
                }
            }
        }
    }

    /// `partial_markup` after its markup was cut short: its `\etc` taken,
    /// markup mode popped, and `partial-markup` of `commands`, placed from
    /// `start`.
    pub(crate) fn partial_markup(&mut self, commands: Value, start: Span) -> Value {
        self.take_ahead();
        self.pushed = self.pushed.saturating_sub(1);
        let span = self.span_from(start);
        self.make_syntax("partial-markup", span, &[commands])
    }
}
