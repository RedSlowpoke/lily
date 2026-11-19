//! `piano-pedal-engraver.cc` and `piano-pedal-align-engraver.cc` (MS8
//! step 8, part 1, 528 and 278 lines): `Piano_pedal_engraver` and
//! `Piano_pedal_align_engraver`, classes of layout's
//! ([`PIANO_PEDAL_ENGRAVER`], [`PIANO_PEDAL_ALIGN_ENGRAVER`]) that shadow
//! the music crate's placeholders of their names (D67, D108).
//!
//! `Piano_pedal_engraver`, for each of the sostenuto, sustain and una
//! corda pedals:
//! - Its events heard (`:211–230`), the timestep's first start and stop by
//!   their `span-direction`.
//! - `process_music` (`:232–265`): with an event, by the pedal's `…Style`,
//!   `text` (or `mixed`) makes text grobs and `bracket` (or `mixed`)
//!   brackets.
//! - Text (`:267–335`): with fewer than three `…Strings`, LilyPond's
//!   warning; else a start the first string, a stop the third, both the
//!   second (a stop without a start LilyPond's warning), the mixed style a
//!   start's only; a string makes the pedal's `…Pedal` caused by the start,
//!   else the stop.
//! - Brackets (`:337–420`): a stop without a bracket LilyPond's warning; a
//!   stop ends the bracket at `currentMusicalColumn`, its right flare 0
//!   without a start, its end announced; a start makes a
//!   `PianoPedalBracket`, its left flare 0 unless one ended here, its
//!   `pedal-text` the mixed style's text.
//! - `stop_translation_timestep` (`:445–464`) and `typeset_all`
//!   (`:466–488`): an ended bracket's right bound and a running one's left
//!   bound `currentMusicalColumn` where they have none; the events and the
//!   text forgotten. `finalize` (`:422–443`) ends a running bracket at
//!   `currentCommandColumn`.
//!
//! `Piano_pedal_align_engraver`: each pedal's grobs are elements of its
//! line spanner (`…PedalLineSpanner`, caused by the first), bounded by its
//! texts and its brackets' bounds, its support the note columns of each
//! timestep it runs (`:110–153`); its end announced once neither a text
//! nor a running bracket carries it. LilyPond's runs at every timestep:
//! here it stays awake while a line spanner runs, and the timestep's
//! support is forgotten at its stop, not the next start, which is the same.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::dynamic_engraver::musical_column;
use crate::grob;
use crate::ledger_line_engraver::set_bound;
use crate::side_position;
use crate::spanner;
use crate::tie_engraver::{event_cause, in_event_class};

type Run<T> = Result<T, Exception>;

/// `NUM_PEDAL_TYPES`, in `Pedal_type`'s order: sostenuto, sustain, una
/// corda.
const PEDALS: usize = 3;
const SUSTAIN: usize = 1;

/// `Pedal_type_info`: `base_name_`, the style and strings properties and
/// `pedal_str_`.
struct PedalType {
    base_name: &'static str,
    style: &'static str,
    strings: &'static str,
    pedal: &'static str,
    line_spanner: &'static str,
    event_class: &'static str,
}

static PEDAL_TYPES: [PedalType; PEDALS] = [
    PedalType {
        base_name: "Sostenuto",
        style: "pedalSostenutoStyle",
        strings: "pedalSostenutoStrings",
        pedal: "SostenutoPedal",
        line_spanner: "SostenutoPedalLineSpanner",
        event_class: "sostenuto-event",
    },
    PedalType {
        base_name: "Sustain",
        style: "pedalSustainStyle",
        strings: "pedalSustainStrings",
        pedal: "SustainPedal",
        line_spanner: "SustainPedalLineSpanner",
        event_class: "sustain-event",
    },
    PedalType {
        base_name: "UnaCorda",
        style: "pedalUnaCordaStyle",
        strings: "pedalUnaCordaStrings",
        pedal: "UnaCordaPedal",
        line_spanner: "UnaCordaPedalLineSpanner",
        event_class: "una-corda-event",
    },
];

pub static PIANO_PEDAL_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Piano_pedal_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["sostenuto-event", "sustain-event", "una-corda-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(PianoPedalEngraver::default())),
    reads: &[
        "currentCommandColumn",
        "pedalSostenutoStrings",
        "pedalSostenutoStyle",
        "pedalSustainStrings",
        "pedalSustainStyle",
        "pedalUnaCordaStrings",
        "pedalUnaCordaStyle",
    ],
    writes: &[],
};

/// `Pedal_info`: `current_bracket_ev_`, `start_ev_`, `event_drul_` (the
/// start first), `item_`, and `bracket_` and `finished_bracket_` by their
/// handles and record numbers.
#[derive(Clone, Copy, Default)]
struct PedalInfo {
    current_bracket_ev: Option<Value>,
    start_ev: Option<Value>,
    event_drul: [Option<Value>; 2],
    item: Option<Value>,
    bracket: Option<(Value, u32)>,
    finished_bracket: Option<(Value, u32)>,
}

const START: usize = 0;
const STOP_: usize = 1;

#[derive(Clone, Default)]
struct PianoPedalEngraver {
    info: [PedalInfo; PEDALS],
}

/// `scm_ilength`: a proper list's length, else -1.
fn ilength(heap: &lily_engine_scheme::Heap, mut v: Value) -> i64 {
    let mut n = 0;
    while let Some(next) = heap.cdr(v) {
        n += 1;
        v = next;
    }
    if v == Value::NULL { n } else { -1 }
}

/// The current musical column's grob, if it is one.
fn command_column(tx: &mut Tx<'_>) -> Option<Value> {
    let col = tx.get("currentCommandColumn");
    grob_of(tx.heap(), col).map(|_| col)
}

impl PianoPedalEngraver {
    fn create_text_grobs(&mut self, tx: &mut Tx<'_>, t: usize, mixed: bool) -> Run<()> {
        let ty = &PEDAL_TYPES[t];
        let p = &mut self.info[t];
        let mut s = Value::NULL;
        let strings = tx.get(ty.strings);

        let length = ilength(tx.heap(), strings);
        if length < 3 {
            let msg = format!("expect 3 strings for piano pedals, found: {length}");
            match p.event_drul[START].or(p.event_drul[STOP_]) {
                Some(m) => tx.event_warning(m, &msg)?,
                None => tx.warning(&msg)?,
            }
            return Ok(());
        }

        let warning = format!("cannot find start of piano pedal: `{}'", ty.base_name);
        let nth = |heap: &mut lily_engine_scheme::Heap, n: usize| {
            let mut v = strings;
            for _ in 0..n {
                v = heap.cdr(v).unwrap_or(Value::NULL);
            }
            heap.car(v).unwrap_or(Value::NULL)
        };
        match (p.event_drul[START], p.event_drul[STOP_]) {
            (Some(start), Some(stop)) => {
                if !mixed {
                    if p.start_ev.is_none() {
                        tx.event_warning(stop, &warning)?;
                    } else {
                        s = nth(tx.heap(), 1);
                    }
                    p.start_ev = Some(start);
                }
            }
            (None, Some(stop)) => {
                if !mixed {
                    if p.start_ev.is_none() {
                        tx.event_warning(stop, &warning)?;
                    } else {
                        s = nth(tx.heap(), 2);
                    }
                    p.start_ev = None;
                }
            }
            (Some(start), None) => {
                p.start_ev = Some(start);
                s = nth(tx.heap(), 0);
            }
            (None, None) => {}
        }

        if tx.heap().str(s).is_some() {
            let cause = p.event_drul[START].or(p.event_drul[STOP_]);
            let cause = cause.map_or(Cause::None, Cause::Event);
            let id = tx.make_defined_grob(ty.pedal, cause)?;
            let item = tx.grob_handle(id);
            p.item = Some(item);
            grob::set_property(tx.heap(), item, "text", s);
        }

        if !mixed {
            p.event_drul = [None, None];
        }
        Ok(())
    }

    fn create_bracket_grobs(&mut self, tx: &mut Tx<'_>, t: usize, mixed: bool) -> Run<()> {
        let ty = &PEDAL_TYPES[t];
        let p = &mut self.info[t];
        if p.bracket.is_none()
            && let Some(stop) = p.event_drul[STOP_]
        {
            let msg = format!(
                "cannot find start of piano pedal bracket: `{}'",
                ty.base_name
            );
            tx.event_warning(stop, &msg)?;
            p.event_drul[STOP_] = None;
        }

        if p.event_drul[STOP_].is_some()
            && let Some((bracket, id)) = p.bracket.take()
        {
            let cmc = musical_column(tx);
            set_bound(tx, bracket, 1, cmc)?;

            // Set properties so that the stencil-creating function will
            // know whether the right edge should be flared ___/
            if p.event_drul[START].is_none() {
                let flare = grob::property(tx.host, bracket, "bracket-flare")?;
                let heap = tx.heap();
                if let Some(car) = heap.car(flare) {
                    let zero = heap.integer(0);
                    let flare = heap.cons(car, zero);
                    grob::set_property(heap, bracket, "bracket-flare", flare);
                }
            }

            p.finished_bracket = Some((bracket, id));
            tx.announce_end_grob(id);
            p.current_bracket_ev = None;
        }

        if let Some(start) = p.event_drul[START] {
            p.start_ev = Some(start);
            p.current_bracket_ev = Some(start);

            let id = tx.make_defined_grob("PianoPedalBracket", Cause::Event(start))?;
            let bracket = tx.grob_handle(id);
            p.bracket = Some((bracket, id));

            // Set properties so that the stencil-creating function will
            // know whether the left edge should be flared \___
            if p.finished_bracket.is_none() {
                let flare = grob::property(tx.host, bracket, "bracket-flare")?;
                let heap = tx.heap();
                // LilyPond's takes the cdr of what it is given.
                if let Some(cdr) = heap.cdr(flare) {
                    let zero = heap.integer(0);
                    let flare = heap.cons(zero, cdr);
                    grob::set_property(heap, bracket, "bracket-flare", flare);
                }
            }

            // Mixed style: Store a pointer to the preceding text for use in
            // calculating the length of the line
            if mixed && let Some(item) = p.item {
                let sym = tx.heap().intern("pedal-text");
                grob::set_object(tx.heap(), bracket, sym, item);
            }
        }

        p.event_drul = [None, None];
        Ok(())
    }

    /// `typeset_all`.
    fn typeset_all(&mut self, tx: &mut Tx<'_>, t: usize) -> Run<()> {
        let p = &mut self.info[t];
        // Handle suicide.
        if let Some((finished, _)) = p.finished_bracket
            && !grob::is_live_grob(tx.heap(), finished)
        {
            p.finished_bracket = None;
        }

        p.item = None;

        if let Some((finished, _)) = p.finished_bracket.take()
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
        {
            let cmc = musical_column(tx);
            set_bound(tx, finished, 1, cmc)?;
        }
        Ok(())
    }
}

impl Work for PianoPedalEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let d = event_property(tx.heap(), ev, "span-direction");
        let side = match d.as_fixnum() {
            Some(-1) => START,
            Some(1) => STOP_,
            // `event_drul_[CENTER]`, which LilyPond's asserts against.
            _ => return Ok(()),
        };
        assign_event_once(tx, &mut self.info[which].event_drul[side], ev).map(drop)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for (t, ty) in PEDAL_TYPES.iter().enumerate() {
            let p = &self.info[t];
            if p.event_drul[STOP_].is_none() && p.event_drul[START].is_none() {
                continue;
            }
            // Choose the appropriate grobs to add to the line spanner.
            // These can be text items or text-spanners.
            //
            //   bracket: |_________/\____|
            //   text:    Ped.     *Ped.  *
            //   mixed:   Ped. _____/\____|
            let style = tx.get(ty.style);
            let heap = tx.heap();
            let mixed = heap.name(style) == Some("mixed");
            let bracket = mixed || heap.name(style) == Some("bracket");
            let text = mixed || heap.name(style) == Some("text");

            if text && self.info[t].item.is_none() {
                self.create_text_grobs(tx, t, mixed)?;
            }
            if bracket {
                self.create_bracket_grobs(tx, t, mixed)?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for t in 0..PEDALS {
            self.typeset_all(tx, t)?;
            if let Some((bracket, _)) = self.info[t].bracket
                && spanner::get_bound(tx.host, bracket, -1)?.is_none()
            {
                let cmc = musical_column(tx);
                set_bound(tx, bracket, -1, cmc)?;
            }
        }

        for p in &mut self.info {
            p.event_drul = [None, None];
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for t in 0..PEDALS {
            if let Some((bracket, _)) = self.info[t].bracket
                && !grob::is_live_grob(tx.heap(), bracket)
            {
                self.info[t].bracket = None;
            }

            if let Some(bracket) = self.info[t].bracket.take() {
                let cc = command_column(tx);
                set_bound(tx, bracket.0, 1, cc)?;

                self.info[t].finished_bracket = Some(bracket);
                self.typeset_all(tx, t)?;
            }
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        for p in &self.info {
            h.maybe_record(p.bracket.map(|b| b.1));
            h.maybe_record(p.finished_bracket.map(|b| b.1));
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        for p in &self.info {
            out.extend(p.current_bracket_ev);
            out.extend(p.start_ev);
            out.extend(p.event_drul.iter().flatten());
            out.extend(p.item);
            out.extend(p.bracket.map(|b| b.0));
            out.extend(p.finished_bracket.map(|b| b.0));
        }
    }
}

// --- Piano_pedal_align_engraver ------------------------------------------------

pub static PIANO_PEDAL_ALIGN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Piano_pedal_align_engraver",
    kind: Kind::Engraver,
    methods: STOP | FINALIZE,
    always: false,
    hears: &[],
    acknowledges: &[
        ("note-column-interface", false),
        ("piano-pedal-bracket-interface", false),
        ("piano-pedal-script-interface", false),
        ("piano-pedal-bracket-interface", true),
    ],
    must_be_last: false,
    make: Some(|| Box::new(PianoPedalAlignEngraver::default())),
    reads: &["currentCommandColumn"],
    writes: &[],
};

/// The acknowledgers' indices in [`PIANO_PEDAL_ALIGN_ENGRAVER`].
const NOTE_COLUMN: usize = 0;
const BRACKET: usize = 1;
const SCRIPT: usize = 2;
const END_BRACKET: usize = 3;

/// `Pedal_align_info`: `line_spanner_` by its handle and record number,
/// `carrying_item_`, `carrying_spanner_` and
/// `finished_carrying_spanner_` by their handles.
#[derive(Clone, Copy, Default)]
struct PedalAlignInfo {
    line_spanner: Option<(Value, u32)>,
    carrying_item: Option<Value>,
    carrying_spanner: Option<Value>,
    finished_carrying_spanner: Option<Value>,
}

impl PedalAlignInfo {
    fn is_finished(&self) -> bool {
        let mut do_continue = self.carrying_item.is_some();
        do_continue |= self.carrying_spanner.is_some() && self.finished_carrying_spanner.is_none();
        do_continue |= self.carrying_spanner.is_some()
            && self.finished_carrying_spanner != self.carrying_spanner;
        !do_continue
    }
}

#[derive(Clone, Default)]
struct PianoPedalAlignEngraver {
    pedal_info: [PedalAlignInfo; PEDALS],
    supports: Vec<Value>,
}

impl PianoPedalAlignEngraver {
    /// `get_grob_pedal_type` of the grob's event cause.
    fn pedal_type(tx: &mut Tx<'_>, g: Value) -> Run<usize> {
        if let Some(cause) = event_cause(tx, g)? {
            for (t, ty) in PEDAL_TYPES.iter().enumerate() {
                if in_event_class(tx.heap(), cause, ty.event_class) {
                    return Ok(t);
                }
            }
        }
        tx.programming_error("Unknown piano pedal type.  Defaulting to sustain")?;
        Ok(SUSTAIN)
    }

    /// `make_line_spanner`: the pedal's line spanner, made caused by the
    /// grob `cause` where there is none.
    fn make_line_spanner(&mut self, tx: &mut Tx<'_>, t: usize, cause: u32) -> Run<Value> {
        if let Some((sp, _)) = self.pedal_info[t].line_spanner {
            return Ok(sp);
        }
        let id = tx.make_defined_grob(PEDAL_TYPES[t].line_spanner, Cause::Grob(cause))?;
        let sp = tx.grob_handle(id);
        self.pedal_info[t].line_spanner = Some((sp, id));
        Ok(sp)
    }
}

impl Work for PianoPedalAlignEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            NOTE_COLUMN => self.supports.push(g),
            BRACKET | SCRIPT => {
                let t = Self::pedal_type(tx, g)?;
                let sp = self.make_line_spanner(tx, t, grob)?;
                axis_group::add_element(tx.host, sp, g)?;
                if which == BRACKET {
                    self.pedal_info[t].carrying_spanner = Some(g);
                } else {
                    self.pedal_info[t].carrying_item = Some(g);
                }
            }
            END_BRACKET => {
                let t = Self::pedal_type(tx, g)?;
                self.pedal_info[t].finished_carrying_spanner = Some(g);
            }
            _ => {}
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for t in 0..PEDALS {
            let pi = self.pedal_info[t];
            if let Some((line, id)) = pi.line_spanner {
                if let Some(item) = pi.carrying_item {
                    if spanner::get_bound(tx.host, line, -1)?.is_none() {
                        spanner::set_bound(tx.host, line, -1, item)?;
                    }
                    spanner::set_bound(tx.host, line, 1, item)?;
                } else if pi.carrying_spanner.is_some() || pi.finished_carrying_spanner.is_some() {
                    if spanner::get_bound(tx.host, line, -1)?.is_none() {
                        // LilyPond's reads the carrying spanner's bound
                        // through a null one.
                        if let Some(carrying) = pi.carrying_spanner
                            && let Some(bound) = spanner::get_bound(tx.host, carrying, -1)?
                        {
                            spanner::set_bound(tx.host, line, -1, bound)?;
                        }
                    }

                    if let Some(finished) = pi.finished_carrying_spanner {
                        let bound = spanner::get_bound(tx.host, finished, 1)?;
                        set_bound(tx, line, 1, bound)?;
                    }
                }

                for &support in &self.supports {
                    side_position::add_support(tx.host, line, support)?;
                }

                if pi.is_finished() {
                    tx.announce_end_grob(id);
                    self.pedal_info[t] = PedalAlignInfo::default();
                }
            }

            self.pedal_info[t].carrying_item = None;
        }

        // `start_translation_timestep`.
        self.supports.clear();
        if self.pedal_info.iter().any(|pi| pi.line_spanner.is_some()) {
            tx.stay_awake();
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for t in 0..PEDALS {
            if let Some((line, _)) = self.pedal_info[t].line_spanner {
                let c = command_column(tx);
                set_bound(tx, line, 1, c)?;
                self.pedal_info[t] = PedalAlignInfo::default();
            }
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        for pi in &self.pedal_info {
            h.maybe_record(pi.line_spanner.map(|l| l.1));
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        for pi in &self.pedal_info {
            out.extend(pi.line_spanner.map(|l| l.0));
            out.extend(pi.carrying_item);
            out.extend(pi.carrying_spanner);
            out.extend(pi.finished_carrying_spanner);
        }
        out.extend(&self.supports);
    }
}
