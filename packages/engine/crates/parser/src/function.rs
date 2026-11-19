//! Music functions applied (02 §3.2, §6's P5, step 18a): a function's
//! signature as `ly:music-function-signature` gives it, its arguments read
//! by `args.rs`, and `ly-syntax-constructors.scm`'s `music-function`, which
//! applies the function when every argument was good and checks its result,
//! with LilyPond's `argument-error` and `music-function-call-error`.
//!
//! The music the grammar passes on (N2, 04 §6, 02 §5.5): the parser keeps
//! its syntax, and makes music of it natively where LilyPond's grammar
//! hands music on (a function's argument, a predicate's, an assignment's
//! value, a score's music), as `parser.yy`'s actions and the constructors
//! of `ly-syntax-constructors.scm` make it (`lily_engine_music::Maker`):
//! each music object with its `origin` at its span and its properties in
//! the order LilyPond sets them, post-events in a list attached as
//! `reverse_music_list` attaches them, then frozen into one tree. A value a
//! function gives back is placed at the call (`music-function`'s
//! `ly:set-origin!`) and held as [`MusicKind::Value`].

use crate::args::Args;
use crate::lexer::Mode;
use crate::partial::Call;
use crate::pitch::Pitch;
use crate::source::Span;
use crate::syntax::{
    ChordElement, Direction, Duration, Music, MusicKind, Note, PostEvent, PostEventKind, Property,
    PropertyOp, Text,
};
use crate::{Parsed, Parser};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::{
    Music as MusicValue, MusicFunction, Rational, add_post_events, deep_copy, freeze,
    is_music_type, music_property, set_origin, set_property,
};
use lily_engine_scheme::{Heap, Tag, Value, Vm};

/// A parameter: its predicate, and for an optional one its default.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Param {
    pub pred: Value,
    pub default: Option<Value>,
}

/// A music function's signature: the predicate of its result, the value a
/// failed call gives (an optional result's default), and its parameters.
struct Signature {
    result: Value,
    fallback: Option<Value>,
    params: Vec<Param>,
}

/// `pred` or `(pred . default)`.
fn param(heap: &Heap, entry: Value) -> Param {
    match (heap.car(entry), heap.cdr(entry)) {
        (Some(pred), Some(default)) if entry.tag() == Tag::Pair => Param {
            pred,
            default: Some(default),
        },
        _ => Param {
            pred: entry,
            default: None,
        },
    }
}

fn signature(heap: &Heap, function: MusicFunction) -> Signature {
    let mut items = Vec::new();
    let mut p = function.signature;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(param(heap, item));
        p = rest;
    }
    let result = items.first().copied().unwrap_or(Param {
        pred: Value::FALSE,
        default: None,
    });
    Signature {
        result: result.pred,
        fallback: result.default,
        params: items.into_iter().skip(1).collect(),
    }
}

/// The predicate of a function's result, to classify its token by
/// (`scan_scm_id`).
pub(crate) fn result_predicate(heap: &Heap, value: Value) -> Option<Value> {
    let function = heap.foreign::<MusicFunction>(value)?;
    let first = heap.car(function.signature)?;
    Some(param(heap, first).pred)
}

/// Whether each parameter of the function `value` is optional, in the
/// signature's order; None for no function.
pub(crate) fn optional_params(heap: &Heap, value: Value) -> Option<Vec<bool>> {
    let function = heap.foreign::<MusicFunction>(value)?;
    let params = signature(heap, *function).params;
    Some(params.iter().map(|p| p.default.is_some()).collect())
}

/// `value->lily-string` of a value no music function may return, as far as
/// `define-music-display-methods.scm` words it without music and markup: a
/// pitch as its name, `pitch_name`, and octave marks; a string as markup is,
/// written; a number or a boolean after `#`, a symbol or a list after `#'`,
/// anything else written after `#` (LilyPond writes durations, music and
/// markup as LilyPond input, which is P7's).
fn lily_string(heap: &Heap, value: Value, pitch_name: impl Fn(u8, i8) -> Option<String>) -> String {
    if let Some(p) = heap.foreign::<lily_engine_music::Pitch>(value) {
        let quarter_tones = p.alteration.numerator() * 4 / p.alteration.denominator().max(1);
        let name = u8::try_from(p.notename)
            .ok()
            .zip(i8::try_from(quarter_tones).ok())
            .and_then(|(step, alteration)| pitch_name(step, alteration))
            .unwrap_or_else(|| String::from("#f"));
        let marks = match p.octave {
            o if o >= 0 => "'".repeat(o as usize + 1),
            o if o < -1 => ",".repeat((-1 - o) as usize),
            _ => String::new(),
        };
        return name + &marks;
    }
    // Floyd's: a cycle is no list.
    let list = {
        let (mut slow, mut fast) = (value, value);
        loop {
            match heap.cdr(fast).and_then(|f| heap.cdr(f)) {
                Some(next) => fast = next,
                None => break heap.cdr(fast).unwrap_or(fast) == Value::NULL,
            }
            slow = heap.cdr(slow).unwrap_or(Value::NULL);
            if slow == fast {
                break false;
            }
        }
    };
    let mut out = String::from(match value.tag() {
        Tag::String => "",
        Tag::Symbol => "#'",
        _ if list => "#'",
        _ => "#",
    });
    heap.write(value, &mut out);
    out
}

/// Defines `type-p-name-alist` in `vm`'s `(lily)` from the five lists of
/// predicates that `lily.scm` appends into it, each `(,predicate . "name")`,
/// as far as `(lily)` has the predicates; `c++.scm`'s `type-name` reads it.
/// Read as data from `lily_scm`, `lily.scm`'s text: nothing in it is
/// evaluated. Gives the number of entries.
pub fn define_type_names(vm: &mut Vm, lily_scm: &str) -> Result<usize, String> {
    const LISTS: [&str; 5] = [
        "r5rs-primary-predicates",
        "r5rs-secondary-predicates",
        "guile-predicates",
        "lilypond-scheme-predicates",
        "lilypond-exported-predicates",
    ];
    let lily = vm
        .eval_text("(resolve-module '(lily))", None)
        .map_err(|e| e.text(vm.heap()))?;
    let mut entries = Vec::new();
    for list in LISTS {
        let head = alloc::format!("(define-public {list}");
        let start = lily_scm
            .find(&head)
            .ok_or_else(|| alloc::format!("no {list}"))?;
        let form = lily_engine_scheme::read(vm.heap_mut(), lily_scm, start, None)
            .map_err(|e| alloc::format!("{list}: {e:?}"))?
            .datum;
        let heap = vm.heap();
        // `(define-public name `(…))`: the quasiquote's datum.
        let mut p = heap
            .cdr(form)
            .and_then(|p| heap.cdr(p))
            .and_then(|p| heap.car(p))
            .and_then(|q| heap.cdr(q))
            .and_then(|p| heap.car(p))
            .ok_or_else(|| alloc::format!("{list} is not `(…)"))?;
        let mut names = Vec::new();
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            // `((unquote predicate) . "name")`.
            let predicate = heap
                .car(entry)
                .and_then(|u| heap.cdr(u))
                .and_then(|p| heap.car(p))
                .and_then(|s| heap.name(s))
                .map(String::from);
            if let (Some(predicate), Some(name)) = (predicate, heap.cdr(entry)) {
                names.push((predicate, name));
            }
            p = rest;
        }
        for (predicate, name) in names {
            if let Some(value) = vm.module_value(lily, &predicate) {
                let heap = vm.heap_mut();
                entries.push(heap.cons(value, name));
            }
        }
    }
    let count = entries.len();
    let alist = vm.heap_mut().list(&entries);
    vm.module_define(lily, "type-p-name-alist", alist);
    Ok(count)
}

/// The module of `ly-syntax-constructors.scm`'s constructors that are
/// Scheme (D93), which `(lily)` does not export (MS8 step 10, OPEN F).
pub const CONSTRUCTOR_MODULE: &str = "lily ly-syntax-constructors";

impl Parser<'_, '_> {
    /// A procedure of the file's scopes, `(lily)`'s included.
    pub(crate) fn procedure(&mut self, name: &str) -> Option<Value> {
        self.env.lookup(name)
    }

    /// A constructor of `ly-syntax-constructors.scm` (D93): in its module,
    /// or in the scopes where there is none.
    pub(crate) fn constructor(&mut self, name: &str) -> Option<Value> {
        let vm = self.env.vm();
        match vm.slot(CONSTRUCTOR_MODULE, name) {
            Some(slot) => vm.slot_value(slot),
            None => self.procedure(name),
        }
    }

    /// Calls `procedure` with `args`, the value kept. An error, which
    /// LilyPond would end the run with, is written as Guile writes it and
    /// fails the file; the value is then None.
    pub(crate) fn call_scheme(&mut self, procedure: Value, args: &[Value]) -> Option<Value> {
        // The innermost scope is current, as `set_current_scope` keeps it.
        let scope = self.env.innermost();
        self.publish();
        let previous = self.env.vm().set_current_module(scope);
        let result = self.env.vm().apply(procedure, args);
        self.env.vm().set_current_module(previous);
        self.called(result)
    }

    /// `ly_call (pred, value)`: whether `pred` holds of `value`.
    pub(crate) fn holds(&mut self, pred: Value, value: Value) -> bool {
        self.call_scheme(pred, &[value])
            .is_some_and(|v| v != Value::FALSE)
    }

    /// `type-name` of a predicate, or its value written when `(lily)` has
    /// no `type-name`.
    fn type_name(&mut self, pred: Value) -> String {
        let named = self
            .procedure("type-name")
            .and_then(|type_name| self.call_scheme(type_name, &[pred]));
        let heap = self.env.vm().heap();
        match named.and_then(|v| heap.str(v)) {
            Some(text) => String::from(text.as_str()),
            None => {
                let mut out = String::new();
                heap.write(pred, &mut out);
                out
            }
        }
    }

    /// `check_scheme_arg`'s error: `argument-error`'s message at `at`,
    /// `found` the value `music->make-music` gives of `shown`.
    pub(crate) fn argument_error(&mut self, at: Span, n: usize, pred: Value, shown: Value) {
        let type_name = self.type_name(pred);
        let made = self
            .procedure("music->make-music")
            .and_then(|f| self.call_scheme(f, &[shown]))
            .unwrap_or(shown);
        let mut found = String::new();
        self.env.vm().heap().write(made, &mut found);
        let message =
            alloc::format!("wrong type for argument {n}.  Expecting {type_name}, found {found}");
        self.error(at, message);
    }

    /// A function's call, at its `MUSIC_FUNCTION`, `EVENT_FUNCTION` or
    /// `SCM_FUNCTION`: its arguments read, then `music-function`. Gives the
    /// call's value, kept, and its span from the function's name.
    pub(crate) fn function_call(&mut self) -> Parsed<(Value, Span)> {
        match self.call(false)? {
            Call::Done(value, span) => Ok((value, span)),
            Call::Partial(_) => unreachable!("a call is cut short only where `\\etc` may stand"),
        }
    }

    /// A function's call, or, with `etc`, `partial_function_scriptable`'s:
    /// its arguments up to an `\etc` where one is due, left to be taken,
    /// the arguments before it its first call's (`partial.rs`).
    pub(crate) fn call(&mut self, etc: bool) -> Parsed<Call> {
        let token = self.next(crate::args::N);
        let function = self.env.value(token.value);
        let heap = self.env.vm().heap();
        let Some(&music_function) = heap.foreign::<MusicFunction>(function) else {
            return Ok(Call::Done(Value::UNSPECIFIED, token.loc));
        };
        let signature = signature(heap, music_function);
        let args = match self.arguments_etc(&signature.params, etc)? {
            Args::Done(args) => args,
            Args::Partial { given, good, inner } => {
                let mut calls = alloc::vec![self.partial_call(function, &given, good)];
                calls.extend(inner);
                return Ok(Call::Partial(calls));
            }
        };
        let span = Span {
            start: token.loc.start,
            ..self.span_from(token.span)
        };
        let value = self.apply_function(music_function, &signature, args, span);
        Ok(Call::Done(value, span))
    }

    /// `music-function`: the function applied to `args` when none failed,
    /// with `(*location*)` the call's, as `MAKE_SYNTAX` binds it; its result
    /// checked by the signature's predicate; else, and for a result that
    /// fails, the signature's fallback, a copy of it if it is music, or `#f`.
    fn apply_function(
        &mut self,
        function: MusicFunction,
        signature: &Signature,
        args: Option<Vec<Value>>,
        span: Span,
    ) -> Value {
        let location = self.origin(span);
        let previous = self.env.vm().set_location(location);
        self.env.keep(previous);
        let applied = args.and_then(|args| self.call_scheme(function.procedure, &args));
        self.env.vm().set_location(previous);
        if let Some(result) = applied {
            if self.holds(signature.result, result) {
                // `ly:set-origin!` of music, which is frozen again (N2).
                if self
                    .env
                    .vm_ref()
                    .heap()
                    .foreign::<MusicValue>(result)
                    .is_some()
                {
                    let origin = self.origin(span);
                    let heap = self.env.vm().heap_mut();
                    set_origin(heap, result, origin);
                    freeze(heap, result);
                }
                return result;
            }
            freeze(self.env.vm().heap_mut(), result);
            let type_name = self.type_name(signature.result);
            let names = |step, alteration| {
                let name = self.lexer.name_of_pitch(step, alteration)?;
                Some(String::from_utf8_lossy(name).into_owned())
            };
            let shown = lily_string(self.env.vm_ref().heap(), result, names);
            self.error(
                span,
                alloc::format!("{type_name} function cannot return {shown}"),
            );
        }
        match signature.fallback {
            // `ly:music-deep-copy` with `(*location*)`.
            Some(fallback) => self.placed_copy(fallback, Some(span)),
            None => Value::FALSE,
        }
    }

    /// `loc_on_copy`, and the copy of a fallback: music is copied.
    pub(crate) fn copy(&mut self, value: Value) -> Value {
        let heap = self.env.vm().heap_mut();
        if heap.foreign::<MusicValue>(value).is_none() {
            return value;
        }
        let copy = deep_copy(heap, value);
        self.env.keep(copy);
        copy
    }

    // --- music values from the syntax

    /// An [`Input`] of `span`, kept: an `origin`, or `(*location*)`; a
    /// template's location in its place (`override_input`).
    pub(crate) fn origin(&mut self, span: Span) -> Value {
        if let Some(input) = self.env.override_input {
            return input;
        }
        let input = self
            .lexer
            .source_of(span.file)
            .input(span.file, span.start, span.end);
        let value = self.env.vm().heap_mut().wrap(input);
        self.env.keep(value);
        value
    }

    /// `MY_MAKE_MUSIC`: music made with its `origin` at `span`, then
    /// `props` set in order.
    pub(crate) fn make_at(&mut self, name: &str, span: Span, props: &[(&str, Value)]) -> Value {
        let origin = self.origin(span);
        let maker = self.env.maker;
        let music = maker.make_at(self.env.vm().heap_mut(), name, origin, props);
        self.kept(music)
    }

    /// Music a constructor made, kept, or unspecified.
    fn kept(&mut self, music: Option<Value>) -> Value {
        match music {
            Some(music) => {
                self.env.keep(music);
                music
            }
            None => Value::UNSPECIFIED,
        }
    }

    /// A list of `items`, kept.
    fn list(&mut self, items: &[Value]) -> Value {
        let list = self.env.vm().heap_mut().list(items);
        self.env.keep(list);
        list
    }

    fn int(&mut self, n: i64) -> Value {
        self.env.vm().heap_mut().integer(n)
    }

    /// `ly:make-pitch` of a pitch, kept.
    pub(crate) fn pitch_value(&mut self, pitch: Pitch) -> Value {
        let heap = self.env.vm().heap_mut();
        let octave = heap.integer(pitch.octave.into());
        let step = heap.integer(pitch.step.into());
        let quarter_tones = heap.integer(pitch.alteration.into());
        let four = heap.integer(4);
        let alteration = heap.div(quarter_tones, four).unwrap_or(quarter_tones);
        self.procedure("ly:make-pitch")
            .and_then(|make| self.call_scheme(make, &[octave, step, alteration]))
            .unwrap_or(Value::UNSPECIFIED)
    }

    /// A duration's value, kept.
    pub(crate) fn duration_value(&mut self, d: Duration) -> Value {
        let (n, d_) = d.factor;
        let factor = match (i64::try_from(n), i64::try_from(d_)) {
            (Ok(n), Ok(d)) => Rational::new(n, d),
            _ => Rational::int(1),
        };
        let duration = lily_engine_music::Duration::new(d.log, d.dots.into());
        let duration = duration.compressed(factor).unwrap_or(duration);
        let value = self.env.vm().heap_mut().wrap(duration);
        self.env.keep(value);
        value
    }

    /// The music of a post-event, at its span: `post_event_nofinger`'s,
    /// `direction_less_event`'s and `direction_reqd_event`'s, a direction
    /// written set last. A music value is as the parser left it
    /// (`notes.rs`).
    pub(crate) fn event_value(&mut self, event: &PostEvent) -> Value {
        let span = event.span;
        let music = match &event.kind {
            PostEventKind::Value(index) => return self.env.value(*index),
            PostEventKind::Fingering(digit) => {
                let digit = self.int((*digit).into());
                self.make_at("FingeringEvent", span, &[("digit", digit)])
            }
            PostEventKind::StringNumber(n) => {
                let n = self.int((*n).into());
                self.make_at("StringNumberEvent", span, &[("string-number", n)])
            }
            PostEventKind::Tremolo(t) => {
                let t = self.int((*t).into());
                self.make_at("TremoloEvent", span, &[("tremolo-type", t)])
            }
            PostEventKind::Script(c) => self.script(*c, span),
            PostEventKind::Hyphen => self.make_at("HyphenEvent", span, &[]),
            PostEventKind::Extender => self.make_at("ExtenderEvent", span, &[]),
            PostEventKind::Text(text) => {
                let text = self.text_value(text);
                self.make_at("TextScriptEvent", span, &[("text", text)])
            }
        };
        let direction = match event.direction {
            Direction::Up => 1,
            Direction::Down => -1,
            Direction::Neutral => return music,
        };
        let heap = self.env.vm().heap_mut();
        if heap.foreign::<MusicValue>(music).is_some() {
            let key = heap.intern("direction");
            let direction = heap.integer(direction);
            set_property(heap, music, key, direction);
        }
        music
    }

    /// A text's value: a string, kept, or the markup.
    fn text_value(&mut self, text: &Text) -> Value {
        match text {
            Text::String(text) => {
                let text = self.env.vm().heap_mut().string(text.as_str());
                self.env.keep(text);
                text
            }
            Text::Markup(index) => self.env.value(*index),
        }
    }

    /// `script_abbreviation`'s music: a copy of the post-event the variable
    /// `dash` and the character's name hold (`dashDot`), placed at `span`;
    /// until `script-init.ly` defines them (part 2), an `ArticulationEvent`.
    fn script(&mut self, c: u8, span: Span) -> Value {
        let name = match c {
            b'^' => "Hat",
            b'+' => "Plus",
            b'-' => "Dash",
            b'!' => "Bang",
            b'>' => "Larger",
            b'.' => "Dot",
            _ => "Underscore",
        };
        let variable = self.env.lookup(&alloc::format!("dash{name}"));
        match variable {
            Some(music) if is_music_type(self.env.vm_ref().heap(), music, "post-event") => {
                self.placed_copy(music, Some(span))
            }
            _ => self.make_at("ArticulationEvent", span, &[]),
        }
    }

    /// The music of post-events, a `PostEvents` music's elements in its
    /// place, as `post_event_cons` gives them.
    fn event_values(&mut self, events: &[PostEvent]) -> Vec<Value> {
        let mut values = Vec::new();
        for event in events {
            let value = self.event_value(event);
            let heap = self.env.vm().heap_mut();
            if !is_music_type(heap, value, "post-event-wrapper") {
                values.push(value);
                continue;
            }
            let key = heap.intern("elements");
            let mut p = music_property(heap, value, key).unwrap_or(Value::NULL);
            while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                values.push(item);
                p = rest;
            }
        }
        values
    }

    /// A note's music, at `span`: `pitch_or_music`'s `NoteEvent`, or
    /// `RestEvent` for `\rest`; a bare pitch's, `make_music_from_simple`'s;
    /// in a chord, `chord_body_element`'s, given the chord's duration by
    /// `note_chord_element`. Its properties are set in the order of their
    /// actions.
    fn note_value(&mut self, note: &Note, chord: bool, span: Span) -> Value {
        let pitch = self.pitch_value(note.pitch);
        let duration = self.duration_value(note.duration);
        if !chord && crate::assign::bare_pitch(note) {
            return self.make_at(
                "NoteEvent",
                span,
                &[("duration", duration), ("pitch", pitch)],
            );
        }
        let mut set = alloc::vec![("pitch", pitch)];
        if !chord {
            set.push(("duration", duration));
            if let Some(octave) = note.octave_check {
                set.push(("absolute-octave", self.int(i64::from(octave) - 1)));
            }
        }
        if note.cautionary {
            set.push(("cautionary", Value::TRUE));
        }
        if note.force_accidental || note.cautionary {
            set.push(("force-accidental", Value::TRUE));
        }
        if !note.post.is_empty() {
            let events = self.event_values(&note.post);
            set.push(("articulations", self.list(&events)));
        }
        if chord {
            if let Some(octave) = note.octave_check {
                set.push(("absolute-octave", self.int(i64::from(octave) - 1)));
            }
            set.push(("duration", duration));
        }
        self.make_at(
            if note.rest { "RestEvent" } else { "NoteEvent" },
            span,
            &set,
        )
    }

    /// `make_music_from_simple`, at `span`: music stays; a word that is a
    /// drum pitch name (in drum mode) is a drum note of the default
    /// duration; in notes or drum mode, a word that is a pitch name, a
    /// pitch, a duration or the `UNSIGNED` of one becomes a note of the
    /// default duration; in chord mode, a pitch name or a pitch a chord of
    /// it (P10); in lyrics mode, markup a lyric event of the default
    /// duration (P8); anything else stays as it is.
    pub(crate) fn simple_music(&mut self, value: Value, span: Span) -> Value {
        enum Simple {
            Name(Pitch),
            Drum(u32),
            Pitch,
            Duration,
            Log(u8),
        }
        let mode = self.lex_mode(Mode::Notes);
        let heap = self.env.vm_ref().heap();
        let simple = if heap.foreign::<MusicValue>(value).is_some() {
            None
        } else if value.tag() == Tag::Symbol {
            let name = String::from(heap.name(value).unwrap_or_default());
            match mode {
                Mode::Notes | Mode::Chords => {
                    self.lexer.pitch_name(name.as_bytes()).map(Simple::Name)
                }
                Mode::Drums => {
                    crate::classify::Scopes::drum_pitch(&mut self.env, &name).map(Simple::Drum)
                }
                _ => None,
            }
        } else if heap.foreign::<lily_engine_music::Pitch>(value).is_some() {
            Some(Simple::Pitch)
        } else if heap.foreign::<lily_engine_music::Duration>(value).is_some() {
            Some(Simple::Duration)
        } else {
            value
                .as_fixnum()
                .and_then(|n| crate::notes::duration_log(u64::try_from(n).ok()))
                .map(Simple::Log)
        };
        let default = self.taken_default();
        if let Some(Simple::Drum(drum)) = simple {
            let duration = self.duration_value(default);
            let drum = self.env.value(drum);
            let props = [("duration", duration), ("drum-type", drum)];
            return self.make_at("NoteEvent", span, &props);
        }
        if mode == Mode::Chords {
            let pitch = match simple {
                Some(Simple::Name(pitch)) => self.pitch_value(pitch),
                Some(Simple::Pitch) => value,
                _ => return value,
            };
            let notes = self.chord_elements(pitch, None, &[], span);
            let notes = self.env.value(notes);
            let origin = self.origin(span);
            let maker = self.env.maker;
            let music = maker.event_chord(self.env.vm().heap_mut(), notes, origin);
            return self.kept(music);
        }
        if mode == Mode::Lyrics {
            // Markup, a string too, is a lyric event of the default duration.
            let music = self
                .env
                .vm_ref()
                .heap()
                .foreign::<MusicValue>(value)
                .is_some();
            if simple.is_none() && !music && self.is_markup(value) {
                let duration = self.duration_value(default);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.lyric_event(self.env.vm().heap_mut(), value, duration, origin);
                return self.kept(music);
            }
            return value;
        }
        if !matches!(mode, Mode::Notes | Mode::Drums) {
            return value;
        }
        match simple {
            None | Some(Simple::Drum(_)) => value,
            Some(Simple::Name(pitch)) => {
                let pitch = self.pitch_value(pitch);
                let duration = self.duration_value(default);
                self.make_at(
                    "NoteEvent",
                    span,
                    &[("duration", duration), ("pitch", pitch)],
                )
            }
            Some(Simple::Pitch) => {
                let duration = self.duration_value(default);
                self.make_at(
                    "NoteEvent",
                    span,
                    &[("duration", duration), ("pitch", value)],
                )
            }
            Some(Simple::Duration) => self.make_at("NoteEvent", span, &[("duration", value)]),
            Some(Simple::Log(log)) => {
                let duration = self.duration_value(Duration::new(log.into(), 0));
                self.make_at("NoteEvent", span, &[("duration", duration)])
            }
        }
    }

    /// `add-lyrics` of `music` and the `\addlyrics` after it, `lyrics`
    /// (`new_lyrics`), made now, as Bison reduces the rule: the voice name
    /// it takes from `get-next-unique-voice-name` (a session variable's
    /// count) is taken in the order of the file. Kept, live.
    pub(crate) fn add_lyrics(&mut self, music: &Music, lyrics: &[(Music, u32)]) -> Value {
        let music = self.build(music);
        let mut items = Vec::new();
        for (lyric, mods) in lyrics {
            let lyric = self.build(lyric);
            items.push((lyric, self.env.value(*mods)));
        }
        let name = match lily_engine_music::takes_voice_name(self.env.vm().heap_mut(), music) {
            true => self
                .procedure("get-next-unique-voice-name")
                .and_then(|next| self.call_scheme(next, &[]))
                .unwrap_or(Value::NULL),
            false => Value::NULL,
        };
        let maker = self.env.maker;
        let made = maker.add_lyrics(self.env.vm().heap_mut(), music, name, &items);
        self.kept(made)
    }

    /// The music of `music`'s syntax, frozen into one tree, kept.
    pub(crate) fn music_value(&mut self, music: &Music) -> Value {
        let value = self.build(music);
        freeze(self.env.vm().heap_mut(), value);
        value
    }

    /// The live music of `music`'s syntax, kept; what was music already (a
    /// music value) is as it is.
    fn build(&mut self, music: &Music) -> Value {
        let span = music.span;
        match &music.kind {
            MusicKind::Sequential(elements) => {
                let list = self.list_value(elements);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.sequential_music(self.env.vm().heap_mut(), list, origin);
                self.kept(music)
            }
            MusicKind::Simultaneous(elements) => {
                let list = self.list_value(elements);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.simultaneous_music(self.env.vm().heap_mut(), list, origin);
                self.kept(music)
            }
            MusicKind::Note(note) => self.note_value(note, false, span),
            MusicKind::Rest {
                skip,
                duration,
                post,
            } => {
                // `simple_element`'s `@$`: the rest and its duration, not
                // the post-events `event_chord` adds.
                let end = duration.span.map_or(span.start + 1, |d| d.end);
                let span = Span { end, ..span };
                let duration = self.duration_value(*duration);
                let name = if *skip { "SkipEvent" } else { "RestEvent" };
                let music = self.make_at(name, span, &[("duration", duration)]);
                self.articulations(music, post)
            }
            MusicKind::MultiMeasureRest { duration, post } => {
                let duration = self.duration_value(*duration);
                let events = self.event_values(post);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let heap = self.env.vm().heap_mut();
                let music = maker.multi_measure_rest(heap, duration, &events, origin);
                self.kept(music)
            }
            MusicKind::ChordRepetition { duration, post } => {
                let duration = self.duration_value(*duration);
                let events = self.event_values(post);
                let events = self.list(&events);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let heap = self.env.vm().heap_mut();
                let music = maker.repetition_chord(heap, duration, events, origin);
                self.kept(music)
            }
            MusicKind::Rhythm { duration, post } => {
                let duration = self.duration_value(*duration);
                let music = self.make_at("NoteEvent", span, &[("duration", duration)]);
                self.articulations(music, post)
            }
            MusicKind::Chord {
                elements,
                duration,
                post,
            } => self.chord_value(elements, *duration, post, span),
            MusicKind::Lyric {
                text,
                duration,
                post,
            } => {
                let text = self.text_value(text);
                let duration = self.duration_value(*duration);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.lyric_event(self.env.vm().heap_mut(), text, duration, origin);
                let music = self.kept(music);
                self.articulations(music, post)
            }
            MusicKind::PostEvent(event) => self.event_value(event),
            MusicKind::PostEvents(events) => {
                let events = self.event_values(events);
                let elements = self.list(&events);
                self.make_at("PostEvents", span, &[("elements", elements)])
            }
            MusicKind::Error => self.make_at("Music", span, &[("error-found", Value::TRUE)]),
            MusicKind::Value(index) => self.env.value(*index),
            MusicKind::Context(context) => {
                let element = self.build(&context.music);
                let values = &self.env;
                let (context_type, ops) = (
                    values.value(context.context_type),
                    values.value(context.ops),
                );
                let id = context.id.map_or(Value::NULL, |id| values.value(id));
                let origin = self.origin(span);
                let maker = self.env.maker;
                let heap = self.env.vm().heap_mut();
                let music = maker.context_specification(
                    heap,
                    context_type,
                    id,
                    ops,
                    context.new,
                    element,
                    origin,
                );
                self.kept(music)
            }
            MusicKind::Property(property) => self.property_value(property, span),
            MusicKind::Repeat(repeat) => {
                let body = self.build(&repeat.body);
                let alternatives = match &repeat.alternative {
                    Some(alternative) => self.build(alternative),
                    None => Value::NULL,
                };
                let heap = self.env.vm_ref().heap();
                let kind = heap
                    .str(self.env.value(repeat.kind))
                    .map(|s| String::from(s.as_str()))
                    .unwrap_or_default();
                let times = self.env.value(repeat.times);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let mut warnings = Vec::new();
                let heap = self.env.vm().heap_mut();
                let music = maker.repeat(
                    heap,
                    &kind,
                    times,
                    body,
                    alternatives,
                    origin,
                    &mut warnings,
                );
                let music = self.kept(music);
                self.report(warnings);
                music
            }
            MusicKind::Alternative(music) => {
                let music = self.build(music);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let mut warnings = Vec::new();
                let heap = self.env.vm().heap_mut();
                let music = maker.alternative(heap, music, origin, &mut warnings);
                let music = self.kept(music);
                self.report(warnings);
                music
            }
            MusicKind::Tempo(tempo) => {
                let text = self.tempo_text(tempo);
                let (unit, count) = match tempo.mark {
                    Some((unit, count)) => (self.duration_value(unit), self.env.value(count)),
                    None => (Value::FALSE, Value::FALSE),
                };
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.tempo(self.env.vm().heap_mut(), text, unit, count, origin);
                self.kept(music)
            }
            MusicKind::LyricCombine {
                id,
                context_type,
                music,
            } => {
                let music = self.build(music);
                let id = self.env.value(*id);
                let context_type = context_type.map_or(Value::NULL, |t| self.env.value(t));
                let origin = self.origin(span);
                let maker = self.env.maker;
                let heap = self.env.vm().heap_mut();
                let music = maker.lyric_combine(heap, id, context_type, music, origin);
                self.kept(music)
            }
            MusicKind::Unrelativable(music) => {
                let music = self.build(music);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let heap = self.env.vm().heap_mut();
                let music = maker.unrelativable_music(heap, music, origin);
                self.kept(music)
            }
            MusicKind::ChordEntry { at, notes, post } => {
                // `new_chord post_events`: the post-events after the notes.
                let heap = self.env.vm_ref().heap();
                let mut items = Vec::new();
                let mut p = self.env.value(*notes);
                while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                    items.push(item);
                    p = rest;
                }
                items.extend(self.event_values(post));
                let list = self.list(&items);
                let origin = self.origin(*at);
                let maker = self.env.maker;
                let music = maker.event_chord(self.env.vm().heap_mut(), list, origin);
                self.kept(music)
            }
            MusicKind::Drum {
                at,
                drum,
                duration,
                post,
            } => {
                let duration = self.duration_value(*duration);
                let drum = self.env.value(*drum);
                let props = [("duration", duration), ("drum-type", drum)];
                let music = self.make_at("NoteEvent", *at, &props);
                self.articulations(music, post)
            }
            MusicKind::Figures {
                figures,
                duration,
                post,
            } => {
                // `note_chord_element`: each figure given the duration.
                let duration = self.duration_value(*duration);
                let mut items = Vec::new();
                for &figure in figures {
                    let figure = self.env.value(figure);
                    let heap = self.env.vm().heap_mut();
                    let key = heap.intern("duration");
                    set_property(heap, figure, key, duration);
                    items.push(figure);
                }
                items.extend(self.event_values(post));
                let list = self.list(&items);
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music = maker.event_chord(self.env.vm().heap_mut(), list, origin);
                self.kept(music)
            }
            MusicKind::Change { context_type, id } => {
                let (context_type, id) = (self.env.value(*context_type), self.env.value(*id));
                let origin = self.origin(span);
                let maker = self.env.maker;
                let music =
                    maker.context_change(self.env.vm().heap_mut(), context_type, id, origin);
                self.kept(music)
            }
        }
    }

    /// What a constructor warned of, where LilyPond reports it: at the
    /// music's origin, or nowhere.
    fn report(&mut self, warnings: Vec<lily_engine_music::Warning>) {
        use crate::diag::{At, Severity};
        use lily_engine_music::Warning;
        for warning in warnings {
            let (severity, music, text) = match warning {
                Warning::Plain(text) => (Severity::Warning, None, text),
                Warning::Music(music, text) => (Severity::Warning, Some(music), text),
                Warning::MusicError(music, text) => (Severity::Error, Some(music), text),
            };
            let at = match music.and_then(|m| self.origin_span(m)) {
                Some(span) => At::Span(span),
                None => At::Nowhere,
            };
            self.diagnose(severity, at, text);
        }
    }

    /// `music_property_def`'s music: `property-set` and the others, or
    /// `unspecified-music` for a wrong path.
    fn property_value(&mut self, property: &Property, span: Span) -> Value {
        let origin = self.origin(span);
        let maker = self.env.maker;
        let value = property
            .value
            .map_or(Value::UNSPECIFIED, |v| self.env.value(v));
        let Some((context, path)) = property.target else {
            let music = maker.unspecified_music(self.env.vm().heap_mut(), origin);
            return self.kept(music);
        };
        let (context, path) = (self.env.value(context), self.env.value(path));
        let heap = self.env.vm().heap_mut();
        let music = match property.op {
            PropertyOp::Set => maker.property_set(heap, context, path, value, origin),
            PropertyOp::Unset => maker.property_unset(heap, context, path, origin),
            PropertyOp::Override => maker.property_override(heap, context, path, value, origin),
            PropertyOp::Revert => maker.property_revert(heap, context, path, origin),
        };
        self.kept(music)
    }

    /// `event_chord`'s action: `post`'s music set as `music`'s
    /// articulations, if there are any.
    fn articulations(&mut self, music: Value, post: &[PostEvent]) -> Value {
        if !post.is_empty() {
            let events = self.event_values(post);
            let list = self.list(&events);
            let heap = self.env.vm().heap_mut();
            if heap.foreign::<MusicValue>(music).is_some() {
                let key = heap.intern("articulations");
                set_property(heap, music, key, list);
            }
        }
        music
    }

    /// `note_chord_element`: `chord_body`'s event chord of its notes, the
    /// post-events before a note dropped (`reverse_music_list` without
    /// `preserve`), each note given `duration`, `post` appended, and the
    /// chord placed at `span`.
    fn chord_value(
        &mut self,
        elements: &[ChordElement],
        duration: Duration,
        post: &[PostEvent],
        span: Span,
    ) -> Value {
        let mut items = Vec::new();
        let mut run = Vec::new();
        for element in elements {
            match element {
                ChordElement::Note { span, note } => {
                    let note = Note {
                        duration,
                        ..note.clone()
                    };
                    let value = self.note_value(&note, true, *span);
                    if let Some(&last) = items.last() {
                        add_post_events(self.env.vm().heap_mut(), last, &run);
                    }
                    run.clear();
                    items.push(value);
                }
                ChordElement::Drum { span, drum, post } => {
                    // `chord_body_element`'s `DRUM_PITCH post_events`.
                    let drum = self.env.value(*drum);
                    let music = self.make_at("NoteEvent", *span, &[("drum-type", drum)]);
                    let music = self.articulations(music, post);
                    let value = self.duration_value(duration);
                    let heap = self.env.vm().heap_mut();
                    let key = heap.intern("duration");
                    set_property(heap, music, key, value);
                    if let Some(&last) = items.last() {
                        add_post_events(self.env.vm().heap_mut(), last, &run);
                    }
                    run.clear();
                    items.push(music);
                }
                ChordElement::PostEvent(event) => {
                    run.extend(self.event_values(core::slice::from_ref(event)));
                }
                ChordElement::Music(index) => {
                    // `note_chord_element` sets each element's duration.
                    let music = self.env.value(*index);
                    let value = self.duration_value(duration);
                    let heap = self.env.vm().heap_mut();
                    let key = heap.intern("duration");
                    set_property(heap, music, key, value);
                    if let Some(&last) = items.last() {
                        add_post_events(self.env.vm().heap_mut(), last, &run);
                    }
                    run.clear();
                    items.push(music);
                }
            }
        }
        if let Some(&last) = items.last() {
            add_post_events(self.env.vm().heap_mut(), last, &run);
        }
        items.extend(self.event_values(post));
        let list = self.list(&items);
        let origin = self.origin(span);
        let maker = self.env.maker;
        let music = maker.event_chord(self.env.vm().heap_mut(), list, origin);
        self.kept(music)
    }

    /// `braced_music_list`'s `reverse_music_list` with `preserve`: the
    /// music of a list's elements, each run of post-events attached to the
    /// music before it (`add_post_events`), or, when it has none or takes
    /// none, put in an event chord of their own at the first one's origin
    /// after it. A syntax error's element is music with `error-found`.
    fn list_value(&mut self, elements: &[Music]) -> Value {
        let mut res: Vec<Value> = Vec::new();
        let mut run: Vec<Value> = Vec::new();
        let mut last: Option<usize> = None;
        for music in elements {
            match &music.kind {
                MusicKind::PostEvent(event) => {
                    run.extend(self.event_values(core::slice::from_ref(event)))
                }
                MusicKind::PostEvents(events) => run.extend(self.event_values(events)),
                _ => {
                    let value = self.build(music);
                    if self
                        .env
                        .vm_ref()
                        .heap()
                        .foreign::<MusicValue>(value)
                        .is_none()
                    {
                        continue;
                    }
                    self.attach(&mut res, last, &mut run);
                    res.push(value);
                    last = Some(res.len() - 1);
                }
            }
        }
        self.attach(&mut res, last, &mut run);
        self.list(&res)
    }

    /// Attaches `run` to `res[last]`, or puts it after it in an event chord.
    fn attach(&mut self, res: &mut Vec<Value>, last: Option<usize>, run: &mut Vec<Value>) {
        if run.is_empty() {
            return;
        }
        let heap = self.env.vm().heap_mut();
        let bad = match last {
            Some(i) => add_post_events(heap, res[i], run),
            None => true,
        };
        if bad {
            let key = heap.intern("origin");
            let origin = music_property(heap, run[0], key).unwrap_or(Value::FALSE);
            let events = heap.list(run);
            let maker = self.env.maker;
            let chord = maker.event_chord(self.env.vm().heap_mut(), events, origin);
            let chord = self.kept(chord);
            res.insert(last.map_or(0, |i| i + 1), chord);
        }
        run.clear();
    }
}
