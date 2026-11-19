//! `gregorian-ligature-engraver.cc`, `gregorian-ligature.cc`,
//! `vaticana-ligature-engraver.cc` and `vaticana-ligature.cc` (MS8 step 9,
//! part 2; 266, 84, 730 and 318 lines): `Vaticana_ligature_engraver`, a
//! class of layout's ([`VATICANA_LIGATURE_ENGRAVER`]) that shadows the music
//! crate's placeholder of its name (D67, D108), a [`LigatureEngraver`] over
//! [`Vaticana`]; and `ly:vaticana-ligature::brew-ligature-primitive` and
//! `ly:vaticana-ligature::print`, Rust under primitives that print as
//! LilyPond's subrs and call it over `%host-call` (D127, [`HOSTS`]).
//!
//! - **`Gregorian_ligature_engraver`**, abstract in LilyPond and
//!   `Vaticana_ligature_engraver`'s only base here: its `pes-or-flexa`
//!   events kept once a timestep (`listen_pes_or_flexa`, read by nothing
//!   else); `build_ligature` (`:236–247`) checks each head's prefixes
//!   (`check_and_fix_all_prefixes`, `:103–193`: LilyPond's warnings for a
//!   prefix ignored or implied, `fix_prefix`, `:57–79`), each head's
//!   `prefix-set` given, then each head's `context-info`, the pes and flexa
//!   it takes part in (`provide_context_info`, `:198–234`), then the
//!   subclass's `transform_heads`.
//! - **`Vaticana_ligature_engraver::transform_heads`** (`:465–691`): each
//!   head's glyph chosen by its prefixes and its neighbours' (stacked heads,
//!   epiphonus and cephalicus, the porrectus's curved flexa, the pes's lower
//!   head), its cauda, its `thickness` the ligature's; its dots taken off
//!   into a DotColumn behind the ligature (`add_mora_column`, `:385–412`,
//!   with LilyPond's warnings for dots of one pitch or a dotted head before
//!   an undotted one); then the heads lined up on the first's column
//!   (`align_heads`, `:162–342`), joined by `add-join` and spaced by
//!   `need_extra_horizontal_space`.
//! - **`brew-ligature-primitive`** (`vaticana_brew_primitive`, `:177–270`):
//!   a head's glyph, or the curved flexa (`vaticana_brew_flexa`,
//!   `:77–162`, a bezier sandwich), or a blank half of one, moved by its
//!   `head-x-offset`, with its cauda or stem (`vaticana_brew_cauda`,
//!   `:31–71`) and its join to the next head (`vaticana_brew_join`,
//!   `:164–175`). `print` gives `'()`: the heads draw the ligature.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Host, Kind, TranslatorClass, Tx, assign_event_once, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Interval, X, Y};
use crate::ligature::{
    Coherent, LIGATURE_ACKNOWLEDGES, LIGATURE_ALWAYS, LIGATURE_METHODS, LigatureEngraver,
    cause_steps, move_related_items_to_column,
};
use crate::lookup::Bezier;
use crate::spacing_options::double_or;
use crate::stem::to_int;
use crate::stencil::Stencil;
use crate::{
    dot_column, font, font_select, grob, item, lookup, rhythmic_head, separation_item,
    staff_symbol_referencer as ssr,
};

type Run<T> = Result<T, Exception>;

// `gregorian-ligature.hh`'s head prefixes.
const VIRGA: i32 = 0x0001;
const STROPHA: i32 = 0x0002;
const INCLINATUM: i32 = 0x0004;
const AUCTUM: i32 = 0x0008;
const DESCENDENS: i32 = 0x0010;
const ASCENDENS: i32 = 0x0020;
const ORISCUS: i32 = 0x0040;
const QUILISMA: i32 = 0x0080;
const DEMINUTUM: i32 = 0x0100;
const CAVUM: i32 = 0x0200;
const LINEA: i32 = 0x0400;
const PES_OR_FLEXA: i32 = 0x0800;

// Its context info, and `vaticana-ligature.hh`'s.
const PES_LOWER: i32 = 0x0001;
const PES_UPPER: i32 = 0x0002;
const FLEXA_LEFT: i32 = 0x0004;
const FLEXA_RIGHT: i32 = 0x0008;
const AFTER_DEMINUTUM: i32 = 0x0020;
const STACKED_HEAD: i32 = 0x0100;

/// Each prefix's grob property, its bit and the name `prefixes_to_str`
/// gives it, in `check_and_fix_all_prefixes`' order.
const PREFIXES: [(&str, i32); 12] = [
    ("virga", VIRGA),
    ("stropha", STROPHA),
    ("inclinatum", INCLINATUM),
    ("auctum", AUCTUM),
    ("descendens", DESCENDENS),
    ("ascendens", ASCENDENS),
    ("oriscus", ORISCUS),
    ("quilisma", QUILISMA),
    ("deminutum", DEMINUTUM),
    ("cavum", CAVUM),
    ("linea", LINEA),
    ("pes-or-flexa", PES_OR_FLEXA),
];

const PRIMITIVE: &str = "ly:vaticana-ligature::brew-ligature-primitive";

pub static VATICANA_LIGATURE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Vaticana_ligature_engraver",
    kind: Kind::Engraver,
    methods: LIGATURE_METHODS,
    always: LIGATURE_ALWAYS,
    hears: &["ligature-event", "pes-or-flexa-event"],
    acknowledges: LIGATURE_ACKNOWLEDGES,
    must_be_last: false,
    make: Some(|| alloc::boxed::Box::new(LigatureEngraver::<Vaticana>::default())),
    reads: &[],
    writes: &[],
};

/// `Gregorian_ligature_engraver`'s `pes_or_flexa_req_` (and
/// `Vaticana_ligature_engraver`'s `augmented_primitives_`, which lives in
/// one `transform_heads` only).
#[derive(Clone, Default)]
pub(crate) struct Vaticana {
    pes_or_flexa: Option<Value>,
}

impl Coherent for Vaticana {
    const SPANNER: &'static str = "VaticanaLigature";
    const PRIMITIVE: Option<&'static str> = Some(PRIMITIVE);

    /// `Gregorian_ligature_engraver::build_ligature`.
    fn build_ligature(
        &mut self,
        tx: &mut Tx<'_>,
        ligature: Value,
        primitives: &[Value],
    ) -> Run<()> {
        // apply style-independent checking and transformation
        check_and_fix_all_prefixes(tx, primitives)?;
        provide_context_info(tx, primitives)?;

        // apply style-specific transformation (including line-up); to be
        // implemented by subclass
        transform_heads(tx, ligature, primitives)
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.pes_or_flexa, ev).map(drop)
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.pes_or_flexa = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.pes_or_flexa);
    }
}

/// `from_scm<std::string>`: the string `v`, Guile's error for another.
fn string(heap: &mut lily_engine_scheme::Heap, v: Value) -> Run<String> {
    match heap.str(v) {
        Some(s) => Ok(String::from(s.as_str())),
        None => Err(crate::wrong_type(heap, None, 0, v, "string")),
    }
}

fn get_int(tx: &mut Tx<'_>, g: Value, name: &str) -> Run<i32> {
    let v = grob::property(tx.host, g, name)?;
    to_int(tx.heap(), v)
}

fn set_int(tx: &mut Tx<'_>, g: Value, name: &str, n: i32) {
    let v = tx.heap().integer(i64::from(n));
    grob::set_property(tx.heap(), g, name, v);
}

fn set_bool(tx: &mut Tx<'_>, g: Value, name: &str, b: bool) {
    grob::set_property(tx.heap(), g, name, Value::bool(b));
}

fn set_string(tx: &mut Tx<'_>, g: Value, name: &str, s: &str) {
    let v = tx.heap().string(s);
    grob::set_property(tx.heap(), g, name, v);
}

fn set_real(tx: &mut Tx<'_>, g: Value, name: &str, x: f64) {
    let v = tx.heap().flonum(x);
    grob::set_property(tx.heap(), g, name, v);
}

// --- Gregorian_ligature_engraver ------------------------------------------------

/// `fix_prefix` (`:57–79`).
fn fix_prefix(
    tx: &mut Tx<'_>,
    name: &str,
    mask: i32,
    current_set: &mut i32,
    min_set: i32,
    max_set: i32,
    primitive: Value,
) -> Run<()> {
    let current = *current_set & mask != 0;
    let min = min_set & mask != 0;
    let max = max_set & mask != 0;
    if !max && min {
        return tx.programming_error("min_set > max_set");
    }
    if min && !current {
        grob::warning(tx.host, primitive, &alloc::format!("\\{name} ignored"))?;
        *current_set &= !mask;
    }
    if !max && current {
        grob::warning(
            tx.host,
            primitive,
            &alloc::format!("implied \\{name} added"),
        )?;
        *current_set |= mask;
    }
    Ok(())
}

/// `fix_prefix_set` (`:81–101`): `pes_or_flexa` is checked by `LINEA`'s
/// bit, as LilyPond's is.
fn fix_prefix_set(
    tx: &mut Tx<'_>,
    current_set: &mut i32,
    min_set: i32,
    max_set: i32,
    primitive: Value,
) -> Run<()> {
    for (name, mask) in [
        ("virga", VIRGA),
        ("stropha", STROPHA),
        ("inclinatum", INCLINATUM),
        ("auctum", AUCTUM),
        ("descendens", DESCENDENS),
        ("ascendens", ASCENDENS),
        ("oriscus", ORISCUS),
        ("quilisma", QUILISMA),
        ("deminutum", DEMINUTUM),
        ("cavum", CAVUM),
        ("linea", LINEA),
        ("pes_or_flexa", LINEA),
    ] {
        fix_prefix(tx, name, mask, current_set, min_set, max_set, primitive)?;
    }
    Ok(())
}

/// `check_and_fix_all_prefixes` (`:103–193`).
fn check_and_fix_all_prefixes(tx: &mut Tx<'_>, primitives: &[Value]) -> Run<()> {
    // Check for invalid head modifier combinations
    for &primitive in primitives {
        // compute head prefix set by inspecting primitive grob properties
        let mut prefix_set = 0;
        for (name, bit) in PREFIXES {
            let v = grob::property(tx.host, primitive, name)?;
            if v == Value::TRUE {
                prefix_set |= bit;
            }
        }
        let p = &mut prefix_set;

        // check: ascendens and descendens exclude each other; same with
        // auctum and deminutum
        if *p & DESCENDENS != 0 {
            let m = *p & !ASCENDENS;
            fix_prefix_set(tx, p, m, m, primitive)?;
        }
        if *p & AUCTUM != 0 {
            let m = *p & !DEMINUTUM;
            fix_prefix_set(tx, p, m, m, primitive)?;
        }

        // check: virga, quilisma and oriscus cannot be combined with any
        // other prefix, but may be part of a pes or flexa
        if *p & VIRGA != 0 {
            fix_prefix_set(tx, p, VIRGA, VIRGA | PES_OR_FLEXA, primitive)?;
        }
        if *p & QUILISMA != 0 {
            fix_prefix_set(tx, p, QUILISMA, QUILISMA | PES_OR_FLEXA, primitive)?;
        }
        if *p & ORISCUS != 0 {
            fix_prefix_set(tx, p, ORISCUS, ORISCUS | PES_OR_FLEXA, primitive)?;
        }

        // check: auctum is the only valid optional prefix for stropha
        if *p & STROPHA != 0 {
            fix_prefix_set(tx, p, STROPHA, STROPHA | AUCTUM, primitive)?;
        }

        // check: inclinatum may be prefixed with auctum or deminutum only
        if *p & INCLINATUM != 0 {
            let max = INCLINATUM | AUCTUM | DEMINUTUM;
            fix_prefix_set(tx, p, INCLINATUM, max, primitive)?;
        }
        // check: semivocalis (deminutum but not inclinatum) must occur in
        // combination with and only with pes or flexa
        else if *p & DEMINUTUM != 0 {
            let m = DEMINUTUM | PES_OR_FLEXA;
            fix_prefix_set(tx, p, m, m, primitive)?;
        }

        // check: cavum and linea (either or both) may be applied only
        // upon core punctum
        if *p & (CAVUM | LINEA) != 0 {
            fix_prefix_set(tx, p, 0, CAVUM | LINEA, primitive)?;
        }

        // all other combinations should be valid (unless I made a
        // mistake)

        set_int(tx, primitive, "prefix-set", prefix_set);
    }
    Ok(())
}

/// `provide_context_info` (`:198–234`): marks those heads that participate
/// in a pes or flexa.
fn provide_context_info(tx: &mut Tx<'_>, primitives: &[Value]) -> Run<()> {
    let mut prev_primitive: Option<Value> = None;
    let mut prev_prefix_set = 0;
    let mut prev_context_info = 0;
    let mut prev_pitch = 0;
    for (i, &primitive) in primitives.iter().enumerate() {
        let mut context_info = 0;
        let pitch = cause_steps(tx, primitive)?;
        let prefix_set = get_int(tx, primitive, "prefix-set")?;

        if prefix_set & PES_OR_FLEXA != 0 {
            if i == 0 {
                // ligature may not start with 2nd head of pes or flexa
                grob::warning(
                    tx.host,
                    primitive,
                    "cannot apply `\\~' on first head of ligature",
                )?;
            } else if pitch > prev_pitch {
                // pes
                prev_context_info |= PES_LOWER;
                context_info |= PES_UPPER;
            } else if pitch < prev_pitch {
                // flexa
                prev_context_info |= FLEXA_LEFT;
                context_info |= FLEXA_RIGHT;
            } else {
                grob::warning(
                    tx.host,
                    primitive,
                    "cannot apply `\\~' on heads with identical pitch",
                )?;
            }
        }
        if prev_prefix_set & DEMINUTUM != 0 {
            context_info |= AFTER_DEMINUTUM;
        }

        if let Some(prev) = prev_primitive {
            set_int(tx, prev, "context-info", prev_context_info);
        }
        prev_primitive = Some(primitive);
        prev_prefix_set = prefix_set;
        prev_context_info = context_info;
        prev_pitch = pitch;
    }
    if let Some(prev) = prev_primitive {
        set_int(tx, prev, "context-info", prev_context_info);
    }
    Ok(())
}

/// `Gregorian_ligature::prefixes_to_str` (`gregorian-ligature.cc:36–53`).
fn prefixes_to_str(tx: &mut Tx<'_>, primitive: Value) -> Run<String> {
    let v = grob::property(tx.host, primitive, "prefix-set")?;
    let prefix_set = to_int(tx.heap(), v)?;
    let mut s = String::new();
    for &(name, mask) in &PREFIXES[..11] {
        if prefix_set & mask != 0 {
            if !s.is_empty() {
                s.push_str(", ");
            }
            s.push_str(name);
        }
    }
    Ok(s)
}

// --- Vaticana_ligature_engraver ---------------------------------------------------

/// `is_stacked_head` (`:93–131`).
fn is_stacked_head(prefix_set: i32, context_info: i32) -> bool {
    // upper head of pes is stacked upon lower head of pes ...
    let mut is_stacked = context_info & PES_UPPER != 0;

    // ... unless this note starts a flexa
    if context_info & FLEXA_LEFT != 0 {
        is_stacked = false;
    }

    // ... or another pes
    if context_info & PES_LOWER != 0 {
        is_stacked = false;
    }

    // ... or the previous note is a semivocalis or inclinatum
    if context_info & AFTER_DEMINUTUM != 0 {
        is_stacked = false;
    }

    // auctum head is never stacked upon preceding note
    if prefix_set & AUCTUM != 0 {
        is_stacked = false;
    }

    // virga is never stacked upon preceding note
    if prefix_set & VIRGA != 0 {
        is_stacked = false;
    }

    // oriscus is never stacked upon preceding note
    if prefix_set & ORISCUS != 0 {
        is_stacked = false;
    }

    if prefix_set & DEMINUTUM != 0
        && prefix_set & INCLINATUM == 0
        && context_info & FLEXA_RIGHT != 0
    {
        is_stacked = true; // semivocalis head of deminutus form
    }

    is_stacked
}

/// `need_extra_horizontal_space` (`:139–175`): extra space is added at
/// most once between two heads.
fn need_extra_horizontal_space(
    prev_prefix_set: i32,
    prefix_set: i32,
    context_info: i32,
    delta_pitch: i32,
) -> bool {
    // After a virga, make an additional small space such that the
    // appendix on the right side of the head does not touch the
    // following head.
    prev_prefix_set & VIRGA != 0
        // Always start a series of inclinatum heads with an extra space.
        || (prefix_set & INCLINATUM != 0 && prev_prefix_set & INCLINATUM == 0)
        // Before a flexa (but not within a torculus), make an additional
        // small space such that the appendix on the left side of the
        // flexa does not touch the this head.
        || (context_info & FLEXA_LEFT != 0 && context_info & PES_UPPER == 0)
        // If there are two adjacent noteheads with the same pitch, add
        // additional small space between them, such that they do not
        // touch each other.
        || delta_pitch == 0
}

/// The X extent's length of the glyph `noteheads.s<name>` in the grob's
/// default font.
fn head_glyph_width(tx: &mut Tx<'_>, g: Value, name: &str) -> Run<f64> {
    let font = font_select::get_default_font(tx.host, g)?;
    let glyph = alloc::format!("noteheads.s{name}");
    Ok(font::find_by_name(tx.heap(), font, &glyph)
        .extent(X)
        .length())
}

/// `align_heads` (`:177–342`): each head moved onto the first's column,
/// its `head-x-offset` and `add-join` given. (The ligature's width it
/// returns is read by no caller.)
fn align_heads(tx: &mut Tx<'_>, primitives: &[Value], flexa_width: f64, thickness: f64) -> Run<()> {
    let Some(&first) = primitives.first() else {
        return tx.programming_error("Vaticana_ligature: empty ligature [ignored]");
    };

    // The paper column where we put the whole ligature into.
    let column = item::get_column(tx.heap(), first);

    let join_thickness = thickness * grob::layout_dimension(tx.host, first, "line-thickness")?;

    // Amount of extra space two put between some particular
    // configurations of adjacent heads.
    //
    // TODO: make this a property of primtive grobs.
    let extra_space = 4.0 * join_thickness;

    // Keep track of the total width of the ligature.
    let mut ligature_width = 0.0;

    let mut prev_primitive: Option<Value> = None;
    let mut prev_prefix_set = 0;
    for &primitive in primitives {
        let prefix_set = get_int(tx, primitive, "prefix-set")?;
        let context_info = get_int(tx, primitive, "context-info")?;

        // Get glyph_name, delta_pitch and context_info for this head.
        let glyph_name_scm = grob::property(tx.host, primitive, "glyph-name")?;
        if glyph_name_scm == Value::NULL {
            grob::programming_error(
                tx.host,
                primitive,
                "Vaticana_ligature: undefined glyph-name -> ignoring grob",
            )?;
            continue;
        }
        let glyph_name = string(tx.heap(), glyph_name_scm)?;

        let mut delta_pitch = 0;
        if let Some(prev) = prev_primitive {
            // urgh, need prev_primitive only here
            let delta_pitch_scm = grob::property(tx.host, prev, "delta-position")?;
            if delta_pitch_scm != Value::NULL {
                delta_pitch = to_int(tx.heap(), delta_pitch_scm)?;
            } else {
                grob::programming_error(
                    tx.host,
                    primitive,
                    "Vaticana_ligature: delta-position undefined -> ignoring grob",
                )?;
                continue;
            }
        }

        // Now determine width and x-offset of head.
        let (head_width, head_x_offset) = if context_info & STACKED_HEAD != 0 {
            // This head is stacked upon the previous one; hence, it does
            // not contribute to the total width of the ligature, and its
            // width is assumed to be 0.0.  Moreover, it is shifted to the
            // left by its width such that the right side of this and the
            // other head are horizontally aligned.
            let w = head_glyph_width(tx, primitive, &glyph_name)?;
            (0.0, join_thickness - w)
        } else if glyph_name == "flexa" || glyph_name.is_empty() {
            // This head represents either half of a flexa shape.  Hence,
            // it is assigned half the width of this shape.
            (0.5 * flexa_width, 0.0)
        } else {
            // This is a regular head, placed right to the previous one.
            // Retrieve its width from corresponding font.
            (head_glyph_width(tx, primitive, &glyph_name)?, 0.0)
        };

        // Save the head's final x-offset.
        set_real(tx, primitive, "head-x-offset", head_x_offset);

        // If the head is the 2nd head of a pes or flexa (but not a flexa
        // shape), mark this head to be joined with the left-side neighbour
        // head (i.e. the previous head) by a vertical beam.
        if context_info & PES_UPPER != 0
            || (context_info & FLEXA_RIGHT != 0 && context_info & PES_LOWER == 0)
        {
            match prev_primitive {
                None => grob::programming_error(
                    tx.host,
                    primitive,
                    "Vaticana ligature: add-join: missing previous primitive",
                )?,
                Some(prev) => {
                    set_bool(tx, prev, "add-join", true);

                    // Create a small overlap of adjacent heads so that the
                    // join can be drawn perfectly between them.
                    ligature_width -= join_thickness;
                }
            }
        }
        // else if glyph_name == "": the 2nd (virtual) head of flexa
        // shape, joined tightly with 1st head, i.e. no additional space,
        // such that next head will not be off from the flexa shape.

        if need_extra_horizontal_space(prev_prefix_set, prefix_set, context_info, delta_pitch) {
            ligature_width += extra_space;
        }

        // Horizontally line-up this head to form a ligature.
        if let Some(column) = column {
            move_related_items_to_column(tx, primitive, column, ligature_width)?;
        }
        ligature_width += head_width;

        prev_primitive = Some(primitive);
        prev_prefix_set = prefix_set;
    }
    Ok(())
}

/// `check_for_prefix_loss` (`:357–383`): a warning for a head of a curved
/// flexa prefixed so that it would be other than a punctum alone.
fn check_for_prefix_loss(tx: &mut Tx<'_>, primitive: Value) -> Run<()> {
    let prefix_set = get_int(tx, primitive, "prefix-set")?;
    if prefix_set & !PES_OR_FLEXA != 0 {
        let prefs = prefixes_to_str(tx, primitive)?;
        let text = alloc::format!(
            "ignored prefix(es) `{prefs}' of this head according to restrictions of the selected ligature style"
        );
        grob::warning(tx.host, primitive, &text)?;
    }
    Ok(())
}

/// `add_mora_column` (`:385–412`): a DotColumn on `column`, a dot of its
/// own for each dotted head.
fn add_mora_column(tx: &mut Tx<'_>, augmented: &[Value], column: Option<Value>) -> Run<()> {
    let Some(&first) = augmented.first() else {
        // no dot for column
        return Ok(());
    };
    let Some(column) = column else {
        // empty ligature???
        return grob::programming_error(tx.host, first, "no paper column to add dot");
    };
    let id = tx.make_defined_grob("DotColumn", Cause::None)?;
    let dotcol = tx.grob_handle(id);
    grob::set_parent(tx.heap(), dotcol, column, X);
    for &primitive in augmented {
        let cause = match grob_of(tx.heap(), primitive) {
            Some((_, id)) => Cause::Grob(id),
            None => Cause::None,
        };
        let id = tx.make_defined_grob("Dots", cause)?;
        let dot = tx.grob_handle(id);
        let one = tx.heap().integer(1);
        grob::set_property(tx.heap(), dot, "dot-count", one);
        grob::set_parent(tx.heap(), dot, primitive, Y);
        let sym = tx.heap().intern("dot");
        grob::set_object(tx.heap(), primitive, sym, dot);
        dot_column::add_head(tx.host, dotcol, primitive)?;

        // FIXME: why isn't the dot picked up by Paper_column_engraver?
        separation_item::add_item(tx.host, column, dot)?;
    }
    Ok(())
}

/// `check_for_ambiguous_dot_pitch` (`:428–451`): a warning for a dotted
/// head of the pitch of one before it.
fn check_for_ambiguous_dot_pitch(
    tx: &mut Tx<'_>,
    augmented: &[Value],
    primitive: Value,
) -> Run<()> {
    let new_pitch = cause_steps(tx, primitive)?;
    for &other in augmented {
        if cause_steps(tx, other)? == new_pitch {
            return grob::warning(
                tx.host,
                primitive,
                "Ambiguous use of dots in ligature: there are multiple dotted notes with the same pitch.  The ligature should be split.",
            ); // supress multiple identical warnings
        }
    }
    Ok(())
}

/// `transform_heads` (`:453–691`).
fn transform_heads(tx: &mut Tx<'_>, ligature: Value, primitives: &[Value]) -> Run<()> {
    let v = grob::property(tx.host, ligature, "flexa-width")?;
    let flexa_width = double_or(tx.host, v, 2.0);
    let v = grob::property(tx.host, ligature, "thickness")?;
    let thickness = double_or(tx.host, v, 1.0);

    let mut prev_primitive: Option<Value> = None;
    let mut prev_prefix_set = 0;
    let mut prev_context_info = 0;
    let mut prev_delta_pitch = 0;
    let mut prev_glyph_name = String::new();
    let mut augmented: Vec<Value> = Vec::new();
    for (i, &primitive) in primitives.iter().enumerate() {
        let delta_pitch_scm = grob::property(tx.host, primitive, "delta-position")?;
        if delta_pitch_scm == Value::NULL {
            grob::programming_error(
                tx.host,
                primitive,
                "Vaticana_ligature: delta-position undefined -> ignoring grob",
            )?;
            continue;
        }
        let delta_pitch = to_int(tx.heap(), delta_pitch_scm)?;

        // retrieve & complete prefix_set and context_info
        let prefix_set = get_int(tx, primitive, "prefix-set")?;
        let mut context_info = get_int(tx, primitive, "context-info")?;

        if rhythmic_head::dot_count(tx.host, primitive)? > 0 {
            // remove dots from primitive and add remember primitive for
            // creating a dot column
            if let Some(dots) = rhythmic_head::get_dots(tx.host, primitive)? {
                set_int(tx, dots, "dot-count", 0);
            }
            // TODO: Maybe completely remove grob "Dots" (dots->suicide ()
            // ?) rather than setting property "dot-count" to 0.

            check_for_ambiguous_dot_pitch(tx, &augmented, primitive)?;
            augmented.push(primitive);
        } else if !augmented.is_empty() {
            grob::warning(
                tx.host,
                primitive,
                "This ligature has a dotted head followed by a non-dotted head.  The ligature should be split after the last dotted head before this head.",
            )?;
        }

        if is_stacked_head(prefix_set, context_info) {
            context_info |= STACKED_HEAD;
            set_int(tx, primitive, "context-info", context_info);
        }

        // Now determine which head to typeset (this is context sensitive
        // information, since it depends on neighbouring heads; therefore,
        // this decision must be made here in the engraver rather than in
        // the backend).
        let mut glyph_name: &str = if prefix_set & VIRGA != 0 {
            set_bool(tx, primitive, "add-stem", true);
            "vaticana.punctum"
        } else if prefix_set & QUILISMA != 0 {
            "vaticana.quilisma"
        } else if prefix_set & ORISCUS != 0 {
            "solesmes.oriscus"
        } else if prefix_set & STROPHA != 0 {
            if prefix_set & AUCTUM != 0 {
                "solesmes.stropha.aucta"
            } else {
                "solesmes.stropha"
            }
        } else if prefix_set & INCLINATUM != 0 {
            if prefix_set & AUCTUM != 0 {
                "solesmes.incl.auctum"
            } else if prefix_set & DEMINUTUM != 0 {
                "solesmes.incl.parvum"
            } else {
                "vaticana.inclinatum"
            }
        } else if prefix_set & DEMINUTUM != 0 {
            if i == 0 {
                // initio debilis
                "vaticana.reverse.plica"
            } else if prev_delta_pitch > 0 {
                // epiphonus
                if prev_context_info & FLEXA_RIGHT == 0 {
                    // correct head of previous primitive
                    prev_glyph_name = String::from(if prev_delta_pitch > 1 {
                        "vaticana.epiphonus"
                    } else {
                        "vaticana.vepiphonus"
                    });
                }
                if prev_delta_pitch > 1 {
                    "vaticana.plica"
                } else {
                    "vaticana.vplica"
                }
            } else if prev_delta_pitch < 0 {
                // cephalicus
                if prev_context_info & FLEXA_RIGHT == 0 {
                    // correct head of previous primitive
                    prev_glyph_name = String::from(if i > 1 {
                        // cephalicus head with fixed size cauda
                        "vaticana.inner.cephalicus"
                    } else {
                        // cephalicus head without cauda
                        "vaticana.cephalicus"
                    });

                    // Flexa has no variable size cauda if its left head is
                    // stacked on the right head.  This is true for
                    // cephalicus.  Hence, remove the cauda.
                    //
                    // Urgh: for the current implementation, this rule only
                    // applies for cephalicus; but it is a fundamental
                    // rule.  Therefore, the following line of code should
                    // be placed somewhere else.
                    if let Some(prev) = prev_primitive {
                        set_bool(tx, prev, "add-cauda", false);
                    }
                }
                if prev_delta_pitch < -1 {
                    "vaticana.reverse.plica"
                } else {
                    "vaticana.reverse.vplica"
                }
            } else {
                // (prev_delta_pitch == 0)
                grob::programming_error(
                    tx.host,
                    primitive,
                    "Vaticana_ligature: deminutum head must have different pitch -> ignoring grob",
                )?;
                ""
            }
        } else if prefix_set & (CAVUM | LINEA) != 0 {
            if prefix_set & CAVUM != 0 && prefix_set & LINEA != 0 {
                "vaticana.linea.punctum.cavum"
            } else if prefix_set & CAVUM != 0 {
                "vaticana.punctum.cavum"
            } else {
                "vaticana.linea.punctum"
            }
        } else if prefix_set & AUCTUM != 0 {
            if prefix_set & ASCENDENS != 0 {
                "solesmes.auct.asc"
            } else {
                "solesmes.auct.desc"
            }
        } else if context_info & STACKED_HEAD != 0 && context_info & PES_UPPER != 0 {
            if prev_delta_pitch > 1 {
                "vaticana.upes"
            } else {
                "vaticana.vupes"
            }
        } else {
            "vaticana.punctum"
        };

        // This head needs a cauda, if it starts a flexa, is not the upper
        // head of a pes, and if it is a punctum.
        if context_info & FLEXA_LEFT != 0
            && context_info & PES_UPPER == 0
            && glyph_name == "vaticana.punctum"
        {
            set_bool(tx, primitive, "add-cauda", true);
        }

        // Execptional rule for porrectus:
        //
        // If the current head is preceded by a \flexa and succeded by a
        // \pes (e.g. "a \flexa g \pes a"), then join the current head and
        // the previous head into a single curved flexa shape.
        if context_info & FLEXA_RIGHT != 0 && context_info & PES_LOWER != 0 {
            if let Some(prev) = prev_primitive {
                check_for_prefix_loss(tx, prev)?;
                prev_glyph_name = String::from("flexa");
                set_int(tx, prev, "flexa-height", prev_delta_pitch);
                set_real(tx, prev, "flexa-width", flexa_width);
                let add_cauda = prev_prefix_set & PES_OR_FLEXA == 0;
                set_bool(tx, prev, "add-cauda", add_cauda);
            }
            check_for_prefix_loss(tx, primitive)?;
            glyph_name = "";
            set_real(tx, primitive, "flexa-width", flexa_width);
        }

        // Exceptional rule for pes:
        //
        // If this head is stacked on the previous one due to a \pes, then
        // set the glyph of the previous head to that for this special
        // case, thereby avoiding potential vertical collision with the
        // current head.
        if prefix_set & PES_OR_FLEXA != 0
            && context_info & PES_UPPER != 0
            && context_info & STACKED_HEAD != 0
            && prev_glyph_name == "vaticana.punctum"
        {
            prev_glyph_name = String::from(if prev_delta_pitch > 1 {
                "vaticana.lpes"
            } else {
                "vaticana.vlpes"
            });
        }

        if let Some(prev) = prev_primitive {
            set_string(tx, prev, "glyph-name", &prev_glyph_name);
        }

        // In the backend, flexa shapes and joins need to know about line
        // thickness.  Hence, for simplicity, let's distribute the
        // ligature grob's value for thickness to each ligature head (even
        // if not all of them need to know).
        set_real(tx, primitive, "thickness", thickness);

        prev_primitive = Some(primitive);
        prev_prefix_set = prefix_set;
        prev_context_info = context_info;
        prev_delta_pitch = delta_pitch;
        prev_glyph_name = String::from(glyph_name);
    }

    // LilyPond reads through a null where every head was ignored.
    let Some(last) = prev_primitive else {
        return Ok(());
    };
    set_string(tx, last, "glyph-name", &prev_glyph_name);

    align_heads(tx, primitives, flexa_width, thickness)?;

    // append all dots to paper column of ligature's last head
    let column = item::get_column(tx.heap(), last);
    add_mora_column(tx, &augmented, column)
}

// --- Vaticana_ligature's callbacks -------------------------------------------------

fn sign(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

fn blank(host: &mut dyn Host) -> Stencil {
    let i = Interval::new(0.0, 0.0);
    lookup::blank(host.heap(), Box::new(i, i))
}

/// `vaticana_brew_cauda` (`:31–71`).
fn brew_cauda(
    host: &mut dyn Host,
    me: Value,
    pos: i32,
    mut delta_pitch: i32,
    thickness: f64,
    blotdiameter: f64,
    warnings: &mut Vec<String>,
) -> Run<Stencil> {
    let on_staffline = ssr::on_line(host, me, pos)?;
    let above_staff = f64::from(pos) > ssr::staff_span(host, me)?.right;

    if delta_pitch > -1 {
        grob::programming_error(host, me, "flexa cauda: invalid delta_pitch; assuming -1")?;
        delta_pitch = -1;
    }
    let length = if on_staffline {
        if delta_pitch >= -1 {
            1.30
        } else if delta_pitch >= -2 {
            1.35
        } else {
            1.85
        }
    } else if delta_pitch >= -1 {
        if above_staff { 1.30 } else { 1.00 }
    } else if delta_pitch >= -2 {
        1.35
    } else if delta_pitch >= -3 {
        1.50
    } else {
        1.85
    };
    let cauda_box = Box::new(Interval::new(0.0, thickness), Interval::new(-length, 0.0));
    Ok(lookup::round_filled_box(
        host.heap(),
        cauda_box,
        blotdiameter,
        warnings,
    ))
}

/// `vaticana_brew_flexa` (`:77–162`), its expressions rooted.
fn brew_flexa(host: &mut dyn Host, me: Value, solid: bool, line_thickness: f64) -> Run<Stencil> {
    let staff_space = ssr::staff_space(host, me)?;
    let right_height = 0.6 * staff_space;

    let flexa_height_scm = grob::property(host, me, "flexa-height")?;
    let interval = if flexa_height_scm != Value::NULL {
        f64::from(to_int(host.heap(), flexa_height_scm)?)
    } else {
        grob::warning(
            host,
            me,
            "Vaticana_ligature: flexa-height undefined; assuming 0",
        )?;
        0.0
    };

    if interval >= 0.0 {
        grob::warning(host, me, "ascending vaticana style flexa")?;
    }

    let v = grob::property(host, me, "flexa-width")?;
    let width = double_or(host, v, 2.0);

    // Compensate curve thickness that appears to be smaller in steep
    // section of bend.
    let left_height = right_height + f64::min(0.12 * interval.abs(), 0.3) * staff_space;

    // Compensate optical illusion regarding vertical position of left and
    // right endings due to curved shape.
    let ypos_correction = -0.1 * staff_space * sign(interval);
    let interval_correction = 0.2 * staff_space * sign(interval);
    let corrected_interval = interval * staff_space + interval_correction;

    // middle curve of flexa shape
    let curve: Bezier = [
        (0.00 * width, 0.0),
        (0.33 * width, corrected_interval / 2.0),
        (0.66 * width, corrected_interval / 2.0),
        (1.00 * width, corrected_interval / 2.0),
    ];

    let (mut top_curve, mut bottom_curve) = (curve, curve);
    for i in 0..4 {
        let fi = i as f64;
        let curve_thickness = 0.33 * ((3.0 - fi) * left_height + fi * right_height);
        top_curve[i].1 += 0.5 * curve_thickness;
        bottom_curve[i].1 -= 0.5 * curve_thickness;
    }

    let heap = host.heap();
    let mut stencil = Stencil::new(Box::EMPTY, Value::NULL);
    if solid {
        let solid_head = lookup::bezier_sandwich(heap, top_curve, bottom_curve, 0.0);
        heap.root(solid_head.expr);
        stencil.add_stencil(heap, &solid_head);
        heap.root(stencil.expr);
    } else {
        // outline
        let mut inner_top_curve = top_curve;
        crate::bezier::translate(&mut inner_top_curve, (0.0, -line_thickness));
        let top_edge = lookup::bezier_sandwich(heap, top_curve, inner_top_curve, 0.0);
        heap.root(top_edge.expr);
        stencil.add_stencil(heap, &top_edge);
        heap.root(stencil.expr);

        let mut inner_bottom_curve = bottom_curve;
        crate::bezier::translate(&mut inner_bottom_curve, (0.0, line_thickness));
        let bottom_edge = lookup::bezier_sandwich(heap, bottom_curve, inner_bottom_curve, 0.0);
        heap.root(bottom_edge.expr);
        stencil.add_stencil(heap, &bottom_edge);
        heap.root(stencil.expr);

        // TODO: Use horizontal slope with proper slope value rather than
        // filled box for left edge, since the filled box stands out from
        // the flexa shape if the interval is big and the line thickness
        // small.  The difficulty here is to compute a proper slope value,
        // as it should roughly be equal with the slope of the left end of
        // the bezier curve.
        let mut warnings = Vec::new();
        let left_edge_box = Box::new(
            Interval::new(0.0, line_thickness),
            Interval::new(-0.5 * left_height, 0.5 * left_height),
        );
        let left_edge = lookup::filled_box(heap, left_edge_box, &mut warnings);
        heap.root(left_edge.expr);
        stencil.add_stencil(heap, &left_edge);
        heap.root(stencil.expr);

        let right_edge_box = Box::new(
            Interval::new(-line_thickness, 0.0),
            Interval::new(-0.5 * right_height, 0.5 * right_height),
        );
        let mut right_edge = lookup::filled_box(heap, right_edge_box, &mut warnings);
        heap.root(right_edge.expr);
        right_edge.translate_axis(heap, width, X);
        heap.root(right_edge.expr);
        right_edge.translate_axis(heap, corrected_interval / 2.0, Y);
        heap.root(right_edge.expr);
        stencil.add_stencil(heap, &right_edge);
        heap.root(stencil.expr);
        for text in &warnings {
            grob::unlocated_warning(host, text)?;
        }
    }
    let heap = host.heap();
    stencil.translate_axis(heap, ypos_correction, Y);
    heap.root(stencil.expr);
    Ok(stencil)
}

/// `vaticana_brew_join` (`:164–175`).
fn brew_join(
    host: &mut dyn Host,
    me: Value,
    delta_pitch: i32,
    join_thickness: f64,
    blotdiameter: f64,
    warnings: &mut Vec<String>,
) -> Run<Stencil> {
    let staff_space = ssr::staff_space(host, me)?;
    if delta_pitch == 0 {
        grob::programming_error(host, me, "Vaticana_ligature: zero join (delta_pitch == 0)")?;
        return Ok(blank(host));
    }
    let x_extent = Interval::new(0.0, join_thickness);
    let d = f64::from(delta_pitch) * 0.5 * staff_space;
    let y_extent = if delta_pitch > 0 {
        Interval::new(0.0, d) // ascending join
    } else {
        Interval::new(d, 0.0) // descending join
    };
    let join_box = Box::new(x_extent, y_extent);
    Ok(lookup::round_filled_box(
        host.heap(),
        join_box,
        blotdiameter,
        warnings,
    ))
}

/// `vaticana_brew_primitive` (`:177–270`), its expressions rooted.
fn brew_primitive(host: &mut dyn Host, me: Value) -> Run<Stencil> {
    let glyph_name_scm = grob::property(host, me, "glyph-name")?;
    if glyph_name_scm == Value::NULL {
        grob::programming_error(
            host,
            me,
            "Vaticana_ligature: undefined glyph-name -> ignoring grob",
        )?;
        return Ok(blank(host));
    }

    let glyph_name = string(host.heap(), glyph_name_scm)?;

    let v = grob::property(host, me, "thickness")?;
    let thickness = double_or(host, v, 1.0);

    let line_thickness = thickness * grob::layout_dimension(host, me, "line-thickness")?;

    let blotdiameter = grob::layout_dimension(host, me, "blot-diameter")?;

    let pos = ssr::rounded_position(host, me, false)?;
    let pos = to_int(host.heap(), pos)?;

    let delta_pitch_scm = grob::property(host, me, "delta-position")?;
    let delta_pitch = if delta_pitch_scm != Value::NULL {
        to_int(host.heap(), delta_pitch_scm)?
    } else {
        0
    };

    let v = grob::property(host, me, "head-x-offset")?;
    let head_x_offset = double_or(host, v, 0.0);

    let add_stem = grob::property(host, me, "add-stem")? == Value::TRUE;
    let add_cauda = grob::property(host, me, "add-cauda")? == Value::TRUE;
    let add_join = grob::property(host, me, "add-join")? == Value::TRUE;

    let mut out = if glyph_name.is_empty() {
        // This is an empty head.  This typically applies for the right
        // side of a curved flexa shape, which is already typeset by the
        // associated left side head.  The only possible thing left to do
        // is to draw a vertical join to the next head.  (Urgh: need
        // flexa_width.)
        let staff_space = ssr::staff_space(host, me)?;
        let v = grob::property(host, me, "flexa-width")?;
        let flexa_width = double_or(host, v, 2.0) * staff_space;
        lookup::blank(
            host.heap(),
            Box::new(
                Interval::new(0.0, 0.5 * flexa_width),
                Interval::new(0.0, 0.0),
            ),
        )
    } else if glyph_name == "flexa" {
        brew_flexa(host, me, true, line_thickness)?
    } else {
        let font = font_select::get_default_font(host, me)?;
        let name = alloc::format!("noteheads.s{glyph_name}");
        font::find_by_name(host.heap(), font, &name)
    };
    let heap = host.heap();
    heap.root(out.expr);
    out.translate_axis(heap, head_x_offset, X);
    heap.root(out.expr);
    let head_width = out.extent(X).length();

    let mut warnings = Vec::new();
    if add_cauda {
        let cauda = brew_cauda(
            host,
            me,
            pos,
            delta_pitch,
            line_thickness,
            blotdiameter,
            &mut warnings,
        )?;
        let heap = host.heap();
        heap.root(cauda.expr);
        out.add_stencil(heap, &cauda);
        heap.root(out.expr);
    }

    if add_stem {
        let mut stem = brew_cauda(
            host,
            me,
            pos,
            -1,
            line_thickness,
            blotdiameter,
            &mut warnings,
        )?;
        let heap = host.heap();
        heap.root(stem.expr);
        stem.translate_axis(heap, head_width - line_thickness, X);
        heap.root(stem.expr);
        out.add_stencil(heap, &stem);
        heap.root(out.expr);
    }

    if add_join {
        let mut join = brew_join(
            host,
            me,
            delta_pitch,
            line_thickness,
            blotdiameter,
            &mut warnings,
        )?;
        let heap = host.heap();
        heap.root(join.expr);
        join.translate_axis(heap, head_width - line_thickness, X);
        heap.root(join.expr);
        out.add_stencil(heap, &join);
        heap.root(out.expr);
    }
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }

    Ok(out)
}

/// `Vaticana_ligature::brew_ligature_primitive`, as above.
pub fn brew_ligature_primitive(host: &mut dyn Host, me: Value) -> Run<Value> {
    let depth = host.heap().root_depth();
    let brewed = brew_primitive(host, me).map(|s| host.heap().wrap(s));
    host.heap().unroot_to(depth);
    brewed
}

/// `Vaticana_ligature::print`: `'()`.
pub fn print(_: &mut dyn Host, _: Value) -> Run<Value> {
    Ok(Value::NULL)
}

/// What `%host-call` runs for the `ly:vaticana-ligature::` callbacks
/// (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "vaticana-ligature",
    f: host_vaticana_ligature,
}];

/// The `ly:vaticana-ligature::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("brew-ligature-primitive", brew_ligature_primitive),
    ("print", print),
];

fn host_vaticana_ligature(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such vaticana-ligature callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_brew_ligature_primitive(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "vaticana-ligature",
        "ly:vaticana-ligature::",
        "brew-ligature-primitive",
        args,
    )
}

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "vaticana-ligature",
        "ly:vaticana-ligature::",
        "print",
        args,
    )
}
