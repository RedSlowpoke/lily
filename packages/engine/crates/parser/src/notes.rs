//! The notes core (02 §6's P2, §5.3): `{ }` and `<< >>`, notes with their
//! pitches, octave marks, `!` and `?`, octave checks, durations with dots
//! and multipliers, `r s R q`, chords, and post-events with their
//! directions, fingerings, string numbers and tremolos; by recursive
//! descent over `parser.yy`'s `music_list`, `music_embedded`, `event_chord`,
//! `pitch_or_music`, `chord_body`, `post_event` and `duration`.
//!
//! The grammar's actions run where Bison reduces their rules, so that their
//! errors come in LilyPond's order among the lexer's: a rule Bison reduces
//! only on seeing the token after it (a duration's dots, a note's
//! post-events) reports once that token is lexed, one it reduces at once
//! (`':' UNSIGNED`, a lone word, a closed music list) before.
//!
//! P3's identifiers in music: a `MUSIC_IDENTIFIER` is a copy of its music,
//! placed at the `\name` (its `origin` set there); a `PITCH_IDENTIFIER` is a
//! pitch; an `EVENT_IDENTIFIER` is a post-event, or the events of a
//! `PostEvents` music; and any other value or lookup is
//! `music_embedded_backup`'s Scheme, which is ignored unless it is music.
//! A direction or a place given to a post-event that is a music value is
//! set on that music as the grammar's actions set it (N2); the rest of the
//! music is made from the syntax by `function.rs`.
//!
//! P5's function calls (`function.rs`): a music function's in music, a
//! Scheme function's where Scheme may stand, and an event function's as a
//! post-event; their values, and music values that `\name`s hold, are
//! [`MusicKind::Value`] and [`PostEventKind::Value`].

use crate::bison;
use crate::classify::Id;
use crate::context::{CHANGE, CONTEXT, NEWCONTEXT};
use crate::diag::{At, Severity};
use crate::lexer::{Mode, Tok, Token, keyword};
use crate::pitch::Pitch;
use crate::property::{OVERRIDE, REVERT, SET, UNSET, starts_property};
use crate::repeat::{ALTERNATIVE, LYRICSTO, REPEAT, SEQUENTIAL, SIMULTANEOUS, TEMPO, starts_basic};
use crate::scheme::scale;
use crate::syntax::{
    ChordElement, Direction, Duration, Music, MusicKind, Note, PostEvent, PostEventKind, Text,
};
use crate::{LYRICMODE, MARKUP, NOTEMODE, Parsed, Parser, Span};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::{
    Music as MusicValue, Pitch as PitchValue, is_music_type, music_name, music_property,
    set_property,
};
use lily_engine_scheme::{Heap, Value};

const REST: Tok = Tok::Keyword(keyword("rest"));
pub(crate) const ADDLYRICS: Tok = Tok::Keyword(keyword("addlyrics"));
const N: Mode = Mode::Notes;
const L: Mode = Mode::Lyrics;

/// Whether `kind` can start `music_embedded` in P2's grammar.
pub(crate) fn starts_music(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Char(b'{' | b'-' | b'^' | b'_' | b':')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | CONTEXT
            | REPEAT
            | ALTERNATIVE
            | SEQUENTIAL
            | SIMULTANEOUS
            | LYRICSTO
            | TEMPO
            | CHANGE
            | SET
            | UNSET
            | OVERRIDE
            | REVERT
            | Tok::AngleOpen
            | Tok::FigureOpen
            | Tok::NotenamePitch
            | Tok::TonicnamePitch
            | Tok::DrumPitch
            | Tok::RestName
            | Tok::ChordRepetition
            | Tok::MultiMeasureRest
            | Tok::Unsigned
            | Tok::EUnsigned
            | Tok::Symbol
            | Tok::String
            | Tok::Hyphen
            | Tok::Extender
            | MARKUP
            | Tok::ScmToken
            | Tok::Identifier(
                Id::Music
                    | Id::Event
                    | Id::Pitch
                    | Id::Duration
                    | Id::Scm
                    | Id::Lookup
                    | Id::Module
                    | Id::MusicFunction
                    | Id::EventFunction
                    | Id::ScmFunction
            )
    ) || crate::chord::starts_mode(kind)
}

/// Whether `kind` can start `music`, which `music_embedded` extends with
/// post-events and durations alone: a score's item, say.
pub(crate) fn starts_plain_music(kind: Tok) -> bool {
    starts_music(kind)
        && !matches!(
            kind,
            Tok::Char(b'-' | b'^' | b'_' | b':')
                | Tok::EUnsigned
                | Tok::Unsigned
                | Tok::Hyphen
                | Tok::Extender
                | Tok::Identifier(Id::Event | Id::EventFunction | Id::Duration)
        )
}

/// Whether `music` takes the post-events after it in a music list
/// (`add_post_events`): rhythmic events and event chords do, and sequential
/// music as its last element does; a run of post-events that ends it became
/// an event chord, which does. Of a music value, its type tells.
fn takes_post_events(heap: &Heap, values: &crate::Env, music: &Music) -> bool {
    match &music.kind {
        MusicKind::Sequential(elements) => elements.last().is_some_and(|last| {
            matches!(last.kind, MusicKind::PostEvent(_)) || takes_post_events(heap, values, last)
        }),
        MusicKind::Value(index) => {
            let value = values.value(*index);
            is_music_type(heap, value, "rhythmic-event")
                || is_music_type(heap, value, "caesura-event")
                || music_name(heap, value) == Some("EventChord")
        }
        // `ContextSpeccedMusic` and `UnrelativableMusic` are wrappers, and
        // `SequentialAlternativeMusic` sequential.
        MusicKind::Context(context) => takes_post_events(heap, values, &context.music),
        MusicKind::Unrelativable(music) => takes_post_events(heap, values, music),
        MusicKind::Alternative(music) => takes_post_events(heap, values, music),
        // Repeats, `TempoChangeEvent` and `LyricCombineMusic` are none of
        // those, and a tempo's sequential music ends with a property's.
        MusicKind::Repeat(_) | MusicKind::Tempo(_) | MusicKind::LyricCombine { .. } => false,
        // Simultaneous music, `R`'s `MultiMeasureRestMusic`, `\change`'s
        // `ContextChange`, a property's operation and a syntax error's music
        // are no rhythmic events.
        MusicKind::Simultaneous(_)
        | MusicKind::Change { .. }
        | MusicKind::Property(_)
        | MusicKind::MultiMeasureRest { .. }
        | MusicKind::Error
        | MusicKind::PostEvent(_)
        | MusicKind::PostEvents(_) => false,
        _ => true,
    }
}

/// The music name of a post-event: what `Unattached` names.
fn event_name(heap: &Heap, values: &crate::Env, kind: &PostEventKind) -> String {
    String::from(match kind {
        PostEventKind::Fingering(_) => "FingeringEvent",
        PostEventKind::StringNumber(_) => "StringNumberEvent",
        PostEventKind::Tremolo(_) => "TremoloEvent",
        // `dashDot` and the other abbreviations are `make-articulation`'s.
        PostEventKind::Script(_) => "ArticulationEvent",
        PostEventKind::Text(_) => "TextScriptEvent",
        PostEventKind::Hyphen => "HyphenEvent",
        PostEventKind::Extender => "ExtenderEvent",
        PostEventKind::Value(index) => music_name(heap, values.value(*index)).unwrap_or("Music"),
    })
}

/// `make_duration` of an `UNSIGNED`: a power of two that `from_scm<int>`
/// takes, as its log.
pub(crate) fn duration_log(value: Option<u64>) -> Option<u8> {
    value
        .filter(|&t| t > 0 && t.is_power_of_two() && t <= i32::MAX as u64)
        .map(|t| t.trailing_zeros() as u8)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `factor` times `num/den`, reduced; past `u64`, the factor stays as it was.
fn times(factor: (u64, u64), num: u64, den: u64) -> (u64, u64) {
    let (n, d) = (factor.0.checked_mul(num), factor.1.checked_mul(den));
    match (n, d) {
        (Some(n), Some(d)) if d != 0 => {
            let g = gcd(n, d).max(1);
            (n / g, d / g)
        }
        _ => factor,
    }
}

impl Parser<'_, '_> {
    /// An `UNSIGNED`'s value, None past `u64`.
    pub(crate) fn unsigned(&self, token: &Token) -> Option<u64> {
        core::str::from_utf8(self.lexer.text_of(token.span))
            .ok()?
            .parse()
            .ok()
    }

    /// `composite_music` of P2 and P3: braced or `<< >>` music, `\notemode`
    /// and one of them, which is that music (`mode_changed_music`), or a
    /// `MUSIC_IDENTIFIER`; P6's `\new` and `\context` and their music
    /// (`context.rs`); P9's `basic_music` (`repeat.rs`); and P8's
    /// `basic_music new_lyrics`, the `\addlyrics` after it.
    pub(crate) fn composite_music(&mut self) -> Parsed<Music> {
        if matches!(self.peek(N).kind, NEWCONTEXT | CONTEXT) {
            let head = self.next(N);
            return self.contexted_music(head);
        }
        let music = self.basic_composite()?;
        self.new_lyrics(music)
    }

    /// `composite_music` but `contexted_basic_music` and `\addlyrics`:
    /// `basic_music`, which is what `\alternative` and a context's
    /// `contextable_music` take, the `\addlyrics` after it left to them.
    pub(crate) fn basic_composite(&mut self) -> Parsed<Music> {
        if starts_basic(self.peek(N).kind) {
            return self.basic_music();
        }
        if self.peek(N).kind == Tok::Identifier(Id::MusicFunction) {
            // `music_function_call`: a post-event it gives stands in the
            // list as one.
            let (value, span) = self.function_call()?;
            return Ok(self.music_of_value(value, span));
        }
        let mut open = self.next(N);
        if open.kind == Tok::Identifier(Id::Music) {
            return Ok(self.music_identifier(&open));
        }
        let mode_changed = open.kind == NOTEMODE;
        if mode_changed {
            // `mode_changing_head` pushes the notes mode, which
            // `mode_changed_music` pops.
            self.pushed += 1;
            self.modes.push(N);
            let token = self.peek(N);
            if matches!(token.kind, SEQUENTIAL | SIMULTANEOUS) {
                let music = self.grouped_music_list()?;
                if !self.aborted {
                    self.pushed -= 1;
                    self.modes.pop();
                }
                return Ok(music);
            }
            if !matches!(token.kind, Tok::Char(b'{') | Tok::DoubleAngleOpen) {
                return Err(self.syntax_error(&token, bison::GROUPED_MUSIC_LIST));
            }
            open = self.next(N);
        }
        let close = if open.kind == Tok::DoubleAngleOpen {
            Tok::DoubleAngleClose
        } else {
            Tok::Char(b'}')
        };
        let music = self.music_list(open, close);
        if mode_changed && !self.aborted {
            self.pushed -= 1;
            self.modes.pop();
        }
        Ok(music)
    }

    /// `new_lyrics` after `music` if an `\addlyrics` follows: each
    /// `\addlyrics optional_context_mods lyric_mode_music`, as many as
    /// follow, since `ADDLYRICS` binds tighter than `COMPOSITE`; else
    /// `music`. The music is made where Bison reduces the rule, by
    /// `add-lyrics` ([`Parser::add_lyrics`]), so that the unique voice
    /// names it may take are taken in LilyPond's order.
    pub(crate) fn new_lyrics(&mut self, music: Music) -> Parsed<Music> {
        let mut lyrics = Vec::new();
        while self.peek(N).kind == ADDLYRICS {
            self.next(N);
            let (_, ops) = self.optional_context_mods()?;
            lyrics.push((self.lyric_mode_music()?, ops));
        }
        if lyrics.is_empty() {
            return Ok(music);
        }
        let span = self.span_from(music.span);
        let value = self.add_lyrics(&music, &lyrics);
        Ok(self.music_of_value(value, span))
    }

    /// `lyric_mode_music`: a `MUSIC_IDENTIFIER`, or `grouped_music_list` in
    /// lyrics mode, which the rule's mid-rule action pushes once the token
    /// that tells the two apart is lexed: `{`, `<<`, `\sequential` or
    /// `\simultaneous`, lexed in the mode before; the mode is popped after
    /// the music. Any other token is a syntax error with lyrics mode
    /// pushed, as the action left it.
    fn lyric_mode_music(&mut self) -> Parsed<Music> {
        let token = self.peek(N);
        if token.kind == Tok::Identifier(Id::Music) {
            self.next(N);
            return Ok(self.music_identifier(&token));
        }
        self.pushed += 1;
        self.modes.push(L);
        // The token looked at stays as it was lexed.
        if let Some((_, mode)) = &mut self.ahead {
            *mode = L;
        }
        if !matches!(
            token.kind,
            Tok::Char(b'{') | Tok::DoubleAngleOpen | SEQUENTIAL | SIMULTANEOUS
        ) {
            return Err(self.syntax_error(&token, bison::GROUPED_MUSIC_LIST));
        }
        let music = self.grouped_music_list()?;
        if !self.aborted {
            self.pushed -= 1;
            self.modes.pop();
        }
        Ok(music)
    }

    /// `\lyricsto simple_string lyric_mode_music` or `\lyricsto symbol '='
    /// simple_string lyric_mode_music` (P12), at `\lyricsto`: a word, a
    /// string or Scheme is read, and the token after it says which, as
    /// Bison's lookahead does.
    pub(crate) fn lyricsto(&mut self) -> Parsed<Music> {
        let head = self.next(N);
        let (first, value) = self.string_or_symbol()?;
        let (id, context_type) = if self.peek(N).kind == Tok::Char(b'=') {
            let context_type = self.symbol_of(&first, value);
            self.next(N);
            (self.simple_string()?, Some(context_type))
        } else {
            (self.simple_string_of(&first, value), None)
        };
        let music = self.lyric_mode_music()?;
        Ok(Music {
            span: self.span_from(head.span),
            kind: MusicKind::LyricCombine {
                id,
                context_type,
                music: alloc::boxed::Box::new(music),
            },
        })
    }

    /// `{ music_list }` or `<< music_list >>`, after `open`: each element
    /// `music_embedded`, and `music_list error` for a syntax error, which
    /// discards tokens until one can start an element or close the list.
    /// At the end of input that aborts the parse, and the list is left as
    /// it was. In a parse that keeps a [`crate::Reuse`], the last parse's
    /// list at the same place is taken where it can be (02 §5.7's step 3,
    /// `reuse.rs`).
    pub(crate) fn music_list(&mut self, open: Token, close: Tok) -> Music {
        let entry = self.block_entry(&open);
        if let Some(music) = entry.as_ref().and_then(|entry| self.reuse_block(entry)) {
            return music;
        }
        let music = self.parse_music_list(open, close);
        if let Some(entry) = entry {
            self.record_block(entry, &music);
        }
        music
    }

    fn parse_music_list(&mut self, open: Token, close: Tok) -> Music {
        let mut elements: Vec<Music> = Vec::new();
        while !self.aborted {
            let token = self.peek(N);
            if token.kind == close {
                self.next(N);
                self.unattached(&elements);
                break;
            }
            if starts_music(token.kind) {
                if let Ok(music) = self.music_embedded() {
                    // Scheme that is no music is not an element.
                    elements.extend(music);
                    continue;
                }
            } else {
                self.syntax_error(&token, bison::MANY);
            }
            if elements.last().is_none_or(|m| m.kind != MusicKind::Error) {
                elements.push(Music {
                    span: token.span,
                    kind: MusicKind::Error,
                });
            }
            self.resync(N, |kind| kind == close || starts_music(kind));
        }
        let span = self.span_from(open.span);
        let kind = if close == Tok::DoubleAngleClose {
            MusicKind::Simultaneous(elements)
        } else {
            MusicKind::Sequential(elements)
        };
        Music { span, kind }
    }

    /// `reverse_music_list`'s warnings for a list just closed: a run of
    /// post-events attaches to the music before it, and when that music
    /// takes none ([`takes_post_events`]), the run's first is `Unattached`; a
    /// run that opens the list is unattached whole.
    fn unattached(&mut self, elements: &[Music]) {
        let mut warnings = Vec::new();
        let heap = self.env.vm_ref().heap();
        let mut before: Option<&Music> = None;
        let mut run_start = true;
        for music in elements {
            let events = match &music.kind {
                MusicKind::PostEvent(event) => core::slice::from_ref(event),
                MusicKind::PostEvents(events) => &events[..],
                _ => {
                    before = Some(music);
                    run_start = true;
                    continue;
                }
            };
            for event in events {
                if before.is_none()
                    || run_start && !before.is_some_and(|m| takes_post_events(heap, &self.env, m))
                {
                    let name = event_name(heap, &self.env, &event.kind);
                    warnings.push((event.span, alloc::format!("Unattached {name}")));
                }
                run_start = false;
            }
        }
        for (span, message) in warnings {
            self.diagnose(Severity::Warning, At::Span(span), message);
        }
    }

    /// Music that is a music value, at `span`: a post-event is one, and a
    /// `PostEvents` music's events are, as [`Parser::events_of`] gives them.
    pub(crate) fn music_of_value(&mut self, value: Value, span: Span) -> Music {
        let heap = self.env.vm_ref().heap();
        if !is_music_type(heap, value, "post-event") {
            let index = self.env.keep(value);
            return Music {
                span,
                kind: MusicKind::Value(index),
            };
        }
        let mut events = self.events_of(value, span);
        let kind = match events.len() {
            1 => MusicKind::PostEvent(events.pop().expect("one event")),
            _ => MusicKind::PostEvents(events.into_iter().rev().collect()),
        };
        Music { span, kind }
    }

    /// Whether an identifier token is a `$` form's value, which keeps the
    /// place of its music; `scan_escaped_word` and `scan_shorthand` place a
    /// copy at the name.
    fn placed(&self, token: &Token) -> bool {
        self.lexer.text_of(token.span).first() != Some(&b'$')
    }

    /// A `MUSIC_IDENTIFIER` just taken: its music, a copy placed at the name
    /// but after `$`.
    pub(crate) fn music_identifier(&mut self, token: &Token) -> Music {
        let value = self.env.value(token.value);
        let copy = self.placed_copy(value, self.placed(token).then_some(token.loc));
        Music {
            span: token.loc,
            kind: MusicKind::Value(self.env.keep(copy)),
        }
    }

    /// `value` copied if it is music, its copy's `origin` set to `at` if
    /// given (`loc_on_copy`; `scan_escaped_word` and `scan_shorthand`, which
    /// place the music they find before `try_special_identifiers` copies it).
    pub(crate) fn placed_copy(&mut self, value: Value, at: Option<Span>) -> Value {
        let copy = self.copy(value);
        if let Some(at) = at
            && self
                .env
                .vm_ref()
                .heap()
                .foreign::<MusicValue>(copy)
                .is_some()
        {
            let origin = self.origin(at);
            lily_engine_music::set_origin(self.env.vm().heap_mut(), copy, origin);
        }
        copy
    }

    /// An `EVENT_IDENTIFIER` just taken, as `post_event_cons` adds it to
    /// post-events: its one event, a copy placed at the name but after `$`,
    /// or the events of its `PostEvents` music, where they were written.
    fn event_identifier(&mut self, token: &Token) -> Vec<PostEvent> {
        let placed = self.placed(token);
        let value = self.env.value(token.value);
        let copy = self.placed_copy(value, placed.then_some(token.loc));
        self.events_of(copy, token.loc)
    }

    /// The events of `music`, a post-event: itself, or a `PostEvents`
    /// music's elements. Each is where its `origin` is, or else at `at`.
    fn events_of(&mut self, music: Value, at: Span) -> Vec<PostEvent> {
        let heap = self.env.vm().heap_mut();
        let events = if is_music_type(heap, music, "post-event-wrapper") {
            let key = heap.intern("elements");
            let mut items = Vec::new();
            let mut p = music_property(heap, music, key).unwrap_or(Value::NULL);
            while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                items.push(item);
                p = rest;
            }
            items
        } else {
            alloc::vec![music]
        };
        events
            .into_iter()
            .map(|event| PostEvent {
                span: self.origin_span(event).unwrap_or(at),
                direction: Direction::Neutral,
                kind: PostEventKind::Value(self.env.keep(event)),
            })
            .collect()
    }

    /// The span of `music`'s `origin`, where LilyPond's messages about it
    /// point.
    pub(crate) fn origin_span(&mut self, music: Value) -> Option<Span> {
        let heap = self.env.vm().heap_mut();
        let key = heap.intern("origin");
        let origin = music_property(heap, music, key)?;
        let input = heap.foreign::<lily_engine_music::Input>(origin)?;
        self.lexer.span_of(input)
    }

    /// The pitch of a `NOTENAME_PITCH` or a `PITCH_IDENTIFIER`.
    pub(crate) fn pitch_of(&mut self, token: &Token) -> Pitch {
        if matches!(token.kind, Tok::NotenamePitch | Tok::TonicnamePitch) {
            return self.lexer.pitch(token);
        }
        let value = self.env.value(token.value);
        match self.env.vm_ref().heap().foreign::<PitchValue>(value) {
            Some(p) => Pitch {
                octave: p.octave.clamp(i8::MIN.into(), i8::MAX.into()) as i8,
                step: p.notename.clamp(0, 6) as u8,
                alteration: p
                    .alteration
                    .checked_mul(lily_engine_music::Rational::int(4))
                    .filter(|q| q.denominator() == 1)
                    .map_or(0, |q| {
                        q.numerator().clamp(i8::MIN.into(), i8::MAX.into()) as i8
                    }),
            },
            None => self.lexer.pitch(token),
        }
    }

    /// `music_embedded_backup` at a `#` form, a `SCM_IDENTIFIER` or a
    /// lookup: its music, a post-event included, or None for a value that
    /// is no music, which is ignored with a warning unless it is
    /// unspecified; in lyrics mode, markup (a string too) is backed up as a
    /// `LYRIC_ELEMENT` and read as `lyric_element_music`.
    fn embedded_scm(&mut self) -> Parsed<Option<Music>> {
        let (value, span, placed) = self.embedded_scm_value()?;
        let music = self.scheme_music(value, span, placed);
        if music.is_none() && self.lex_mode(N) == L && self.is_markup(value) {
            let text = Text::Markup(self.env.keep(value));
            let kind = self.lyric_after(text)?;
            let span = Span {
                start: span.start,
                ..self.span_from(span)
            };
            return Ok(Some(Music { span, kind }));
        }
        if music.is_none() && value != lily_engine_scheme::Value::UNSPECIFIED {
            self.diagnose(
                Severity::Warning,
                At::Span(span),
                "Ignoring non-music expression",
            );
        }
        Ok(music)
    }

    /// `embedded_scm` at a `#` form, a `SCM_IDENTIFIER`, a Scheme
    /// function's call or a lookup: its value, its span, and whether music
    /// it holds is placed at it.
    pub(crate) fn embedded_scm_value(&mut self) -> Parsed<(lily_engine_scheme::Value, Span, bool)> {
        let start = self.peek(N);
        let value = match start.kind {
            Tok::Identifier(Id::Scm) => {
                self.next(N);
                self.env.value(start.value)
            }
            Tok::ScmToken => {
                self.next(N);
                self.scheme_value(&start)
            }
            Tok::Identifier(Id::ScmFunction) => self.function_call()?.0,
            _ => self.lookup()?,
        };
        let span = Span {
            start: start.loc.start,
            ..self.span_from(start.span)
        };
        // A lookup's music is a copy placed at it (`loc_on_copy`); a `#`
        // form's keeps its place.
        let placed = matches!(start.kind, Tok::Identifier(Id::Lookup | Id::Module));
        Ok((value, span, placed))
    }

    /// The music `value` of [`Parser::embedded_scm_value`] is, a post-event
    /// included, which the list attaches, a copy placed at `span` if
    /// `placed`; None for a value that is no music.
    pub(crate) fn scheme_music(
        &mut self,
        value: lily_engine_scheme::Value,
        span: Span,
        placed: bool,
    ) -> Option<Music> {
        self.env.vm_ref().heap().foreign::<MusicValue>(value)?;
        let value = match placed {
            true => self.placed_copy(value, Some(span)),
            false => value,
        };
        Some(self.music_of_value(value, span))
    }

    /// `music_embedded`, at a token that [`starts_music`]; None for Scheme
    /// that is no music.
    pub(crate) fn music_embedded(&mut self) -> Parsed<Option<Music>> {
        let start = self.peek(N);
        let kind = match start.kind {
            Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | CONTEXT
            | Tok::Identifier(Id::Music | Id::MusicFunction) => {
                return self.composite_music().map(Some);
            }
            kind if starts_basic(kind) => return self.composite_music().map(Some),
            TEMPO => self.tempo_event()?,
            CHANGE => self.context_change()?,
            kind if starts_property(kind) => self.music_property_def()?,
            Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction)
            | Tok::ScmToken => {
                return self.embedded_scm();
            }
            Tok::NotenamePitch | Tok::Identifier(Id::Pitch) => MusicKind::Note(self.note()?),
            Tok::TonicnamePitch => self.chord_entry()?,
            Tok::DrumPitch => self.drum_note()?,
            Tok::FigureOpen => self.figure_chord()?,
            Tok::RestName | Tok::ChordRepetition | Tok::MultiMeasureRest => {
                self.next(N);
                let duration = self.optional_duration()?;
                let post = self.post_events()?;
                match start.kind {
                    Tok::RestName => MusicKind::Rest {
                        skip: start.value == u32::from(b's'),
                        duration,
                        post,
                    },
                    Tok::ChordRepetition => MusicKind::ChordRepetition { duration, post },
                    _ => MusicKind::MultiMeasureRest { duration, post },
                }
            }
            Tok::AngleOpen => self.chord()?,
            Tok::Unsigned | Tok::Identifier(Id::Duration) => {
                let duration = self.duration()?;
                self.default_duration = duration;
                let post = self.post_events()?;
                MusicKind::Rhythm { duration, post }
            }
            Tok::Symbol | Tok::String | MARKUP => self.lyric_element_music()?,
            _ => match self.post_event()? {
                Some(mut events) if events.len() == 1 => {
                    MusicKind::PostEvent(events.pop().expect("one event"))
                }
                // In a music list, `post_event_cons` adds a `PostEvents`
                // music's events last first.
                Some(events) => MusicKind::PostEvents(events.into_iter().rev().collect()),
                None => return Err(self.syntax_error(&start, bison::MANY)),
            },
        };
        // `@1` of a `STRING` is where its token is located: its closing quote.
        let span = Span {
            start: start.loc.start,
            ..self.span_from(start.span)
        };
        Ok(Some(Music { span, kind }))
    }

    /// `lyric_element_music` at a `SYMBOL`, a `STRING` or `\markup`:
    /// `lyric_element optional_notemode_duration post_events`, a lyric
    /// event. Outside lyrics mode the element is an error, reported where
    /// Bison reduces it, before the next token is lexed. A string is placed
    /// where its token is located: at its closing quote.
    fn lyric_element_music(&mut self) -> Parsed<MusicKind> {
        let start = self.next(N);
        let lyrics = self.lex_mode(N) == L;
        let text = match start.kind {
            Tok::String => {
                if !lyrics {
                    self.error(start.loc, "string outside of text script or \\lyricmode");
                }
                Text::String(String::from(self.lexer.string(&start)))
            }
            MARKUP => {
                let markup = self.full_markup()?;
                if !lyrics {
                    let at = self.span_from(start.span);
                    self.error(at, "markup outside of text script or \\lyricmode");
                }
                Text::Markup(self.env.keep(markup))
            }
            // A word of lyrics mode has its `_` made spaces.
            _ if lyrics => Text::String(String::from(self.lexer.string(&start))),
            _ => {
                let text = String::from_utf8_lossy(self.lexer.text_of(start.span));
                self.error(start.loc, alloc::format!("not a note name: {text}"));
                Text::String(text.into_owned())
            }
        };
        self.lyric_after(text)
    }

    /// `lyric_element_music` after its element, `text`: the duration and
    /// the post-events.
    pub(crate) fn lyric_after(&mut self, text: Text) -> Parsed<MusicKind> {
        let duration = self.optional_duration()?;
        let post = self.post_events()?;
        Ok(MusicKind::Lyric {
            text,
            duration,
            post,
        })
    }

    /// `pitch_or_music` at a `NOTENAME_PITCH` or a `PITCH_IDENTIFIER`: `pitch exclamations questions
    /// octave_check maybe_notemode_duration erroneous_quotes optional_rest
    /// post_events`.
    fn note(&mut self) -> Parsed<Note> {
        let token = self.next(N);
        let (octaves, _) = self.quotes();
        let pitch = self.pitch_of(&token).up(octaves);
        self.note_after(pitch)
    }

    /// `pitch_or_music` after its pitch and the octave marks on it: what a
    /// pitch argument reads once a note of it is what its predicate takes.
    pub(crate) fn note_after(&mut self, pitch: Pitch) -> Parsed<Note> {
        let mut note = self.note_marks(pitch);
        let written = self.maybe_duration()?;
        let (misplaced, at) = self.quotes();
        note.rest = self.peek(N).kind == REST;
        if note.rest {
            self.next(N);
        }
        note.post = self.post_events()?;
        if let Some(at) = at {
            let message = if written.is_some() {
                "octave marks must precede duration"
            } else {
                "badly placed octave marks"
            };
            self.error(at, message);
            // Sorted to where they likely belong, as the action does.
            match &mut note.octave_check {
                Some(check) => *check = check.saturating_add(misplaced),
                None => note.pitch = note.pitch.up(misplaced),
            }
        }
        note.duration = written.unwrap_or(self.taken_default());
        Ok(note)
    }

    /// `pitch exclamations questions octave_check`, and in a chord
    /// `post_events` after them; the note's duration is left for the caller.
    /// Returns where the pitch's token is.
    fn chord_note(&mut self) -> Parsed<(Note, Token)> {
        let token = self.next(N);
        let pitch = self.pitch_of(&token);
        let (octaves, _) = self.quotes();
        Ok((self.note_marks(pitch.up(octaves)), token))
    }

    /// `exclamations questions octave_check` after a note's pitch.
    fn note_marks(&mut self, pitch: Pitch) -> Note {
        let mut note = Note {
            pitch,
            force_accidental: false,
            cautionary: false,
            octave_check: None,
            duration: self.taken_default(),
            rest: false,
            post: Vec::new(),
        };
        while self.peek(N).kind == Tok::Char(b'!') {
            self.next(N);
            note.force_accidental = true;
        }
        while self.peek(N).kind == Tok::Char(b'?') {
            self.next(N);
            note.cautionary = true;
        }
        if self.peek(N).kind == Tok::Char(b'=') {
            self.next(N);
            note.octave_check = Some(self.quotes().0);
        }
        note
    }

    /// `quotes`: a run of `'` or of `,`, as octaves, and where it starts.
    pub(crate) fn quotes(&mut self) -> (i8, Option<Span>) {
        let first = self.peek(N);
        let step = match first.kind {
            Tok::Char(b'\'') => 1,
            Tok::Char(b',') => -1,
            _ => return (0, None),
        };
        let mut octaves: i8 = 0;
        while self.peek(N).kind == first.kind {
            self.next(N);
            octaves = octaves.saturating_add(step);
        }
        (octaves, Some(first.loc))
    }

    /// `chord_body optional_notemode_duration post_events`, at `<`.
    fn chord(&mut self) -> Parsed<MusicKind> {
        self.next(N);
        let mut elements = Vec::new();
        loop {
            let token = self.peek(N);
            match token.kind {
                Tok::AngleClose => {
                    self.next(N);
                    break;
                }
                Tok::NotenamePitch | Tok::TonicnamePitch | Tok::Identifier(Id::Pitch) => {
                    let (mut note, token) = self.chord_note()?;
                    note.post = self.post_events()?;
                    let span = self.span_from(token.span);
                    elements.push(ChordElement::Note { span, note });
                }
                Tok::DrumPitch => {
                    self.next(N);
                    let post = self.post_events()?;
                    let span = self.span_from(token.span);
                    let drum = token.value;
                    elements.push(ChordElement::Drum { span, drum, post });
                }
                Tok::Identifier(
                    Id::MusicFunction | Id::Music | Id::Scm | Id::ScmFunction | Id::Lookup,
                )
                | Tok::ScmToken => {
                    if let Some(element) = self.chord_body_music()? {
                        elements.push(element);
                    }
                }
                _ => match self.post_event()? {
                    Some(events) => {
                        elements.extend(events.into_iter().map(ChordElement::PostEvent))
                    }
                    None => return Err(self.syntax_error(&token, bison::MANY)),
                },
            }
        }
        let duration = self.optional_duration()?;
        for element in &mut elements {
            if let ChordElement::Note { note, .. } = element {
                note.duration = duration;
            }
        }
        let post = self.post_events()?;
        Ok(MusicKind::Chord {
            elements,
            duration,
            post,
        })
    }

    /// `chord_body_element`'s `music_function_chord_body`: a music
    /// function's call, a `MUSIC_IDENTIFIER` or `embedded_scm`. A
    /// post-event is one; other music is unwrapped of its music wrappers
    /// and must be a rhythmic event, else `not a rhythmic event`; what is
    /// no music is dropped.
    fn chord_body_music(&mut self) -> Parsed<Option<ChordElement>> {
        let token = self.peek(N);
        let (value, span) = match token.kind {
            Tok::Identifier(Id::MusicFunction) => self.function_call()?,
            Tok::Identifier(Id::Music) => {
                self.next(N);
                let value = self.env.value(token.value);
                let span = self.from(&token);
                (self.placed_copy(value, Some(span)), span)
            }
            _ => {
                let (value, span, _) = self.embedded_scm_value()?;
                (value, span)
            }
        };
        let element = self.env.vm().heap_mut().intern("element");
        let heap = self.env.vm().heap_mut();
        if heap.foreign::<MusicValue>(value).is_none() {
            return Ok(None);
        }
        if lily_engine_music::is_music_type(heap, value, "post-event") {
            return Ok(Some(ChordElement::PostEvent(PostEvent {
                span,
                direction: Direction::Neutral,
                kind: PostEventKind::Value(self.env.keep(value)),
            })));
        }
        let mut music = value;
        while lily_engine_music::is_music_type(heap, music, "music-wrapper-music") {
            music = music_property(heap, music, element).unwrap_or(Value::FALSE);
        }
        if !lily_engine_music::is_music_type(heap, music, "rhythmic-event") {
            let at = self.span_from(token.span);
            self.error(at, "not a rhythmic event");
            return Ok(None);
        }
        Ok(Some(ChordElement::Music(self.env.keep(music))))
    }

    /// `optional_notemode_duration`: the duration written, which becomes
    /// the default, or the default.
    pub(crate) fn optional_duration(&mut self) -> Parsed<Duration> {
        Ok(self.maybe_duration()?.unwrap_or(self.taken_default()))
    }

    /// The default duration, as a note that writes none takes it.
    pub(crate) fn taken_default(&self) -> Duration {
        Duration {
            span: None,
            ..self.default_duration
        }
    }

    /// `maybe_notemode_duration`: a duration if one is written there, which
    /// becomes the default.
    pub(crate) fn maybe_duration(&mut self) -> Parsed<Option<Duration>> {
        if !matches!(
            self.peek(N).kind,
            Tok::Unsigned | Tok::Identifier(Id::Duration)
        ) {
            return Ok(None);
        }
        let duration = self.duration()?;
        self.default_duration = duration;
        Ok(Some(duration))
    }

    /// `duration`: `steno_duration multipliers`, at an `UNSIGNED` or a
    /// `DURATION_IDENTIFIER`.
    pub(crate) fn duration(&mut self) -> Parsed<Duration> {
        let token = self.next(N);
        self.duration_from(token)
    }

    /// `duration` after its `UNSIGNED` or `DURATION_IDENTIFIER`, `token`,
    /// was taken: an identifier's duration with the dots after it added
    /// (`make_duration`).
    pub(crate) fn duration_from(&mut self, token: Token) -> Parsed<Duration> {
        let mut dots: u8 = 0;
        while self.peek(N).kind == Tok::Char(b'.') {
            self.next(N);
            dots = dots.saturating_add(1);
        }
        let mut duration = if token.kind == Tok::Identifier(Id::Duration) {
            let value = self.env.value(token.value);
            let mut duration = self.duration_of(value);
            duration.dots = duration.dots.saturating_add(dots);
            duration
        } else {
            match duration_log(self.unsigned(&token)) {
                Some(log) => Duration::new(log.into(), dots),
                None => {
                    self.error(token.loc, "not a duration");
                    Duration::new(0, 0)
                }
            }
        };
        while self.peek(N).kind == Tok::Char(b'*') {
            self.next(N);
            let factor = self.peek(N);
            if let Tok::Identifier(Id::Number | Id::Scm) | Tok::ScmToken = factor.kind {
                // `multiplier_scm`, reduced before the next token is lexed.
                self.next(N);
                let value = if factor.kind == Tok::ScmToken {
                    self.scheme_value(&factor)
                } else {
                    self.env.value(factor.value)
                };
                match scale(self.env.vm().heap(), value) {
                    Some((num, den)) => duration.factor = times(duration.factor, num, den),
                    None => self.error(factor.loc, "not a multiplier"),
                }
                continue;
            }
            let (num, den) = match factor.kind {
                Tok::Unsigned => (self.unsigned(&factor), Some(1)),
                Tok::Fraction => {
                    let text = self.lexer.text_of(factor.span);
                    let slash = text.iter().position(|&b| b == b'/').unwrap_or(0);
                    let part = |t: &[u8]| core::str::from_utf8(t).ok()?.parse::<u64>().ok();
                    (part(&text[..slash]), part(&text[slash + 1..]))
                }
                _ => return Err(self.syntax_error(&factor, bison::MANY)),
            };
            self.next(N);
            if let (Some(num), Some(den)) = (num, den) {
                duration.factor = times(duration.factor, num, den);
            }
        }
        duration.span = Some(self.span_from(token.span));
        Ok(duration)
    }

    pub(crate) fn post_events(&mut self) -> Parsed<Vec<PostEvent>> {
        let mut events = Vec::new();
        while let Some(event) = self.post_event()? {
            events.extend(event);
        }
        Ok(events)
    }

    /// `post_event`, or None when the next token starts none: one event, or
    /// an `EVENT_IDENTIFIER`'s.
    pub(crate) fn post_event(&mut self) -> Parsed<Option<Vec<PostEvent>>> {
        let start = self.peek(N);
        match start.kind {
            Tok::Identifier(Id::EventFunction) => {
                Ok(Some(alloc::vec![self.event_call(Direction::Neutral)?]))
            }
            Tok::Char(b'^' | b'_' | b'-') => {
                self.next(N);
                self.after_direction(start).map(Some)
            }
            Tok::EUnsigned | Tok::Char(b':') => {
                let kind = self.direction_less()?;
                let span = self.span_from(start.span);
                Ok(Some(alloc::vec![PostEvent {
                    span,
                    direction: Direction::Neutral,
                    kind,
                }]))
            }
            Tok::Identifier(Id::Event) => {
                self.next(N);
                Ok(Some(self.event_identifier(&start)))
            }
            // `HYPHEN` and `EXTENDER`, which only lyrics mode lexes.
            Tok::Hyphen | Tok::Extender => {
                self.next(N);
                if self.lex_mode(N) != L {
                    self.error(start.loc, "have to be in Lyric mode for lyrics");
                }
                let kind = match start.kind {
                    Tok::Hyphen => PostEventKind::Hyphen,
                    _ => PostEventKind::Extender,
                };
                Ok(Some(alloc::vec![PostEvent {
                    span: start.loc,
                    direction: Direction::Neutral,
                    kind,
                }]))
            }
            _ => Ok(None),
        }
    }

    /// The post-event after `start`, its direction (`script_dir`), just
    /// taken. An `EVENT_IDENTIFIER`'s events take `^` and `_`; `-` leaves
    /// their own.
    pub(crate) fn after_direction(&mut self, start: Token) -> Parsed<Vec<PostEvent>> {
        let direction = match start.kind {
            Tok::Char(b'^') => Direction::Up,
            Tok::Char(b'_') => Direction::Down,
            _ => Direction::Neutral,
        };
        let token = self.peek(N);
        if let Tok::Identifier(Id::EventFunction | Id::MusicFunction | Id::ScmFunction) = token.kind
        {
            let (value, span) = self.function_call()?;
            return self.script_done(start, token, value, span);
        }
        if token.kind == Tok::Identifier(Id::Event) {
            self.next(N);
            let mut events = self.event_identifier(&token);
            for event in &mut events {
                event.direction = direction;
                self.set_direction(event);
            }
            // `post_event_cons` gives a `PostEvents` music's direction to
            // its events, but not its place.
            if let [event] = &mut events[..] {
                event.span = self.span_from(start.span);
                self.place_event(event);
            }
            return Ok(events);
        }
        let kind = match token.kind {
            Tok::Unsigned => {
                self.next(N);
                PostEventKind::Fingering(self.unsigned(&token).unwrap_or(u64::MAX) as u32)
            }
            // `gen_text_def`: markup, a string or a word.
            MARKUP => {
                self.next(N);
                let markup = self.full_markup()?;
                PostEventKind::Text(Text::Markup(self.env.keep(markup)))
            }
            Tok::String => {
                self.next(N);
                PostEventKind::Text(self.lexer.string(&token).into())
            }
            Tok::Symbol => {
                self.next(N);
                let text = String::from_utf8_lossy(self.lexer.text_of(token.span));
                PostEventKind::Text(Text::String(text.into_owned()))
            }
            // `gen_text_def`'s `embedded_scm`: `create-script`.
            Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                self.next(N);
                let value = match token.kind {
                    Tok::ScmToken => self.scheme_value(&token),
                    _ => self.env.value(token.value),
                };
                return Ok(self.script_scheme(start, token, value));
            }
            Tok::EUnsigned | Tok::Char(b':') => self.direction_less()?,
            Tok::Char(c @ (b'^' | b'+' | b'-' | b'!' | b'.' | b'_')) => {
                self.next(N);
                PostEventKind::Script(c)
            }
            Tok::AngleClose => {
                self.next(N);
                PostEventKind::Script(b'>')
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        Ok(alloc::vec![PostEvent {
            span: self.span_from(start.span),
            direction,
            kind,
        }])
    }

    /// The post-event of a call after a direction, `start`, the call's
    /// first token `token`, its value and its span: an event function's
    /// event (`direction_reqd_event`); a music function's, which must be
    /// a post-event (`script_dir music_function_call`), else `post-event
    /// expected` and none; a Scheme function's value as `create-script`
    /// takes it. Placed from the direction, its direction set.
    pub(crate) fn script_done(
        &mut self,
        start: Token,
        token: Token,
        value: Value,
        span: Span,
    ) -> Parsed<Vec<PostEvent>> {
        let direction = match start.kind {
            Tok::Char(b'^') => Direction::Up,
            Tok::Char(b'_') => Direction::Down,
            _ => Direction::Neutral,
        };
        match token.kind {
            Tok::Identifier(Id::EventFunction) => {
                let event = PostEvent {
                    span: self.span_from(start.span),
                    direction,
                    kind: PostEventKind::Value(self.env.keep(value)),
                };
                self.set_direction(&event);
                self.place_event(&event);
                Ok(alloc::vec![event])
            }
            Tok::Identifier(Id::MusicFunction) => {
                if !is_music_type(self.env.vm_ref().heap(), value, "post-event") {
                    self.error(span, "post-event expected");
                    return Ok(Vec::new());
                }
                let event = PostEvent {
                    span: self.span_from(start.span),
                    direction,
                    kind: PostEventKind::Value(self.env.keep(value)),
                };
                self.place_event(&event);
                self.set_direction(&event);
                Ok(alloc::vec![event])
            }
            _ => Ok(self.script_scheme(start, token, value)),
        }
    }

    /// `gen_text_def`'s `embedded_scm` after a direction, `start`, its
    /// first token `token` and its value: markup is a text script; a
    /// post-event is itself (`create-script`), placed from the direction,
    /// its direction set; else `not an articulation` and none.
    fn script_scheme(&mut self, start: Token, token: Token, value: Value) -> Vec<PostEvent> {
        let direction = match start.kind {
            Tok::Char(b'^') => Direction::Up,
            Tok::Char(b'_') => Direction::Down,
            _ => Direction::Neutral,
        };
        let at = Span {
            start: token.loc.start,
            ..self.span_from(token.span)
        };
        let kind = if self.is_markup(value) {
            PostEventKind::Text(Text::Markup(self.env.keep(value)))
        } else if self
            .env
            .vm_ref()
            .heap()
            .foreign::<MusicValue>(value)
            .is_some()
            && is_music_type(self.env.vm_ref().heap(), value, "post-event")
        {
            let event = PostEvent {
                span: self.span_from(start.span),
                direction,
                kind: PostEventKind::Value(self.env.keep(value)),
            };
            self.set_direction(&event);
            self.place_event(&event);
            return alloc::vec![event];
        } else {
            self.error(at, "not an articulation");
            return Vec::new();
        };
        alloc::vec![PostEvent {
            span: self.span_from(start.span),
            direction,
            kind,
        }]
    }

    /// `event_function_event`, after a direction or none: the call's value,
    /// its `direction` set to a direction written, as `post_event_nofinger`
    /// sets it on music.
    fn event_call(&mut self, direction: Direction) -> Parsed<PostEvent> {
        let (value, span) = self.function_call()?;
        let event = PostEvent {
            span,
            direction,
            kind: PostEventKind::Value(self.env.keep(value)),
        };
        self.set_direction(&event);
        Ok(event)
    }

    /// `post_event_nofinger`'s `set_property (m, "direction", …)` on a
    /// post-event that is a music value, for a direction written.
    fn set_direction(&mut self, event: &PostEvent) {
        let dir = match event.direction {
            Direction::Up => 1,
            Direction::Down => -1,
            Direction::Neutral => return,
        };
        if let PostEventKind::Value(index) = event.kind {
            let value = self.env.value(index);
            let heap = self.env.vm().heap_mut();
            if heap.foreign::<MusicValue>(value).is_some() {
                let key = heap.intern("direction");
                let dir = heap.integer(dir);
                set_property(heap, value, key, dir);
            }
        }
    }

    /// `post_event_nofinger`'s `set_spot` on a post-event that is a music
    /// value: its `origin` from the direction to its end.
    fn place_event(&mut self, event: &PostEvent) {
        if let PostEventKind::Value(index) = event.kind {
            let value = self.env.value(index);
            let origin = self.origin(event.span);
            lily_engine_music::set_origin(self.env.vm().heap_mut(), value, origin);
        }
    }

    /// `direction_less_event` in P2: `string_number_event` and `tremolo_type`.
    fn direction_less(&mut self) -> Parsed<PostEventKind> {
        let token = self.next(N);
        if token.kind == Tok::EUnsigned {
            let digits = &self.lexer.text_of(token.span)[1..];
            let number = core::str::from_utf8(digits)
                .ok()
                .and_then(|d| d.parse().ok());
            return Ok(PostEventKind::StringNumber(number.unwrap_or(u32::MAX)));
        }
        let value = self.peek(N);
        if value.kind != Tok::Unsigned {
            return Ok(PostEventKind::Tremolo(self.default_tremolo));
        }
        // `':' UNSIGNED` is reduced before the next token is lexed.
        self.next(N);
        let number = self.unsigned(&value);
        Ok(PostEventKind::Tremolo(match duration_log(number) {
            Some(_) => {
                self.default_tremolo = number.unwrap_or(0) as u32;
                self.default_tremolo
            }
            None => {
                self.error(value.loc, "not a duration");
                self.default_tremolo
            }
        }))
    }
}
