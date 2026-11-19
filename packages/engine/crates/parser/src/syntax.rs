//! Spanned syntax (02 §5.3, §5.5): what the parser builds from a file before
//! there is music. Each node keeps the span of its text, from its first
//! token's first byte to its last token's last byte, and what the grammar's
//! actions computed from the tokens (pitches with their octave marks, the
//! duration a note takes, by default included), but no music object: the
//! parser makes music of it with the music crate's constructors where
//! LilyPond's grammar hands music on (`function.rs`, N2), and refers to
//! music made already, a function's result or a variable's, by
//! [`MusicKind::Value`].

use crate::pitch::Pitch;
use crate::source::Span;
use alloc::string::String;
use alloc::vec::Vec;

/// A toplevel item: `lilypond`'s `toplevel_expression`.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub span: Span,
    pub kind: ItemKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// `composite_music`, for `toplevel-music-handler`.
    Music(Music),
    /// `\header { … }`: its fields, defined into the header's module.
    Header(Vec<Assignment>),
    /// `\markup …` or `\markuplist …`, for `toplevel-text-handler`, or
    /// `\markup name = …`.
    Markup,
    /// `name = value`, defined into the file's module.
    Assignment(Assignment),
    /// A `\name` or `\name.key` whose value is no music
    /// (`embedded_scm_active`): a string is text for the toplevel's
    /// handler; anything else LilyPond refuses unless P5 and P7 take it.
    /// Or a `#` form, evaluated and ignored.
    Scheme(Span),
    /// `\score { … }`: its items, music, headers, output definitions and
    /// Scheme, of which the first music is the score's.
    Score(Vec<Item>),
    /// `\book { … }`: its items, music, scores, books' parts, `\paper`,
    /// headers, markup and Scheme.
    Book(Vec<Item>),
    /// `\bookpart { … }`: a book's items but books' parts.
    BookPart(Vec<Item>),
    /// `\paper`, `\layout` or `\midi { … }`: its items, assignments,
    /// defined into its scope, Scheme and music.
    OutputDef(OutputKind, Vec<Item>),
    /// `\context { … }` (`context_def_spec_block`): its mods, in an output
    /// definition or as a value.
    ContextDef(Vec<ContextMod>),
}

/// An output definition's kind, its `output-def-kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Paper,
    Layout,
    Midi,
}

impl OutputKind {
    /// Its name, its keyword's and its `output-def-kind`'s.
    pub fn name(self) -> &'static str {
        match self {
            OutputKind::Paper => "paper",
            OutputKind::Layout => "layout",
            OutputKind::Midi => "midi",
        }
    }
}

/// `assignment`: a name, as its word or string, the path after it in
/// `name.path = value`, and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub name: Span,
    pub path: Vec<Key>,
    pub value: Value,
}

/// A key of a symbol list or a property path: a symbol or an index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Symbol(String),
    Index(u64),
}

/// `identifier_init` as far as P3 reads it. Each value is also defined
/// into the module: music and post-events as their music, frozen, and
/// pitches as the music crate's.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A string, a word, or an unknown command's name.
    String(String),
    /// A number expression (`number_expression`), where it was written.
    Number(Span),
    /// `a.b,3`: `symbol_list_part_bare '.' property_path`.
    SymbolList(Vec<Key>),
    Music(Music),
    /// `post_event_nofinger post_events`.
    PostEvents(Vec<PostEvent>),
    /// A pitch with nothing that would make it a note.
    Pitch(Pitch),
    /// Another Scheme value, where it was written: a fraction, a `\name`
    /// whose value is one, a `\name.key` lookup, or a `#` form.
    Scheme(Span),
    /// A `\score`, a `\book`, a `\bookpart`, an output definition, a
    /// `\header` (`header_modification`) or a `\context { … }`, as a
    /// toplevel item.
    Block(alloc::boxed::Box<Item>),
    /// `\with` (`context_modification`).
    With(With),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Music {
    pub span: Span,
    pub kind: MusicKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MusicKind {
    /// `{ … }`.
    Sequential(Vec<Music>),
    /// `<< … >>`.
    Simultaneous(Vec<Music>),
    /// A pitch with what may follow it (`pitch_or_music`, `pitch_as_music`).
    Note(Note),
    /// `r` (a rest) or `s` (a skip), with a duration and post-events.
    Rest {
        skip: bool,
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// `R`.
    MultiMeasureRest {
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// `q`.
    ChordRepetition {
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// `< … >` with a duration and post-events (`note_chord_element`).
    Chord {
        elements: Vec<ChordElement>,
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// A duration alone, a note without a pitch (`duration post_events` in
    /// `music_embedded`).
    Rhythm {
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// A word, a string or markup in music (`lyric_element_music`): an
    /// error outside lyric mode, which the parser reported.
    Lyric {
        text: Text,
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// A post-event standing in a music list by itself.
    PostEvent(PostEvent),
    /// An `EVENT_IDENTIFIER` of several events standing in a music list, its
    /// events as `post_event_cons` adds them: last first.
    PostEvents(Vec<PostEvent>),
    /// What a syntax error left of a music list's element (`music_list
    /// error`, music with `error-found`).
    Error,
    /// A music value: a music function's result, or what a `\name` or a `#`
    /// form gives; its index in [`crate::Env::value`].
    Value(u32),
    /// `\new` or `\context` and the music it applies to
    /// (`contexted_basic_music`).
    Context(alloc::boxed::Box<ContextMusic>),
    /// `\change Type = id` (`context_change`): the indices in
    /// [`crate::Env::value`] of the type's symbol and the id's string.
    Change { context_type: u32, id: u32 },
    /// `\set`, `\unset`, `\override` or `\revert`
    /// (`music_property_def`).
    Property(Property),
    /// `\repeat` (`repeated_music`, P9).
    Repeat(alloc::boxed::Box<Repeat>),
    /// `\alternative` and its music (`alternative_music`).
    Alternative(alloc::boxed::Box<Music>),
    /// `\tempo` (`tempo_event`).
    Tempo(Tempo),
    /// `\chordmode` and `\chords`' `UnrelativableMusic` (P10).
    Unrelativable(alloc::boxed::Box<Music>),
    /// A chord of chord mode (`new_chord post_events`): its notes, made by
    /// `construct-chord-elements` and placed at `at`, the chord's own span,
    /// as a list's index in [`crate::Env::value`]; and its post-events.
    ChordEntry {
        at: Span,
        notes: u32,
        post: Vec<PostEvent>,
    },
    /// A drum note (`DRUM_PITCH optional_notemode_duration post_events`),
    /// placed at `at`, its drum pitch and duration: its drum type's index.
    Drum {
        at: Span,
        drum: u32,
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// Figures in `< >` with a duration and post-events: each figure's
    /// `BassFigureEvent`, made as it was parsed, by its index.
    Figures {
        figures: Vec<u32>,
        duration: Duration,
        post: Vec<PostEvent>,
    },
    /// `\lyricsto id` and its lyrics (P8), or `\lyricsto Type = id`
    /// (P12): the indices in [`crate::Env::value`] of the id's string and
    /// the type's symbol.
    LyricCombine {
        id: u32,
        context_type: Option<u32>,
        music: alloc::boxed::Box<Music>,
    },
}

/// `\repeat type count music`, and the `\alternative` after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Repeat {
    /// The indices in [`crate::Env::value`] of the type's string
    /// (`simple_string`) and the count (`unsigned_integer`).
    pub kind: u32,
    pub times: u32,
    pub body: Music,
    /// The [`MusicKind::Alternative`] after the body, or None.
    pub alternative: Option<Music>,
}

/// `\tempo`: a text, a metronome mark, or both.
#[derive(Clone, Debug, PartialEq)]
pub struct Tempo {
    /// The text's index in [`crate::Env::value`]: a string or markup.
    pub text: Option<u32>,
    /// The mark's unit (`steno_duration`) and the index of its count
    /// (`tempo_range`): a number, or a pair of them.
    pub mark: Option<(Duration, u32)>,
}

/// `music_property_def`: an operation on a property of a context or of a
/// grob, in a context.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub op: PropertyOp,
    /// The indices in [`crate::Env::value`] of the context's symbol and of
    /// the property's (`\set`, `\unset`) or the grob's path (`\override`,
    /// `\revert`); None when the path or a type was wrong, which makes
    /// `unspecified-music`.
    pub target: Option<(u32, u32)>,
    /// The value of `\set` and `\override` (`scalar`): its index.
    pub value: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyOp {
    Set,
    Unset,
    Override,
    Revert,
}

/// `context_prefix` and its music.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMusic {
    /// `\new` (`context-create`), or `\context` (`context-find-or-create`).
    pub new: bool,
    /// The context's type, as `symbol` gives it: its index in
    /// [`crate::Env::value`].
    pub context_type: u32,
    /// `= id` (`optional_id`): its string's index, or None.
    pub id: Option<u32>,
    /// The `\with`s after it.
    pub with: Vec<With>,
    /// Their mods appended (`optional_context_mods`): the list's index.
    pub ops: u32,
    pub music: Music,
}

/// `context_modification`: `\with { … }`, or `\with` and a value.
#[derive(Clone, Debug, PartialEq)]
pub struct With {
    pub span: Span,
    /// The mods in its braces; none for a value.
    pub mods: Vec<ContextMod>,
    /// The context modification it gives: its index in [`crate::Env::value`].
    pub value: u32,
}

/// An item of `\with { … }` (`context_mod_list`) or of `\context { … }`
/// (`context_def_spec_body`).
#[derive(Clone, Debug, PartialEq)]
pub enum ContextMod {
    /// `\consists` and the others of `context_def_mod`, and its argument:
    /// the mod's symbol (`consists`, `default-child`, `context-name`, …).
    Mod { span: Span, name: &'static str },
    /// A `\with` in `\context { … }`.
    With(With),
    /// `property_operation`: `name = value`, `\unset`, `\override` or
    /// `\revert`.
    Property(Span),
    /// A context modification, a context definition, music or a Scheme
    /// value (`context_mod_arg`).
    Value(Span),
}

/// A note's pitch and what the grammar reads around it.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// With its octave marks applied, and the octave marks misplaced after
    /// the duration too when there is no octave check (as the action sorts
    /// them).
    pub pitch: Pitch,
    /// `!`: `force-accidental`.
    pub force_accidental: bool,
    /// `?`: `cautionary` (and `force-accidental`).
    pub cautionary: bool,
    /// `=` and its octave marks, the misplaced ones added: the octave the
    /// pitch must have (`absolute-octave` is this less one).
    pub octave_check: Option<i8>,
    pub duration: Duration,
    /// `\rest` after the duration: a rest at the pitch.
    pub rest: bool,
    pub post: Vec<PostEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChordElement {
    /// A note of the chord: its duration is the chord's.
    Note {
        span: Span,
        note: Note,
    },
    PostEvent(PostEvent),
    /// `music_function_chord_body`'s music, a rhythmic event, the index of
    /// its value: its duration is the chord's.
    Music(u32),
    /// A drum note of the chord, its drum type's index: its duration is the
    /// chord's.
    Drum {
        span: Span,
        drum: u32,
        post: Vec<PostEvent>,
    },
}

/// A duration as `make_duration` gives it: `log` is 0 for a whole note, 2
/// for a quarter; `factor` is the product of the multipliers, reduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    /// Where it was written; None when the note took the default duration
    /// (the last written one, in parse order).
    pub span: Option<Span>,
    /// Its log, below 0 for `\breve`, `\longa` and `\maxima`.
    pub log: i32,
    pub dots: u8,
    pub factor: (u64, u64),
}

impl Duration {
    /// `Duration (log, dots)`.
    pub const fn new(log: i32, dots: u8) -> Duration {
        Duration {
            span: None,
            log,
            dots,
            factor: (1, 1),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostEvent {
    pub span: Span,
    /// `^` up, `_` down, `-` or nothing: neither.
    pub direction: Direction,
    pub kind: PostEventKind,
}

/// A text event's text: a string or a word as written, or markup (P7), its
/// index in [`crate::Env::value`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Text {
    String(String),
    Markup(u32),
}

impl From<&str> for Text {
    fn from(text: &str) -> Text {
        Text::String(text.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Neutral,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PostEventKind {
    /// `-1`, `^2`: a `FingeringEvent`.
    Fingering(u32),
    /// `\1`: a `StringNumberEvent`.
    StringNumber(u32),
    /// `:` or `:16`: a `TremoloEvent` of that type.
    Tremolo(u32),
    /// `-.`, `->`, …: the character after the direction, whose script is
    /// the variable `dash` and the character's name (`dashDot`), looked up
    /// when P3 has a module.
    Script(u8),
    /// `-"text"`, `^word` or `_\markup …` (`gen_text_def`): a
    /// `TextScriptEvent`.
    Text(Text),
    /// `--` in lyrics: a `HyphenEvent` (P8).
    Hyphen,
    /// `__` in lyrics: an `ExtenderEvent`.
    Extender,
    /// A post-event's music value: an event function's result, or what an
    /// `EVENT_IDENTIFIER` or a `#` form gives; its index in
    /// [`crate::Env::value`].
    Value(u32),
}
