//! `staff-collecting-engraver.cc` (MS3 step 48, OPEN A21; the whole file,
//! 85 lines, which no part's split had listed): `Staff_collecting_engraver`,
//! a class of layout's ([`STAFF_COLLECTING_ENGRAVER`]) that the core passes
//! at its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108). It keeps `stavesFound`, which
//! `Bar_number_engraver` gives its `BarNumber` as `side-support-elements`
//! (and LilyPond's `Mark_engraver`, `Jump_engraver` and `Volta_engraver`
//! read).
//!
//! - Each staff symbol acknowledged (`acknowledge_staff_symbol`, `:37–44`)
//!   consed onto `stavesFound`, as seen from the context, and the list set
//!   in the engraver's own context.
//! - The end of one (`acknowledge_end_staff_symbol`, `:46–53`): every
//!   entry of it deleted from the list (`scm_delq`), which is set again.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Kind, TranslatorClass, Tx, Work};
use lily_engine_scheme::Exception;

type Run<T> = Result<T, Exception>;

pub static STAFF_COLLECTING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Staff_collecting_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[
        ("staff-symbol-interface", false),
        ("staff-symbol-interface", true),
    ],
    must_be_last: false,
    make: Some(|| Box::new(StaffCollectingEngraver)),
    reads: &["stavesFound"],
    writes: &["stavesFound"],
};

/// The index of the end acknowledger in `acknowledges`.
const END: usize = 1;

#[derive(Clone)]
struct StaffCollectingEngraver;

impl Work for StaffCollectingEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        let staffs = tx.get("stavesFound");
        let heap = tx.heap();
        let staffs = if which == END {
            let mut kept = Vec::new();
            let mut l = staffs;
            while let (Some(s), Some(rest)) = (heap.car(l), heap.cdr(l)) {
                if s != g {
                    kept.push(s);
                }
                l = rest;
            }
            heap.list(&kept)
        } else {
            heap.cons(g, staffs)
        };
        tx.set("stavesFound", staffs);
        Ok(())
    }
}
