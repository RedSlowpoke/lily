//! `forbid-break-engraver.cc` (MS3 step 21): `Forbid_line_break_engraver`,
//! a class of layout's ([`FORBID_LINE_BREAK_ENGRAVER`]) that the core
//! passes at its `\layout` call and that a group finds before the music
//! crate's placeholder of the name (D67, D108).
//!
//! - `pre_process_music` (`:41–62`): `busyGrobs`' leading entries whose
//!   end's main part is now's are passed over (`:50–52`), and if any later
//!   one is a grob with `rhythmic-grob-interface`, a note head or rest
//!   still playing, the Score's `forbidBreak` is set to `#t`
//!   (`:54–61`). As it must run at every timestep, the class runs always.

use alloc::boxed::Box;

use lily_engine_music::{Kind, Moment, PRE_PROCESS, Rational, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::grob;
use crate::paper_column_engraver::find_score_context;

type Run<T> = Result<T, Exception>;

pub static FORBID_LINE_BREAK_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Forbid_line_break_engraver",
    kind: Kind::Engraver,
    methods: PRE_PROCESS,
    // `pre_process_music` reads `busyGrobs` every timestep.
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(ForbidLineBreakEngraver)),
    reads: &["busyGrobs"],
    writes: &["forbidBreak"],
};

#[derive(Clone)]
struct ForbidLineBreakEngraver;

/// Whether `busyGrobs` entry `entry` ends at main part `main`.
fn ends_at(heap: &Heap, entry: Value, main: Rational) -> bool {
    heap.car(entry)
        .and_then(|end| heap.foreign::<Moment>(end).copied())
        .is_some_and(|end| end.main == main)
}

impl Work for ForbidLineBreakEngraver {
    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut busy = tx.get("busyGrobs");
        let now = tx.now();
        let heap = tx.heap();
        while let (Some(entry), Some(rest)) = (heap.car(busy), heap.cdr(busy)) {
            if !ends_at(heap, entry, now.main) {
                break;
            }
            busy = rest;
        }

        let mut forbid = false;
        while let (Some(entry), Some(rest)) = (heap.car(busy), heap.cdr(busy)) {
            if let Some(g) = heap.cdr(entry)
                && grob::has_interface(heap, g, "rhythmic-grob-interface")
            {
                forbid = true;
            }
            busy = rest;
        }
        if forbid {
            let score = find_score_context(tx);
            let sym = tx.heap().intern("forbidBreak");
            tx.t.arena.borrow_mut().set(score, sym, Value::TRUE);
        }
        Ok(())
    }
}
