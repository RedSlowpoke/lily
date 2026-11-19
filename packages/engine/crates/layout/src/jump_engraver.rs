//! `jump-engraver.cc` (MS8 step 7, 281 lines): `Jump_engraver`, a class of
//! layout's ([`JUMP_ENGRAVER`]) that shadows the music crate's placeholder
//! of its name (D67, D108): a `JumpScript` for each `ad-hoc-jump-event`,
//! `dal-segno-event` and `fine-event` heard, placed outside the staves of
//! `stavesFound`.
//!
//! - `process_music` (`:85–186`): an ad-hoc jump's text the event's; a
//!   D.S.'s (or D.C.'s) `dalSegnoTextFormatter`'s, given the last segno's
//!   formatted label, the first coda mark of the alternatives' and the
//!   performance mark that follows, or `fineText` once a fine was printed;
//!   a fine's `fineText`. LilyPond's warning when a text is not a markup.
//! - `stop_translation_timestep` (`:188–213`): each text's
//!   `side-support-elements` the grob array of `stavesFound`.
//! - `finalize` (`:215–234`): a fine of the last timestep killed unless
//!   `finalFineTextVisibility`. The engraver stays awake after a fine's
//!   timestep, so that the next one's `start_translation_timestep` forgets
//!   it as LilyPond's, which runs at every timestep.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PROCESS, START, STOP, StreamEvent, TranslatorClass, Tx, Work,
    assign_event_once, current_performance_mark, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array::grob_list_to_grob_array;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static JUMP_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Jump_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["ad-hoc-jump-event", "dal-segno-event", "fine-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(JumpEngraver::default())),
    reads: &[
        "codaMarkCount",
        "codaMarkFormatter",
        "dalSegnoTextFormatter",
        "finalFineTextVisibility",
        "fineText",
        "segnoMarkCount",
        "segnoMarkFormatter",
        "stavesFound",
    ],
    writes: &[],
};

const WARNING: &str = "jump text must be a markup object";

/// The engraver's members, the grobs and events by their handles.
#[derive(Clone, Default)]
struct JumpEngraver {
    printed_fine: bool,
    final_fine_text_visibility: bool,
    ad_hoc_jump_text: Option<Value>,
    ds_text: Option<Value>,
    fine_text: Option<Value>,
    ad_hoc_jump_ev: Option<Value>,
    ds_ev: Option<Value>,
    fine_ev: Option<Value>,
}

/// `from_scm<size_t> (x, 0)`: a non-negative exact integer, or 0.
fn count(tx: &mut Tx<'_>, x: Value) -> i64 {
    tx.heap().to_i64(x).filter(|&n| n >= 0).unwrap_or(0)
}

/// `to_scm` of a `size_t` that is `n`, a negative `n` wrapped round 2^64.
fn size_t(tx: &mut Tx<'_>, n: i64) -> Value {
    let heap = tx.heap();
    let value = heap.integer(n);
    if n >= 0 {
        return value;
    }
    let half = heap.integer(1 << 62);
    let four = heap.integer(4);
    heap.mul(half, four)
        .and_then(|wrap| heap.add(wrap, value))
        .unwrap_or(value)
}

/// `make_item ("JumpScript", ev)`, its handle.
fn make_script(tx: &mut Tx<'_>, ev: Value) -> Run<Value> {
    let id = tx.make_defined_grob("JumpScript", Cause::Event(ev))?;
    Ok(tx.grob_handle(id))
}

/// The text `m` set on `script` if it is a markup, else the event's warning.
fn set_text(tx: &mut Tx<'_>, script: Value, ev: Value, m: Value) -> Run<()> {
    if is_markup(tx.host, m)? {
        grob::set_property(tx.heap(), script, "text", m);
        Ok(())
    } else {
        tx.event_warning(ev, WARNING)
    }
}

impl JumpEngraver {
    /// The D.S.'s text (`:101–166`), what it rooted unrooted again.
    fn dal_segno_text(&self, tx: &mut Tx<'_>, ev: Value) -> Run<Value> {
        let depth = tx.heap().root_depth();
        let m = self.formatted(tx, ev);
        tx.heap().unroot_to(depth);
        m
    }

    fn formatted(&self, tx: &mut Tx<'_>, ev: Value) -> Run<Value> {
        let context = tx.t.arena.borrow().handle(tx.context);
        // We indicate D.S. to the most recent segno mark (D.C. with none).
        let mut body_start = Value::FALSE;
        let segno_count = tx.get("segnoMarkCount");
        let segno_count = count(tx, segno_count);
        if segno_count > 0 {
            let proc = tx.get("segnoMarkFormatter");
            if tx.heap().is_procedure(proc) {
                let n = tx.heap().integer(segno_count);
                body_start = tx.apply(proc, &[n, context])?;
            }
        }
        tx.heap().root(body_start);
        let mut body_end = Value::FALSE;
        let mut next = Value::FALSE;
        let alt = event_property(tx.heap(), ev, "alternative-number");
        let alt_num = count(tx, alt);
        if alt_num > 0 {
            // The first coda mark of the alternatives', the coda marks of
            // the current group being sequential (`size_t`'s wrap kept).
            let coda = tx.get("codaMarkCount");
            let coda = count(tx, coda) - (alt_num - 1);
            let proc = tx.get("codaMarkFormatter");
            if tx.heap().is_procedure(proc) {
                let n = size_t(tx, coda);
                body_end = tx.apply(proc, &[n, context])?;
                tx.heap().root(body_end);
            }
            // `Mark_engraver::get_current_performance_mark_text`, `'()`
            // as `#f`.
            let ev = tx.get("currentPerformanceMarkEvent");
            if tx.heap().foreign::<StreamEvent>(ev).is_some() {
                let (_, text) = current_performance_mark(tx, ev)?;
                if text != Value::NULL {
                    next = text;
                    tx.heap().root(next);
                }
            }
        }
        if next == Value::FALSE && self.printed_fine {
            // "al Fine" if there was a "Fine" at any prior point.
            body_end = tx.get("fineText");
            tx.heap().root(body_end);
        }
        let proc = tx.get("dalSegnoTextFormatter");
        if !tx.heap().is_procedure(proc) {
            return Ok(Value::NULL);
        }
        let returns = event_property(tx.heap(), ev, "return-count");
        let returns = tx.heap().to_i64(returns).unwrap_or(1);
        let returns = tx.heap().integer(returns);
        let markups = tx.heap().list(&[body_start, body_end, next]);
        tx.heap().root(markups);
        tx.apply(proc, &[context, returns, markups])
    }
}

impl Work for JumpEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let slot = match which {
            0 => &mut self.ad_hoc_jump_ev,
            1 => &mut self.ds_ev,
            _ => &mut self.fine_ev,
        };
        assign_event_once(tx, slot, ev)?;
        Ok(())
    }

    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.fine_text = None;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.ad_hoc_jump_ev {
            let script = make_script(tx, ev)?;
            self.ad_hoc_jump_text = Some(script);
            let m = event_property(tx.heap(), ev, "text");
            set_text(tx, script, ev, m)?;
        }
        if let Some(ev) = self.ds_ev {
            let script = make_script(tx, ev)?;
            self.ds_text = Some(script);
            let m = self.dal_segno_text(tx, ev)?;
            let depth = tx.heap().root_depth();
            tx.heap().root(m);
            let set = set_text(tx, script, ev, m);
            tx.heap().unroot_to(depth);
            set?;
        }
        if let Some(ev) = self.fine_ev {
            let script = make_script(tx, ev)?;
            self.fine_text = Some(script);
            let m = tx.get("fineText");
            set_text(tx, script, ev, m)?;
            // Honoured at `finalize` if this is the last timestep.
            self.final_fine_text_visibility = tx.get("finalFineTextVisibility") == Value::TRUE;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for text in [self.ds_text, self.fine_text, self.ad_hoc_jump_text]
            .into_iter()
            .flatten()
        {
            let staves = tx.get("stavesFound");
            let (array, warnings) = grob_list_to_grob_array(tx.heap(), staves);
            let sym = tx.heap().intern("side-support-elements");
            grob::set_object(tx.heap(), text, sym, array);
            for w in warnings {
                tx.warning(&w)?;
            }
        }
        if self.fine_ev.is_some() {
            self.printed_fine = true;
        }
        // Awake for the next timestep's start, which forgets the fine, so
        // that only a fine of the last timestep meets `finalize`.
        if self.fine_text.is_some() {
            tx.stay_awake();
        }
        self.ds_ev = None;
        self.ds_text = None;
        self.fine_ev = None;
        self.ad_hoc_jump_ev = None;
        self.ad_hoc_jump_text = None;
        Ok(())
    }

    /// By default, no "Fine" at the written end of the music.
    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(fine) = self.fine_text.take()
            && !self.final_fine_text_visibility
        {
            grob::suicide(tx.heap(), fine);
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        h.flags(&[self.printed_fine, self.final_fine_text_visibility]);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.ad_hoc_jump_text);
        out.extend(self.ds_text);
        out.extend(self.fine_text);
        out.extend(self.ad_hoc_jump_ev);
        out.extend(self.ds_ev);
        out.extend(self.fine_ev);
    }
}
