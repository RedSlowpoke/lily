//! P10 (02 §6): figures (`chord_body`'s `FIGURE_OPEN figure_list
//! FIGURE_CLOSE`, `br_bass_figure`, `bass_figure`, `bass_number` and
//! `figured_bass_modification`), in figure mode.
//!
//! Each figure's `BassFigureEvent` is made where Bison reduces its
//! `bass_figure`, at its number, string, word or markup, and its brackets,
//! alterations and modifications are set on it as their rules are reduced,
//! so that `Dropping surplus alteration symbols` comes in its place. The
//! figures' `EventChord`, their duration and the post-events after them are
//! made with the rest of the music (`function.rs`).

use crate::bison;
use crate::classify::Id;
use crate::diag::{At, Severity};
use crate::lexer::{Mode, Tok};
use crate::syntax::MusicKind;
use crate::{MARKUP, Parsed, Parser};
use alloc::vec::Vec;
use lily_engine_music::{music_property, set_property};
use lily_engine_scheme::{Tag, Value};

const N: Mode = Mode::Notes;

impl Parser<'_, '_> {
    /// `chord_body optional_notemode_duration post_events` at a
    /// `FIGURE_OPEN`: the figures, each `br_bass_figure`.
    pub(crate) fn figure_chord(&mut self) -> Parsed<MusicKind> {
        self.next(N);
        let mut figures = Vec::new();
        loop {
            let token = self.peek(N);
            match token.kind {
                Tok::FigureClose => {
                    self.next(N);
                    break;
                }
                Tok::Char(b'[') => {
                    self.next(N);
                    let figure = self.bass_figure()?;
                    self.set_flag(figure, "bracket-start");
                    figures.push(figure);
                }
                _ => figures.push(self.bass_figure()?),
            }
        }
        let duration = self.optional_duration()?;
        let post = self.post_events()?;
        Ok(MusicKind::Figures {
            figures,
            duration,
            post,
        })
    }

    /// `bass_figure`: a `BassFigureEvent` of `_` or of a `bass_number`, and
    /// the `]`, alterations and modifications after it. Its index.
    fn bass_figure(&mut self) -> Parsed<u32> {
        let token = self.peek(N);
        let (at, figure) = match token.kind {
            Tok::FigureSpace => {
                self.next(N);
                (token.loc, None)
            }
            Tok::Unsigned => {
                self.next(N);
                (token.loc, Some(self.number(&token, false)))
            }
            Tok::String => {
                self.next(N);
                let text = alloc::string::String::from(self.lexer.string(&token));
                (token.loc, Some(self.env.vm().heap_mut().string(text)))
            }
            Tok::Symbol => {
                self.next(N);
                let text = alloc::string::String::from_utf8_lossy(self.lexer.text_of(token.span));
                (token.loc, Some(self.env.vm().heap_mut().string(text)))
            }
            MARKUP => {
                self.next(N);
                let markup = self.full_markup()?;
                (self.from(&token), Some(markup))
            }
            // `embedded_scm_bare`: a number not negative, or markup.
            Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                self.next(N);
                let value = match token.kind {
                    Tok::ScmToken => self.scheme_value(&token),
                    _ => self.env.value(token.value),
                };
                let at = self.from(&token);
                let heap = self.env.vm_ref().heap();
                let fine = match heap.is_exact_integer(value) {
                    true => heap.is_negative(value) == Ok(false),
                    false => self.is_markup(value),
                };
                if !fine {
                    self.error(at, "bass number expected");
                }
                (
                    at,
                    Some(if fine {
                        value
                    } else {
                        Value::fixnum(0).unwrap_or(value)
                    }),
                )
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        if let Some(value) = figure {
            self.env.keep(value);
        }
        let props = match figure {
            Some(value) if is_number(value) => alloc::vec![("figure", value)],
            Some(value) if self.is_markup(value) => alloc::vec![("text", value)],
            _ => Vec::new(),
        };
        let music = self.make_at("BassFigureEvent", at, &props);
        let index = self.env.keep(music);
        loop {
            let token = self.peek(N);
            match token.kind {
                Tok::Char(b']') => {
                    self.next(N);
                    self.set_flag(index, "bracket-stop");
                }
                Tok::FigureAlteration => {
                    self.next(N);
                    self.alteration(index, self.lexer.text_of(token.span));
                }
                Tok::EPlus => {
                    self.next(N);
                    self.set_flag(index, "augmented");
                }
                Tok::EExclamation => {
                    self.next(N);
                    self.set_flag(index, "no-continuation");
                }
                Tok::Char(b'/') => {
                    self.next(N);
                    self.set_flag(index, "diminished");
                }
                Tok::EBackslash => {
                    self.next(N);
                    self.set_flag(index, "augmented-slash");
                }
                _ => return Ok(index),
            }
        }
    }

    /// `bass_figure FIGURE_ALTERATION_EXPR`: the alteration of `text`'s
    /// `+`, `-` and `!` (which starts it again at 0) and its bracket, unless
    /// the figure has one, which is a warning.
    fn alteration(&mut self, figure: u32, text: &[u8]) {
        let music = self.env.value(figure);
        let heap = self.env.vm().heap_mut();
        let key = heap.intern("alteration");
        if music_property(heap, music, key).is_some_and(is_number) {
            if let Some(span) = self.origin_span(music) {
                self.diagnose(
                    Severity::Warning,
                    At::Span(span),
                    "Dropping surplus alteration symbols for bass figure.",
                );
            }
            return;
        }
        // In halves of a tone, as SHARP_ALTERATION and FLAT_ALTERATION are.
        let mut halves: i64 = 0;
        for &c in text {
            match c {
                b'!' => halves = 0,
                b'+' => halves += 1,
                b'-' => halves -= 1,
                _ => {}
            }
        }
        let bracket = text.contains(&b'[');
        let heap = self.env.vm().heap_mut();
        let (halves, two) = (heap.integer(halves), heap.integer(2));
        let alteration = heap.div(halves, two).unwrap_or(halves);
        set_property(heap, music, key, alteration);
        if bracket {
            self.set_flag(figure, "alteration-bracket");
        }
    }

    /// Sets the property `name` of the music of index `music` to `#t`.
    fn set_flag(&mut self, music: u32, name: &str) {
        let music = self.env.value(music);
        let heap = self.env.vm().heap_mut();
        let key = heap.intern(name);
        set_property(heap, music, key, Value::TRUE);
    }
}

/// Whether `value` is a number (`scm_is_number`).
fn is_number(value: Value) -> bool {
    matches!(
        value.tag(),
        Tag::Fixnum | Tag::Bignum | Tag::Flonum | Tag::Ratio | Tag::Complex
    )
}
