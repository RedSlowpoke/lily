//! A function's arguments (02 §3.2, §5.3): `parser.yy`'s 96 productions of
//! `function_arglist`, `function_arglist_backup`, `function_arglist_nonbackup`
//! and their `_reparse` and `_skip` rules, as one loop over the parameters,
//! left to right, which is the order Bison reads their tokens in.
//!
//! A parameter is read by its context: mandatory; optional with a mandatory
//! one after its run of optional parameters (Bison's backup rules), where a
//! value its predicate rejects is put back for that mandatory parameter
//! (`cursor.rs`) and the rest of the run takes its defaults; or optional at
//! the end (the nonbackup rules), where a value is checked as a mandatory
//! one's is. `\default` gives a parameter and the rest of its run their
//! defaults; the end of the arguments needs no token.
//!
//! An argument is read by its first token, and its readings are tried in
//! the rules' order, each a call of the predicate, so that a predicate with
//! effects sees what LilyPond's sees: a word as a string, a symbol list and
//! a symbol (`try_word_variants`); a pitch as the note of the default
//! duration, then the note or pitch written (`pitch_or_music`); an
//! `UNSIGNED` as a number, a symbol list, a duration and a note of that
//! duration. A reading the grammar ends with is checked once more
//! (`check_scheme_arg`); one that fails is `wrong type for argument N`, at
//! the argument, and the call fails.
//!
//! P7: `\markup …` as an argument is taken if the predicate holds of it,
//! else as the lyric event it makes (`make_music_from_simple`), else
//! checked or backed up as a `SCM_IDENTIFIER`; `\markuplist …` is Scheme.
//!
//! P8: in lyrics mode, a word, a string or markup whose lyric event the
//! predicate takes is pushed back as a `LYRIC_ELEMENT` and read as
//! `lyric_element_music`, with the duration and post-events after it.
//!
//! P11 (MS2 step 34): with `etc`, the arguments end at an `\etc` where an
//! argument is due, or at a last argument that is itself cut short
//! (`function_arglist_partial`, `partial.rs`); `\markup` cut short is an
//! argument (`partial_markup`).
//!
//! P12, brought forward (MS2 step 34): a `DURATION_IDENTIFIER` is read as
//! an `UNSIGNED`'s duration is.

use crate::bison;
use crate::classify::{Id, is_key};
use crate::function::Param;
use crate::lexer::{Mode, Tok, Token, keyword};
use crate::source::Span;
use crate::syntax::{Music, MusicKind, Text};
use crate::{LYRICMODE, MARKUP, MARKUPLIST, NOTEMODE, Parsed, Parser};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Tag, Value};

pub(crate) const N: Mode = Mode::Notes;
const DEFAULT: Tok = Tok::Keyword(keyword("default"));

/// How a parameter is read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    Mandatory,
    /// Optional, a mandatory parameter after its run: a rejected value
    /// backs up.
    Backup,
    /// Optional, at the end: a value is checked.
    Last,
}

/// What a call's arguments gave.
pub(crate) enum Args {
    /// All of them, or None when one was rejected, which was reported.
    Done(Option<Vec<Value>>),
    /// Cut short by `\etc` (`function_arglist_partial`): the arguments
    /// given before it, whether each was good, and the calls of a last
    /// argument that was itself cut short (`partial_function`), outermost
    /// first.
    Partial {
        given: Vec<Value>,
        good: bool,
        inner: Vec<Value>,
    },
}

/// What a parameter got.
pub(crate) enum Got {
    Value(Value),
    /// A value its predicate rejected, reported.
    Bad,
    /// Nothing: its argument was put back, and its run takes its defaults.
    BackedUp,
}

/// `is_regular_identifier`: letters (and any byte past ASCII), with `-` and
/// `_` between them, and with `multiple`, `.` and `,`.
pub(crate) fn is_regular_identifier(text: &str, multiple: bool) -> bool {
    let mut middle = false;
    for b in text.bytes() {
        if b.is_ascii_alphabetic() || b > 0x7f {
            middle = true;
        } else if middle && (b == b'-' || b == b'_' || multiple && (b == b'.' || b == b',')) {
            middle = false;
        } else {
            return false;
        }
    }
    middle
}

/// Whether `kind` starts `music_assign`, which `embedded_scm_arg` takes.
fn starts_music_arg(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | crate::context::NEWCONTEXT
            | Tok::AngleOpen
            | Tok::RestName
            | Tok::ChordRepetition
            | Tok::MultiMeasureRest
            | Tok::Identifier(Id::Music | Id::MusicFunction)
            | crate::repeat::TEMPO
    ) || crate::property::starts_property(kind)
        || crate::repeat::starts_basic(kind)
}

impl Parser<'_, '_> {
    /// The arguments of `params`: their values, kept, or None when one was
    /// rejected, which was reported; with `etc`, up to an `\etc` at a
    /// parameter, not taken, or to a last argument that a `\etc` cuts short
    /// (`partial.rs`).
    pub(crate) fn arguments_etc(&mut self, params: &[Param], etc: bool) -> Parsed<Args> {
        let mut values = Vec::with_capacity(params.len());
        let mut good = true;
        let mut i = 0;
        while i < params.len() {
            let param = params[i];
            if etc {
                let next = self.peek(N).kind;
                if next == crate::partial::ETC {
                    return Ok(Args::Partial {
                        given: values,
                        good,
                        inner: Vec::new(),
                    });
                }
                if i + 1 == params.len() && crate::partial::starts_partial(next) {
                    let context = match param.default {
                        None => Context::Mandatory,
                        Some(_) => Context::Last,
                    };
                    match self.partial_arg(param.pred, context, i + 1)? {
                        Ok(Got::Value(value)) => values.push(value),
                        Ok(_) => {
                            good = false;
                            values.push(Value::UNSPECIFIED);
                        }
                        Err(inner) => {
                            return Ok(Args::Partial {
                                given: values,
                                good,
                                inner,
                            });
                        }
                    }
                    break;
                }
            }
            // The end of the run of optional parameters from here.
            let end = params[i..]
                .iter()
                .position(|p| p.default.is_none())
                .map_or(params.len(), |k| i + k);
            let context = match param.default {
                None => Context::Mandatory,
                Some(_) if end < params.len() => Context::Backup,
                Some(_) => Context::Last,
            };
            if context != Context::Mandatory && self.peek(N).kind == DEFAULT {
                self.next(N);
                self.defaults(&params[i..end], &mut values);
                i = end;
                continue;
            }
            match self.argument(param.pred, context, i + 1)? {
                Got::Value(value) => values.push(value),
                Got::Bad => {
                    good = false;
                    values.push(Value::UNSPECIFIED);
                }
                Got::BackedUp => {
                    self.defaults(&params[i..end], &mut values);
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
        Ok(Args::Done(good.then_some(values)))
    }

    /// The defaults of skipped parameters, music copied (`loc_on_copy`).
    fn defaults(&mut self, params: &[Param], values: &mut Vec<Value>) {
        for param in params {
            let value = self.copy(param.default.unwrap_or(Value::FALSE));
            values.push(value);
        }
    }

    /// `check_scheme_arg`: `value` if `pred` holds of it; else the error,
    /// at `at`, with `shown` as what was found.
    fn check(
        &mut self,
        pred: Value,
        n: usize,
        value: Value,
        at: Span,
        shown: Option<Value>,
    ) -> Got {
        if self.holds(pred, value) {
            return Got::Value(value);
        }
        self.argument_error(at, n, pred, shown.unwrap_or(value));
        Got::Bad
    }

    /// The place of an argument that began with `first`: from where
    /// LilyPond locates that token to the last token taken.
    pub(crate) fn from(&self, first: &Token) -> Span {
        Span {
            start: first.loc.start,
            ..self.span_from(first.span)
        }
    }

    /// One parameter's argument, by its first token.
    fn argument(&mut self, pred: Value, context: Context, n: usize) -> Parsed<Got> {
        let token = self.peek(N);
        let got = match token.kind {
            // `embedded_scm_bare_arg`'s `context_modification`.
            crate::context::WITH => {
                let with = self.context_modification()?;
                let value = self.env.value(with.value);
                self.embedded(pred, context, n, value, &token)
            }
            // `context_def_spec_block`, or music.
            crate::context::CONTEXT => {
                let head = self.next(N);
                let value = match self.context_def_or_music(head)? {
                    crate::context::DefOrMusic::Def(_, value) => value,
                    crate::context::DefOrMusic::Music(music) => self.music_value(&music),
                };
                self.embedded(pred, context, n, value, &token)
            }
            Tok::ScmToken
            | Tok::ScmArg
            | Tok::Fraction
            | Tok::Identifier(Id::Lookup | Id::Module | Id::ScmFunction) => {
                let value = self.scheme_arg()?;
                self.embedded(pred, context, n, value, &token)
            }
            kind if starts_music_arg(kind) => {
                let music = self.music_embedded()?;
                let value = match music {
                    Some(music) => self.music_value(&music),
                    None => Value::UNSPECIFIED,
                };
                self.embedded(pred, context, n, value, &token)
            }
            // `full_markup`, or `embedded_scm_bare_arg`'s `partial_markup`.
            MARKUP => {
                self.next(N);
                match self.markup_mode_etc(true)? {
                    crate::markup::FullMarkup::Word(_, markup)
                    | crate::markup::FullMarkup::Markup(markup) => {
                        self.markup_arg(pred, context, n, markup, &token)?
                    }
                    crate::markup::FullMarkup::Partial(commands) => {
                        let value = self.partial_markup(commands, token.span);
                        self.embedded(pred, context, n, value, &token)
                    }
                }
            }
            MARKUPLIST => {
                self.next(N);
                let list = self.full_markup_list()?;
                self.embedded(pred, context, n, list, &token)
            }
            Tok::Identifier(Id::Scm) | Tok::String | Tok::Symbol => self.word(pred, context, n)?,
            Tok::NotenamePitch | Tok::Identifier(Id::Pitch) => self.pitch_arg(pred, context, n)?,
            Tok::Unsigned => self.unsigned_arg(pred, context, n)?,
            Tok::Identifier(Id::Duration) => {
                let token = self.next(N);
                let value = self.env.value(token.value);
                match self.duration_readings(pred, n, token, value)? {
                    Some(got) => got,
                    None if context == Context::Backup => {
                        self.put_back(&[token]);
                        Got::BackedUp
                    }
                    None => self.check(pred, n, value, token.loc, None),
                }
            }
            Tok::Real | Tok::Identifier(Id::Number) => {
                self.next(N);
                let value = self.number_value(&token);
                self.number_arg(pred, context, n, value, &token)
            }
            Tok::Char(b'-') => self.minus_arg(pred, context, n)?,
            Tok::Char(b'^' | b'_' | b':')
            | Tok::EUnsigned
            | Tok::Identifier(Id::Event | Id::EventFunction) => {
                let events = self.post_event()?.unwrap_or_default();
                self.event_arg(pred, context, n, &events, &token)
            }
            _ => {
                let expected = if context == Context::Backup {
                    bison::BACKED_UP
                } else {
                    bison::ARGUMENT
                };
                return Err(self.syntax_error(&token, expected));
            }
        };
        Ok(got)
    }

    /// `embedded_scm_bare_arg` but music: a `#` form's value, an argument
    /// put back, a fraction, a lookup, or a Scheme function's call.
    pub(crate) fn scheme_arg(&mut self) -> Parsed<Value> {
        let token = self.peek(N);
        Ok(match token.kind {
            Tok::ScmToken => {
                self.next(N);
                self.scheme_value(&token)
            }
            Tok::ScmArg => {
                self.next(N);
                self.env.value(token.value)
            }
            Tok::Fraction => {
                self.next(N);
                let value = self.fraction(&token);
                self.env.keep(value);
                value
            }
            Tok::Identifier(Id::ScmFunction) => self.function_call()?.0,
            _ => self.lookup()?,
        })
    }

    /// `embedded_scm_arg`: taken if `pred` holds; else checked again, or
    /// backed up as an `SCM_ARG`.
    pub(crate) fn embedded(
        &mut self,
        pred: Value,
        context: Context,
        n: usize,
        value: Value,
        first: &Token,
    ) -> Got {
        let at = self.from(first);
        if self.holds(pred, value) {
            return Got::Value(value);
        }
        if context == Context::Backup {
            let token = self.value_token(Tok::ScmArg, value, at);
            self.put_back(&[token]);
            return Got::BackedUp;
        }
        self.check(pred, n, value, at, None)
    }

    /// `full_markup` as an argument: the markup if `pred` holds of it, else
    /// the lyric event it makes (`LYRIC_ELEMENT`); else, after optional
    /// parameters, backed up as a `SCM_IDENTIFIER`, or checked.
    pub(crate) fn markup_arg(
        &mut self,
        pred: Value,
        context: Context,
        n: usize,
        markup: Value,
        first: &Token,
    ) -> Parsed<Got> {
        let at = self.from(first);
        if self.holds(pred, markup) {
            return Ok(Got::Value(markup));
        }
        if context != Context::Backup {
            let music = self.simple_music(markup, at);
            if music != markup && self.holds(pred, music) {
                return self.lyric_element_arg(pred, n, markup, first);
            }
        }
        Ok(self.embedded(pred, context, n, markup, first))
    }

    /// `try_string_variants`: the value, a key as a list of it, a string as
    /// a list of its symbol, then as its symbol; None when `pred` holds of
    /// none.
    pub(crate) fn string_variants(&mut self, pred: Value, value: Value) -> Option<Value> {
        if self.holds(pred, value) {
            return Some(value);
        }
        if is_key(self.env.vm_ref().heap(), value) {
            let list = self.env.vm().heap_mut().list(&[value]);
            self.env.keep(list);
            return self.holds(pred, list).then_some(list);
        }
        let heap = self.env.vm().heap_mut();
        let text = String::from(heap.str(value)?.as_str());
        let symbol = heap.intern(&text);
        let list = heap.list(&[symbol]);
        self.env.keep(list);
        if self.holds(pred, list) {
            return Some(list);
        }
        self.holds(pred, symbol).then_some(symbol)
    }

    /// `try_word_variants`: the word as a string, then split at `.` and
    /// `,` as a symbol list, then as its one symbol.
    fn word_variants(&mut self, pred: Value, value: Value, text: &str) -> Option<Value> {
        if self.holds(pred, value) {
            return Some(value);
        }
        if !is_regular_identifier(text, true) {
            return None;
        }
        let heap = self.env.vm().heap_mut();
        let symbols: Vec<Value> = text.split(['.', ',']).map(|s| heap.intern(s)).collect();
        let list = heap.list(&symbols);
        self.env.keep(list);
        if self.holds(pred, list) {
            return Some(list);
        }
        match symbols[..] {
            [symbol] => self.holds(pred, symbol).then_some(symbol),
            _ => None,
        }
    }

    /// `symbol_list_arg` after a symbol list a reading gave: more parts
    /// after `.` or `,` are appended.
    fn symbol_list_arg(&mut self, list: Value) -> Parsed<Value> {
        if !matches!(self.peek(N).kind, Tok::Char(b'.' | b',')) {
            return Ok(list);
        }
        self.next(N);
        let keys = self.property_path()?;
        let mut items = Vec::new();
        let heap = self.env.vm().heap();
        let mut p = list;
        while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            items.push(item);
            p = rest;
        }
        for key in &keys {
            items.push(self.scm_key(key));
        }
        let list = self.env.vm().heap_mut().list(&items);
        self.env.keep(list);
        Ok(list)
    }

    /// A `SCM_IDENTIFIER`, a `STRING` or a `SYMBOL`.
    fn word(&mut self, pred: Value, context: Context, n: usize) -> Parsed<Got> {
        let token = self.next(N);
        let (value, text) = match token.kind {
            Tok::Identifier(_) => (self.env.value(token.value), None),
            _ => {
                // A word of lyrics mode has its `_` made spaces.
                let text = match token.kind {
                    Tok::String => String::from(self.lexer.string(&token)),
                    _ if self.lex_mode(N) == Mode::Lyrics => {
                        String::from(self.lexer.string(&token))
                    }
                    _ => String::from_utf8_lossy(self.lexer.text_of(token.span)).into_owned(),
                };
                let value = self.env.vm().heap_mut().string(text.as_str());
                self.env.keep(value);
                (value, Some(text))
            }
        };
        let at = token.loc;
        let found = match (&text, token.kind) {
            (Some(text), Tok::Symbol) => self.word_variants(pred, value, text),
            _ => self.string_variants(pred, value),
        };
        if let Some(found) = found {
            // Any pair is read on as a symbol list, as LilyPond reads it.
            if found.tag() == Tag::Pair {
                let list = self.symbol_list_arg(found)?;
                let at = Span {
                    start: at.start,
                    ..self.span_from(token.span)
                };
                return Ok(self.check(pred, n, list, at, None));
            }
            return Ok(match context {
                Context::Backup => Got::Value(found),
                _ => self.check(pred, n, found, at, None),
            });
        }
        if context == Context::Backup {
            // A word backs up as a `STRING`.
            let back = match (token.kind, text) {
                (Tok::Symbol, Some(text)) => Token {
                    kind: Tok::String,
                    value: self.lexer.add_string(text),
                    ..token
                },
                _ => token,
            };
            self.put_back(&[back]);
            return Ok(Got::BackedUp);
        }
        // The lyric element reading, which a note made of the word reaches.
        let music = self.simple_music(value, at);
        if self.holds(pred, music) {
            if music != value && self.lex_mode(N) == Mode::Lyrics {
                return self.lyric_element_arg(pred, n, value, &token);
            }
            return Ok(self.check(pred, n, music, at, None));
        }
        Ok(self.check(pred, n, value, at, None))
    }

    /// `REPARSE lyric_element_music` after a `LYRIC_ELEMENT` was pushed
    /// for `text`, a word, a string or markup whose lyric event `pred`
    /// holds of, at `first`: the lyric event of the duration and the
    /// post-events after it, checked.
    fn lyric_element_arg(
        &mut self,
        pred: Value,
        n: usize,
        text: Value,
        first: &Token,
    ) -> Parsed<Got> {
        let text = Text::Markup(self.env.keep(text));
        let kind = self.lyric_after(text)?;
        let span = self.from(first);
        let music = self.music_value(&Music { span, kind });
        Ok(self.check(pred, n, music, span, None))
    }

    /// A pitch with its octave marks: the note it makes, then what
    /// `pitch_or_music` reads after it, or the pitch.
    fn pitch_arg(&mut self, pred: Value, context: Context, n: usize) -> Parsed<Got> {
        let token = self.next(N);
        let (octaves, _) = self.quotes();
        let pitch = self.pitch_of(&token).up(octaves);
        let value = self.pitch_value(pitch);
        let note = self.simple_music(value, self.from(&token));
        if self.holds(pred, note) {
            let note = self.note_after(pitch)?;
            let at = self.from(&token);
            let written = if crate::assign::bare_pitch(&note) {
                value
            } else {
                let music = Music {
                    span: at,
                    kind: MusicKind::Note(note),
                };
                self.music_value(&music)
            };
            if self.holds(pred, written) {
                return Ok(Got::Value(written));
            }
            let shown = (context != Context::Backup).then_some(written);
            let music = self.simple_music(written, at);
            return Ok(self.check(pred, n, music, at, shown));
        }
        let at = self.from(&token);
        if context != Context::Backup {
            return Ok(self.check(pred, n, value, at, None));
        }
        if self.holds(pred, value) {
            return Ok(Got::Value(value));
        }
        let back = self.value_token(Tok::Identifier(Id::Pitch), value, at);
        self.put_back(&[back]);
        Ok(Got::BackedUp)
    }

    /// An `UNSIGNED`: a number with its unit, a symbol list, a duration, the
    /// note of a duration.
    fn unsigned_arg(&mut self, pred: Value, context: Context, n: usize) -> Parsed<Got> {
        let token = self.next(N);
        let value = self.number_value(&token);
        if self.holds(pred, value) {
            let value = self.bare_number(value);
            let at = self.from(&token);
            return Ok(self.check(pred, n, value, at, None));
        }
        let list = self.env.vm().heap_mut().list(&[value]);
        self.env.keep(list);
        if self.holds(pred, list) {
            let list = self.symbol_list_arg(list)?;
            let at = self.from(&token);
            return Ok(self.check(pred, n, list, at, None));
        }
        if let Some(log) = crate::notes::duration_log(self.unsigned(&token)) {
            let duration = self.duration_value(crate::syntax::Duration::new(log.into(), 0));
            if let Some(got) = self.duration_readings(pred, n, token, duration)? {
                return Ok(got);
            }
        }
        if context == Context::Backup {
            self.put_back(&[token]);
            return Ok(Got::BackedUp);
        }
        Ok(self.check(pred, n, value, token.loc, None))
    }

    /// The readings of a duration, `duration`, whose `UNSIGNED` or
    /// `DURATION_IDENTIFIER` `token` was just taken: the duration written,
    /// with its dots and multipliers, or the note of it
    /// (`reparsed_rhythm`), with its post-events, its duration the default;
    /// None when `pred` holds of neither.
    fn duration_readings(
        &mut self,
        pred: Value,
        n: usize,
        token: Token,
        duration: Value,
    ) -> Parsed<Option<Got>> {
        if self.holds(pred, duration) {
            let written = self.duration_from(token)?;
            let value = self.duration_value(written);
            let at = self.from(&token);
            return Ok(Some(self.check(pred, n, value, at, None)));
        }
        let note = self.simple_music(duration, self.from(&token));
        if !self.holds(pred, note) {
            return Ok(None);
        }
        let written = self.duration_from(token)?;
        self.default_duration = written;
        let post = self.post_events()?;
        let at = self.from(&token);
        let music = Music {
            span: at,
            kind: MusicKind::Rhythm {
                duration: written,
                post,
            },
        };
        let value = self.music_value(&music);
        Ok(Some(self.check(pred, n, value, at, None)))
    }

    /// A `REAL` or a `NUMBER_IDENTIFIER`, just taken.
    fn number_arg(
        &mut self,
        pred: Value,
        context: Context,
        n: usize,
        value: Value,
        first: &Token,
    ) -> Got {
        let real = first.kind == Tok::Real;
        if context == Context::Backup {
            if !self.holds(pred, value) {
                self.put_back(&[*first]);
                return Got::BackedUp;
            }
            if !real {
                return Got::Value(value);
            }
        }
        let value = if real { self.bare_number(value) } else { value };
        let at = self.from(first);
        self.check(pred, n, value, at, None)
    }

    pub(crate) fn negated(&mut self, value: Value) -> Value {
        let heap = self.env.vm().heap_mut();
        let minus = heap.integer(-1);
        let value = heap.mul(value, minus).unwrap_or(value);
        self.env.keep(value);
        value
    }

    /// `-` and a number, or a post-event after it.
    fn minus_arg(&mut self, pred: Value, context: Context, n: usize) -> Parsed<Got> {
        let minus = self.next(N);
        self.minus_arg_after(minus, pred, context, n)
    }

    /// [`Parser::minus_arg`] after its `-`, `minus`, was taken.
    pub(crate) fn minus_arg_after(
        &mut self,
        minus: Token,
        pred: Value,
        context: Context,
        n: usize,
    ) -> Parsed<Got> {
        let number = self.peek(N);
        if !matches!(
            number.kind,
            Tok::Unsigned | Tok::Real | Tok::Identifier(Id::Number)
        ) {
            let events = self.after_direction(minus)?;
            return Ok(self.event_arg(pred, context, n, &events, &minus));
        }
        self.next(N);
        let written = self.number_value(&number);
        let value = self.negated(written);
        Ok(match number.kind {
            Tok::Unsigned => {
                if context != Context::Last && self.holds(pred, value) {
                    let value = self.bare_number(value);
                    return Ok(self.check(pred, n, value, number.loc, None));
                }
                if context == Context::Last && self.holds(pred, value) {
                    return Ok(Got::Value(value));
                }
                let digit = self.unsigned(&number).unwrap_or(u64::MAX) as u32;
                let fingering = self.event_value(&crate::syntax::PostEvent {
                    span: number.span,
                    direction: crate::syntax::Direction::Neutral,
                    kind: crate::syntax::PostEventKind::Fingering(digit),
                });
                match context {
                    Context::Mandatory => {
                        if self.holds(pred, fingering) {
                            self.check(pred, n, fingering, number.loc, None)
                        } else {
                            self.check(pred, n, written, number.loc, None)
                        }
                    }
                    Context::Backup => {
                        if self.holds(pred, fingering) {
                            Got::Value(fingering)
                        } else {
                            self.put_back(&[minus, number]);
                            Got::BackedUp
                        }
                    }
                    Context::Last => self.check(pred, n, fingering, minus.loc, Some(value)),
                }
            }
            Tok::Real => match context {
                Context::Mandatory => self.check(pred, n, value, number.loc, None),
                Context::Last => self.check(pred, n, value, minus.loc, None),
                Context::Backup if self.holds(pred, value) => {
                    let value = self.bare_number(value);
                    self.check(pred, n, value, number.loc, None)
                }
                Context::Backup => {
                    let back = self.value_token(Tok::Identifier(Id::Number), value, number.loc);
                    self.put_back(&[back]);
                    Got::BackedUp
                }
            },
            _ => match context {
                Context::Mandatory => self.check(pred, n, value, number.loc, None),
                Context::Last => self.check(pred, n, value, minus.loc, None),
                Context::Backup if self.holds(pred, value) => Got::Value(value),
                Context::Backup => {
                    let back = self.value_token(Tok::Identifier(Id::Number), value, number.loc);
                    self.put_back(&[back]);
                    Got::BackedUp
                }
            },
        })
    }

    /// `post_event_nofinger`, read: checked, or backed up as an
    /// `EVENT_IDENTIFIER`.
    pub(crate) fn event_arg(
        &mut self,
        pred: Value,
        context: Context,
        n: usize,
        events: &[crate::syntax::PostEvent],
        first: &Token,
    ) -> Got {
        let value = match events {
            [event] => self.event_value(event),
            _ => {
                let music = Music {
                    span: first.span,
                    kind: MusicKind::PostEvents(events.to_vec()),
                };
                self.music_value(&music)
            }
        };
        let at = self.from(first);
        if context != Context::Backup {
            return self.check(pred, n, value, at, None);
        }
        if self.holds(pred, value) {
            return Got::Value(value);
        }
        let back = self.value_token(Tok::Identifier(Id::Event), value, at);
        self.put_back(&[back]);
        Got::BackedUp
    }
}
