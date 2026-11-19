//! `spanner-break-forbid-engraver.cc` (MS3 step 22):
//! `Spanner_break_forbid_engraver`, a class of layout's
//! ([`SPANNER_BREAK_FORBID_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each spanner with `unbreakable-spanner-interface` acknowledged
//!   (`acknowledge_unbreakable_spanner`, `:58–64`) is kept running unless
//!   its `breakable` is `#t` (`from_scm<bool>`: `#t` alone is true).
//! - The end of one acknowledged (`acknowledge_end_unbreakable_spanner`,
//!   `:48–56`): its first entry among the running ones dropped.
//! - `pre_process_music` (`:41–46`): while any runs, the Score's
//!   `forbidBreak` set to `#t`. The class stays awake while any runs, as
//!   it is woken by nothing else between the start and the end.
//!
//! The running spanners are kept by their record numbers. An acknowledged
//! non-spanner, where LilyPond's trampoline gives its type error, is taken
//! as a spanner.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Kind, PRE_PROCESS, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::paper_column_engraver::find_score_context;

type Run<T> = Result<T, Exception>;

pub static SPANNER_BREAK_FORBID_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Spanner_break_forbid_engraver",
    kind: Kind::Engraver,
    methods: PRE_PROCESS,
    always: false,
    hears: &[],
    acknowledges: &[
        ("unbreakable-spanner-interface", true),
        ("unbreakable-spanner-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SpannerBreakForbidEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The index of the end acknowledger in `acknowledges`.
const END: usize = 0;

/// `running_spanners_`.
#[derive(Clone, Default)]
struct SpannerBreakForbidEngraver {
    running_spanners: Vec<u32>,
}

impl Work for SpannerBreakForbidEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.records(&self.running_spanners);
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.running_spanners.is_empty() {
            let score = find_score_context(tx);
            let sym = tx.heap().intern("forbidBreak");
            tx.t.arena.borrow_mut().set(score, sym, Value::TRUE);
            tx.stay_awake();
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if which == END {
            if let Some(i) = self.running_spanners.iter().position(|&s| s == grob) {
                self.running_spanners.remove(i);
            }
            return Ok(());
        }

        let g = tx.grob_handle(grob);
        if grob::property(tx.host, g, "breakable")? != Value::TRUE {
            self.running_spanners.push(grob);
            tx.stay_awake();
        }
        Ok(())
    }
}
