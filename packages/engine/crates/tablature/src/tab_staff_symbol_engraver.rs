//! `tab-staff-symbol-engraver.cc` (MS4 step 7a): `Tab_staff_symbol_engraver`
//! ([`TAB_STAFF_SYMBOL_ENGRAVER`]). Each staff symbol acknowledged
//! (`:32–38`) gets `stringTunings`' length as its `line-count`, unless that
//! is not a proper list.

use alloc::boxed::Box;

use lily_engine_layout::grob;
use lily_engine_music::{Kind, TranslatorClass, Tx, Work};
use lily_engine_scheme::Exception;

use crate::tab_note_heads_engraver::ilength;

pub static TAB_STAFF_SYMBOL_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tab_staff_symbol_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[("staff-symbol-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(TabStaffSymbolEngraver)),
    reads: &["stringTunings"],
    writes: &[],
};

#[derive(Clone)]
struct TabStaffSymbolEngraver;

impl Work for TabStaffSymbolEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Result<(), Exception> {
        let tunings = tx.get("stringTunings");
        if let Some(k) = ilength(tx.heap(), tunings) {
            let g = tx.grob_handle(grob);
            let k = tx.heap().integer(k as i64);
            grob::set_property(tx.heap(), g, "line-count", k);
        }
        Ok(())
    }
}
