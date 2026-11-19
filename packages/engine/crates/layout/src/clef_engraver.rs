//! `clef-engraver.cc` (MS3 step 17): `Clef_engraver`, a class of layout's
//! ([`CLEF_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! `process_music` (`:119–128`) inspects the clef's properties
//! (`inspect_clef_properties`, `:138–171`): when `clefGlyph`,
//! `clefPosition` or `clefTransposition` changed, `clefPosition` is unset
//! or `forceClef` is `#t`, the image's `invalidate-alterations` is called
//! on the context and those below it (`apply_on_children`, `:130–136`),
//! the Clef's `glyph` is reverted and pushed as `clefGlyph`
//! (`set_glyph`, `:69–78`), a `Clef` is made unless it is the first and
//! `firstClef` is not `#t`, and it is marked `non-default`; `forceClef`
//! is set to `'()` where it is defined. Then a `Clef` is made where a
//! break is allowed and `clefGlyph` is a string (`create_clef`,
//! `:80–118`): its `staff-position` `clefPosition`, and for a
//! `clefTransposition` a `ClefModifier`, its text from
//! `clefTranspositionFormatter`, supported by the clef and its parent on
//! both axes. `stop_translation_timestep` (`:173–189`) gives a
//! `non-default` clef `explicitClefVisibility` as its `break-visibility`
//! and forgets the grobs.
//!
//! LilyPond's `Break_align_engraver` makes the first timestep's `LeftEdge`
//! through this engraver (`break-align-engraver.cc:121–124`); that is
//! MS3 step 21's.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, pop, push};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::{X, Y};
use crate::grob;
use crate::side_position;
use crate::stem_engraver::int_or;

type Run<T> = Result<T, Exception>;

pub static CLEF_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Clef_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(ClefEngraver::default())),
    reads: &[
        "clefGlyph",
        "clefTransposition",
        "clefTranspositionStyle",
        "clefPosition",
        "explicitClefVisibility",
        "forbidBreak",
        "forceBreak",
        "forceClef",
    ],
    writes: &[],
};

/// `clef_` and `modifier_` by their handles, `prev_glyph_`, `prev_cpos_`
/// and `prev_transposition_`.
#[derive(Clone)]
struct ClefEngraver {
    clef: Option<Value>,
    modifier: Option<Value>,
    prev_glyph: Value,
    prev_cpos: Value,
    prev_transposition: i32,
}

impl Default for ClefEngraver {
    /// A clef at the start, since `#f` is not `'()`.
    fn default() -> ClefEngraver {
        ClefEngraver {
            clef: None,
            modifier: None,
            prev_glyph: Value::FALSE,
            prev_cpos: Value::FALSE,
            prev_transposition: 0,
        }
    }
}

/// `from_scm<bool>`: `#t` only.
pub(crate) fn is_true(v: Value) -> bool {
    v == Value::TRUE
}

/// `break_allowed` (`context.cc:1044–1050`): nothing forbade a break, or
/// one was forced.
pub(crate) fn break_allowed(tx: &mut Tx<'_>) -> bool {
    !is_true(tx.get("forbidBreak")) || is_true(tx.get("forceBreak"))
}

impl ClefEngraver {
    /// `set_glyph`: the Clef's `glyph` reverted, then pushed as
    /// `clefGlyph` (`execute_pushpop_property`).
    fn set_glyph(tx: &mut Tx<'_>) -> Run<()> {
        let glyph = tx.get("clefGlyph");
        let heap = tx.host.heap();
        let basic = heap.intern("Clef");
        let sym = heap.intern("glyph");
        let path = heap.list(&[sym]);
        pop(heap, &mut tx.t.arena.borrow_mut(), tx.context, basic, path);
        push(tx.host, tx.t, tx.context, basic, path, glyph)?;
        Ok(())
    }

    fn create_clef(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.clef.is_some() {
            return Ok(());
        }
        let id = tx.make_defined_grob("Clef", Cause::None)?;
        let clef = tx.grob_handle(id);
        self.clef = Some(clef);
        let cpos = tx.get("clefPosition");
        if tx.heap().is_number(cpos) {
            grob::set_property(tx.heap(), clef, "staff-position", cpos);
        }

        let transposition = tx.get("clefTransposition");
        let transp = int_or(tx.heap(), transposition, 0);
        if transp != 0 {
            let id = tx.make_defined_grob("ClefModifier", Cause::None)?;
            let g = tx.grob_handle(id);

            let dir = transp.signum();
            let abs_transp = transp.wrapping_abs().wrapping_add(1);
            let txt = tx.heap().string(format!("{abs_transp}"));
            let style = tx.get("clefTranspositionStyle");
            let formatter = tx.get("clefTranspositionFormatter");
            if tx.heap().is_procedure(formatter) {
                let text = tx.host.call(formatter, &[txt, style])?;
                grob::set_property(tx.heap(), g, "text", text);
            }

            side_position::add_support(tx.host, g, clef)?;

            let heap = tx.heap();
            grob::set_parent(heap, g, clef, Y);
            grob::set_parent(heap, g, clef, X);
            let dir = heap.integer(i64::from(dir));
            grob::set_property(heap, g, "direction", dir);
            self.modifier = Some(g);
        }
        Ok(())
    }

    fn inspect_clef_properties(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let glyph = tx.get("clefGlyph");
        let clefpos = tx.get("clefPosition");
        let transposition = tx.get("clefTransposition");
        let transposition = int_or(tx.heap(), transposition, 0);
        let force_clef = tx.get("forceClef");

        let heap = tx.heap();
        if clefpos == Value::NULL
            || !heap.equal(glyph, self.prev_glyph)
            || !heap.equal(clefpos, self.prev_cpos)
            || transposition != self.prev_transposition
            || is_true(force_clef)
        {
            let c = tx.context;
            apply_on_children(tx, c)?;

            Self::set_glyph(tx)?;
            if self.prev_cpos != Value::FALSE || is_true(tx.get("firstClef")) {
                self.create_clef(tx)?;
            }

            if let Some(clef) = self.clef {
                grob::set_property(tx.heap(), clef, "non-default", Value::TRUE);
            }

            self.prev_cpos = clefpos;
            self.prev_glyph = glyph;
            self.prev_transposition = transposition;
        }

        if is_true(force_clef) {
            let sym = tx.heap().intern("forceClef");
            let mut a = tx.t.arena.borrow_mut();
            if let Some((w, _)) = a.where_defined(tx.context, sym) {
                a.set(w, sym, Value::NULL);
            }
        }
        Ok(())
    }
}

/// `apply_on_children` with `Lily::invalidate_alterations`: the image's
/// procedure called on `c`, then on each context below it.
fn apply_on_children(tx: &mut Tx<'_>, c: lily_engine_music::CtxId) -> Run<()> {
    let (handle, children) = {
        let a = tx.t.arena.borrow();
        (a.handle(c), a.children(c).to_vec())
    };
    tx.call("invalidate-alterations", &[handle])?;
    for child in children {
        apply_on_children(tx, child)?;
    }
    Ok(())
}

impl Work for ClefEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.prev_transposition.into());
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.inspect_clef_properties(tx)?;
        // Efficiency: only create a clef at points where it might be
        // visible, namely where a break has not been forbidden (yet).
        let glyph = tx.get("clefGlyph");
        if glyph.tag() == Tag::String && break_allowed(tx) {
            self.create_clef(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(clef) = self.clef.take() {
            if is_true(grob::property(tx.host, clef, "non-default")?) {
                let vis = tx.get("explicitClefVisibility");
                if tx.heap().vector_items(vis).is_some() {
                    grob::set_property(tx.heap(), clef, "break-visibility", vis);
                }
            }
            self.modifier = None;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.clef);
        out.extend(self.modifier);
        out.extend([self.prev_glyph, self.prev_cpos]);
    }
}
