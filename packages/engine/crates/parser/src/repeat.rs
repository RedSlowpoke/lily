//! P9 (02 §6): `\repeat` and `\alternative` (`repeated_music`,
//! `alternative_music`), `\tempo` (`tempo_event`), and `\sequential` and
//! `\simultaneous` before braces (`grouped_music_list`), which `\notemode`
//! takes too (`mode_changing_head`). `\rest` after a note is P2's
//! (`notes.rs`). Their music is made by the music crate's constructors
//! (`function.rs`): `repeat`, `repeat-alt`, `alternative` and `tempo` of
//! `ly-syntax-constructors.scm`, with what they warn of reported at the
//! music's origin.

use crate::bison;
use crate::classify::Id;
use crate::lexer::{Mode, Tok, keyword};
use crate::syntax::{Music, MusicKind, Repeat, Tempo};
use crate::{Parsed, Parser, Span};
use lily_engine_scheme::Value;

pub(crate) const REPEAT: Tok = Tok::Keyword(keyword("repeat"));
pub(crate) const ALTERNATIVE: Tok = Tok::Keyword(keyword("alternative"));
pub(crate) const TEMPO: Tok = Tok::Keyword(keyword("tempo"));
pub(crate) const SEQUENTIAL: Tok = Tok::Keyword(keyword("sequential"));
pub(crate) const SIMULTANEOUS: Tok = Tok::Keyword(keyword("simultaneous"));
pub(crate) const LYRICSTO: Tok = Tok::Keyword(keyword("lyricsto"));
const N: Mode = Mode::Notes;
const ZERO: Value = match Value::fixnum(0) {
    Some(zero) => zero,
    None => Value::FALSE,
};

/// Whether `kind` starts `basic_music` by one of P9's heads, P8's
/// `\lyricsto`, or a mode's head (P10's and P8's): what `composite_music`
/// takes beyond P2–P7's.
pub(crate) fn starts_basic(kind: Tok) -> bool {
    matches!(
        kind,
        REPEAT | ALTERNATIVE | SEQUENTIAL | SIMULTANEOUS | LYRICSTO
    ) || crate::chord::starts_mode(kind)
}

impl Parser<'_, '_> {
    /// `basic_music` at one of [`starts_basic`]'s heads.
    pub(crate) fn basic_music(&mut self) -> Parsed<Music> {
        match self.peek(N).kind {
            REPEAT => self.repeated_music(),
            ALTERNATIVE => self.alternative_music(),
            LYRICSTO => self.lyricsto(),
            kind if crate::chord::starts_mode(kind) => self.mode_changed_music(),
            _ => self.grouped_music_list(),
        }
    }

    /// `grouped_music_list`: `{ … }`, `<< … >>`, or `\sequential` or
    /// `\simultaneous` and braces (`braced_music_list`), whose music is
    /// placed from the keyword.
    pub(crate) fn grouped_music_list(&mut self) -> Parsed<Music> {
        let head = self.next(N);
        let close = match head.kind {
            Tok::DoubleAngleOpen => Tok::DoubleAngleClose,
            Tok::Char(b'{') => Tok::Char(b'}'),
            _ => {
                let open = self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
                let list = self.music_list(open, Tok::Char(b'}'));
                let MusicKind::Sequential(elements) = list.kind else {
                    unreachable!("a braced list is sequential")
                };
                let kind = match head.kind {
                    SIMULTANEOUS => MusicKind::Simultaneous(elements),
                    _ => MusicKind::Sequential(elements),
                };
                return Ok(Music {
                    span: self.span_from(head.span),
                    kind,
                });
            }
        };
        Ok(self.music_list(head, close))
    }

    /// `repeated_music`, at `\repeat`: `simple_string unsigned_integer
    /// music`, and `alternative_music` if it follows.
    fn repeated_music(&mut self) -> Parsed<Music> {
        match self.repeated_music_etc(false)? {
            Ok(music) => Ok(music),
            Err(_) => unreachable!("a repeat is cut short only with `etc`"),
        }
    }

    /// `repeated_music`, or with `etc`, `\repeat` cut short after its kind
    /// or its count (`partial_function`): its call, `(repeat kind)` or
    /// `(repeat count kind)`, and those of a `partial_function` after it.
    pub(crate) fn repeated_music_etc(
        &mut self,
        etc: bool,
    ) -> Parsed<Result<Music, alloc::vec::Vec<Value>>> {
        let head = self.next(N);
        let kind = self.simple_string()?;
        let function = self.constructor("repeat").unwrap_or(Value::FALSE);
        let kind_value = self.env.value(kind);
        if etc {
            let call = self.env.vm().heap_mut().list(&[function, kind_value]);
            self.env.keep(call);
            let token = self.peek(N);
            match self.partial_after(etc, call)? {
                Some(Err(calls)) => return Ok(Err(calls)),
                // Music where the count is due.
                Some(Ok(_)) => return Err(self.syntax_error(&token, bison::MANY)),
                None => {}
            }
        }
        let times = self.unsigned_integer()?;
        let count = self.env.value(times);
        let call = self
            .env
            .vm()
            .heap_mut()
            .list(&[function, count, kind_value]);
        self.env.keep(call);
        let token = self.peek(N);
        let body = match self.partial_after(etc, call)? {
            Some(Err(calls)) => return Ok(Err(calls)),
            Some(Ok(music)) => music,
            None => {
                if !crate::notes::starts_plain_music(token.kind)
                    || matches!(
                        token.kind,
                        Tok::ScmToken
                            | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction)
                    )
                {
                    return Err(self.syntax_error(&token, bison::MANY));
                }
                match self.music_embedded()? {
                    Some(music) => music,
                    None => return Err(self.syntax_error(&token, bison::MANY)),
                }
            }
        };
        let alternative = match self.peek(N).kind {
            ALTERNATIVE => Some(self.alternative_music()?),
            _ => None,
        };
        let repeat = Repeat {
            kind,
            times,
            body,
            alternative,
        };
        Ok(Ok(Music {
            span: self.span_from(head.span),
            kind: MusicKind::Repeat(alloc::boxed::Box::new(repeat)),
        }))
    }

    /// `alternative_music`, at `\alternative`: `basic_music`.
    fn alternative_music(&mut self) -> Parsed<Music> {
        let head = self.next(N);
        let token = self.peek(N);
        let music = match token.kind {
            Tok::Identifier(Id::MusicFunction) => {
                let (value, span) = self.function_call()?;
                self.music_of_value(value, span)
            }
            Tok::Identifier(Id::Music) => {
                self.next(N);
                self.music_identifier(&token)
            }
            Tok::Char(b'{') | Tok::DoubleAngleOpen | crate::NOTEMODE => self.basic_composite()?,
            kind if starts_basic(kind) => self.basic_music()?,
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        Ok(Music {
            span: self.span_from(head.span),
            kind: MusicKind::Alternative(alloc::boxed::Box::new(music)),
        })
    }

    /// `unsigned_integer`: an `UNSIGNED`, or a number or Scheme value that
    /// is one, else `not an unsigned integer` and 0. Its index.
    fn unsigned_integer(&mut self) -> Parsed<u32> {
        let token = self.peek(N);
        let (value, span) = match token.kind {
            Tok::Unsigned => {
                self.next(N);
                let value = self.number(&token, false);
                return Ok(self.env.keep(value));
            }
            Tok::Identifier(Id::Number) => {
                self.next(N);
                (self.env.value(token.value), token.loc)
            }
            Tok::ScmToken
            | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                let (value, span, _) = self.embedded_scm_value()?;
                (value, span)
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        let heap = self.env.vm_ref().heap();
        let good = heap.is_exact_integer(value) && heap.is_negative(value) == Ok(false);
        if !good {
            self.error(span, "not an unsigned integer");
            return Ok(self.env.keep(ZERO));
        }
        Ok(self.env.keep(value))
    }

    /// `exact_unsigned_number`, as [`Parser::unsigned_integer`] but of any
    /// exact number: `not an exact unsigned number`.
    fn exact_unsigned_number(&mut self) -> Parsed<Value> {
        let token = self.peek(N);
        let (value, span) = match token.kind {
            Tok::Unsigned => {
                self.next(N);
                return Ok(self.number(&token, false));
            }
            Tok::Identifier(Id::Number) => {
                self.next(N);
                (self.env.value(token.value), token.loc)
            }
            Tok::ScmToken
            | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                let (value, span, _) = self.embedded_scm_value()?;
                (value, span)
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        let heap = self.env.vm_ref().heap();
        let good = heap.is_exact(value) == Ok(true) && heap.is_negative(value) == Ok(false);
        if !good {
            self.error(span, "not an exact unsigned number");
            return Ok(ZERO);
        }
        Ok(value)
    }

    /// `tempo_event`, at `\tempo`: `steno_duration '=' tempo_range`, after
    /// a `text` or not, or a `text` alone.
    pub(crate) fn tempo_event(&mut self) -> Parsed<MusicKind> {
        self.next(N);
        let token = self.peek(N);
        let text = match token.kind {
            Tok::Unsigned | Tok::Identifier(Id::Duration) => None,
            Tok::String | Tok::Symbol => {
                self.next(N);
                let text = match token.kind {
                    Tok::String => alloc::string::String::from(self.lexer.string(&token)),
                    _ => alloc::string::String::from_utf8_lossy(self.lexer.text_of(token.span))
                        .into_owned(),
                };
                let value = self.env.vm().heap_mut().string(text);
                Some(self.env.keep(value))
            }
            crate::MARKUP => {
                self.next(N);
                let markup = self.full_markup()?;
                Some(self.env.keep(markup))
            }
            Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                self.next(N);
                let value = match token.kind {
                    Tok::ScmToken => self.scheme_value(&token),
                    _ => self.env.value(token.value),
                };
                let span = Span {
                    start: token.loc.start,
                    ..self.span_from(token.span)
                };
                if self.is_markup(value) {
                    Some(self.env.keep(value))
                } else {
                    self.error(span, "markup expected");
                    let empty = self.env.vm().heap_mut().string("");
                    Some(self.env.keep(empty))
                }
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        if text.is_some()
            && !matches!(
                self.peek(N).kind,
                Tok::Unsigned | Tok::Identifier(Id::Duration)
            )
        {
            return Ok(MusicKind::Tempo(Tempo { text, mark: None }));
        }
        let unit = self.steno_duration()?;
        self.expect(Tok::Char(b'='), N, bison::EQUALS)?;
        let low = self.exact_unsigned_number()?;
        let count = if self.peek(N).kind == Tok::Char(b'-') {
            self.next(N);
            let high = self.exact_unsigned_number()?;
            self.env.vm().heap_mut().cons(low, high)
        } else {
            low
        };
        let count = self.env.keep(count);
        Ok(MusicKind::Tempo(Tempo {
            text,
            mark: Some((unit, count)),
        }))
    }

    /// `steno_duration`: an `UNSIGNED` or a `DURATION_IDENTIFIER` and its
    /// dots, with no multiplier, which does not become the default.
    fn steno_duration(&mut self) -> Parsed<crate::syntax::Duration> {
        let token = self.peek(N);
        let mut duration = match token.kind {
            Tok::Unsigned => {
                self.next(N);
                match crate::notes::duration_log(self.unsigned(&token)) {
                    Some(log) => crate::syntax::Duration::new(log.into(), 0),
                    None => {
                        self.error(token.loc, "not a duration");
                        crate::syntax::Duration::new(0, 0)
                    }
                }
            }
            Tok::Identifier(Id::Duration) => {
                self.next(N);
                let value = self.env.value(token.value);
                self.duration_of(value)
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        while self.peek(N).kind == Tok::Char(b'.') {
            self.next(N);
            duration.dots = duration.dots.saturating_add(1);
        }
        duration.span = Some(self.span_from(token.span));
        Ok(duration)
    }

    /// The syntax of a duration value, a `DURATION_IDENTIFIER`'s.
    pub(crate) fn duration_of(&mut self, value: Value) -> crate::syntax::Duration {
        let heap = self.env.vm_ref().heap();
        match heap.foreign::<lily_engine_music::Duration>(value) {
            Some(&d) => {
                let factor = d.factor();
                crate::syntax::Duration {
                    span: None,
                    log: d.log(),
                    dots: d.dots().clamp(0, u8::MAX.into()) as u8,
                    factor: (
                        factor.numerator().unsigned_abs(),
                        factor.denominator().unsigned_abs(),
                    ),
                }
            }
            None => crate::syntax::Duration::new(0, 0),
        }
    }

    /// The text of a tempo mark, `'()` for none.
    pub(crate) fn tempo_text(&mut self, tempo: &Tempo) -> Value {
        match tempo.text {
            Some(index) => self.env.value(index),
            None => Value::NULL,
        }
    }
}
