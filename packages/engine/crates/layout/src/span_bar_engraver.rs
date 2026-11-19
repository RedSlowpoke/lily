//! `span-bar-engraver.cc` (MS3 step 42): `Span_bar_engraver`, a class of
//! layout's ([`SPAN_BAR_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of
//! the name (D67, D108).
//!
//! - The bar lines acknowledged (`acknowledge_bar_line`, `:62–72`) that
//!   are no span bars are the timestep's; once there are two, a span bar
//!   is due. The span bar stubs acknowledged (`:74–78`) are kept too.
//! - `process_acknowledged` (`:80–96`): the due `SpanBar`, with no cause,
//!   takes the bar lines kept so far as its `elements`; bar lines
//!   acknowledged after it are not added.
//! - `stop_translation_timestep` (`:98–146`): with a span bar, each stub
//!   whose `allow-span-bar` or `allow-span-bar-above` is not `#t` (one
//!   `Bar_engraver` made where it made no bar line) joins the bar lines
//!   and the span bar's `elements`; the bar lines, sorted (stably) by
//!   their axis groups' indices, each get `has-span-bar`: the span bar
//!   below unless it is the bottom one or its `allow-span-bar` or the next
//!   one's `allow-span-bar-above` is not `#t`, and above as the one before
//!   got it below. Then the timestep's bars and stubs are forgotten.
//!
//! The SpanBar's callbacks (`ly:span-bar::print`, `::width`,
//! `::calc-glyph-name`, `::calc-anchor`, `::before-line-breaking`) and the
//! bar lines' widening towards it (`bar-line::widen-bar-extent-on-span`)
//! are the image's `bar-line.scm`, which reads `has-span-bar`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::grob;
use crate::grob_array;

type Run<T> = Result<T, Exception>;

pub static SPAN_BAR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Span_bar_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("bar-line-interface", false),
        ("span-bar-stub-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SpanBarEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The acknowledgers' indices in [`SPAN_BAR_ENGRAVER`].
const BAR_LINE: usize = 0;

/// `spanbar_`, `make_spanbar_`, `bars_` and `stubs_`, by their handles.
#[derive(Clone, Default)]
struct SpanBarEngraver {
    spanbar: Option<Value>,
    make_spanbar: bool,
    bars: Vec<Value>,
    stubs: Vec<Value>,
}

impl Work for SpanBarEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.make_spanbar]);
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let it = tx.grob_handle(grob);
        if which != BAR_LINE {
            self.stubs.push(it);
        } else if !grob::has_interface(tx.heap(), it, "span-bar-interface") {
            self.bars.push(it);

            if self.bars.len() >= 2 && self.spanbar.is_none() {
                self.make_spanbar = true;
            }
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.make_spanbar {
            let id = tx.make_defined_grob("SpanBar", Cause::None)?;
            let spanbar = tx.grob_handle(id);
            self.spanbar = Some(spanbar);
            let elements = tx.heap().intern("elements");
            for &bar in &self.bars {
                grob_array::add_grob(tx.host, spanbar, elements, bar)?;
            }
            // TODO: More bar lines could be acknowledged, but they won't be added to
            // the group.  That might not happen currently, but it could conceivably
            // happen after enhancements to bar-line engraving.
            self.make_spanbar = false;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(spanbar) = self.spanbar.take() {
            // Span_bar_stub_engraver creates stubs in contexts where no bar line was
            // created.  Usually, this is because there is no Bar_engraver operating
            // in these contexts; however, if Bar_engraver is operating in one of
            // those contexts and has been configured not to create a BarLine at this
            // point, it sets SpanBarStub.allow-span-bar and .allow-span-bar-above to
            // #f to signal that.  In that case, we include this stub among the
            // SpanBar's elements so that the SpanBar avoids drawing through the staff
            // (or whatever context it happens to be).
            let elements = tx.heap().intern("elements");
            for &stub in &self.stubs {
                if !is_true(grob::property(tx.host, stub, "allow-span-bar")?)
                    || !is_true(grob::property(tx.host, stub, "allow-span-bar-above")?)
                {
                    self.bars.push(stub);
                    grob_array::add_grob(tx.host, spanbar, elements, stub)?;
                }
            }

            // Because of alignAboveContext and alignBelowContext, grobs are not
            // necessarily announced in the order that they should be laid out, so
            // they need to be sorted.
            let mut keyed = Vec::with_capacity(self.bars.len());
            for &bar in &self.bars {
                keyed.push((grob::get_vertical_axis_group_index(tx.host, bar)?, bar));
            }
            keyed.sort_by_key(|&(index, _)| index);
            self.bars = keyed.into_iter().map(|(_, bar)| bar).collect();

            let has_span_bar = tx.heap().intern("has-span-bar");
            let num_bars = self.bars.len();
            let mut allow_above = false;
            for i in 0..num_bars {
                let bar = self.bars[i];
                let is_bottom = (i + 1) == num_bars;
                let allow_below = !is_bottom
                    && is_true(grob::property(tx.host, bar, "allow-span-bar")?)
                    && is_true(grob::property(
                        tx.host,
                        self.bars[i + 1],
                        "allow-span-bar-above",
                    )?);
                let heap = tx.heap();
                let pick = |allow: bool| if allow { spanbar } else { Value::FALSE };
                let pair = heap.cons(pick(allow_below), pick(allow_above));
                grob::set_object(heap, bar, has_span_bar, pair);
                allow_above = allow_below;
            }
        }
        self.bars.clear();
        self.stubs.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.spanbar);
        out.extend(self.bars.iter().copied());
        out.extend(self.stubs.iter().copied());
    }
}
