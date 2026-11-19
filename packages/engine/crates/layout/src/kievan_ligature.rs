//! `kievan-ligature-engraver.cc` and `kievan-ligature.cc` (MS8 step 9,
//! part 2; 164 and 42 lines): `Kievan_ligature_engraver`, a class of
//! layout's ([`KIEVAN_LIGATURE_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108), a [`LigatureEngraver`] over
//! [`Kievan`]; and `ly:kievan-ligature::print`, Rust under a primitive that
//! prints as LilyPond's subr and calls it over `%host-call` (D127,
//! [`HOSTS`]), which gives `'()`: the heads draw the ligature.
//!
//! `build_ligature` (`:130–141`): the heads moved onto the first's column
//! one after the other, `padding` apart and more before an accidental, their
//! dots behind them (`fold_up_primitives`, `:61–128`); the ligature's
//! `minimum-length` at least their width. Its heads keep their own stencils.

use lily_engine_music::{Host, Kind, TranslatorClass, Tx};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::X;
use crate::ligature::{
    Coherent, LIGATURE_ACKNOWLEDGES, LIGATURE_ALWAYS, LIGATURE_METHODS, LigatureEngraver,
    move_related_items_to_column, translate,
};
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::{grob, item, rhythmic_head};

type Run<T> = Result<T, Exception>;

pub static KIEVAN_LIGATURE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Kievan_ligature_engraver",
    kind: Kind::Engraver,
    methods: LIGATURE_METHODS,
    always: LIGATURE_ALWAYS,
    hears: &["ligature-event"],
    acknowledges: LIGATURE_ACKNOWLEDGES,
    must_be_last: false,
    make: Some(|| alloc::boxed::Box::new(LigatureEngraver::<Kievan>::default())),
    reads: &[],
    writes: &[],
};

/// `Kievan_ligature_engraver`'s own part: it keeps nothing, and sets no
/// stencil on its heads.
#[derive(Clone, Default)]
pub(crate) struct Kievan;

impl Coherent for Kievan {
    const SPANNER: &'static str = "KievanLigature";
    const PRIMITIVE: Option<&'static str> = None;

    fn build_ligature(
        &mut self,
        tx: &mut Tx<'_>,
        ligature: Value,
        primitives: &[Value],
    ) -> Run<()> {
        let v = grob::property(tx.host, ligature, "padding")?;
        let padding = double_or(tx.host, v, 0.0);
        let min_length = fold_up_primitives(tx, primitives, padding)?;
        let v = grob::property(tx.host, ligature, "minimum-length")?;
        if double_or(tx.host, v, 0.0) < min_length {
            let v = tx.heap().flonum(min_length);
            grob::set_property(tx.heap(), ligature, "minimum-length", v);
        }
        Ok(())
    }
}

/// The X extent's length of `g` relative to itself.
fn own_width(tx: &mut Tx<'_>, g: Value) -> Run<f64> {
    Ok(grob::extent(tx.host, g, g, X)?.length())
}

/// The grob of `g`'s `accidental-grob`, if it has one.
fn accidental(tx: &mut Tx<'_>, g: Value) -> Run<Option<Value>> {
    let sym = tx.heap().intern("accidental-grob");
    let a = grob::object(tx.host, g, sym)?;
    Ok(lily_engine_music::grob_of(tx.heap(), a).map(|_| a))
}

/// `fold_up_primitives` (`:61–128`): the ligature's least length.
fn fold_up_primitives(tx: &mut Tx<'_>, primitives: &[Value], mut padding: f64) -> Run<f64> {
    let mut first: Option<Value> = None;
    let mut accumul_acc_space = 0.0;
    // start us off with some padding on the left
    let mut min_length = padding;

    for (i, &current) in primitives.iter().enumerate() {
        let mut head_width = own_width(tx, current)?;
        if i == 0 {
            first = Some(current);
        }

        // must keep track of accidentals in spacing problem
        if i > 0
            && let Some(acc_gr) = accidental(tx, current)?
        {
            accumul_acc_space += own_width(tx, acc_gr)?;
        }

        // LilyPond reads through a first head of no column; here nothing
        // moves.
        if let Some(column) = first.and_then(|f| item::get_column(tx.heap(), f)) {
            move_related_items_to_column(tx, current, column, min_length)?;
        }

        // check if we have any dots
        if rhythmic_head::dot_count(tx.host, current)? != 0
            && let Some(dot_gr) = rhythmic_head::get_dots(tx.host, current)?
        {
            // This is ugly and should probably be handled by configuring
            // the DotColumn appropriately.  Note that these dots will be
            // disconnected from their dot column.  See
            // move_related_items_to_column.
            //
            // This also means the padding isn't configurable as
            // DotColumn.padding is.
            let stil = grob::property(tx.host, dot_gr, "dot-stencil")?;
            let dot_width = tx
                .heap()
                .foreign::<Stencil>(stil)
                .map_or(0.0, |s| s.extent(X).length());
            head_width += dot_width - 0.5 * (padding - accumul_acc_space);

            translate(
                tx,
                dot_gr,
                0.5 * (padding - accumul_acc_space) + dot_width,
                X,
            )?;
        }

        // add more padding if we have an accidental coming up
        if i + 1 < primitives.len()
            && let Some(acc_gr) = accidental(tx, primitives[i + 1])?
        {
            padding += own_width(tx, acc_gr)?;
        }

        min_length += head_width + padding - accumul_acc_space;
    }
    Ok(min_length)
}

/// `Kievan_ligature::print`: `'()`.
pub fn print(_: &mut dyn Host, _: Value) -> Run<Value> {
    Ok(Value::NULL)
}

/// What `%host-call` runs for the `ly:kievan-ligature::` callback (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "kievan-ligature",
    f: host_kievan_ligature,
}];

/// The `ly:kievan-ligature::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("print", print)];

fn host_kievan_ligature(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such kievan-ligature callback")
}

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> Result<Value, lily_engine_scheme::Thrown> {
    crate::tie::host_call_of(cx, "kievan-ligature", "ly:kievan-ligature::", "print", args)
}
