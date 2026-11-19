//! LilyPond's translators (`translator.cc`, `translator-group.cc`,
//! `engraver-group.cc`, `performer-group.cc`, `score-engraver.cc`,
//! `score-performer.cc`, `translator-ctors.cc`, `scheme-engraver.cc`): N5
//! (04 §9.1–§9.2, D79, MS2 steps 25–26).
//!
//! - **Classes are static** ([`TranslatorClass`]): a name, whether it
//!   engraves, performs or both, the protocol methods it has, the event
//!   classes it hears and the grob interfaces it acknowledges, and how its
//!   state ([`Work`]) is made. Its methods are Rust calls on that state.
//!   [`CLASSES`] has LilyPond 2.26.0's 128 C++ classes: `Timing_translator`
//!   (`timing.rs`), `Mark_tracking_translator` (`marks.rs`),
//!   `Grace_engraver` (`grace.rs`) and `Repeat_acknowledge_engraver`
//!   (`volta.rs`), which make no grobs; the
//!   slur, tie and beam engravers and performers, of which only what sets
//!   the melisma properties (`melisma.rs`, D98); and, as D67's
//!   placeholders, the others of chapters 05 and 07, which are named and
//!   have no work.
//! - **A group per context** ([`Group`]), made when the parent's group hears
//!   the new context's `AnnounceNewContext` (`create_child_translator`):
//!   its definition's `consists` after the context's `\with`, in LilyPond's
//!   order (a `must_be_last` one after the others), those of the other
//!   output filtered out. A symbol names a class here, else one
//!   `ly:register-translator` keeps; a procedure is called with the
//!   context, and an alist is a Scheme translator
//!   (`scheme_translator.rs`, step 26); `unknown translator` and `cannot
//!   find` as LilyPond warns them. Its translators' listeners are added to the
//!   context's events below; an `Engraver_group` hears the context's
//!   overrides and reverts; a Score's group hears Global's `Prepare`,
//!   `OneTimeStep` and `Finish` and runs the timestep protocol over the
//!   tree, as `Score_engraver` and `Score_performer` do.
//! - **`Score_engraver`'s layout part** (D108, MS3 step 15): the work of
//!   the translation's class named [`SCORE_ENGRAVER`], if layout gave one,
//!   runs in a Score's `Score_engraver` group, which no `\consists` names:
//!   its `initialize` before the translators', each grob whose start is
//!   announced to the group given to its `acknowledge` once the method that
//!   made it has returned (`add_grob_to_announce_locally_only`, which a
//!   primitive such as `ly:engraver-make-grob` cannot run), and its
//!   `stop_translation_timestep` and `finalize` as `typeset_all`, after the
//!   timestep's and after the translators' `finalize`.
//! - **Only translators with work run.** A phase calls a translator that
//!   has the method if its class runs at every timestep (`always`), or if
//!   it is awake: one of its events arrived, a grob it acknowledges was
//!   announced, or it asked to stay awake ([`Tx::stay_awake`]) in the
//!   timestep before. `initialize` and `finalize` are called on every
//!   translator that has them. A Scheme translator with a timestep method
//!   runs always, since what it does there is its own.
//! - **Acknowledging** dispatches by grob type: per group, grob name and
//!   start or end, the list of acknowledgers is made once and kept, as
//!   `Engraver_group::acknowledge_grobs` hashes it.
//!
//! The Scheme face is LilyPond's: `ly:translator?`,
//! `ly:translator-context`, `ly:translator-name`,
//! `ly:translator-description` and `ly:get-all-translators`, the last three
//! over the classes' creators and `ly:register-translator`'s.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Tag, Tracer, Value};

use crate::R;
use crate::audio::Audio;
use crate::context::{Arena, CtxId, GLOBAL, Host, Translation, diagnose};
use crate::context_def::ContextDef;
use crate::dispatcher::{Handler, Listener, event_property};
use crate::input::Input;
use crate::rational::Moment;
use crate::records::{AudioRecord, Cause, GrobRecord, Origin};
use crate::scheme_translator::SchemeTranslator;

type Run<T> = Result<T, Exception>;

/// What a translator makes: grobs, audio items, or neither, which runs in
/// both outputs (`is_layout`, `is_midi`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Engraver,
    Performer,
    Translator,
}

impl Kind {
    fn is_layout(self) -> bool {
        self != Kind::Performer
    }

    fn is_midi(self) -> bool {
        self != Kind::Engraver
    }
}

/// The protocol's methods, as bits of [`TranslatorClass::methods`].
pub const INITIALIZE: u8 = 1;
pub const START: u8 = 1 << 1;
pub const PRE_PROCESS: u8 = 1 << 2;
pub const PROCESS: u8 = 1 << 3;
pub const PROCESS_ACKNOWLEDGED: u8 = 1 << 4;
pub const STOP: u8 = 1 << 5;
pub const FINALIZE: u8 = 1 << 6;
/// `acknowledge_audio_element`, a performer's.
pub const ACKNOWLEDGE_AUDIO: u8 = 1 << 7;

/// A translator class.
pub struct TranslatorClass {
    pub name: &'static str,
    pub kind: Kind,
    /// The protocol methods it has.
    pub methods: u8,
    /// Whether its timestep methods run at every timestep, awake or not.
    pub always: bool,
    /// The event classes it listens to: [`Work::listen`] is given the index.
    pub hears: &'static [&'static str],
    /// The grob interfaces it acknowledges, true for a grob's end:
    /// [`Work::acknowledge`] is given the index.
    pub acknowledges: &'static [(&'static str, bool)],
    pub must_be_last: bool,
    /// Its state, made for each instance; `None` for a placeholder (D67).
    pub make: Option<fn() -> Box<dyn Work>>,
    /// The properties it reads and writes, for its description.
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
}

/// The name of the class that gives `Score_engraver`'s layout part
/// (D108): found among a translation's classes by the Score's group, and
/// not by a `\consists`, as LilyPond's translator groups are not.
pub const SCORE_ENGRAVER: &str = "Score_engraver";

/// The translator number [`Tx`] has in `Score_engraver`'s layout part:
/// none of its group's.
const SCORE_PART: u16 = u16::MAX;

/// A C++ class of a later milestone (chapters 05 and 07): named, no work.
const fn placeholder(name: &'static str, kind: Kind) -> TranslatorClass {
    TranslatorClass {
        name,
        kind,
        methods: 0,
        always: false,
        hears: &[],
        acknowledges: &[],
        must_be_last: false,
        make: None,
        reads: &[],
        writes: &[],
    }
}

const fn e(name: &'static str) -> TranslatorClass {
    placeholder(name, Kind::Engraver)
}

/// LilyPond 2.26.0's C++ translator classes (`ADD_TRANSLATOR`), by name.
pub static CLASSES: [TranslatorClass; 128] = [
    e("Accidental_engraver"),
    e("Ambitus_engraver"),
    e("Arpeggio_engraver"),
    e("Auto_beam_engraver"),
    TranslatorClass {
        must_be_last: true,
        ..e("Axis_group_engraver")
    },
    e("Balloon_engraver"),
    e("Bar_engraver"),
    e("Bar_number_engraver"),
    e("Beam_collision_engraver"),
    crate::melisma::BEAM_ENGRAVER,
    crate::melisma::BEAM_PERFORMER,
    e("Break_align_engraver"),
    e("Caesura_engraver"),
    e("Chord_tremolo_engraver"),
    e("Clef_engraver"),
    e("Cluster_spanner_engraver"),
    e("Collision_engraver"),
    e("Completion_heads_engraver"),
    e("Completion_rest_engraver"),
    e("Concurrent_hairpin_engraver"),
    crate::performers::CONTROL_TRACK_PERFORMER,
    e("Cue_clef_engraver"),
    e("Dot_column_engraver"),
    e("Dots_engraver"),
    e("Double_percent_repeat_engraver"),
    crate::performers::DRUM_NOTE_PERFORMER,
    e("Drum_notes_engraver"),
    e("Dynamic_align_engraver"),
    e("Dynamic_engraver"),
    crate::dynamic_performer::DYNAMIC_PERFORMER,
    e("Episema_engraver"),
    e("Extender_engraver"),
    e("Figured_bass_engraver"),
    e("Figured_bass_position_engraver"),
    e("Fingering_column_engraver"),
    e("Fingering_engraver"),
    e("Font_size_engraver"),
    e("Footnote_engraver"),
    e("Forbid_line_break_engraver"),
    e("Fretboard_engraver"),
    e("Grace_auto_beam_engraver"),
    crate::melisma::GRACE_BEAM_ENGRAVER,
    crate::grace::CLASS,
    e("Grace_spacing_engraver"),
    e("Grid_line_span_engraver"),
    e("Grid_point_engraver"),
    e("Grob_pq_engraver"),
    e("Horizontal_bracket_engraver"),
    e("Hyphen_engraver"),
    e("Instrument_name_engraver"),
    e("Instrument_switch_engraver"),
    e("Jump_engraver"),
    e("Keep_alive_together_engraver"),
    e("Key_engraver"),
    crate::control_performers::KEY_PERFORMER,
    e("Kievan_ligature_engraver"),
    e("Laissez_vibrer_engraver"),
    e("Ledger_line_engraver"),
    e("Ligature_bracket_engraver"),
    e("Lyric_engraver"),
    crate::mark_performers::LYRIC_PERFORMER,
    e("Mark_engraver"),
    crate::mark_performers::MARK_PERFORMER,
    crate::marks::CLASS,
    e("Melody_engraver"),
    e("Mensural_ligature_engraver"),
    e("Metronome_mark_engraver"),
    crate::control_performers::MIDI_CONTROL_CHANGE_PERFORMER,
    e("Multi_measure_rest_engraver"),
    e("New_fingering_engraver"),
    e("Non_musical_script_column_engraver"),
    e("Note_head_line_engraver"),
    e("Note_heads_engraver"),
    e("Note_name_engraver"),
    crate::performers::NOTE_PERFORMER,
    e("Note_spacing_engraver"),
    e("Ottava_spanner_engraver"),
    e("Page_turn_engraver"),
    e("Paper_column_engraver"),
    e("Parenthesis_engraver"),
    e("Part_combine_engraver"),
    e("Percent_repeat_engraver"),
    e("Phrasing_slur_engraver"),
    e("Piano_pedal_align_engraver"),
    e("Piano_pedal_engraver"),
    crate::control_performers::PIANO_PEDAL_PERFORMER,
    e("Pitch_squash_engraver"),
    e("Pitched_trill_engraver"),
    e("Pure_from_neighbor_engraver"),
    crate::volta::CLASS,
    e("Repeat_tie_engraver"),
    e("Rest_collision_engraver"),
    e("Rest_engraver"),
    e("Rhythmic_column_engraver"),
    e("Script_column_engraver"),
    e("Script_engraver"),
    e("Script_row_engraver"),
    e("Separating_line_group_engraver"),
    e("Slash_repeat_engraver"),
    crate::melisma::SLUR_ENGRAVER,
    crate::melisma::SLUR_PERFORMER,
    e("Spacing_engraver"),
    e("Span_arpeggio_engraver"),
    e("Span_bar_engraver"),
    e("Span_bar_stub_engraver"),
    e("Spanner_break_forbid_engraver"),
    e("Staff_collecting_engraver"),
    crate::performers::STAFF_PERFORMER,
    e("Staff_symbol_engraver"),
    e("Stanza_number_align_engraver"),
    e("Stanza_number_engraver"),
    e("Stem_engraver"),
    e("System_start_delimiter_engraver"),
    e("Tab_note_heads_engraver"),
    e("Tab_staff_symbol_engraver"),
    crate::performers::TEMPO_PERFORMER,
    e("Text_engraver"),
    e("Text_spanner_engraver"),
    crate::melisma::TIE_ENGRAVER,
    crate::melisma::TIE_PERFORMER,
    e("Time_signature_engraver"),
    crate::control_performers::TIME_SIGNATURE_PERFORMER,
    crate::timing::CLASS,
    e("Tuplet_engraver"),
    e("Tweak_engraver"),
    e("Vaticana_ligature_engraver"),
    e("Vertical_align_engraver"),
    e("Volta_engraver"),
];

/// A translator's state and methods. Each method of the protocol is called
/// only if its class names it in `methods`; `listen` and `acknowledge` are
/// given the index of the class's `hears` or `acknowledges` entry.
///
/// A method that makes a value and then calls Scheme roots it first
/// (D83): while a method runs, its state is out of the arena, and only
/// what [`Work::values`] gave is rooted.
pub trait Work: WorkClone {
    /// `connect_to_context`.
    fn connect(&mut self, tx: &mut Tx<'_>) {
        let _ = tx;
    }
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let _ = tx;
        Ok(())
    }
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let _ = (tx, which, ev);
        Ok(())
    }
    /// The grob record `grob` announced, which the class's `which`
    /// acknowledger takes.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let _ = (tx, which, grob);
        Ok(())
    }
    /// The audio record `audio` announced.
    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        let _ = (tx, audio);
        Ok(())
    }
    /// The Scheme values it holds.
    fn values(&self, out: &mut Vec<Value>) {
        let _ = out;
    }
    /// What it holds in Rust beyond [`Work::values`], for N8's state
    /// digest (D240, OPEN C12): its numbers, flags and moments, the records
    /// it names by number, and the contexts.
    fn held(&self, h: &mut dyn Held) {
        let _ = h;
    }
    /// A Scheme translator's listeners and acknowledgers.
    fn scheme(&self) -> Option<&SchemeTranslator> {
        None
    }
}

/// A translator's state copied whole, for N8's snapshots (MS6 step 5,
/// D241): every [`Work`] is `Clone`.
pub trait WorkClone {
    fn clone_work(&self) -> Box<dyn Work>;
}

impl<T: Work + Clone + 'static> WorkClone for T {
    fn clone_work(&self) -> Box<dyn Work> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Work> {
    fn clone(&self) -> Box<dyn Work> {
        self.clone_work()
    }
}

/// What N8's state digest takes of a translator's Rust state
/// ([`Work::held`]): a number or flag as it is, a record by its place among
/// its context's at its moment (`checkpoint.rs`), a context by its key,
/// which an edit in another partition does not change.
pub trait Held {
    fn num(&mut self, x: i64);
    fn moment(&mut self, m: Moment);
    /// A grob record in a `\layout` translation, an audio record in a
    /// `\midi` one.
    fn record(&mut self, id: u32);
    fn context(&mut self, c: CtxId);
}

impl dyn Held + '_ {
    pub fn flags(&mut self, flags: &[bool]) {
        for &f in flags {
            self.num(f.into());
        }
    }

    pub fn rational(&mut self, r: crate::Rational) {
        self.moment(Moment::new(r, crate::Rational::default()));
    }

    pub fn float(&mut self, x: f64) {
        self.num(x.to_bits() as i64);
    }

    pub fn text(&mut self, t: &str) {
        self.num(t.len() as i64);
        for b in t.bytes() {
            self.num(b.into());
        }
    }

    pub fn records(&mut self, ids: &[u32]) {
        self.num(ids.len() as i64);
        for &id in ids {
            self.record(id);
        }
    }

    pub fn maybe_record(&mut self, id: Option<u32>) {
        match id {
            Some(id) => self.record(id),
            None => self.num(-1),
        }
    }

    pub fn maybe_context(&mut self, c: Option<CtxId>) {
        match c {
            Some(c) => self.context(c),
            None => self.num(-1),
        }
    }
}

/// What a translator of a group is.
#[derive(Clone, Copy)]
pub(crate) enum What {
    Class(&'static TranslatorClass),
    /// A Scheme translator (`Scheme_engraver`): its definition, an alist.
    Scheme(Value),
}

/// A translator in its group.
#[derive(Clone)]
pub(crate) struct Slot {
    what: What,
    work: Option<Box<dyn Work>>,
    /// Its protocol methods, and whether they run at every timestep.
    methods: u8,
    always: bool,
    /// Whether it runs in `\layout` and in `\midi` (`is_layout`, `is_midi`).
    layout: bool,
    midi: bool,
    must_be_last: bool,
    /// Woken for this timestep, and asked to be for the next.
    awake: bool,
    keep: bool,
    /// Its Scheme handle, made when it is first asked for, or `#f`.
    handle: Value,
}

impl Slot {
    fn of_class(class: &'static TranslatorClass) -> Slot {
        Slot {
            what: What::Class(class),
            work: class.make.map(|make| make()),
            methods: class.methods,
            always: class.always,
            layout: class.kind.is_layout(),
            midi: class.kind.is_midi(),
            must_be_last: class.must_be_last,
            awake: false,
            keep: false,
            handle: Value::FALSE,
        }
    }

    fn of_scheme(def: Value, work: SchemeTranslator) -> Slot {
        let methods = work.methods();
        Slot {
            what: What::Scheme(def),
            methods,
            always: methods & (START | PRE_PROCESS | PROCESS | PROCESS_ACKNOWLEDGED | STOP) != 0,
            layout: work.layout,
            midi: work.midi,
            must_be_last: work.must_be_last,
            work: Some(Box::new(work)),
            awake: false,
            keep: false,
            handle: Value::FALSE,
        }
    }

    /// The event classes it listens to, by its listeners' numbers.
    fn hears(&self, heap: &mut Heap) -> Vec<Value> {
        match self.what {
            What::Class(c) => c.hears.iter().map(|name| heap.intern(name)).collect(),
            What::Scheme(_) => self
                .work
                .as_ref()
                .and_then(|w| w.scheme())
                .map(|s| s.listeners.iter().map(|&(class, _)| class).collect())
                .unwrap_or_default(),
        }
    }
}

/// The kind of a group (`translator-type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupKind {
    /// `Translator_group`: Global's.
    Plain,
    Engraver,
    ScoreEngraver,
    Performer,
    ScorePerformer,
}

impl GroupKind {
    fn engraves(self) -> bool {
        matches!(self, GroupKind::Engraver | GroupKind::ScoreEngraver)
    }

    fn performs(self) -> bool {
        matches!(self, GroupKind::Performer | GroupKind::ScorePerformer)
    }
}

/// A group's acknowledgers of a grob name and end: slot and index, shared.
type Acknowledgers = Rc<[(u16, u8)]>;

/// A context's translator group.
#[derive(Clone)]
pub(crate) struct Group {
    pub(crate) kind: GroupKind,
    slots: Vec<Slot>,
    /// The grobs announced to it this pass, with true for an end.
    announced: Vec<(u32, bool)>,
    audio_announced: Vec<u32>,
    /// Per grob name, end and class interfaces (`item-interface` 1,
    /// `spanner-interface` 2), its acknowledgers: slot and index.
    acks: BTreeMap<(u64, bool, u8), Acknowledgers>,
    /// A `Score_engraver`'s layout part ([`SCORE_ENGRAVER`]), and the grobs
    /// whose starts were announced to the group that it has not been given
    /// yet.
    score: Option<Box<dyn Work>>,
    untypeset: Vec<u32>,
}

impl Group {
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        let mut values = Vec::new();
        for s in &self.slots {
            if let What::Scheme(v) = s.what {
                tracer.trace(v);
            }
            tracer.trace(s.handle);
            if let Some(w) = &s.work {
                w.values(&mut values);
            }
        }
        if let Some(w) = &self.score {
            w.values(&mut values);
        }
        for v in values {
            tracer.trace(v);
        }
    }

    /// Its translators' flags, one word each (N8's checkpoints,
    /// `checkpoint.rs`), the Scheme values they hold put in `values`, and
    /// what they hold in Rust given to `held`.
    pub(crate) fn state(&self, values: &mut Vec<Value>, held: &mut dyn Held) -> Vec<u64> {
        let mut flags = Vec::with_capacity(self.slots.len() + 1);
        for s in &self.slots {
            flags.push(
                u64::from(s.awake) | u64::from(s.keep) << 1 | u64::from(s.work.is_some()) << 2,
            );
            if let Some(w) = &s.work {
                w.values(values);
                w.held(held);
            }
        }
        if let Some(w) = &self.score {
            w.values(values);
            w.held(held);
        }
        flags.push(self.announced.len() as u64);
        flags
    }

    /// The names of its translators, in order, for a test.
    pub(crate) fn names(&self) -> Vec<String> {
        (0..self.slots.len())
            .filter_map(|i| self.name(i).map(String::from))
            .collect()
    }

    /// The classes of its translators that have no work (D67).
    pub(crate) fn placeholders(&self) -> Vec<&'static str> {
        self.slots
            .iter()
            .filter_map(|s| match s.what {
                What::Class(c) if c.make.is_none() => Some(c.name),
                _ => None,
            })
            .collect()
    }

    /// The class of its translator `i`.
    pub(crate) fn name(&self, i: usize) -> Option<&'static str> {
        self.slots.get(i).map(|s| match s.what {
            What::Class(c) => c.name,
            What::Scheme(_) => "Scheme_engraver",
        })
    }
}

/// What a method runs with: the VM, the translation, and where it is.
pub struct Tx<'a> {
    pub host: &'a mut dyn Host,
    pub t: &'a Translation,
    /// Its context.
    pub context: CtxId,
    slot: u16,
    keep: bool,
}

impl Tx<'_> {
    pub fn heap(&mut self) -> &mut Heap {
        self.host.heap()
    }

    /// `get_property`: `name` seen from the context, `'()` if unset.
    pub fn get(&mut self, name: &str) -> Value {
        let sym = self.host.heap().intern(name);
        self.t.arena.borrow().get(self.context, sym)
    }

    /// `set_property (context (), …)`, unchecked, as LilyPond's C++ sets.
    pub fn set(&mut self, name: &str, value: Value) {
        let sym = self.host.heap().intern(name);
        self.t.arena.borrow_mut().set(self.context, sym, value);
    }

    /// `context ()->unset_property`.
    pub fn unset(&mut self, name: &str) {
        let sym = self.host.heap().intern(name);
        self.t.arena.borrow_mut().unset(self.context, sym);
    }

    pub fn now(&self) -> Moment {
        self.t.arena.borrow().now()
    }

    /// Global's `previous_moment`.
    pub fn prev(&self) -> Moment {
        self.t.arena.borrow().prev()
    }

    /// The moment the context was added to the tree.
    pub fn init_mom(&self) -> Moment {
        self.t.arena.borrow().init_mom(self.context)
    }

    /// `add_moment_to_process`, LilyPond's programming error for one past.
    pub fn add_moment(&mut self, m: Moment) -> Run<()> {
        if !self.t.arena.borrow_mut().add_moment(m) {
            return self.programming_error("trying to freeze in time");
        }
        Ok(())
    }

    pub fn warning(&mut self, text: &str) -> Run<()> {
        diagnose(self.host, "warning", text)
    }

    pub fn programming_error(&mut self, text: &str) -> Run<()> {
        diagnose(self.host, "programming-error", text)
    }

    /// `Stream_event::warning`: at the event's origin, or nowhere.
    pub fn event_warning(&mut self, ev: Value, text: &str) -> Run<()> {
        let origin = event_property(self.host.heap(), ev, "origin");
        if self.host.heap().foreign::<Input>(origin).is_none() {
            return self.warning(text);
        }
        if let Some(proc) = self.host.lily("ly:input-warning") {
            let text = self.host.heap().string(text.replace('~', "~~"));
            self.host.call(proc, &[origin, text])?;
        }
        Ok(())
    }

    /// Calls `(lily)`'s procedure `name`, `'()` if it has none, sending what
    /// it broadcasts when it returns (D85).
    pub fn call(&mut self, name: &str, args: &[Value]) -> Run<Value> {
        match self.host.lily(name) {
            Some(proc) => crate::context::call(self.host, self.t, proc, args),
            None => Ok(Value::NULL),
        }
    }

    /// Calls the procedure `proc`, sending what it broadcasts when it
    /// returns (D85).
    pub fn apply(&mut self, proc: Value, args: &[Value]) -> Run<Value> {
        crate::context::call(self.host, self.t, proc, args)
    }

    /// The translator's Scheme handle.
    pub fn handle(&mut self) -> Value {
        let (t, c, slot) = (self.t, self.context, self.slot);
        translator_handle(self.host.heap(), t, c, slot).unwrap_or(Value::FALSE)
    }

    /// Wakes the translator at the next timestep too.
    pub fn stay_awake(&mut self) {
        self.keep = true;
    }

    /// `make_grob` and `announce_grob`: a grob record of the type `name`
    /// with `interfaces` (a list of symbols), announced to the groups up
    /// from the context.
    pub fn make_grob(&mut self, name: Value, cause: Cause, interfaces: Value) -> u32 {
        let heap = self.host.heap();
        let mut a = self.t.arena.borrow_mut();
        let now = a.now();
        let interfaces = a.records.interfaces.set(heap, interfaces);
        let id = a.records.grobs.len() as u32;
        a.records.grobs.push(GrobRecord {
            name,
            origin: (self.context, self.slot),
            cause,
            moment: now,
            ended: None,
            interfaces,
            props: Value::NULL,
            definition: Value::NULL,
            handle: Value::FALSE,
        });
        let made = a.records.grobs[id as usize].clone();
        a.effect(crate::replay::Effect::Grob(id, Box::new(made)));
        announce_grob(&mut a, self.context, id, false);
        id
    }

    /// A grob record of the type `name` whose definition is its properties
    /// seen from the context (`Grob_property_info::updated`, `'()` if it
    /// has none), with the definition's interfaces, not announced: as
    /// `Score_engraver` makes the System (`new System (props)`).
    pub fn make_unannounced_grob(&mut self, name: Value, cause: Cause) -> u32 {
        let heap = self.host.heap();
        let mut a = self.t.arena.borrow_mut();
        let props = crate::override_::updated(heap, &a, self.context, name);
        crate::scheme_translator::push_record(
            heap,
            &mut a,
            (self.context, self.slot),
            name,
            cause,
            props,
        )
    }

    /// The translator: its context and its number there, the origin of the
    /// records it makes (`Grob_info::origin_engraver`).
    pub fn origin(&self) -> Origin {
        (self.context, self.slot)
    }

    /// `internal_make_grob`'s record: a grob record of the type `name`
    /// whose definition is its properties seen from the context, announced
    /// to the groups up from it, as `ly:engraver-make-grob` makes one;
    /// LilyPond's error, a `misc-error` here, for a type with no
    /// definition.
    pub fn make_defined_grob(&mut self, name: &str, cause: Cause) -> Run<u32> {
        self.make_defined_grob_from(self.origin(), name, cause)
    }

    /// [`Self::make_defined_grob`] as the translator `origin` makes it,
    /// `trans->make_item` of another engraver (`origin_engraver` of a grob
    /// it acknowledged, as `accidental-engraver.cc:296` makes `Accidental`
    /// through the note head's): its definition seen from that translator's
    /// context, its origin that translator, announced to the groups up from
    /// that context. Only the record is made; the translator `origin` is
    /// neither called nor woken.
    pub fn make_defined_grob_from(&mut self, origin: Origin, name: &str, cause: Cause) -> Run<u32> {
        self.make_defined_grob_at(origin.0, origin, name, cause)
    }

    /// [`Self::make_defined_grob`] in the context `c`, a child's: its
    /// definition seen from `c`, announced to the groups up from `c`, its
    /// origin this translator, as `span-bar-stub-engraver.cc:174–179`
    /// makes a `SpanBarStub` (`new Item (Grob_property_info (ctx,
    /// …).updated ())`, `announce_grob (gi, ctx)`).
    pub fn make_defined_grob_in(&mut self, c: CtxId, name: &str, cause: Cause) -> Run<u32> {
        let origin = self.origin();
        self.make_defined_grob_at(c, origin, name, cause)
    }

    fn make_defined_grob_at(
        &mut self,
        c: CtxId,
        origin: Origin,
        name: &str,
        cause: Cause,
    ) -> Run<u32> {
        let heap = self.host.heap();
        let sym = heap.intern(name);
        let mut a = self.t.arena.borrow_mut();
        let props = crate::override_::updated(heap, &a, c, sym);
        if props.tag() != Tag::Pair {
            let key = heap.intern("misc-error");
            let text = heap.string(format!("No grob definition found for `{name}'."));
            let args = heap.list(&[Value::FALSE, text, Value::NULL, Value::FALSE]);
            return Err(Exception { key, args });
        }
        let id = crate::scheme_translator::push_record(heap, &mut a, origin, sym, cause, props);
        announce_grob(&mut a, c, id, false);
        Ok(id)
    }

    /// `Grob_property_info (context (), name).updated ()`: the properties
    /// of the grob type `name` seen from the context, `'()` if it has none.
    pub fn updated(&mut self, name: &str) -> Value {
        let heap = self.host.heap();
        let sym = heap.intern(name);
        let a = self.t.arena.borrow();
        crate::override_::updated(heap, &a, self.context, sym)
    }

    /// `new Spanner (props)` and `make_grob_info`: a grob record of the
    /// type `name` whose definition is `props`, made by this translator for
    /// `cause` and not announced, as `Auto_beam_engraver` makes its beam
    /// from the settings it kept when the beam began
    /// (`auto-beam-engraver.cc:198–213`).
    pub fn make_grob_with(&mut self, name: &str, cause: Cause, props: Value) -> u32 {
        let origin = self.origin();
        let heap = self.host.heap();
        let sym = heap.intern(name);
        let mut a = self.t.arena.borrow_mut();
        crate::scheme_translator::push_record(heap, &mut a, origin, sym, cause, props)
    }

    /// `announce_grob_locally_only` (`end`:
    /// `announce_end_grob_locally_only`): the grob record `grob` announced
    /// to the context's group only.
    pub fn announce_locally(&mut self, grob: u32, end: bool) {
        if end {
            crate::context::grob_ended(&self.t.arena, grob);
        }
        let mut a = self.t.arena.borrow_mut();
        a.effect(crate::replay::Effect::Announce {
            grob,
            ctx: self.context,
            end,
            local: true,
        });
        if end {
            a.end_grob(grob);
        }
        if let Some(g) = a.group_mut(self.context)
            && g.kind.engraves()
        {
            g.announced.push((grob, end));
            if !end && g.score.is_some() {
                g.untypeset.push(grob);
            }
        }
    }

    /// `announce_grob (info, ctx)` (`end`: `announce_end_grob (info,
    /// ctx)`): the grob record `grob` announced to `c`'s group and each
    /// engraver group above it.
    pub fn announce_in(&mut self, c: CtxId, grob: u32, end: bool) {
        if end {
            crate::context::grob_ended(&self.t.arena, grob);
        }
        let mut a = self.t.arena.borrow_mut();
        if end {
            a.end_grob(grob);
        }
        announce_grob(&mut a, c, grob, end);
    }

    /// The handle of the grob record `id`.
    pub fn grob_handle(&mut self, id: u32) -> Value {
        let t = self.t;
        crate::scheme_translator::grob_handle(self.host.heap(), &t.arena, t.value, id)
    }

    /// `announce_end_grob`.
    pub fn announce_end_grob(&mut self, grob: u32) {
        crate::context::grob_ended(&self.t.arena, grob);
        let mut a = self.t.arena.borrow_mut();
        a.end_grob(grob);
        announce_grob(&mut a, self.context, grob, true);
    }

    /// An audio record, announced to the performer groups up from the
    /// context (`announce_element`).
    pub fn make_audio(&mut self, name: Value, cause: Cause, value: Value) -> u32 {
        self.push_audio(name, cause, value, Audio::Other)
    }

    /// `Performer::announce`: the audio element `audio` (`audio.rs`),
    /// made for `cause` and announced to the performer groups up from the
    /// context, a staff that reaches the Score's among the performance's.
    pub fn announce(&mut self, audio: Audio, cause: Cause) -> u32 {
        let name = self.host.heap().intern(audio.name());
        self.push_audio(name, cause, Value::FALSE, audio)
    }

    fn push_audio(&mut self, name: Value, cause: Cause, value: Value, audio: Audio) -> u32 {
        let mut a = self.t.arena.borrow_mut();
        let now = a.now();
        let record = AudioRecord {
            name,
            origin: (self.context, self.slot),
            cause,
            moment: now,
            value,
            audio,
            column: None,
            channel: 0,
        };
        add_audio(&mut a, record)
    }
}

/// `r` an audio record of the translation, announced to the performer groups
/// up from its origin's context.
pub(crate) fn add_audio(a: &mut Arena, r: AudioRecord) -> u32 {
    let id = a.records.audio.len() as u32;
    let mut at = Some(r.origin.0);
    a.effect(crate::replay::Effect::Audio(id, Box::new(r.clone())));
    a.records.audio.push(r);
    while let Some(c) = at {
        match a.group_mut(c) {
            Some(g) if g.kind.performs() => {
                g.audio_announced.push(id);
                if g.kind == GroupKind::ScorePerformer {
                    crate::performance::announced(a, id);
                }
            }
            _ => break,
        }
        at = a.parent(c);
    }
    id
}

/// `announce_grob_locally_only` of the grob record `grob` in `c` (N8's
/// replay, MS6 step 7b).
pub(crate) fn announce_locally_in(a: &mut Arena, c: CtxId, grob: u32, end: bool) {
    if let Some(g) = a.group_mut(c)
        && g.kind.engraves()
    {
        g.announced.push((grob, end));
        if !end && g.score.is_some() {
            g.untypeset.push(grob);
        }
    }
}

/// The translator `slot` of `c` stays awake for the next timestep (N8's
/// replay of `stay_awake`).
pub(crate) fn keep_awake(a: &mut Arena, c: CtxId, slot: u16) {
    if let Some(s) = a
        .group_mut(c)
        .and_then(|g| g.slots.get_mut(usize::from(slot)))
    {
        s.keep = true;
    }
}

/// `Engraver_group::add_grob_to_announce`: to `c`'s group and each engraver
/// group above it. A Score's group with a layout part keeps each start for
/// it too (`Score_engraver::add_grob_to_announce_locally_only`).
pub(crate) fn announce_grob(a: &mut Arena, c: CtxId, grob: u32, end: bool) {
    a.effect(crate::replay::Effect::Announce {
        grob,
        ctx: c,
        end,
        local: false,
    });
    let mut at = Some(c);
    while let Some(x) = at {
        match a.group_mut(x) {
            Some(g) if g.kind.engraves() => {
                g.announced.push((grob, end));
                if !end && g.score.is_some() {
                    g.untypeset.push(grob);
                }
            }
            _ => break,
        }
        at = a.parent(x);
    }
}

// --- groups ----------------------------------------------------------------

/// `ly:make-global-translator`: Global's group, which makes its children's.
pub fn connect_global(host: &mut dyn Host, t: &Translation) {
    let heap = host.heap();
    let mut a = t.arena.borrow_mut();
    a.set_group(GLOBAL, new_group(GroupKind::Plain, Vec::new()));
    let source = a.event_source(GLOBAL);
    a.add_listener(
        heap,
        source,
        Listener::Context(GLOBAL, Handler::CreateChildTranslator),
        "AnnounceNewContext",
    );
}

fn new_group(kind: GroupKind, slots: Vec<Slot>) -> Group {
    Group {
        kind,
        slots,
        announced: Vec::new(),
        audio_announced: Vec::new(),
        acks: BTreeMap::new(),
        score: None,
        untypeset: Vec::new(),
    }
}

/// `create_child_translator`: the group of the context `ev` announces, its
/// translators connected and initialized.
pub(crate) fn create_child_translator(host: &mut dyn Host, t: &Translation, ev: Value) -> Run<()> {
    let child = event_property(host.heap(), ev, "context");
    let Some(child) = host
        .heap()
        .foreign::<crate::ContextHandle>(child)
        .map(|h| h.id)
    else {
        return Ok(());
    };
    let (def, mods) = {
        let a = t.arena.borrow();
        (a.def(child), a.mods(child))
    };
    let heap = host.heap();
    let names = ContextDef::names_of(heap, def, mods);
    let depth = heap.root_depth();
    heap.root(names);
    let ty = heap
        .foreign::<ContextDef>(def)
        .map_or(Value::NULL, ContextDef::group_type);
    let kind = match heap.name(ty) {
        Some("Engraver_group") => GroupKind::Engraver,
        Some("Score_engraver") => GroupKind::ScoreEngraver,
        Some("Performer_group") => GroupKind::Performer,
        Some("Score_performer") => GroupKind::ScorePerformer,
        _ => {
            let mut text = String::new();
            heap.write(ty, &mut text);
            let text = format!(
                "Couldn't find translator type {text} (should be Engraver_group, \
                 Performer_group, Score_engraver or Score_performer)"
            );
            if let Err(e) = diagnose(host, "programming-error", &text) {
                host.heap().unroot_to(depth);
                return Err(e);
            }
            GroupKind::Plain
        }
    };
    let mut list: Vec<Slot> = Vec::new();
    let mut p = names;
    let mut add = |slot: Slot| {
        if slot.must_be_last || list.is_empty() {
            list.push(slot);
        } else {
            list.insert(0, slot);
        }
    };
    let ran = (|| {
        while let (Some(name), Some(rest)) = (host.heap().car(p), host.heap().cdr(p)) {
            p = rest;
            if let Some(slot) = resolve(host, t, child, name)? {
                add(slot);
            }
        }
        // The playback map's performers, where `timing.ly`'s `\consists`
        // would put them (`playback.rs`).
        if kind.performs()
            && let Some(class) = crate::playback::class_for(host.heap(), &t.arena.borrow(), def)
        {
            add(Slot::of_class(class));
        }
        Ok(())
    })();
    host.heap().unroot_to(depth);
    ran?;
    // `\with { \consists … }` may name one of the other output.
    list.retain(|s| {
        if kind.engraves() {
            s.layout
        } else if kind.performs() {
            s.midi
        } else {
            true
        }
    });
    let n = list.len();
    let mut group = new_group(kind, list);
    if kind == GroupKind::ScoreEngraver {
        let extra = t.arena.borrow().extra_classes.clone();
        group.score = extra
            .iter()
            .find(|c| c.name == SCORE_ENGRAVER)
            .and_then(|c| c.make)
            .map(|make| make());
    }
    {
        let heap = host.heap();
        let mut a = t.arena.borrow_mut();
        a.set_group(child, group);
        let (source, below) = (a.event_source(child), a.events_below(child));
        a.add_listener(
            heap,
            source,
            Listener::Context(child, Handler::CreateChildTranslator),
            "AnnounceNewContext",
        );
        for i in 0..n {
            let Some(slot) = a.group(child).map(|g| &g.slots[i]) else {
                continue;
            };
            let (hears, scheme) = (slot.hears(heap), matches!(slot.what, What::Scheme(_)));
            // A class's `listener_list_` holds the last added first; a
            // Scheme translator's listeners are its alist's, reversed.
            let order: Vec<usize> = if scheme {
                (0..hears.len()).collect()
            } else {
                (0..hears.len()).rev().collect()
            };
            for j in order {
                let l = Listener::Translator(child, i as u16, j as u8);
                if let Some(class) = a.class_number(heap, hears[j]) {
                    a.dispatchers.add_listener(below, l, class);
                }
            }
        }
    }
    for i in 0..n {
        call_slot(host, t, child, i, |w, tx| {
            w.connect(tx);
            Ok(())
        })?;
    }
    {
        let heap = host.heap();
        let mut a = t.arena.borrow_mut();
        if kind.engraves() {
            let source = a.event_source(child);
            let l = |h| Listener::Context(child, h);
            a.add_listener(heap, source, l(Handler::Override), "Override");
            a.add_listener(heap, source, l(Handler::Revert), "Revert");
        }
        if matches!(kind, GroupKind::ScoreEngraver | GroupKind::ScorePerformer) {
            let top = a.event_source(GLOBAL);
            let l = |h| Listener::Context(child, h);
            a.add_listener(heap, top, l(Handler::OneTimeStep), "OneTimeStep");
            a.add_listener(heap, top, l(Handler::TimestepStart), "Prepare");
            a.add_listener(heap, top, l(Handler::Finish), "Finish");
        }
    }
    // `Score_engraver::initialize` or `Score_performer::initialize`, then
    // its translators'.
    if kind == GroupKind::ScorePerformer {
        crate::performance::initialize(host, t, child);
    }
    call_score(host, t, child, |w, tx| w.initialize(tx))?;
    run_phase(host, t, child, INITIALIZE)?;
    crate::trace::created(host.heap(), &mut t.arena.borrow_mut(), child);
    Ok(())
}

/// The translator `name`, an item of a definition's `consists`, makes in
/// `child`, as `create_child_translator` does: a class of the translation
/// or of [`CLASSES`]; else a creator `ly:register-translator` keeps, or a
/// procedure, called with the context; and an alist, a Scheme translator.
/// `None` with LilyPond's warnings. What it made stays rooted for the
/// caller to unroot once its group holds it.
fn resolve(host: &mut dyn Host, t: &Translation, child: CtxId, name: Value) -> Run<Option<Slot>> {
    let mut def = name;
    if name.tag() == Tag::Symbol {
        let text = String::from(host.heap().name(name).unwrap_or_default());
        let extra = t.arena.borrow().extra_classes.clone();
        if let Some(&c) = extra
            .iter()
            .find(|c| c.name == text && c.name != SCORE_ENGRAVER)
        {
            return Ok(Some(Slot::of_class(c)));
        }
        if let Some(c) = CLASSES.iter().find(|c| c.name == text) {
            return Ok(Some(Slot::of_class(c)));
        }
        def = match host.lily("%translator-creator") {
            Some(proc) => host.call(proc, &[name])?,
            None => Value::FALSE,
        };
        if def == Value::FALSE {
            diagnose(host, "warning", &format!("unknown translator: `{text}'"))?;
        }
    }
    if def == name {
        // A translator the score gives as a procedure or an alist, which
        // might read any context: N8's partitions are one (`partition.rs`).
        if let Some(r) = &mut t.arena.borrow_mut().n8 {
            r.parts.merge_all("a translator of the score's own");
        }
    }
    if host.heap().is_procedure(def) {
        let context = t.arena.borrow().handle(child);
        def = crate::context::call(host, t, def, &[context])?;
    }
    if !matches!(def.tag(), Tag::Pair | Tag::Null) {
        return cannot_find(host, name);
    }
    host.heap().root(def);
    let work = SchemeTranslator::new(host.heap(), def);
    Ok(Some(Slot::of_scheme(def, work)))
}

fn cannot_find(host: &mut dyn Host, name: Value) -> Run<Option<Slot>> {
    let mut text = String::new();
    host.heap().write(name, &mut text);
    diagnose(host, "warning", &format!("cannot find: `{text}'"))?;
    Ok(None)
}

/// `disconnect_from_context`, when `c` is removed: its translators'
/// listeners and the group's taken from the context's dispatchers.
pub(crate) fn disconnect(heap: &mut Heap, a: &mut Arena, c: CtxId) {
    let Some(g) = a.group(c) else {
        return;
    };
    let kind = g.kind;
    let hears: Vec<Vec<Value>> = (0..g.slots.len())
        .map(|i| {
            a.group(c)
                .map(|g| g.slots[i].hears(heap))
                .unwrap_or_default()
        })
        .collect();
    let (source, below) = (a.event_source(c), a.events_below(c));
    let mut remove = |a: &mut Arena, d, l, class: &str| {
        let sym = heap.intern(class);
        if let Some(n) = a.classes.number(sym) {
            a.dispatchers.remove_listener(d, l, n);
        }
    };
    for (i, classes) in hears.into_iter().enumerate() {
        for (j, class) in classes.into_iter().enumerate() {
            if let Some(n) = a.classes.number(class) {
                let l = Listener::Translator(c, i as u16, j as u8);
                a.dispatchers.remove_listener(below, l, n);
            }
        }
    }
    if kind.engraves() {
        remove(
            a,
            source,
            Listener::Context(c, Handler::Override),
            "Override",
        );
        remove(a, source, Listener::Context(c, Handler::Revert), "Revert");
    }
    let l = Listener::Context(c, Handler::CreateChildTranslator);
    remove(a, source, l, "AnnounceNewContext");
}

// --- the protocol ----------------------------------------------------------

/// Calls `f` on the state of `c`'s translator `i`, if it has one, out of
/// the arena, with its values rooted (D83).
fn call_slot(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    i: usize,
    f: impl FnOnce(&mut dyn Work, &mut Tx<'_>) -> Run<()>,
) -> Run<()> {
    let work = t
        .arena
        .borrow_mut()
        .group_mut(c)
        .and_then(|g| g.slots.get_mut(i))
        .and_then(|s| s.work.take());
    let Some(mut work) = work else {
        return Ok(());
    };
    // A replayed part's translator: the old run's call made again
    // (MS6 step 7b).
    let replayed = crate::replay::instead(host, t, c, i as u16);
    if !matches!(replayed, Ok(false)) {
        if let Some(s) = t
            .arena
            .borrow_mut()
            .group_mut(c)
            .and_then(|g| g.slots.get_mut(i))
        {
            s.work = Some(work);
        }
        replayed?;
        return typeset_announced(host, t, c);
    }
    let depth = host.heap().root_depth();
    host.heap().root_all(|roots| work.values(roots));
    let mut tx = Tx {
        host,
        t,
        context: c,
        slot: i as u16,
        keep: false,
    };
    // The writes it makes to grobs are this context's (N8, MS6 step 7a),
    // and what it does is logged, a partition's (MS6 step 7b).
    let was = t.arena.borrow().writer.replace(Some(c));
    let log = crate::replay::begin(&mut *tx.host, t, c);
    let ran = f(&mut *work, &mut tx);
    let keep = tx.keep;
    if keep {
        t.arena.borrow().effect(crate::replay::Effect::Keep);
    }
    if let Some(log) = log {
        let errors = tx.host.error_len();
        log.borrow_mut().end(tx.host.heap(), errors);
    }
    t.arena.borrow().writer.set(was);
    host.heap().unroot_to(depth);
    if let Some(s) = t
        .arena
        .borrow_mut()
        .group_mut(c)
        .and_then(|g| g.slots.get_mut(i))
    {
        s.work = Some(work);
        s.keep |= keep;
    }
    ran?;
    typeset_announced(host, t, c)
}

/// Calls `f` on the layout part of `score`'s `Score_engraver`, if it has
/// one, as [`call_slot`] calls a translator.
fn call_score(
    host: &mut dyn Host,
    t: &Translation,
    score: CtxId,
    f: impl FnOnce(&mut dyn Work, &mut Tx<'_>) -> Run<()>,
) -> Run<()> {
    let work = t
        .arena
        .borrow_mut()
        .group_mut(score)
        .and_then(|g| g.score.take());
    let Some(mut work) = work else {
        return Ok(());
    };
    let depth = host.heap().root_depth();
    host.heap().root_all(|roots| work.values(roots));
    let mut tx = Tx {
        host,
        t,
        context: score,
        slot: SCORE_PART,
        keep: false,
    };
    let ran = f(&mut *work, &mut tx);
    host.heap().unroot_to(depth);
    if let Some(g) = t.arena.borrow_mut().group_mut(score) {
        g.score = Some(work);
    }
    ran
}

/// `Score_engraver::add_grob_to_announce_locally_only`'s part: the grobs
/// whose starts were announced to the Score group above `c`, given to its
/// layout part in the order they were announced.
fn typeset_announced(host: &mut dyn Host, t: &Translation, c: CtxId) -> Run<()> {
    let score = {
        let a = t.arena.borrow();
        let mut at = c;
        loop {
            match a.group(at) {
                Some(g) if g.kind == GroupKind::ScoreEngraver => break at,
                Some(g) if g.kind.engraves() => match a.parent(at) {
                    Some(p) => at = p,
                    None => return Ok(()),
                },
                _ => return Ok(()),
            }
        }
    };
    loop {
        let pending = t
            .arena
            .borrow_mut()
            .group_mut(score)
            .map(|g| core::mem::take(&mut g.untypeset))
            .unwrap_or_default();
        if pending.is_empty() {
            return Ok(());
        }
        for grob in pending {
            call_score(host, t, score, |w, tx| w.acknowledge(tx, 0, grob))?;
        }
    }
}

/// `precomputed_translator_foreach` of `c`'s group: each translator that
/// has `method`, of those that run always or are awake for a timestep's
/// method; after `stop_translation_timestep`, each is awake for the next
/// timestep only if it asked.
fn run_phase(host: &mut dyn Host, t: &Translation, c: CtxId, method: u8) -> Run<()> {
    let n = t.arena.borrow().group(c).map_or(0, |g| g.slots.len());
    for i in 0..n {
        let call = {
            let a = t.arena.borrow();
            let Some(s) = a.group(c).and_then(|g| g.slots.get(i)) else {
                break;
            };
            s.methods & method != 0
                && (s.always || s.awake || method & (INITIALIZE | FINALIZE) != 0)
        };
        if call {
            call_slot(host, t, c, i, |w, tx| match method {
                INITIALIZE => w.initialize(tx),
                START => w.start_translation_timestep(tx),
                PRE_PROCESS => w.pre_process_music(tx),
                PROCESS => w.process_music(tx),
                PROCESS_ACKNOWLEDGED => w.process_acknowledged(tx),
                STOP => w.stop_translation_timestep(tx),
                _ => w.finalize(tx),
            })?;
        }
        if method == STOP
            && let Some(s) = t
                .arena
                .borrow_mut()
                .group_mut(c)
                .and_then(|g| g.slots.get_mut(i))
        {
            s.awake = core::mem::take(&mut s.keep);
        }
    }
    Ok(())
}

/// `precomputed_recurse_over_translators`: `method` over `c`'s tree, each
/// group before its children (`down`) or after them.
fn recurse(host: &mut dyn Host, t: &Translation, c: CtxId, method: u8, down: bool) -> Run<()> {
    if down {
        run_phase(host, t, c, method)?;
    }
    let children = t.arena.borrow().children(c).to_vec();
    for child in children {
        recurse(host, t, child, method, down)?;
    }
    if !down {
        run_phase(host, t, c, method)?;
    }
    Ok(())
}

/// `recurse_over_translators` of `finalize`, children first, each context's
/// finalization recorded by a traced translation.
pub(crate) fn finalize(host: &mut dyn Host, t: &Translation, c: CtxId) -> Run<()> {
    let children = t.arena.borrow().children(c).to_vec();
    for child in children {
        finalize(host, t, child)?;
    }
    run_phase(host, t, c, FINALIZE)?;
    // `Score_engraver::finalize`'s `typeset_all`.
    typeset_announced(host, t, c)?;
    call_score(host, t, c, |w, tx| w.finalize(tx))?;
    crate::trace::finalized(host, t, c);
    Ok(())
}

/// The translator `i` of `c` heard `ev` with its class's listener `which`:
/// it is awake for the timestep.
pub(crate) fn listen(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    i: u16,
    which: u8,
    ev: Value,
) -> Run<()> {
    wake(t, c, i);
    call_slot(host, t, c, usize::from(i), |w, tx| {
        w.listen(tx, usize::from(which), ev)
    })
}

fn wake(t: &Translation, c: CtxId, i: u16) {
    if let Some(s) = t
        .arena
        .borrow_mut()
        .group_mut(c)
        .and_then(|g| g.slots.get_mut(usize::from(i)))
    {
        s.awake = true;
    }
}

/// A Score group's handlers of Global's events: `prepare`, `one_time_step`
/// and `finish` of `Score_engraver` or `Score_performer`.
pub(crate) fn score_handler(
    host: &mut dyn Host,
    t: &Translation,
    score: CtxId,
    handler: Handler,
) -> Run<()> {
    let kind = t.arena.borrow().group(score).map(|g| g.kind);
    let performs = kind == Some(GroupKind::ScorePerformer);
    match handler {
        Handler::TimestepStart => {
            if performs {
                crate::performance::prepare(&mut t.arena.borrow_mut());
            }
            recurse(host, t, score, START, !performs)
        }
        Handler::OneTimeStep => {
            let skip = if performs {
                // The timestep's audio column and `midiSkipOffset`.
                crate::performance::one_time_step(host, t, score)
            } else {
                let heap = host.heap();
                let sym = heap.intern("skipTypesetting");
                t.arena.borrow().get(score, sym) == Value::TRUE
            };
            // `Score_engraver` pre-processes even when it skips typesetting,
            // since some engravers need its columns; `Score_performer` not.
            if !performs {
                recurse(host, t, score, PRE_PROCESS, false)?;
            }
            if !skip {
                if performs {
                    recurse(host, t, score, PRE_PROCESS, false)?;
                }
                recurse(host, t, score, PROCESS, false)?;
                if performs {
                    announce_audio(host, t, score)?;
                } else {
                    do_announces(host, t, score)?;
                }
            }
            recurse(host, t, score, STOP, false)?;
            if !performs {
                // `typeset_all`.
                typeset_announced(host, t, score)?;
                call_score(host, t, score, |w, tx| w.stop_translation_timestep(tx))?;
            }
            crate::trace::one_time_step(host.heap(), &mut t.arena.borrow_mut());
            Ok(())
        }
        _ => {
            if performs {
                crate::performance::finish(host, t, score);
            }
            finalize(host, t, score)
        }
    }
}

/// `Engraver_group::do_announces`: the children's first, then
/// `process_acknowledged` and the grobs announced to `c`'s group
/// acknowledged, until none are left here or below.
fn do_announces(host: &mut dyn Host, t: &Translation, c: CtxId) -> Run<()> {
    loop {
        let children = t.arena.borrow().children(c).to_vec();
        for child in children {
            let engraves = t
                .arena
                .borrow()
                .group(child)
                .is_some_and(|g| g.kind.engraves());
            if engraves {
                do_announces(host, t, child)?;
            }
        }
        loop {
            run_phase(host, t, c, PROCESS_ACKNOWLEDGED)?;
            if t.arena
                .borrow()
                .group(c)
                .is_none_or(|g| g.announced.is_empty())
            {
                break;
            }
            // The list may grow while it is acknowledged.
            let mut j = 0;
            loop {
                let info = t
                    .arena
                    .borrow()
                    .group(c)
                    .and_then(|g| g.announced.get(j).copied());
                let Some((grob, end)) = info else {
                    break;
                };
                crate::trace::grob_heard(host.heap(), &mut t.arena.borrow_mut(), c, grob, end);
                acknowledge(host, t, c, grob, end)?;
                j += 1;
            }
            if let Some(g) = t.arena.borrow_mut().group_mut(c) {
                g.announced.clear();
            }
        }
        if !pending(&t.arena.borrow(), c) {
            return Ok(());
        }
    }
}

/// `pending_grobs`: grobs announced to `c`'s group or a group below.
fn pending(a: &Arena, c: CtxId) -> bool {
    a.group(c).is_some_and(|g| !g.announced.is_empty())
        || a.children(c)
            .iter()
            .any(|&k| a.group(k).is_some_and(|g| g.kind.engraves()) && pending(a, k))
}

/// `acknowledge_grobs` of one announcement: the acknowledgers of `c`'s group
/// for the grob's type and class, but the translator that made it. A grob that died
/// since it was announced, its interfaces gone, is skipped, as LilyPond
/// skips one with no `meta` (`engraver-group.cc`).
fn acknowledge(host: &mut dyn Host, t: &Translation, c: CtxId, grob: u32, end: bool) -> Run<()> {
    let (list, origin) = {
        let mut a = t.arena.borrow_mut();
        let Some(g) = a.records.grobs.get(grob as usize) else {
            return Ok(());
        };
        let (name, interfaces, origin) = (g.name, g.interfaces, g.origin);
        if interfaces.iter().next().is_none() {
            return Ok(());
        }
        // A type of several classes (a sticky grob's: Footnote,
        // BalloonText, Parentheses) is acknowledged by its class, as
        // LilyPond's typed acknowledgers (`Grob_info_t<Item>`) take only
        // their class (MS8 step 8's part 2: a spanner's Footnote is no
        // item).
        let heap = host.heap();
        let mut classes = 0u8;
        for (bit, iface) in [(1u8, "item-interface"), (2, "spanner-interface")] {
            let sym = heap.intern(iface);
            if a.records
                .interfaces
                .number(sym)
                .is_some_and(|n| interfaces.contains(n))
            {
                classes |= bit;
            }
        }
        let key = (name.to_bits(), end, classes);
        let cached = a.group(c).and_then(|g| g.acks.get(&key).cloned());
        let list = match cached {
            Some(list) => list,
            None => {
                let heap = host.heap();
                // Each translator's acknowledgers: its class's, or a Scheme
                // translator's `acknowledgers` and `end-acknowledgers`.
                let mut numbers: Vec<(u16, u8, Value, bool)> = Vec::new();
                for (i, s) in a.group(c).map_or(&[][..], |g| &g.slots).iter().enumerate() {
                    match s.what {
                        What::Class(class) => {
                            for (k, &(iface, e)) in class.acknowledges.iter().enumerate() {
                                numbers.push((i as u16, k as u8, heap.intern(iface), e));
                            }
                        }
                        What::Scheme(_) => {
                            let acks = s.work.as_ref().and_then(|w| w.scheme());
                            for (k, &(iface, _, e)) in acks
                                .map_or(&[][..], |w| &w.acknowledgers)
                                .iter()
                                .enumerate()
                            {
                                numbers.push((i as u16, k as u8, iface, e));
                            }
                        }
                    }
                }
                let mut list = Vec::new();
                for (i, k, sym, e) in numbers {
                    let n = a.records.interfaces.number(sym);
                    if e == end && n.is_some_and(|n| interfaces.contains(n)) {
                        list.push((i, k));
                    }
                }
                let list: Acknowledgers = list.into();
                if let Some(g) = a.group_mut(c) {
                    g.acks.insert(key, Rc::clone(&list));
                }
                list
            }
        };
        (list, origin)
    };
    for &(i, k) in list.iter().filter(|&&(i, _)| (c, i) != origin) {
        wake(t, c, i);
        call_slot(host, t, c, usize::from(i), |w, tx| {
            w.acknowledge(tx, usize::from(k), grob)
        })?;
    }
    Ok(())
}

/// `Performer_group::do_announces`: the children's first, then each audio
/// item announced to `c`'s group acknowledged by its performers but the
/// one that made it.
fn announce_audio(host: &mut dyn Host, t: &Translation, c: CtxId) -> Run<()> {
    let children = t.arena.borrow().children(c).to_vec();
    for child in children {
        let performs = t
            .arena
            .borrow()
            .group(child)
            .is_some_and(|g| g.kind.performs());
        if performs {
            announce_audio(host, t, child)?;
        }
    }
    // `Score_performer::acknowledge_audio_elements`: the items announced
    // to the Score so far in this pass are the timestep column's.
    {
        let mut a = t.arena.borrow_mut();
        let score = a.group(c).filter(|g| g.kind == GroupKind::ScorePerformer);
        if let Some(list) = score.map(|g| g.audio_announced.clone()) {
            crate::performance::add_to_column(&mut a, &list);
        }
    }
    let mut j = 0;
    loop {
        let (audio, takers) = {
            let a = t.arena.borrow();
            let Some(g) = a.group(c) else {
                return Ok(());
            };
            let Some(&audio) = g.audio_announced.get(j) else {
                break;
            };
            let origin = a.records.audio[audio as usize].origin;
            let takers: Vec<u16> = g
                .slots
                .iter()
                .enumerate()
                .filter(|&(i, s)| {
                    matches!(s.what, What::Class(class)
                        if class.kind == Kind::Performer && class.methods & ACKNOWLEDGE_AUDIO != 0)
                        && (c, i as u16) != origin
                })
                .map(|(i, _)| i as u16)
                .collect();
            (audio, takers)
        };
        for i in takers {
            wake(t, c, i);
            call_slot(host, t, c, usize::from(i), |w, tx| {
                w.acknowledge_audio(tx, audio)
            })?;
        }
        j += 1;
    }
    if let Some(g) = t.arena.borrow_mut().group_mut(c) {
        g.audio_announced.clear();
    }
    Ok(())
}

// --- the Scheme face -------------------------------------------------------

/// A translator's Scheme handle (`ly:translator?`).
pub struct TranslatorHandle {
    pub(crate) arena: Rc<RefCell<Arena>>,
    pub(crate) translation: Value,
    pub context: CtxId,
    pub slot: u16,
}

impl Foreign for TranslatorHandle {
    fn type_name(&self) -> &'static str {
        "Translator"
    }

    /// `#<Translator Timing_translator >`.
    fn print(&self, _: &Heap, out: &mut String) {
        let a = self.arena.borrow();
        out.push_str("#<Translator ");
        match a
            .group(self.context)
            .and_then(|g| g.slots.get(usize::from(self.slot)))
            .map(|s| s.what)
        {
            Some(What::Class(c)) => out.push_str(c.name),
            Some(What::Scheme(_)) => out.push_str("Scheme_engraver"),
            None => {}
        }
        out.push_str(" >");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.translation);
    }
}

/// The handle of `c`'s translator `slot`, made the first time.
pub fn translator_handle(heap: &mut Heap, t: &Translation, c: CtxId, slot: u16) -> Option<Value> {
    let mut a = t.arena.borrow_mut();
    let s = a.group_mut(c)?.slots.get_mut(usize::from(slot))?;
    if s.handle == Value::FALSE {
        s.handle = heap.wrap(TranslatorHandle {
            arena: Rc::clone(&t.arena),
            translation: t.value,
            context: c,
            slot,
        });
    }
    Some(s.handle)
}

/// A C++ class's creator (`Translator_creator`), by its place in
/// [`CLASSES`].
#[derive(Clone, Copy)]
struct Creator(u16);

impl Foreign for Creator {
    fn type_name(&self) -> &'static str {
        "Translator_creator"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Translator_creator ");
        out.push_str(CLASSES[usize::from(self.0)].name);
        out.push('>');
    }
}

/// `(ly:translator? x)`.
pub(crate) fn is_translator(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<TranslatorHandle>(cx, args[0])
}

/// `(ly:translator-context trans)`.
pub(crate) fn ly_translator_context(cx: &mut Cx, args: &[Value]) -> R {
    let Some((arena, c)) = cx
        .heap()
        .foreign::<TranslatorHandle>(args[0])
        .map(|h| (Rc::clone(&h.arena), h.context))
    else {
        return Err(cx.wrong_type("ly:translator-context", 1, args[0], "Translator"));
    };
    Ok(arena.borrow().handle(c))
}

/// `(%translator-creators)`: a creator of each of [`CLASSES`].
pub(crate) fn creators(cx: &mut Cx, _: &[Value]) -> R {
    let heap = cx.heap_mut();
    let mut list = Value::NULL;
    for i in (0..CLASSES.len()).rev() {
        let c = heap.wrap(Creator(i as u16));
        list = heap.cons(c, list);
    }
    Ok(list)
}

/// `(%class-creator-name x)`: the name of the class `x` creates, or `#f`.
pub(crate) fn creator_name(cx: &mut Cx, args: &[Value]) -> R {
    let Some(&Creator(i)) = cx.heap().foreign::<Creator>(args[0]) else {
        return Ok(Value::FALSE);
    };
    Ok(cx.heap_mut().intern(CLASSES[usize::from(i)].name))
}

/// `(%class-creator-description x)`: the description of the class `x`
/// creates, as `static_translator_description` makes it, or `#f`. Its
/// documentation and the grobs it creates are chapter 05's and empty.
pub(crate) fn creator_description(cx: &mut Cx, args: &[Value]) -> R {
    let Some(&Creator(i)) = cx.heap().foreign::<Creator>(args[0]) else {
        return Ok(Value::FALSE);
    };
    let class = &CLASSES[usize::from(i)];
    let heap = cx.heap_mut();
    let symbols = |heap: &mut Heap, names: &[&str]| {
        let items: Vec<Value> = names.iter().map(|n| heap.intern(n)).collect();
        heap.list(&items)
    };
    let mut alist = Value::NULL;
    let entries: [(&str, Value); 5] = [
        ("grobs-created", Value::NULL),
        ("description", heap.string("")),
        ("events-accepted", symbols(heap, class.hears)),
        ("properties-read", symbols(heap, class.reads)),
        ("properties-written", symbols(heap, class.writes)),
    ];
    for (key, value) in entries {
        let key = heap.intern(key);
        let entry = heap.cons(key, value);
        alist = heap.cons(entry, alist);
    }
    Ok(alist)
}

/// The Scheme half of the face: a creator's name and description from the
/// classes or from what `ly:register-translator` keeps (`%translators`:
/// name, creator and description), with `translator-ctors.cc`'s errors,
/// and the creator a name has there, which a group makes of a name no
/// class has.
pub(crate) const PRELUDE: &str = r#"
(define (%registered-creator creator)
  (hash-fold (lambda (name entry found) (or found (and (eq? (cadr entry) creator) entry)))
             #f %translators))
(define (%translator-definition subr creator)
  (or (%registered-creator creator)
      (scm-error 'wrong-type-arg subr
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list 1 "translator definition" creator) (list creator))))
(define-public (ly:translator-name creator)
  (or (%class-creator-name creator)
      (car (%translator-definition "ly_translator_name" creator))))
(define-public (ly:translator-description creator)
  (or (%class-creator-description creator)
      (caddr (%translator-definition "ly_translator_description" creator))))
(define-public (ly:get-all-translators)
  (append (%translator-creators)
          (hash-map->list (lambda (name entry) (cadr entry)) %translators)))
(define (%translator-creator name)
  (let ((entry (hashq-ref %translators name)))
    (and entry (cadr entry))))
"#;
