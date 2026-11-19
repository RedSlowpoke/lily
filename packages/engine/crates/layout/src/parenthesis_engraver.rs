//! `parenthesis-engraver.cc` (MS8 step 7, part 2, 140 lines):
//! `Parenthesis_engraver`, a class of layout's ([`PARENTHESIS_ENGRAVER`])
//! that shadows the music crate's placeholder of its name (D67, D108).
//!
//! - Each grob acknowledged (`:60–118`) whose `parenthesized` is `#t`, but
//!   an accidental's (whose stencil has its own parentheses) and a tab note
//!   head's: one `Parentheses` for the grobs of the timestep with the same
//!   `parenthesis-id` (a symbol), or one of its own. A new one is made by
//!   the grob's own engraver as a sticky grob of the grob's class
//!   (`Engraver::make_sticky`, `engraver.cc:262–278`, with
//!   `make_sticky_same_type`): caused by the grob, its `sticky-host`, a child
//!   of it on both axes; LilyPond's programming error for a type without
//!   the `sticky-grob-interface`. The grob is one of its `elements`, its
//!   `font-size` added to the parentheses'.
//! - `stop_translation_timestep` (`:120–124`) forgets the timestep's ids.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::{X, Y};
use crate::item::{self, Class};
use crate::spacing_options::double_or;
use crate::{grob, grob_array};

type Run<T> = Result<T, Exception>;

pub static PARENTHESIS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Parenthesis_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(ParenthesisEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `id_alist_`: the timestep's parentheses by their `parenthesis-id`.
#[derive(Clone, Default)]
struct ParenthesisEngraver {
    ids: Vec<(Value, Value)>,
}

/// `make_sticky`: a sticky `name` of `host`'s class made by `host`'s
/// engraver, caused by `cause` (the host, or an event: a balloon's, a
/// footnote's).
pub(crate) fn make_sticky(
    tx: &mut Tx<'_>,
    name: &str,
    host_id: u32,
    host: Value,
    cause: Cause,
) -> Run<Value> {
    let origin = tx.t.arena.borrow().records.grobs[host_id as usize].origin;
    let class = item::class(tx.heap(), host);
    if class == Some(Class::PaperColumn) {
        // Paper_column::make_sticky_same_type's error.
        let text = format!("refusing to create a grob of type {name} sticking to a PaperColumn");
        tx.programming_error(&text)?;
    }
    let id = tx.make_defined_grob_from(origin, name, cause)?;
    let sticky = tx.grob_handle(id);
    let class = if class == Some(Class::Spanner) {
        Class::Spanner
    } else {
        Class::Item
    };
    item::set_class(tx.heap(), sticky, class);
    if !grob::has_interface(tx.heap(), sticky, "sticky-grob-interface") {
        let text = format!(
            "sticky grob {name} created with a type that does not have the sticky-grob-interface"
        );
        tx.programming_error(&text)?;
    }
    let heap = tx.heap();
    let sym = heap.intern("sticky-host");
    grob::set_object(heap, sticky, sym, host);
    grob::set_parent(heap, sticky, host, X);
    grob::set_parent(heap, sticky, host, Y);
    Ok(sticky)
}

impl Work for ParenthesisEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob_id: u32) -> Run<()> {
        let g = tx.grob_handle(grob_id);
        if grob::property(tx.host, g, "parenthesized")? != Value::TRUE
            // AccidentalCautionary has its own implementation of parentheses,
            // and so has TabNoteHead.
            || grob::has_interface(tx.heap(), g, "accidental-interface")
            || grob::has_interface(tx.heap(), g, "tab-note-head-interface")
        {
            return Ok(());
        }
        let id = grob::property(tx.host, g, "parenthesis-id")?;
        let mut must_add = false;
        let mut paren = None;
        if id.tag() == Tag::Symbol {
            match self.ids.iter().find(|&&(k, _)| k == id) {
                Some(&(_, p)) => paren = Some(p),
                None => must_add = true,
            }
        }
        let paren = match paren {
            Some(p) => p,
            None => make_sticky(tx, "Parentheses", grob_id, g, Cause::Grob(grob_id))?,
        };
        if must_add {
            self.ids.push((id, paren));
        }
        let sym = tx.heap().intern("elements");
        grob_array::add_grob(tx.host, paren, sym, g)?;
        let mine = grob::property(tx.host, paren, "font-size")?;
        let mine = double_or(tx.host, mine, 0.0);
        let its = grob::property(tx.host, g, "font-size")?;
        let its = double_or(tx.host, its, 0.0);
        let size = tx.heap().flonum(mine + its);
        grob::set_property(tx.heap(), paren, "font-size", size);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.ids.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for &(k, v) in &self.ids {
            out.extend([k, v]);
        }
    }
}
