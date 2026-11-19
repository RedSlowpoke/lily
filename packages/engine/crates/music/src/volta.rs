//! LilyPond's `Repeat_acknowledge_engraver`
//! (`repeat-acknowledge-engraver.cc`), N7's first part (04 §13, MS2 step
//! 37): it makes no grobs. It empties `repeatCommands` where it is defined
//! at the start of each timestep, and adds `(start-repeat n)` and
//! `(end-repeat n)` to it when the volta iterators' repeat styler sends
//! `VoltaRepeatStartEvent` and `VoltaRepeatEndEvent` (`iterator.rs`), the
//! first of each a timestep, so that the bar line engraver of MS3 sees
//! `\repeat volta` as it sees manual repeats.

use alloc::boxed::Box;

use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::dispatcher::event_property;
use crate::translator::{INITIALIZE, Kind, START, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

pub(crate) const CLASS: TranslatorClass = TranslatorClass {
    name: "Repeat_acknowledge_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | STOP,
    // `start_translation_timestep` empties `repeatCommands` every timestep.
    always: true,
    hears: &["volta-repeat-end-event", "volta-repeat-start-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(RepeatAcknowledge::default())),
    reads: &[],
    writes: &["repeatCommands"],
};

const END: usize = 0;

#[derive(Clone, Default)]
struct RepeatAcknowledge {
    heard_end: bool,
    heard_start: bool,
}

/// `ly_cheap_is_list`: a list that ends in `'()`.
fn is_list(heap: &Heap, mut v: Value) -> bool {
    while v.tag() == Tag::Pair {
        v = heap.cdr(v).unwrap_or(Value::NULL);
    }
    v == Value::NULL
}

impl RepeatAcknowledge {
    /// `add_repeat_command`: `(command count)` consed onto
    /// `repeatCommands` where it is defined, if it is a list.
    fn add(&mut self, tx: &mut Tx<'_>, command: &str, count: i64) {
        let sym = tx.heap().intern("repeatCommands");
        let found = tx.t.arena.borrow().where_defined(tx.context, sym);
        let Some((at, cmds)) = found else {
            return;
        };
        let heap = tx.heap();
        if !is_list(heap, cmds) {
            return;
        }
        let name = heap.intern(command);
        let count = heap.integer(count);
        let new = heap.list(&[name, count]);
        let cmds = heap.cons(new, cmds);
        tx.t.arena.borrow_mut().set(at, sym, cmds);
    }
}

impl Work for RepeatAcknowledge {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.heard_end, self.heard_start]);
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("repeatCommands", Value::NULL);
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let sym = tx.heap().intern("repeatCommands");
        let found = tx.t.arena.borrow().where_defined(tx.context, sym);
        let at = found.map_or(tx.context, |(c, _)| c);
        tx.t.arena.borrow_mut().set(at, sym, Value::NULL);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.heard_end = false;
        self.heard_start = false;
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let (key, heard, command, least) = if which == END {
            ("return-count", self.heard_end, "end-repeat", 0)
        } else {
            ("repeat-count", self.heard_start, "start-repeat", 1)
        };
        if heard {
            return Ok(());
        }
        let v = event_property(tx.heap(), ev, key);
        let count = tx.heap().to_i64(v).unwrap_or(0);
        if count >= least {
            if which == END {
                self.heard_end = true;
            } else {
                self.heard_start = true;
            }
            self.add(tx, command, count);
        }
        Ok(())
    }
}
