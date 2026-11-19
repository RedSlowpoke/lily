//! `cue-clef-engraver.cc` (MS8 step 7, part 2, 238 lines):
//! `Cue_clef_engraver`, a class of layout's ([`CUE_CLEF_ENGRAVER`]) that
//! shadows the music crate's placeholder of its name (D67, D108), as
//! `clef_engraver.rs`' `Clef_engraver` is the plain clef's.
//!
//! `process_music` (`:155–166`) inspects the cue clef's properties
//! (`inspect_clef_properties`, `:168–191`): when `cueClefGlyph`,
//! `cueClefPosition` or `cueClefTransposition` changed (each `equal?` to the
//! last, `'()` at the start), the `CueClef`'s `glyph` is reverted and pushed
//! as `cueClefGlyph` and the `CueEndClef`'s as `clefGlyph` (`set_glyph`,
//! `:70–83`); a `CueClef` is made for a glyph that is a string, marked
//! `non-default`, and a `CueEndClef` otherwise. Then a `CueClef` is made
//! where a break is allowed and `cueClefGlyph` is a string. A clef's
//! `staff-position` is `cueClefPosition` (the end clef's `clefPosition`),
//! and a transposition that is a non-zero number makes a `ClefModifier`
//! from the cue's (or the clef's) transposition, style and formatter.
//! `stop_translation_timestep` (`:193–208`) gives a `non-default` clef
//! `explicitCueClefVisibility` as its `break-visibility`.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, pop, push};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::clef_engraver::{break_allowed, is_true};
use crate::geometry::{X, Y};
use crate::grob;
use crate::side_position;

type Run<T> = Result<T, Exception>;

pub static CUE_CLEF_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Cue_clef_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(CueClefEngraver::default())),
    reads: &[
        "cueClefGlyph",
        "cueClefPosition",
        "cueClefTransposition",
        "cueClefTranspositionFormatter",
        "cueClefTranspositionStyle",
        "explicitCueClefVisibility",
        "forbidBreak",
        "forceBreak",
    ],
    writes: &[],
};

/// `clef_` by its handle, `prev_glyph_`, `prev_cpos_` and
/// `prev_transposition_`, `'()` at the start.
#[derive(Clone)]
struct CueClefEngraver {
    clef: Option<Value>,
    prev_glyph: Value,
    prev_cpos: Value,
    prev_transposition: Value,
}

impl Default for CueClefEngraver {
    fn default() -> CueClefEngraver {
        CueClefEngraver {
            clef: None,
            prev_glyph: Value::NULL,
            prev_cpos: Value::NULL,
            prev_transposition: Value::NULL,
        }
    }
}

/// `execute_pushpop_property` twice: the `glyph` of `grob` reverted, then
/// pushed as the context's `property`.
fn set_glyph_of(tx: &mut Tx<'_>, grob: &str, property: &str) -> Run<()> {
    let glyph = tx.get(property);
    let heap = tx.host.heap();
    let basic = heap.intern(grob);
    let sym = heap.intern("glyph");
    let path = heap.list(&[sym]);
    pop(heap, &mut tx.t.arena.borrow_mut(), tx.context, basic, path);
    push(tx.host, tx.t, tx.context, basic, path, glyph)?;
    Ok(())
}

impl CueClefEngraver {
    /// `set_glyph`.
    fn set_glyph(tx: &mut Tx<'_>) -> Run<()> {
        set_glyph_of(tx, "CueClef", "cueClefGlyph")?;
        set_glyph_of(tx, "CueEndClef", "clefGlyph")
    }

    /// `create_clef_modifier`: a `ClefModifier` of a transposition that is
    /// a non-zero number, over the clef.
    fn create_clef_modifier(
        &mut self,
        tx: &mut Tx<'_>,
        transp: &str,
        style: &str,
        formatter: &str,
    ) -> Run<()> {
        let Some(clef) = self.clef else {
            return Ok(());
        };
        let transp = tx.get(transp);
        if !tx.heap().is_number(transp) {
            return Ok(());
        }
        let transp = crate::stem::to_int(tx.heap(), transp)?;
        if transp == 0 {
            return Ok(());
        }
        let id = tx.make_defined_grob("ClefModifier", Cause::None)?;
        let g = tx.grob_handle(id);
        let dir = transp.signum();
        let abs_transp = transp.wrapping_abs().wrapping_add(1);
        let txt = tx.heap().string(format!("{abs_transp}"));
        let style = tx.get(style);
        let formatter = tx.get(formatter);
        if tx.heap().is_procedure(formatter) {
            let depth = tx.heap().root_depth();
            tx.heap().root(txt);
            let text = tx.host.call(formatter, &[txt, style]);
            tx.heap().unroot_to(depth);
            grob::set_property(tx.heap(), g, "text", text?);
        }
        side_position::add_support(tx.host, g, clef)?;
        let heap = tx.heap();
        grob::set_parent(heap, g, clef, Y);
        grob::set_parent(heap, g, clef, X);
        let dir = heap.integer(i64::from(dir));
        grob::set_property(heap, g, "direction", dir);
        Ok(())
    }

    /// `create_clef`: a `CueClef` unless one was made this timestep.
    fn create_clef(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.clef.is_some() {
            return Ok(());
        }
        let id = tx.make_defined_grob("CueClef", Cause::None)?;
        let clef = tx.grob_handle(id);
        self.clef = Some(clef);
        let cpos = tx.get("cueClefPosition");
        if tx.heap().is_number(cpos) {
            grob::set_property(tx.heap(), clef, "staff-position", cpos);
        }
        self.create_clef_modifier(
            tx,
            "cueClefTransposition",
            "cueClefTranspositionStyle",
            "cueClefTranspositionFormatter",
        )
    }

    /// `create_end_clef`: a `CueEndClef` unless a clef was made this
    /// timestep.
    fn create_end_clef(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.clef.is_some() {
            return Ok(());
        }
        let id = tx.make_defined_grob("CueEndClef", Cause::None)?;
        let clef = tx.grob_handle(id);
        self.clef = Some(clef);
        let cpos = tx.get("clefPosition");
        if tx.heap().is_number(cpos) {
            grob::set_property(tx.heap(), clef, "staff-position", cpos);
        }
        self.create_clef_modifier(
            tx,
            "clefTransposition",
            "clefTranspositionStyle",
            "clefTranspositionFormatter",
        )
    }

    fn inspect_clef_properties(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let glyph = tx.get("cueClefGlyph");
        let clefpos = tx.get("cueClefPosition");
        let transposition = tx.get("cueClefTransposition");
        let heap = tx.heap();
        if !heap.equal(glyph, self.prev_glyph)
            || !heap.equal(clefpos, self.prev_cpos)
            || !heap.equal(transposition, self.prev_transposition)
        {
            Self::set_glyph(tx)?;
            if glyph.tag() == Tag::String {
                self.create_clef(tx)?;
                if let Some(clef) = self.clef {
                    grob::set_property(tx.heap(), clef, "non-default", Value::TRUE);
                }
            } else {
                self.create_end_clef(tx)?;
            }
            self.prev_cpos = clefpos;
            self.prev_glyph = glyph;
            self.prev_transposition = transposition;
        }
        Ok(())
    }
}

impl Work for CueClefEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.inspect_clef_properties(tx)?;
        // Efficiency: don't create a default clef if it's not going to be
        // visible. A default clef can only be visible at the start of the
        // line.
        let glyph = tx.get("cueClefGlyph");
        if glyph.tag() == Tag::String && break_allowed(tx) {
            self.create_clef(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(clef) = self.clef.take()
            && is_true(grob::property(tx.host, clef, "non-default")?)
        {
            let vis = tx.get("explicitCueClefVisibility");
            if tx.heap().vector_items(vis).is_some() {
                grob::set_property(tx.heap(), clef, "break-visibility", vis);
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.clef);
        out.extend([self.prev_glyph, self.prev_cpos, self.prev_transposition]);
    }
}
