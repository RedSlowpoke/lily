//! LilyPond's `Grace_engraver` (`grace-engraver.cc`), N6's first part (04
//! §13, MS2 step 35): it makes no grobs, and pushes the overrides of
//! `graceSettings` when grace time begins in its context and pops them when
//! it ends. The grace iterator's `GraceChange` tells it so; a timestep it
//! did not hear of is found by `process_music`, as LilyPond's does when the
//! iterator has moved to another context. It hears `GraceChange` on its
//! context's events below, as every translator here does, where LilyPond's
//! listens on the context's own event source: the same in the bottom
//! contexts LilyPond's definitions put it in.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Tag, Value};

use crate::context::CtxId;
use crate::override_;
use crate::rational::{Moment, Rational};
use crate::translator::{FINALIZE, INITIALIZE, Kind, PROCESS, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

pub(crate) const CLASS: TranslatorClass = TranslatorClass {
    name: "Grace_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | PROCESS | FINALIZE,
    // `process_music` looks at every timestep for a change it did not hear.
    always: true,
    hears: &["GraceChange"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(Grace::default())),
    reads: &["graceSettings"],
    writes: &[],
};

#[derive(Clone)]
struct Grace {
    /// `last_moment_`, `-Moment::infinity ()` at first.
    last: Moment,
    /// `grace_settings_`: each override pushed, by its context, its grob
    /// and the cell [`override_::matched_pop`] takes back.
    pushed: Vec<(CtxId, Value, Value)>,
}

impl Default for Grace {
    fn default() -> Grace {
        Grace {
            last: Moment::new(-Rational::INFINITY, Rational::ZERO),
            pushed: Vec::new(),
        }
    }
}

impl Grace {
    /// `consider_change_grace_settings`: out of grace time the overrides
    /// popped; into it from main time those of `graceSettings` pushed, each
    /// `(context grob property value)` in the nearest context of that name
    /// above, or LilyPond's programming error.
    fn consider(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if now.grace.is_zero() {
            for (c, grob, cell) in core::mem::take(&mut self.pushed) {
                let a = tx.t.arena.borrow();
                override_::matched_pop(tx.host.heap(), &a, c, grob, cell);
            }
        } else if self.last.grace.is_zero() {
            self.pushed.clear();
            let mut p = tx.get("graceSettings");
            while let (Some(entry), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
                p = rest;
                let heap = tx.heap();
                let at = |n: usize| {
                    let mut l = entry;
                    for _ in 0..n {
                        l = heap.cdr(l).unwrap_or(Value::NULL);
                    }
                    heap.car(l).unwrap_or(Value::NULL)
                };
                let (name, grob, sym, value) = (at(0), at(1), at(2), at(3));
                let path = if sym.tag() == Tag::Pair {
                    sym
                } else {
                    heap.list(&[sym])
                };
                let Some(c) = context_above(tx, name) else {
                    let mut text = String::from("cannot find context from graceSettings: ");
                    tx.heap().display(name, &mut text);
                    tx.programming_error(&text)?;
                    continue;
                };
                let depth = tx.heap().root_depth();
                tx.heap().root(path);
                let cell = override_::push(tx.host, tx.t, c, grob, path, value);
                tx.heap().unroot_to(depth);
                self.pushed.push((c, grob, cell?));
            }
            // LilyPond conses each onto the front, and pops them in that
            // order.
            self.pushed.reverse();
        }
        self.last = now;
        Ok(())
    }
}

/// `find_context_above`: the context or the nearest above it whose name
/// or an alias is `name`.
fn context_above(tx: &mut Tx<'_>, name: Value) -> Option<CtxId> {
    let a = tx.t.arena.borrow();
    let mut at = Some(tx.context);
    while let Some(c) = at {
        if a.is_alias(tx.host.heap(), c, name) {
            return Some(c);
        }
        at = a.parent(c);
    }
    None
}

impl Work for Grace {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.moment(self.last);
        h.num(self.pushed.len() as i64);
        for &(c, _, _) in &self.pushed {
            h.context(c);
        }
    }

    /// Already in grace time when it starts, it starts in grace mode: the
    /// grace iterator's `GraceChange` is not to be expected yet.
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.consider(tx)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.last != tx.now() {
            self.consider(tx)?;
        }
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, _: Value) -> Run<()> {
        self.consider(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.pushed.iter().flat_map(|&(_, grob, cell)| [grob, cell]));
    }
}
