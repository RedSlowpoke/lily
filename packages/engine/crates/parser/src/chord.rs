//! P10 (02 §6): the heads of chord, drum and figure mode
//! (`mode_changing_head`, `mode_changing_head_with_context`), chord entry
//! (`new_chord`, `chord_separator`, `chord_items`, `step_numbers`) and drum
//! notes (`simple_element`'s and `chord_body_element`'s `DRUM_PITCH`).
//! Figures are `figure.rs`'s. P8's `\lyricmode` and `\lyrics` are heads of
//! the same rules.
//!
//! `\chordmode`, `\drummode`, `\figuremode` and `\lyricmode` push their
//! mode ([`Parser::modes`]) when Bison reduces them, before the music's
//! first token is lexed, and the music's rule pops it; `\chords`, `\drums`,
//! `\figures` and `\lyrics` also make `\new ChordNames`, `DrumStaff`,
//! `FiguredBass` or `Lyrics` of it (`context-create`), with the `\with`s
//! after them. Chord mode's music is `UnrelativableMusic`.
//!
//! A chord's notes are made where Bison reduces it, as `make_chord_elements`
//! makes them: by `construct-chord-elements`, which is the `chords` part's
//! (D74) and loads it on its first call, then placed at the chord. Its
//! post-events are appended when the chord's `EventChord` is made
//! (`function.rs`).

use crate::bison;
use crate::lexer::{Mode, Tok, Token, keyword};
use crate::pitch::Pitch;
use crate::repeat::{SEQUENTIAL, SIMULTANEOUS};
use crate::syntax::{ContextMusic, Duration, Music, MusicKind};
use crate::{Parsed, Parser, Span};
use alloc::vec::Vec;
use lily_engine_scheme::Value;

pub(crate) const CHORDMODE: Tok = Tok::Keyword(keyword("chordmode"));
pub(crate) const CHORDS: Tok = Tok::Keyword(keyword("chords"));
pub(crate) const DRUMMODE: Tok = Tok::Keyword(keyword("drummode"));
pub(crate) const DRUMS: Tok = Tok::Keyword(keyword("drums"));
pub(crate) const FIGUREMODE: Tok = Tok::Keyword(keyword("figuremode"));
pub(crate) const FIGURES: Tok = Tok::Keyword(keyword("figures"));
pub(crate) const LYRICS: Tok = Tok::Keyword(keyword("lyrics"));
const N: Mode = Mode::Notes;

/// Whether `kind` is a head of P10's modes or of lyrics mode.
pub(crate) fn starts_mode(kind: Tok) -> bool {
    matches!(
        kind,
        CHORDMODE | CHORDS | DRUMMODE | DRUMS | FIGUREMODE | FIGURES | crate::LYRICMODE | LYRICS
    )
}

impl Parser<'_, '_> {
    /// `mode_changed_music` at one of [`starts_mode`]'s heads: the head,
    /// `optional_context_mods` after a head with a context, and
    /// `grouped_music_list` in the head's mode.
    pub(crate) fn mode_changed_music(&mut self) -> Parsed<Music> {
        let head = self.next(N);
        let (mode, context) = match head.kind {
            CHORDMODE => (Mode::Chords, None),
            CHORDS => (Mode::Chords, Some("ChordNames")),
            DRUMMODE => (Mode::Drums, None),
            DRUMS => (Mode::Drums, Some("DrumStaff")),
            FIGUREMODE => (Mode::Figures, None),
            FIGURES => (Mode::Figures, Some("FiguredBass")),
            crate::LYRICMODE => (Mode::Lyrics, None),
            _ => (Mode::Lyrics, Some("Lyrics")),
        };
        self.pushed += 1;
        self.modes.push(mode);
        let mods = match context {
            Some(_) => Some(self.optional_context_mods()?),
            None => None,
        };
        let token = self.peek(N);
        if !matches!(
            token.kind,
            Tok::Char(b'{') | Tok::DoubleAngleOpen | SEQUENTIAL | SIMULTANEOUS
        ) {
            return Err(self.syntax_error(&token, bison::GROUPED_MUSIC_LIST));
        }
        let mut music = self.grouped_music_list()?;
        if !self.aborted {
            self.pushed -= 1;
            self.modes.pop();
        }
        let span = self.span_from(head.span);
        if let (Some(name), Some((with, ops))) = (context, mods) {
            let symbol = self.env.vm().heap_mut().intern(name);
            let context = ContextMusic {
                new: true,
                context_type: self.env.keep(symbol),
                id: None,
                with,
                ops,
                music,
            };
            music = Music {
                span,
                kind: MusicKind::Context(alloc::boxed::Box::new(context)),
            };
        }
        if mode == Mode::Chords {
            music = Music {
                span,
                kind: MusicKind::Unrelativable(alloc::boxed::Box::new(music)),
            };
        }
        Ok(music)
    }

    /// `steno_tonic_pitch` after its `TONICNAME_PITCH`, `token`, was taken:
    /// the pitch with the octave marks after it.
    fn steno_tonic_pitch(&mut self, token: &Token) -> Pitch {
        let (octaves, _) = self.quotes();
        self.lexer.pitch(token).up(octaves)
    }

    /// `new_chord post_events` at a `TONICNAME_PITCH`: the pitch, a
    /// duration, and a separator and the chord's items, its notes made
    /// where Bison reduces `new_chord`; a pitch with neither a duration nor
    /// a separator is made a chord of the default duration after the
    /// post-events, by `make_music_from_simple` or `new_chord post_events`.
    pub(crate) fn chord_entry(&mut self) -> Parsed<crate::syntax::MusicKind> {
        let token = self.next(N);
        let pitch = self.steno_tonic_pitch(&token);
        let written = self.maybe_duration()?;
        let mut modifications = Vec::new();
        // `chord_separator chord_items`.
        let separated = starts_separator(self.peek(N).kind);
        if separated {
            loop {
                let item = self.peek(N);
                let value = match item.kind {
                    kind if starts_separator(kind) => self.chord_separator()?,
                    Tok::Unsigned => self.step_numbers()?,
                    Tok::ChordModifier => {
                        self.next(N);
                        self.env.value(item.value)
                    }
                    _ => break,
                };
                self.env.keep(value);
                modifications.push(value);
            }
        }
        let at = self.span_from(token.span);
        let pitch = self.pitch_value(pitch);
        let notes = match written.is_some() || separated {
            true => Some(self.chord_elements(pitch, written, &modifications, at)),
            false => None,
        };
        let post = self.post_events()?;
        let notes = match notes {
            Some(notes) => notes,
            None => self.chord_elements(pitch, None, &[], at),
        };
        Ok(MusicKind::ChordEntry { at, notes, post })
    }

    /// `make_chord_elements`: `construct-chord-elements` of `pitch`, a
    /// pitch value kept, the duration written or the default, and
    /// `modifications`, each note placed at `at`; the index of the list of
    /// notes, `'()` if the call failed.
    pub(crate) fn chord_elements(
        &mut self,
        pitch: Value,
        duration: Option<Duration>,
        modifications: &[Value],
        at: Span,
    ) -> u32 {
        let duration = duration.unwrap_or(self.taken_default());
        let duration = self.duration_value(duration);
        let modifications = self.env.vm().heap_mut().list(modifications);
        self.env.keep(modifications);
        let notes = self
            .procedure("construct-chord-elements")
            .and_then(|construct| self.call_scheme(construct, &[pitch, duration, modifications]))
            .unwrap_or(Value::NULL);
        let origin = self.origin(at);
        lily_engine_music::set_origin(self.env.vm().heap_mut(), notes, origin);
        self.env.keep(notes)
    }

    /// `chord_separator`: `:` and `^` as `chord-colon` and `chord-caret`,
    /// `/` and `/+` and a `steno_tonic_pitch` as `(chord-slash pitch)` and
    /// `(chord-bass pitch)`.
    fn chord_separator(&mut self) -> Parsed<Value> {
        let token = self.next(N);
        let name = match token.kind {
            Tok::ChordColon => return Ok(self.env.vm().heap_mut().intern("chord-colon")),
            Tok::ChordCaret => return Ok(self.env.vm().heap_mut().intern("chord-caret")),
            Tok::ChordSlash => "chord-slash",
            _ => "chord-bass",
        };
        let tonic = self.peek(N);
        if tonic.kind != Tok::TonicnamePitch {
            return Err(self.syntax_error(&tonic, bison::TONICNAME_PITCH));
        }
        self.next(N);
        let pitch = self.steno_tonic_pitch(&tonic);
        let pitch = self.pitch_value(pitch);
        let heap = self.env.vm().heap_mut();
        let name = heap.intern(name);
        Ok(heap.list(&[name, pitch]))
    }

    /// `step_numbers`: `step_number`s joined by `.`, each an `UNSIGNED` and
    /// `+` or `-` after it or not, as a list of pitches (`make_chord_step`).
    fn step_numbers(&mut self) -> Parsed<Value> {
        let mut steps = Vec::new();
        loop {
            let token = self.expect(Tok::Unsigned, N, bison::MANY)?;
            let alteration = match self.peek(N).kind {
                Tok::Char(b'+') => 2,
                Tok::ChordMinus => -2,
                _ => 0,
            };
            if alteration != 0 {
                self.next(N);
            }
            let step = self.unsigned(&token).map_or(i64::MAX, |n| n as i64);
            let pitch = self.pitch_value(chord_step(step, alteration));
            self.env.keep(pitch);
            steps.push(pitch);
            if self.peek(N).kind != Tok::Char(b'.') {
                break;
            }
            self.next(N);
        }
        Ok(self.env.vm().heap_mut().list(&steps))
    }

    /// `simple_element`'s `DRUM_PITCH optional_notemode_duration`, and
    /// `event_chord`'s post-events after it.
    pub(crate) fn drum_note(&mut self) -> Parsed<MusicKind> {
        let token = self.next(N);
        let duration = self.optional_duration()?;
        let at = self.span_from(token.span);
        let post = self.post_events()?;
        Ok(MusicKind::Drum {
            at,
            drum: token.value,
            duration,
            post,
        })
    }
}

/// Whether `kind` starts `chord_separator`.
fn starts_separator(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::ChordColon | Tok::ChordCaret | Tok::ChordSlash | Tok::ChordBass
    )
}

/// `make_chord_step`: the pitch of step `step` above the root's octave,
/// with `alteration` in quarter tones, a seventh flattened.
fn chord_step(step: i64, alteration: i8) -> Pitch {
    // `from_scm<int>`, and the steps of `Pitch (0, step - 1, alter)`.
    let steps = step.clamp(i32::MIN.into(), i32::MAX.into()) - 1;
    let (octave, notename) = (steps.div_euclid(7), steps.rem_euclid(7) as u8);
    Pitch {
        octave: octave.clamp(i8::MIN.into(), i8::MAX.into()) as i8,
        step: notename,
        alteration: alteration - if notename == 6 { 2 } else { 0 },
    }
}
