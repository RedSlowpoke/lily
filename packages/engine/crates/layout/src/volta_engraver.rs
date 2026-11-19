//! `volta-engraver.cc` (MS3 step 20): `Volta_engraver`, a class of
//! layout's ([`VOLTA_ENGRAVER`]) that the core passes at its `\layout` call
//! and that a group finds before the music crate's placeholder of the name
//! (D67, D108).
//!
//! - A layer of volta spans for each depth of nested folded repeats
//!   (`Volta_layer`), the first made at once for `repeatCommands`. A
//!   `volta-span-event` heard (`:138–229`) is its `volta-depth`'s layer's
//!   start or stop by its `span-direction`: a timestep's first start, a
//!   start with other `volta-numbers` LilyPond's warning unless an empty
//!   alternative's stop was heard; a stop with the start's numbers that
//!   empty alternative's, else the timestep's first stop of the running
//!   bracket's numbers. A `dal-segno-event` or `fine-event` closes the
//!   bracket's end (`should_close_end_`).
//! - `process_music` (`:231–391`): each layer's events of a repeat count
//!   below 2 dropped unless `printTrivialVoltaRepeats`; the first layer's
//!   `(volta label)` and `(volta #f)` of `repeatCommands`. The running
//!   bracket ends at a stop, at `VoltaBracket.musical-length` or else at
//!   `voltaBracketMusicalLength`, and at a start but where a start with
//!   the running bracket's numbers comes at its moment's main part (issue
//!   #34), with LilyPond's warnings. A start makes a `VoltaBracket`,
//!   caused by its event, and the layer's `VoltaBracketSpanner` if it has
//!   none (`:367–386`), whose `outside-staff-priority` is less by the
//!   depth; the bracket is the spanner's element.
//! - Each bar line acknowledged (`:393–418`) is the running bracket's bar
//!   at its left and the ended one's at its right
//!   (`Volta_bracket_interface::add_bar`), and the spanner's support; its
//!   `glyph-left` closes the end where the image's
//!   `volta-bracket::calc-hook-visibility` (`bar-line.scm`) hides the hook.
//! - `stop_translation_timestep` (`:427–533`): a started bracket's end
//!   moment from its `musical-length`, a moment to process; it is killed
//!   where that is not after its start, or where its alternative is empty.
//!   Else it takes the label and the event's `volta-numbers`. An ended
//!   bracket takes `currentCommandColumn` where no bar line was
//!   acknowledged, gives the spanner its right bound, and the spanner, if
//!   no bracket runs, takes `stavesFound`'s grobs as support and ends; the
//!   bracket's right `edge-height` is 0 unless the end is closed. Running
//!   brackets and spanners take their left bounds.
//!
//! Without a command column, where LilyPond's would crash, no bound is set
//! (`ledger_line_engraver::set_bound`), nor where the ended bracket has no
//! right bound to give the spanner. `stavesFound` is
//! `Staff_collecting_engraver`'s, which no step of part 2 ports (as
//! `bar_number_engraver.rs` says): until it is, the ended spanner takes no
//! staves where LilyPond's takes the staff symbols. A non-grob of
//! `stavesFound`, which LilyPond's would add as a null support, is passed
//! over. The programming error for an event's invalid direction is given
//! unlocated.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, Rational, START, STOP, TranslatorClass, Tx, Work, event_property,
    grob_of,
};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::axis_group;
use crate::clef_engraver::is_true;
use crate::dynamic_align_engraver::direction;
use crate::grob;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::side_position;
use crate::spanner;
use crate::tie_engraver::event_cause;
use crate::volta_bracket;

type Run<T> = Result<T, Exception>;

pub static VOLTA_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Volta_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP,
    // `process_music` reads `repeatCommands` and ends brackets by their
    // length at timesteps no event wakes.
    always: true,
    hears: &["dal-segno-event", "fine-event", "volta-span-event"],
    acknowledges: &[("bar-line-interface", false)],
    must_be_last: false,
    make: Some(|| {
        Box::new(VoltaEngraver {
            // At least one layer, for the manual repeat commands.
            layers: vec![Layer::default()],
            acknowledged_bar_line: false,
            should_close_end: false,
        })
    }),
    reads: &[
        "currentCommandColumn",
        "printTrivialVoltaRepeats",
        "repeatCommands",
        "stavesFound",
    ],
    writes: &[],
};

/// The listeners' indices in [`VOLTA_ENGRAVER`].
const VOLTA_SPAN: usize = 2;

/// `Volta_layer`: its events, moments, brackets and spanner by their
/// handles (the brackets with their record numbers), and its label.
#[derive(Clone)]
struct Layer {
    start_ev: Option<Value>,
    stop_prev_ev: Option<Value>,
    /// To handle an empty bracket.
    stop_curr_ev: Option<Value>,
    start_mom: Moment,
    stop_mom: Moment,
    bracket: Option<(Value, u32)>,
    end_bracket: Option<(Value, u32)>,
    spanner: Option<Value>,
    text: Value,
    start_bracket_this_timestep: bool,
}

impl Default for Layer {
    fn default() -> Layer {
        Layer {
            start_ev: None,
            stop_prev_ev: None,
            stop_curr_ev: None,
            start_mom: Moment::default(),
            stop_mom: Moment::default(),
            bracket: None,
            end_bracket: None,
            spanner: None,
            text: Value::NULL,
            start_bracket_this_timestep: false,
        }
    }
}

/// `layers_`, `acknowledged_bar_line_` and `should_close_end_`.
#[derive(Clone)]
struct VoltaEngraver {
    layers: Vec<Layer>,
    acknowledged_bar_line: bool,
    should_close_end: bool,
}

/// `Moment::infinity ()`.
fn infinity() -> Moment {
    Moment::new(Rational::INFINITY, Rational::ZERO)
}

/// `from_scm (v, Moment::infinity ())`.
fn moment_or_infinity(heap: &Heap, v: Value) -> Moment {
    heap.foreign::<Moment>(v).copied().unwrap_or_else(infinity)
}

/// `from_scm<size_t> (v, 0)`: an exact integer that is no less than 0.
fn depth(heap: &Heap, v: Value) -> usize {
    heap.to_i64(v)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
}

/// `are_volta_numbers_equal`: whether the events' `volta-numbers` are
/// `equal?`.
fn are_volta_numbers_equal(heap: &mut Heap, a: Value, b: Value) -> bool {
    let (a, b) = (
        event_property(heap, a, "volta-numbers"),
        event_property(heap, b, "volta-numbers"),
    );
    heap.equal(a, b)
}

/// `scm_difference (pri, n)`, Guile's type error for what is no number.
fn difference(heap: &mut Heap, pri: Value, n: usize) -> Run<Value> {
    let n = heap.integer(n as i64);
    heap.sub(pri, n).map_err(|_| {
        let key = heap.intern("wrong-type-arg");
        let subr = heap.string("-");
        let message = heap.string("Wrong type argument in position ~A: ~S");
        let one = heap.integer(1);
        let items = heap.list(&[one, pri]);
        let rest = heap.list(&[pri]);
        let args = heap.list(&[subr, message, items, rest]);
        Exception { key, args }
    })
}

/// `from_scm (v, Drul_array (2.0, 2.0))`: a pair of reals, else the
/// default.
fn edge_height(heap: &Heap, v: Value) -> (f64, f64) {
    let real = |x: Value| heap.is_real(x).then(|| heap.to_f64(x)).flatten();
    match (heap.car(v).and_then(real), heap.cdr(v).and_then(real)) {
        (Some(l), Some(r)) => (l, r),
        _ => (2.0, 2.0),
    }
}

impl VoltaEngraver {
    fn listen_volta_span(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let v = event_property(tx.heap(), ev, "volta-depth");
        let layer_no = depth(tx.heap(), v);
        if layer_no >= self.layers.len() {
            self.layers.resize_with(layer_no + 1, Layer::default);
        }
        let layer = &mut self.layers[layer_no];

        // It is common to have the same repeat structure in multiple voices, so we
        // ignore simultaneous events; but it is nice to perform some consistency
        // checks to catch likely errors and improve the user's debugging experience.

        let dir = direction(event_property(tx.heap(), ev, "span-direction"));
        if dir == -1 {
            match layer.start_ev {
                None => layer.start_ev = Some(ev),
                Some(start) if !are_volta_numbers_equal(tx.heap(), start, ev) => {
                    if layer.stop_curr_ev.is_some() {
                        // We previously observed a zero-duration volta span, but now we
                        // are starting a new span.  Discard the old one.  The reason we
                        // didn't discard it on receipt of the stop event is so that a
                        // final zero-duration span can be forced to appear by overriding
                        // VoltaBracket.musical-length.
                        layer.start_ev = Some(ev);
                        layer.stop_curr_ev = None;
                    } else {
                        // Include the volta numbers in the message because they might not
                        // be obvious if the source has the legacy \alternative syntax
                        // with implied \volta.
                        let mut nums = String::new();
                        let v = event_property(tx.heap(), ev, "volta-numbers");
                        tx.heap().write(v, &mut nums);
                        let text = format!("discarding conflicting volta numbers: {nums}");
                        tx.event_warning(ev, &text)?;
                    }
                }
                Some(_) => {}
            }
        } else if dir == 1 {
            if let Some(start) = layer.start_ev
                && are_volta_numbers_equal(tx.heap(), start, ev)
            {
                // A simultaneous start and stop with the same volta spec can be
                // generated by a zero-duration alternative.
                layer.stop_curr_ev = Some(ev);
            } else if layer.stop_prev_ev.is_none() {
                // If there is a current bracket which was created by an event, ignore
                // unrelated stop events, which are recognized by a difference in
                // their list of volta numbers.
                //
                // Unrelated stop events might arise from differences in grace notes
                // among elements of simultaneous music (issue #34).  In this case, at
                // the earliest grace time, we expect the current bracket to end.  Any
                // events for that ended bracket which arrive at a later grace time
                // are quietly ignored rather than allowed to end a different bracket.
                let accept_event = match layer.bracket {
                    // lacking information required to filter
                    None => true,
                    Some((bracket, _)) => match event_cause(tx, bracket)? {
                        // lacking information required to filter
                        None => true,
                        Some(bracket_start_ev) => {
                            are_volta_numbers_equal(tx.heap(), bracket_start_ev, ev)
                        }
                    },
                };

                if accept_event {
                    layer.stop_prev_ev = Some(ev);
                }
            }
        } else {
            tx.programming_error("invalid direction of volta-span-event")?;
        }
        Ok(())
    }

    /// `process_music` of the layer `layer_no`.
    fn process_layer(&mut self, tx: &mut Tx<'_>, layer_no: usize) -> Run<()> {
        let print_trivial = is_true(tx.get("printTrivialVoltaRepeats"));
        let layer = &mut self.layers[layer_no];

        // Ignore events for trivial repeats (when configured)
        for ev in [
            &mut layer.start_ev,
            &mut layer.stop_prev_ev,
            &mut layer.stop_curr_ev,
        ] {
            let Some(e) = *ev else {
                continue;
            };

            let v = event_property(tx.heap(), e, "repeat-count");
            let rep_count = tx.heap().to_i64(v).unwrap_or(1);
            if rep_count < 2 && !print_trivial {
                *ev = None;
            }
        }

        let mut manual_start = false;
        let mut manual_end = false;

        if layer_no == 0 {
            // manual repeat commands
            let mut p = tx.get("repeatCommands");
            let heap = tx.heap();
            let volta = heap.intern("volta");
            while p.tag() == Tag::Pair {
                let c = heap.car(p).unwrap_or(Value::NULL);
                if heap.car(c) == Some(volta)
                    && let Some(rest) = heap.cdr(c)
                    && rest.tag() == Tag::Pair
                {
                    let label = heap.car(rest).unwrap_or(Value::NULL);
                    if label == Value::FALSE {
                        manual_end = true;
                    } else {
                        manual_start = true;
                        layer.text = label;
                    }
                }
                p = heap.cdr(p).unwrap_or(Value::NULL);
            }
        }

        let now = tx.now();
        let mut end = manual_end || layer.stop_prev_ev.is_some();
        if !end && layer.bracket.is_some() {
            if layer.stop_mom.compare(infinity()).is_lt() {
                // VoltaBracket.musical-length was specified.  Check it and
                // disregard voltaBracketMusicalLength.
                end = now.compare(layer.stop_mom).is_ge();
            } else {
                let v = tx.get("voltaBracketMusicalLength");
                let volta_bracket_musical_length = moment_or_infinity(tx.heap(), v);
                let since = now.checked_sub(layer.start_mom).unwrap_or_else(infinity);
                end = volta_bracket_musical_length.compare(since).is_le();
            }
        }

        layer.start_bracket_this_timestep = 'start: {
            if !(manual_start || layer.start_ev.is_some()) {
                break 'start false; // We have no reason to start a bracket.
            }

            // If there is no current bracket, or if the current bracket will stop
            // here, we can start a new one without trouble.
            let Some((bracket, _)) = layer.bracket.filter(|_| !end) else {
                break 'start true;
            };

            // We have reason to start a new bracket, but no clear reason to stop
            // the current one.  Differences in grace notes among elements of
            // simultaneous music cause volta events to be announced in more than
            // one time step (issue #34).  Do not split the current bracket just for
            // this; let it continue.
            if layer.start_mom.main.compare(now.main).is_eq() {
                // Compare volta specs to gain confidence that we are handling grace
                // synchronization rather than incongruous repeat structure.
                //
                // Manually created (via repeatCommands) brackets do not have volta
                // numbers: they have only markup.  We refrain from trying to fix
                // them blindly.
                if let Some(start) = layer.start_ev
                    && let Some(prev_ev) = event_cause(tx, bracket)?
                    && are_volta_numbers_equal(tx.heap(), prev_ev, start)
                {
                    // The volta numbers are the same as for the current
                    // bracket.  We're probably seeing issue #34: don't
                    // start a new bracket.
                    break 'start false;
                }
            }

            true
        };

        if layer.start_bracket_this_timestep
            && let Some((bracket, _)) = layer.bracket
            && !end
        {
            grob::warning(
                tx.host,
                bracket,
                "already have a VoltaBracket; ending it prematurely",
            )?;
            end = true;
        }

        if end {
            if let Some(bracket) = layer.bracket.take() {
                layer.end_bracket = Some(bracket);
            } else if manual_end {
                // FIXME: Be more verbose?
                tx.warning("no VoltaBracket to end")?;
            }
        }

        if layer.start_bracket_this_timestep {
            layer.start_mom = now;
            layer.stop_mom = infinity();
            let cause = layer.start_ev.map_or(Cause::None, Cause::Event);
            let id = tx.make_defined_grob("VoltaBracket", cause)?;
            let bracket = tx.grob_handle(id);
            layer.bracket = Some((bracket, id));

            let spanner = match layer.spanner {
                Some(spanner) => spanner,
                None => {
                    let id = tx.make_defined_grob("VoltaBracketSpanner", cause)?;
                    let spanner = tx.grob_handle(id);
                    layer.spanner = Some(spanner);

                    // Set the vertical order of the layers by adjusting
                    // outside-staff-priority.
                    if layer_no != 0 {
                        let pri = grob::property(tx.host, spanner, "outside-staff-priority")?;
                        let pri = difference(tx.heap(), pri, layer_no)?;
                        grob::set_property(tx.heap(), spanner, "outside-staff-priority", pri);
                    }
                    spanner
                }
            };

            axis_group::add_element(tx.host, spanner, bracket)?;
        }
        Ok(())
    }

    fn acknowledge_bar_line(&mut self, tx: &mut Tx<'_>, item: Value) -> Run<()> {
        self.acknowledged_bar_line = true;

        for layer in &self.layers {
            if let Some((bracket, _)) = layer.bracket {
                volta_bracket::add_bar(tx.host, bracket, item, -1)?;
            }
            if let Some((end_bracket, _)) = layer.end_bracket {
                volta_bracket::add_bar(tx.host, end_bracket, item, 1)?;
            }

            if let Some(spanner) = layer.spanner {
                side_position::add_support(tx.host, spanner, item)?;
            }
        }

        // Certain bar lines cause volta brackets to hook down at the end.
        // See the function allow-volta-hook in bar-line.scm.
        if !self.should_close_end {
            let glyph = grob::property(tx.host, item, "glyph-left")?;
            let hidden = tx.call("volta-bracket::calc-hook-visibility", &[glyph])?;
            self.should_close_end = !is_true(hidden);
        }
        Ok(())
    }

    /// `stop_translation_timestep` of a layer, the timestep's command
    /// column `ci`.
    fn stop_layer(&mut self, tx: &mut Tx<'_>, at: usize, ci: Option<Value>) -> Run<()> {
        let acknowledged_bar_line = self.acknowledged_bar_line;
        let should_close_end = self.should_close_end;
        let layer = &mut self.layers[at];

        if layer.start_bracket_this_timestep
            && let Some((bracket, _)) = layer.bracket
        {
            let v = grob::property(tx.host, bracket, "musical-length")?;
            let length = moment_or_infinity(tx.heap(), v);
            layer.stop_mom = layer.start_mom.checked_add(length).unwrap_or_else(infinity);

            // Cancel the bracket if it will not end during a future timestep.
            if layer.stop_mom.compare(layer.start_mom).is_le() {
                // Should we warn if VoltaBracket.musical-length was negative?
                grob::suicide(tx.heap(), bracket);
                layer.bracket = None;
                layer.start_bracket_this_timestep = false;
            } else if layer.stop_mom.compare(infinity()).is_lt() {
                // VoltaBracket.musical-length is valid; use it.
                tx.add_moment(layer.stop_mom)?;
            } else {
                // VoltaBracket.musical-length is unlimited, so the bracket
                // continues until the end of the alternative or until
                // voltaBracketMusicalLength applies.
                if layer.stop_curr_ev.is_some() {
                    // the current alternative is empty
                    grob::suicide(tx.heap(), bracket);
                    layer.bracket = None;
                    layer.start_bracket_this_timestep = false;
                }
            }
        }

        if layer.start_bracket_this_timestep
            && let Some((bracket, _)) = layer.bracket
        {
            if layer.text != Value::NULL {
                // explicit label from repeatCommands
                grob::set_property(tx.heap(), bracket, "text", layer.text);
            }

            if let Some(start) = layer.start_ev {
                let nums = event_property(tx.heap(), start, "volta-numbers");
                grob::set_property(tx.heap(), bracket, "volta-numbers", nums);
            }

            layer.start_bracket_this_timestep = false;
        }

        if let Some((end_bracket, end_id)) = layer.end_bracket {
            if !acknowledged_bar_line && let Some(ci) = ci {
                spanner::add_bound_item(tx.host, end_bracket, ci)?;
            }

            if let Some(spanner) = layer.spanner {
                let right = spanner::get_bound(tx.host, end_bracket, 1)?;
                set_bound(tx, spanner, 1, right)?;
            }

            if layer.bracket.is_none() {
                if let Some(spanner) = layer.spanner {
                    let mut p = tx.get("stavesFound");
                    while let (Some(g), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
                        if grob_of(tx.heap(), g).is_some() {
                            side_position::add_support(tx.host, spanner, g)?;
                        }
                        p = rest;
                    }
                }

                layer.spanner = None;
            }

            // TODO: Now that we attempt to handle nested repeats, consider
            // whether there is a case in which one layer should have an end hook
            // and the other should not, and how important it is to get it right.
            if !should_close_end {
                let v = grob::property(tx.host, end_bracket, "edge-height")?;
                let (left, right) = edge_height(tx.heap(), v);
                if right != 0.0 {
                    let heap = tx.heap();
                    let (left, right) = (heap.flonum(left), heap.flonum(0.0));
                    let eh = heap.cons(left, right);
                    grob::set_property(heap, end_bracket, "edge-height", eh);
                }
            }

            tx.announce_end_grob(end_id);
            layer.end_bracket = None;
        }

        if let Some((bracket, _)) = layer.bracket
            && spanner::get_bound(tx.host, bracket, -1)?.is_none()
        {
            set_bound(tx, bracket, -1, ci)?;
        }

        if let (Some(spanner), Some((bracket, _))) = (layer.spanner, layer.bracket)
            && spanner::get_bound(tx.host, spanner, -1)?.is_none()
        {
            let left = spanner::get_bound(tx.host, bracket, -1)?;
            set_bound(tx, spanner, -1, left)?;
        }

        layer.start_ev = None;
        layer.stop_prev_ev = None;
        layer.stop_curr_ev = None;
        layer.text = Value::NULL;
        Ok(())
    }
}

impl Work for VoltaEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.layers.len() as i64);
        for l in &self.layers {
            h.moment(l.start_mom);
            h.moment(l.stop_mom);
            let some = [
                l.start_ev.is_some(),
                l.stop_prev_ev.is_some(),
                l.stop_curr_ev.is_some(),
                l.spanner.is_some(),
            ];
            h.flags(&some);
            h.flags(&[l.start_bracket_this_timestep]);
            h.maybe_record(l.bracket.map(|b| b.1));
            h.maybe_record(l.end_bracket.map(|b| b.1));
        }
        h.flags(&[self.acknowledged_bar_line, self.should_close_end]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            VOLTA_SPAN => self.listen_volta_span(tx, ev),
            // `listen_dal_segno`, `listen_fine`.
            _ => {
                self.should_close_end = true;
                Ok(())
            }
        }
    }

    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.acknowledged_bar_line = false;
        self.should_close_end = false;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for layer_no in 0..self.layers.len() {
            self.process_layer(tx, layer_no)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);
        self.acknowledge_bar_line(tx, item)
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let ci = command_column(tx);
        for at in 0..self.layers.len() {
            self.stop_layer(tx, at, ci)?;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for layer in &self.layers {
            out.extend(layer.start_ev);
            out.extend(layer.stop_prev_ev);
            out.extend(layer.stop_curr_ev);
            out.extend(layer.bracket.map(|(b, _)| b));
            out.extend(layer.end_bracket.map(|(b, _)| b));
            out.extend(layer.spanner);
            out.push(layer.text);
        }
    }
}
