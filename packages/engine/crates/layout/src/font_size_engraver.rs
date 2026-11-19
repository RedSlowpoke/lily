//! `font-size-engraver.cc` (MS3 step 48, OPEN A34; the whole file, 91
//! lines, which no part's split had listed): `Font_size_engraver`, a class
//! of layout's ([`FONT_SIZE_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108). It puts the context's `fontSize`
//! into its grobs' `font-size`, which `\magnifyStaff`, `\small`, `\tiny`
//! and the others set.
//!
//! - `process_music` (`:41–45`): the size, `fontSize` as seen from the
//!   context (`from_scm<double>`, 0 if no real), kept until the next
//!   timestep's; it runs at every timestep, as LilyPond's does, since a
//!   `\set` of `fontSize` wakes no engraver.
//! - Each grob with `font-interface` acknowledged (`acknowledge_font`,
//!   `:47–62`) whose origin engraver is in the engraver's own context,
//!   unless the size is 0: its `font-size` (read with its callback, 0 if no
//!   real) plus the size, set as its `font-size`.

use alloc::boxed::Box;

use lily_engine_music::{Kind, PROCESS, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::grob;

type Run<T> = Result<T, Exception>;

pub static FONT_SIZE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Font_size_engraver",
    kind: Kind::Engraver,
    methods: PROCESS,
    always: true,
    hears: &[],
    acknowledges: &[("font-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(FontSizeEngraver::default())),
    reads: &["fontSize"],
    writes: &[],
};

/// `size`.
#[derive(Clone, Default)]
struct FontSizeEngraver {
    size: f64,
}

/// `from_scm<double> (v, fallback)`: a real's value, else `fallback`.
fn double_or(heap: &Heap, v: Value, fallback: f64) -> f64 {
    if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(fallback)
    } else {
        fallback
    }
}

impl Work for FontSizeEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.float(self.size);
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let size = tx.get("fontSize");
        self.size = double_or(tx.heap(), size, 0.0);
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _which: usize, grob: u32) -> Run<()> {
        // We only want to process a grob once.
        if self.size == 0.0 {
            return Ok(());
        }
        let origin = tx.t.arena.borrow().records.grobs[grob as usize].origin;
        if origin.0 != tx.origin().0 {
            return Ok(());
        }
        let g = tx.grob_handle(grob);
        let own = grob::property(tx.host, g, "font-size")?;
        let font_size = self.size + double_or(tx.heap(), own, 0.0);
        let v = tx.heap().flonum(font_size);
        grob::set_property(tx.heap(), g, "font-size", v);
        Ok(())
    }
}
