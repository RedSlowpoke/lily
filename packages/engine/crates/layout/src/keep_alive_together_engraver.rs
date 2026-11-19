//! `keep-alive-together-engraver.cc` (MS3 step 21):
//! `Keep_alive_together_engraver`, a class of layout's
//! ([`KEEP_ALIVE_TOGETHER_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - Each hara-kiri group acknowledged (`:44–48`) is kept, in the order
//!   they are acknowledged.
//! - `finalize` (`:50–123`): for each group whose `remove-layer` is not
//!   `#f` (`:55–57`), each other group (`:66–67`) is taken by a symbol layer
//!   (`:69–98`): `any` every one, `above` the one just before it, `below`
//!   the one just after it, and any other symbol is LilyPond's warning, once
//!   for each other group; else (`:100–116`) one whose layer is `#f` is
//!   not, an unset layer takes all the others, and an integer only an
//!   integer: an equal one keeps it alive, a lower one makes it dead. Each
//!   of the two fresh arrays is set as `keep-alive-with` and
//!   `make-dead-when` only if it is not empty (`:118–121`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{FINALIZE, Kind, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::grob;
use crate::grob_array::GrobArray;

type Run<T> = Result<T, Exception>;

pub static KEEP_ALIVE_TOGETHER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Keep_alive_together_engraver",
    kind: Kind::Engraver,
    methods: FINALIZE,
    always: false,
    hears: &[],
    acknowledges: &[("hara-kiri-group-spanner-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(KeepAliveTogetherEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `group_spanners_`, the groups by their handles.
#[derive(Clone, Default)]
struct KeepAliveTogetherEngraver {
    group_spanners: Vec<Value>,
}

/// `group` set to a fresh grob array of `grobs` in its object `name`
/// (`set_object` of `Grob_array::make_array`), if there are any.
fn set_array(tx: &mut Tx<'_>, group: Value, name: &str, grobs: Vec<Value>) {
    if !grobs.is_empty() {
        let heap = tx.heap();
        let sym = heap.intern(name);
        let arr = heap.wrap(GrobArray {
            grobs,
            ordered: true,
        });
        grob::set_object(heap, group, sym, arr);
    }
}

impl Work for KeepAliveTogetherEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        self.group_spanners.push(tx.grob_handle(grob));
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let groups = &self.group_spanners;
        for i in 0..groups.len() {
            let this_layer = grob::property(tx.host, groups[i], "remove-layer")?;
            if this_layer.is_false() {
                continue;
            }

            let mut live = Vec::new();
            let mut dead = Vec::new();

            for j in 0..groups.len() {
                if i == j {
                    continue;
                }

                if this_layer.tag() == Tag::Symbol {
                    match tx.heap().name(this_layer) {
                        // layer is kept alive by any other layer
                        Some("any") => live.push(groups[j]),
                        // layer is kept alive by the layer preceding it
                        Some("above") => {
                            if i == j + 1 {
                                live.push(groups[j]);
                            }
                        }
                        // layer is kept alive by the layer following it
                        // (`i == j - 1` in `vsize`, never at j = 0)
                        Some("below") => {
                            if i + 1 == j {
                                live.push(groups[j]);
                            }
                        }
                        name => {
                            let text = alloc::format!(
                                "unknown remove-layer value `{}'",
                                name.unwrap_or_default()
                            );
                            grob::warning(tx.host, groups[i], &text)?;
                        }
                    }
                    continue;
                }

                let that_layer = grob::property(tx.host, groups[j], "remove-layer")?;

                if that_layer.is_false() {
                    continue;
                }
                let heap = tx.heap();
                if !heap.is_integer(this_layer) {
                    // unset layers are kept alive by all but ignored layers
                    live.push(groups[j]);
                    continue;
                }
                // an explicit layer is only affected by explicit layers
                if !heap.is_integer(that_layer) {
                    continue;
                }
                if heap.num_eq(that_layer, this_layer) == Ok(true) {
                    live.push(groups[j]);
                } else if heap.less(that_layer, this_layer) == Ok(true) {
                    dead.push(groups[j]);
                }
            }
            set_array(tx, groups[i], "keep-alive-with", live);
            set_array(tx, groups[i], "make-dead-when", dead);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.group_spanners.iter().copied());
    }
}
