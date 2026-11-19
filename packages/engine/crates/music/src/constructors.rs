//! The music the parser's actions make (N2, 04 §6.2), natively:
//! `define-music-types.scm`'s `make-music`, `parser.yy`'s `MY_MAKE_MUSIC`,
//! `add_post_events` and `ly:set-origin!`, and the constructors of
//! `ly-syntax-constructors.scm` that P2–P5's actions call
//! (`sequential-music`, `simultaneous-music`, `event-chord`,
//! `multi-measure-rest`, `repetition-chord`, `lyric-event`,
//! `unspecified-music`), and P6's `context-create`,
//! `context-find-or-create`, `context-change`, `property-set`,
//! `property-unset`, `property-override` and `property-revert`, and P9's
//! `repeat` and `repeat-alt` (with `music-functions.scm`'s `make-repeat`),
//! `alternative` and `tempo`, P8's `lyric-combine` and `add-lyrics`, and
//! P10's `unrelativable-music`. The rest of that file (markups) comes with
//! P11. What LilyPond warns of while it makes them is given back as
//! [`Warning`]s, for the parser to report in its place.
//!
//! What they make is live music, its properties in the order LilyPond's
//! code sets them: a property not there yet goes in front, as
//! `ly:music-set-property!` puts it. The parser freezes it once it hands it
//! on ([`crate::freeze`]), so that a toplevel item's music is one tree.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Heap, Value};

use crate::music::{Live, Music, State, assq_named, set_property};
use crate::{Duration, Input, is_music_type, music_property};

/// A warning of a constructor: `ly:warning`'s, which has no place, or
/// `ly:music-warning`'s and `ly:music-error`'s, at the music's origin.
#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    Plain(String),
    Music(Value, String),
    MusicError(Value, String),
}

/// The items of a proper list.
fn items(heap: &Heap, mut list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
    out
}

const JUNKED: &str = "More alternatives than repeats.  Junking excess alternatives";

/// What makes music of the 131 types: `%music-properties`, the table of
/// their immutable properties that `make-music` reads.
#[derive(Clone, Copy, Debug)]
pub struct Maker {
    table: Value,
}

impl Maker {
    /// A maker of `table`, `(lily)`'s `%music-properties`.
    pub fn new(table: Value) -> Maker {
        Maker { table }
    }

    /// `make-music`: live music of type `name` with `props` set in order;
    /// None for a type the table does not have.
    pub fn make(&self, heap: &mut Heap, name: &str, props: &[(&str, Value)]) -> Option<Value> {
        let immutable = heap.cdr(assq_named(heap, name, self.table)?)?;
        let music = heap.wrap(Music(State::Live(Live {
            kind: crate::types::kind(name),
            immutable,
            props: Value::NULL,
        })));
        for &(key, value) in props {
            let key = heap.intern(key);
            set_property(heap, music, key, value);
        }
        Some(music)
    }

    /// `MY_MAKE_MUSIC`: music made with its `origin` set first, then
    /// `props`.
    pub fn make_at(
        &self,
        heap: &mut Heap,
        name: &str,
        origin: Value,
        props: &[(&str, Value)],
    ) -> Option<Value> {
        let music = self.make(heap, name, &[("origin", origin)])?;
        for &(key, value) in props {
            let key = heap.intern(key);
            set_property(heap, music, key, value);
        }
        Some(music)
    }

    /// `make-music` of `name` and then `ly:set-origin!`, as most
    /// constructors are.
    fn make_set(
        &self,
        heap: &mut Heap,
        name: &str,
        props: &[(&str, Value)],
        origin: Value,
    ) -> Option<Value> {
        let music = self.make(heap, name, props)?;
        set_origin(heap, music, origin);
        Some(music)
    }

    /// `sequential-music`: of a list of music.
    pub fn sequential_music(
        &self,
        heap: &mut Heap,
        elements: Value,
        origin: Value,
    ) -> Option<Value> {
        self.make_set(heap, "SequentialMusic", &[("elements", elements)], origin)
    }

    /// `simultaneous-music`: of a list of music.
    pub fn simultaneous_music(
        &self,
        heap: &mut Heap,
        elements: Value,
        origin: Value,
    ) -> Option<Value> {
        self.make_set(heap, "SimultaneousMusic", &[("elements", elements)], origin)
    }

    /// `event-chord`: of a list of music.
    pub fn event_chord(&self, heap: &mut Heap, elements: Value, origin: Value) -> Option<Value> {
        self.make_set(heap, "EventChord", &[("elements", elements)], origin)
    }

    /// `multi-measure-rest`: its text scripts and articulations become
    /// `MultiMeasureTextEvent`s and `MultiMeasureArticulationEvent`s
    /// (`script-to-mmrest-text`).
    pub fn multi_measure_rest(
        &self,
        heap: &mut Heap,
        duration: Value,
        articulations: &[Value],
        origin: Value,
    ) -> Option<Value> {
        let mut items = Vec::with_capacity(articulations.len());
        for &event in articulations {
            items.push(self.script_to_mmrest_text(heap, event)?);
        }
        let articulations = heap.list(&items);
        let props = [("articulations", articulations), ("duration", duration)];
        self.make_set(heap, "MultiMeasureRestMusic", &props, origin)
    }

    /// `script-to-mmrest-text`: `(make-music 'MultiMeasureTextEvent music)`
    /// of a text script.
    fn script_to_mmrest_text(&self, heap: &mut Heap, event: Value) -> Option<Value> {
        let name = if is_music_type(heap, event, "text-script-event") {
            "MultiMeasureTextEvent"
        } else if is_music_type(heap, event, "articulation-event") {
            "MultiMeasureArticulationEvent"
        } else {
            return Some(event);
        };
        self.make_from(heap, name, event)
    }

    /// `(make-music name music)`: music of type `name` that takes the
    /// mutable properties of `music`, in their order.
    fn make_from(&self, heap: &mut Heap, name: &str, music: Value) -> Option<Value> {
        let mut props = Vec::new();
        let mut p = crate::music::mutable_properties(heap, music);
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            props.push((heap.car(entry)?, heap.cdr(entry)?));
            p = rest;
        }
        let made = self.make(heap, name, &[])?;
        for &(key, value) in props.iter().rev() {
            set_property(heap, made, key, value);
        }
        Some(made)
    }

    /// `repetition-chord`: `q` as an event chord of its duration and
    /// post-events, which `expand-repeat-chords!` fills in.
    pub fn repetition_chord(
        &self,
        heap: &mut Heap,
        duration: Value,
        articulations: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [("duration", duration), ("elements", articulations)];
        self.make_set(heap, "EventChord", &props, origin)
    }

    /// `lyric-event` (`make-lyric-event`).
    pub fn lyric_event(
        &self,
        heap: &mut Heap,
        text: Value,
        duration: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [("duration", duration), ("text", text)];
        self.make_set(heap, "LyricEvent", &props, origin)
    }

    /// `unspecified-music`.
    pub fn unspecified_music(&self, heap: &mut Heap, origin: Value) -> Option<Value> {
        self.make_set(heap, "Music", &[], origin)
    }

    /// `context-create` (`\new`, with `create_new`) and
    /// `context-find-or-create` (`\context`): `music-functions.scm`'s
    /// `context-spec-music` of `music`, its `context-id` if `id` is a
    /// string, and `ops`, a list of mods, as its `property-operations`.
    #[allow(clippy::too_many_arguments)]
    pub fn context_specification(
        &self,
        heap: &mut Heap,
        context_type: Value,
        id: Value,
        ops: Value,
        create_new: bool,
        music: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [("element", music), ("context-type", context_type)];
        let csm = self.make(heap, "ContextSpeccedMusic", &props)?;
        let mut set = Vec::new();
        if id.tag() == lily_engine_scheme::Tag::String {
            set.push(("context-id", id));
        }
        // `(if mods …)`: the parser's mods are a list, true even empty.
        if ops != Value::FALSE {
            set.push(("property-operations", ops));
        }
        if create_new {
            set.push(("create-new", Value::TRUE));
        }
        for (key, value) in set {
            let key = heap.intern(key);
            set_property(heap, csm, key, value);
        }
        set_origin(heap, csm, origin);
        Some(csm)
    }

    /// `unrelativable-music`: `\chordmode`'s and `\chords`' wrapper of
    /// `music`, which `\relative` leaves as it is.
    pub fn unrelativable_music(
        &self,
        heap: &mut Heap,
        music: Value,
        origin: Value,
    ) -> Option<Value> {
        self.make_set(heap, "UnrelativableMusic", &[("element", music)], origin)
    }

    /// `context-change`: `\change Type = id`.
    pub fn context_change(
        &self,
        heap: &mut Heap,
        context_type: Value,
        id: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [("change-to-type", context_type), ("change-to-id", id)];
        self.make_set(heap, "ContextChange", &props, origin)
    }
    /// `property-set`: `\set`'s `PropertySet` of `symbol` to `value`, in
    /// `context-spec-music` of `context`. Both are placed at `origin`: the
    /// one by `ly:set-origin!`, the other as a syntax function's call
    /// places its result. The parser checks the arguments' types.
    pub fn property_set(
        &self,
        heap: &mut Heap,
        context: Value,
        symbol: Value,
        value: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [("symbol", symbol), ("value", value)];
        let music = self.make_set(heap, "PropertySet", &props, origin)?;
        self.in_context(heap, context, music, origin)
    }

    /// `property-unset`: `\unset`'s `PropertyUnset` of `symbol`, in
    /// `context-spec-music` of `context`, both placed at `origin`.
    pub fn property_unset(
        &self,
        heap: &mut Heap,
        context: Value,
        symbol: Value,
        origin: Value,
    ) -> Option<Value> {
        let music = self.make_set(heap, "PropertyUnset", &[("symbol", symbol)], origin)?;
        self.in_context(heap, context, music, origin)
    }

    /// `property-override`: `\override`'s `OverrideProperty` of the grob
    /// that `path` begins with, its property path the rest of `path`, set
    /// to `value`, in `context-spec-music` of `context`, both placed at
    /// `origin`, as [`Maker::property_set`].
    pub fn property_override(
        &self,
        heap: &mut Heap,
        context: Value,
        path: Value,
        value: Value,
        origin: Value,
    ) -> Option<Value> {
        let (grob, rest) = (heap.car(path)?, heap.cdr(path)?);
        let props = [
            ("symbol", grob),
            ("grob-property-path", rest),
            ("grob-value", value),
            ("pop-first", Value::TRUE),
        ];
        let music = self.make_set(heap, "OverrideProperty", &props, origin)?;
        self.in_context(heap, context, music, origin)
    }

    /// `property-revert`: `\revert`'s `RevertProperty` of the grob that
    /// `path` begins with and the rest of `path`, in `context-spec-music`
    /// of `context`, both placed at `origin`.
    pub fn property_revert(
        &self,
        heap: &mut Heap,
        context: Value,
        path: Value,
        origin: Value,
    ) -> Option<Value> {
        let (grob, rest) = (heap.car(path)?, heap.cdr(path)?);
        let props = [("symbol", grob), ("grob-property-path", rest)];
        let music = self.make_set(heap, "RevertProperty", &props, origin)?;
        self.in_context(heap, context, music, origin)
    }

    /// `context-spec-music` of `music` and `context`, with no id and no
    /// mods, placed at `origin`.
    fn in_context(
        &self,
        heap: &mut Heap,
        context: Value,
        music: Value,
        origin: Value,
    ) -> Option<Value> {
        let (id, ops) = (Value::NULL, Value::FALSE);
        self.context_specification(heap, context, id, ops, false, music, origin)
    }
}

impl Maker {
    /// `alternative`: `\alternative`'s music as `SequentialAlternativeMusic`,
    /// which it is already, or which takes sequential music's properties;
    /// anything else is an error but unspecified music, and gives one
    /// with no elements.
    pub fn alternative(
        &self,
        heap: &mut Heap,
        music: Value,
        origin: Value,
        warnings: &mut Vec<Warning>,
    ) -> Option<Value> {
        if is_music_type(heap, music, "sequential-alternative-music") {
            return Some(music);
        }
        let made = if is_music_type(heap, music, "sequential-music") {
            self.make_from(heap, "SequentialAlternativeMusic", music)?
        } else {
            let key = heap.intern("void");
            if music_property(heap, music, key).is_none_or(|v| v == Value::FALSE) {
                warnings.push(Warning::MusicError(
                    music,
                    "sequential music required".into(),
                ));
            }
            self.make(heap, "SequentialAlternativeMusic", &[])?
        };
        set_origin(heap, made, origin);
        Some(made)
    }

    /// `repeat` and `repeat-alt`: `make-repeat` of `name`, `times`, `body`
    /// and `alternatives` (`'()` for none), placed at `origin`. Each
    /// `SequentialAlternativeMusic` in `body`, but in repeats in it, and
    /// `alternatives` get volta numbers unless one of their elements has
    /// them (`elaborate-alternative-music`).
    #[allow(clippy::too_many_arguments)]
    pub fn repeat(
        &self,
        heap: &mut Heap,
        name: &str,
        times: Value,
        body: Value,
        alternatives: Value,
        origin: Value,
        warnings: &mut Vec<Warning>,
    ) -> Option<Value> {
        let times = match heap.to_i64(times) {
            Some(n) if n >= 1 => heap.integer(n),
            Some(_) => heap.integer(1),
            None => times,
        };
        let count = heap.to_i64(times).unwrap_or(i64::MAX);
        let kind = match name {
            "volta" => "VoltaRepeatedMusic",
            "segno" => "SegnoRepeatedMusic",
            "unfold" => "UnfoldedRepeatedMusic",
            "percent" => "PercentRepeatedMusic",
            "tremolo" => "TremoloRepeatedMusic",
            _ => {
                let mut text = String::from("unknown repeat type `");
                let quoted = heap.string(name);
                heap.write(quoted, &mut text);
                text.push_str("': must be volta, unfold, percent, or tremolo");
                warnings.push(Warning::Plain(text));
                "VoltaRepeatedMusic"
            }
        };
        let alt_music = if heap.foreign::<Music>(alternatives).is_some() {
            if !is_music_type(heap, alternatives, "sequential-alternative-music") {
                let text = "alternative music expected".into();
                warnings.push(Warning::Music(alternatives, text));
            }
            alternatives
        } else {
            let key = heap.intern("origin");
            let at = music_property(heap, body, key).unwrap_or(Value::NULL);
            let props = [("elements", alternatives), ("origin", at)];
            self.make(heap, "SequentialAlternativeMusic", &props)?
        };
        let element = self.map_alternatives(heap, body, count, body, warnings)?;
        let elaborated = self.elaborate(heap, alt_music, count, body, warnings)?;
        let key = heap.intern("elements");
        let elements = music_property(heap, elaborated, key).unwrap_or(Value::NULL);
        let props = [
            ("element", element),
            ("repeat-count", times),
            ("elements", elements),
        ];
        let music = self.make(heap, kind, &props)?;
        set_origin(heap, music, origin);
        Some(music)
    }

    /// `music-selective-map` with `pass-over-repeated-music` and
    /// `map-alternatives`: `music` with its alternatives elaborated, its
    /// elements, articulations and element first, but those of a repeat.
    fn map_alternatives(
        &self,
        heap: &mut Heap,
        music: Value,
        times: i64,
        main: Value,
        warnings: &mut Vec<Warning>,
    ) -> Option<Value> {
        if !is_music_type(heap, music, "repeated-music") {
            for name in ["elements", "articulations"] {
                let key = heap.intern(name);
                let list = music_property(heap, music, key).unwrap_or(Value::NULL);
                if list.tag() != lily_engine_scheme::Tag::Pair {
                    continue;
                }
                let old = items(heap, list);
                let mut new = Vec::with_capacity(old.len());
                for &item in &old {
                    new.push(self.map_alternatives(heap, item, times, main, warnings)?);
                }
                if new != old {
                    let list = heap.list(&new);
                    set_property(heap, music, key, list);
                }
            }
            let key = heap.intern("element");
            if let Some(e) = music_property(heap, music, key)
                && heap.foreign::<Music>(e).is_some()
            {
                let mapped = self.map_alternatives(heap, e, times, main, warnings)?;
                if mapped != e {
                    set_property(heap, music, key, mapped);
                }
            }
        }
        if is_music_type(heap, music, "sequential-alternative-music") {
            return self.elaborate(heap, music, times, main, warnings);
        }
        Some(music)
    }

    /// `elaborate-alternative-music`: a new `SequentialAlternativeMusic`
    /// of `alt`'s elements, those past `times` junked with a warning at
    /// `main` and at `alt`, each wrapped in `VoltaSpeccedMusic` of its
    /// volta numbers unless one is a volta specification already.
    fn elaborate(
        &self,
        heap: &mut Heap,
        alt: Value,
        times: i64,
        main: Value,
        warnings: &mut Vec<Warning>,
    ) -> Option<Value> {
        let key = heap.intern("elements");
        let list = music_property(heap, alt, key).unwrap_or(Value::NULL);
        let mut alts = items(heap, list);
        if usize::try_from(times).is_ok_and(|t| t < alts.len()) {
            warnings.push(Warning::Music(main, JUNKED.into()));
            warnings.push(Warning::Music(alt, JUNKED.into()));
            alts.truncate(times as usize);
        }
        let specced = alts
            .iter()
            .any(|&m| is_music_type(heap, m, "volta-specification"));
        if !specced && !alts.is_empty() {
            let first = times.saturating_add(1).saturating_sub(alts.len() as i64);
            let key = heap.intern("origin");
            for (i, item) in alts.iter_mut().enumerate() {
                let numbers: Vec<Value> = if i == 0 {
                    (1..=first).map(|n| heap.integer(n)).collect()
                } else {
                    alloc::vec![heap.integer(first + i as i64)]
                };
                let numbers = heap.list(&numbers);
                let at = music_property(heap, *item, key).unwrap_or(Value::NULL);
                let props = [
                    ("element", *item),
                    ("origin", at),
                    ("volta-numbers", numbers),
                ];
                *item = self.make(heap, "VoltaSpeccedMusic", &props)?;
            }
        }
        let list = heap.list(&alts);
        self.make(heap, "SequentialAlternativeMusic", &[("elements", list)])
    }

    /// `tempo`: `\tempo`'s `TempoChangeEvent` of `text` (`'()` for none),
    /// and with a metronome mark, `unit` and `count` (else `#f`), in
    /// sequential music with `tempoWholesPerMinute` set in `Score`; only
    /// the event is placed at `origin`.
    pub fn tempo(
        &self,
        heap: &mut Heap,
        text: Value,
        unit: Value,
        count: Value,
        origin: Value,
    ) -> Option<Value> {
        let props = [
            ("text", text),
            ("tempo-unit", unit),
            ("metronome-count", count),
        ];
        let change = self.make_set(heap, "TempoChangeEvent", &props, origin)?;
        let Some(length) = heap.foreign::<Duration>(unit).and_then(|d| d.length()) else {
            return Some(change);
        };
        let (n, d) = (
            heap.integer(length.numerator()),
            heap.integer(length.denominator()),
        );
        let whole = heap.div(n, d).ok()?;
        let count = match (heap.car(count), heap.cdr(count)) {
            (Some(a), Some(b)) if count.tag() == lily_engine_scheme::Tag::Pair => {
                let sum = heap.add(a, b).ok()?;
                let two = heap.integer(2);
                heap.div(sum, two).ok()?
            }
            _ if heap.num(count).is_some() => count,
            _ => return Some(change),
        };
        let wpm = heap.mul(whole, count).ok()?;
        let symbol = heap.intern("tempoWholesPerMinute");
        let set = self.make(heap, "PropertySet", &[("symbol", symbol), ("value", wpm)])?;
        let score = heap.intern("Score");
        let (id, ops) = (Value::NULL, Value::FALSE);
        let spec = self.context_specification(heap, score, id, ops, false, set, Value::FALSE)?;
        let list = heap.list(&[change, spec]);
        self.make(heap, "SequentialMusic", &[("elements", list)])
    }

    /// `add-lyrics`: `music` and the `\addlyrics` after it, each lyrics'
    /// music and the mods of its `\with`s in `lyrics`, as simultaneous
    /// music of the voice and a `\new Lyrics` of each, its lyrics combined
    /// with that voice (`make-lyric-combine`), both placed at the lyrics.
    /// The voice is `music` if it is a `ContextSpeccedMusic` with an id, or
    /// one `\new` creates, which is given `name`; else a `Voice` of that
    /// name around it. `name` is what [`takes_voice_name`] says it takes.
    pub fn add_lyrics(
        &self,
        heap: &mut Heap,
        music: Value,
        name: Value,
        lyrics: &[(Value, Value)],
    ) -> Option<Value> {
        let (id_key, new_key, type_key, origin_key) = (
            heap.intern("context-id"),
            heap.intern("create-new"),
            heap.intern("context-type"),
            heap.intern("origin"),
        );
        let context = crate::music_name(heap, music) == Some("ContextSpeccedMusic");
        let id = music_property(heap, music, id_key)
            .filter(|&id| context && heap.str(id).is_some_and(|s| !s.as_str().is_empty()));
        let created =
            context && music_property(heap, music, new_key).is_some_and(|new| new != Value::FALSE);
        let (voice_name, voice) = match id {
            Some(id) => (id, music),
            None if created => {
                set_property(heap, music, id_key, name);
                (name, music)
            }
            None => {
                let origin = music_property(heap, music, origin_key).unwrap_or(Value::NULL);
                let voice = heap.intern("Voice");
                let props = [
                    ("element", music),
                    ("context-type", voice),
                    ("context-id", name),
                    ("origin", origin),
                ];
                (name, self.make(heap, "ContextSpeccedMusic", &props)?)
            }
        };
        let voice_type = music_property(heap, voice, type_key).unwrap_or(Value::NULL);
        let mut elements = alloc::vec![voice];
        for &(lyric, mods) in lyrics {
            let at = music_property(heap, lyric, origin_key).unwrap_or(Value::NULL);
            let combined = self.lyric_combine(heap, voice_name, voice_type, lyric, at)?;
            let lyrics_type = heap.intern("Lyrics");
            let props = [
                ("create-new", Value::TRUE),
                ("context-type", lyrics_type),
                ("property-operations", mods),
                ("element", combined),
            ];
            let context = self.make(heap, "ContextSpeccedMusic", &props)?;
            set_origin(heap, context, at);
            elements.push(context);
        }
        let list = heap.list(&elements);
        self.make(heap, "SimultaneousMusic", &[("elements", list)])
    }

    /// `lyric-combine`: `\lyricsto`'s `LyricCombineMusic` of `music`, a
    /// `CompletizeExtenderEvent` appended to its elements, synchronized
    /// with the context `sync` of type `sync_type` (`'()` for any).
    pub fn lyric_combine(
        &self,
        heap: &mut Heap,
        sync: Value,
        sync_type: Value,
        music: Value,
        origin: Value,
    ) -> Option<Value> {
        let key = heap.intern("elements");
        // `append!` changes a list in place, and leaves `'()` as it is.
        if let Some(list) = music_property(heap, music, key)
            && list.tag() == lily_engine_scheme::Tag::Pair
        {
            let mut elements = items(heap, list);
            elements.push(self.make(heap, "CompletizeExtenderEvent", &[])?);
            let list = heap.list(&elements);
            set_property(heap, music, key, list);
        }
        let props = [
            ("element", music),
            ("associated-context", sync),
            ("associated-context-type", sync_type),
        ];
        self.make_set(heap, "LyricCombineMusic", &props, origin)
    }
}

/// Whether `add-lyrics` of `music` takes a unique voice name
/// (`get-next-unique-voice-name`): unless `music` is a
/// `ContextSpeccedMusic` with an id that is a string not empty
/// (`get-first-context-id!`), it takes one, for the context `\new` creates
/// or for a `Voice` around `music`.
pub fn takes_voice_name(heap: &mut Heap, music: Value) -> bool {
    let key = heap.intern("context-id");
    let id = music_property(heap, music, key);
    let named = id.is_some_and(|id| heap.str(id).is_some_and(|s| !s.as_str().is_empty()));
    !(crate::music_name(heap, music) == Some("ContextSpeccedMusic") && named)
}

/// `ly:set-origin!` with an origin given: each music of `m`, a music or a
/// list of them however nested, gets `origin`, an [`Input`]; `#f` and `'()`
/// change nothing.
pub fn set_origin(heap: &mut Heap, m: Value, origin: Value) {
    if heap.foreign::<Input>(origin).is_none() {
        return;
    }
    let key = heap.intern("origin");
    crate::music::set_origin(heap, m, key, origin);
}

/// `parser.yy`'s `add_post_events`: `events` appended to the articulations
/// of a rhythmic event or a caesura, or to the elements of an event chord,
/// found in `m` through the last element of sequential music and the
/// element of a wrapper or of time-scaled music. True if there was nothing
/// to add them to.
pub fn add_post_events(heap: &mut Heap, m: Value, events: &[Value]) -> bool {
    if events.is_empty() {
        return false;
    }
    let mut m = Some(m);
    while let Some(music) = m {
        let key = if is_music_type(heap, music, "rhythmic-event")
            || is_music_type(heap, music, "caesura-event")
        {
            "articulations"
        } else if is_music_type(heap, music, "event-chord") {
            "elements"
        } else if is_music_type(heap, music, "sequential-music") {
            let key = heap.intern("elements");
            let mut p = music_property(heap, music, key).unwrap_or(Value::NULL);
            let mut last = None;
            while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                last = Some(item);
                p = rest;
            }
            match last {
                Some(last) => {
                    m = Some(last).filter(|&l| heap.foreign::<Music>(l).is_some());
                    continue;
                }
                None => return true,
            }
        } else if is_music_type(heap, music, "music-wrapper-music")
            || is_music_type(heap, music, "time-scaled-music")
        {
            let key = heap.intern("element");
            m = music_property(heap, music, key).filter(|&e| heap.foreign::<Music>(e).is_some());
            continue;
        } else {
            break;
        };
        let key = heap.intern(key);
        let mut items = Vec::new();
        let mut p = music_property(heap, music, key).unwrap_or(Value::NULL);
        while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            items.push(item);
            p = rest;
        }
        items.extend_from_slice(events);
        let list = heap.list(&items);
        set_property(heap, music, key, list);
        return false;
    }
    true
}
