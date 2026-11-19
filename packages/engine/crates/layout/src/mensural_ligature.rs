//! `mensural-ligature-engraver.cc` and `mensural-ligature.cc` (MS8 step 9,
//! part 1; 562 and 318 lines): `Mensural_ligature_engraver`, a class of
//! layout's ([`MENSURAL_LIGATURE_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108), a [`LigatureEngraver`] over
//! [`Mensural`]; and `ly:mensural-ligature::brew-ligature-primitive` and
//! `ly:mensural-ligature::print`, Rust under primitives that print as
//! LilyPond's subrs and call it over `%host-call` (D127, [`HOSTS`]).
//!
//! - `build_ligature` (`:478–496`): the heads' shapes chosen
//!   (`transform_heads`, `:86–347`: each head's `primitive`, the `MLP_`
//!   bits of `mensural-ligature.hh`, by its duration, its neighbours' pitches
//!   and its `ligature-flexa`, `left-down-stem`, `right-down-stem` and
//!   `right-up-stem`; a flexa made of two heads; `add-join` where a head
//!   joins the next; LilyPond's warnings for what cannot be a ligature),
//!   the ligature's `thickness` and the heads' widths given to them
//!   (`propagate_properties`, `:363–420`), and the heads moved onto the
//!   first's column one after the other, their dots after them
//!   (`fold_up_primitives`, `:422–476`); the ligature's `minimum-length`
//!   at least the heads' width.
//! - `brew-ligature-primitive` (`internal_brew_primitive`, `:120–295`): a
//!   head's glyph of its shape and colour (`style`), or half a flexa
//!   (`brew_flexa`, `:39–118`), with its left stem and its right join or
//!   stem as boxes. `print` gives `'()`: the heads draw the ligature.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, Kind, TranslatorClass, Tx};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Interval, X, Y};
use crate::ligature::{
    Coherent, LIGATURE_ACKNOWLEDGES, LIGATURE_ALWAYS, LIGATURE_METHODS, LigatureEngraver,
    cause_steps, move_related_items_to_column, translate,
};
use crate::spacing_options::double_or;
use crate::stem::{int_or, to_int};
use crate::stencil::Stencil;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::{font, font_select, grob, item, lookup, rhythmic_head, staff_symbol_referencer};

type Run<T> = Result<T, Exception>;

// `mensural-ligature.hh`'s primitives.
const MLP_NONE: i32 = 0x00;
const MLP_UP: i32 = 0x01;
const MLP_DOWN: i32 = 0x02;
const MLP_JOIN_UP: i32 = 0x04;
const MLP_JOIN_DOWN: i32 = 0x08;
const MLP_BREVIS: i32 = 0x10;
const MLP_LONGA: i32 = 0x20;
const MLP_MAXIMA: i32 = 0x40;
const MLP_FLEXA_BEGIN: i32 = 0x80;
const MLP_FLEXA_END: i32 = 0x100;
const MLP_STEM: i32 = MLP_UP | MLP_DOWN;
const MLP_RIGHT_STEM: i32 = MLP_JOIN_UP | MLP_JOIN_DOWN;
const MLP_SINGLE_HEAD: i32 = MLP_BREVIS | MLP_LONGA | MLP_MAXIMA;
const MLP_FLEXA: i32 = MLP_FLEXA_BEGIN | MLP_FLEXA_END;
const MLP_ANY: i32 = MLP_FLEXA | MLP_SINGLE_HEAD;

const PRIMITIVE: &str = "ly:mensural-ligature::brew-ligature-primitive";

pub static MENSURAL_LIGATURE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Mensural_ligature_engraver",
    kind: Kind::Engraver,
    methods: LIGATURE_METHODS,
    always: LIGATURE_ALWAYS,
    hears: &["ligature-event"],
    acknowledges: LIGATURE_ACKNOWLEDGES,
    must_be_last: false,
    make: Some(|| alloc::boxed::Box::new(LigatureEngraver::<Mensural>::default())),
    reads: &[],
    writes: &[],
};

/// `Mensural_ligature_engraver`'s own part: it keeps nothing.
#[derive(Clone, Default)]
pub(crate) struct Mensural;

impl Coherent for Mensural {
    const SPANNER: &'static str = "MensuralLigature";
    const PRIMITIVE: Option<&'static str> = Some(PRIMITIVE);

    fn build_ligature(
        &mut self,
        tx: &mut Tx<'_>,
        ligature: Value,
        primitives: &[Value],
    ) -> Run<()> {
        // the X extent of the actual graphics representing the ligature;
        // less space than that means collision
        transform_heads(tx, primitives)?;
        let mut min_length = propagate_properties(tx, ligature, primitives)?;
        fold_up_primitives(tx, primitives, &mut min_length)?;

        let v = grob::property(tx.host, ligature, "minimum-length")?;
        if double_or(tx.host, v, 0.0) < min_length {
            let v = tx.heap().flonum(min_length);
            grob::set_property(tx.heap(), ligature, "minimum-length", v);
        }
        Ok(())
    }
}

fn flag(tx: &mut Tx<'_>, g: Value, name: &str) -> Run<bool> {
    Ok(grob::property(tx.host, g, name)? == Value::TRUE)
}

fn set_int(tx: &mut Tx<'_>, g: Value, name: &str, n: i32) {
    let v = tx.heap().integer(i64::from(n));
    grob::set_property(tx.heap(), g, name, v);
}

fn get_int(tx: &mut Tx<'_>, g: Value, name: &str) -> Run<i32> {
    let v = grob::property(tx.host, g, name)?;
    to_int(tx.heap(), v)
}

/// `transform_heads` (`:86–347`).
fn transform_heads(tx: &mut Tx<'_>, primitives: &[Value]) -> Run<()> {
    if primitives.len() < 2 {
        return tx.warning("ligature with less than 2 heads; skipping");
    }
    let mut prev_pitch = 0;
    let mut at_beginning = true;

    // needed so that we can check whether
    // the previous note can be turned into a flexa
    let mut prev_brevis_shape = false;

    let mut prev_semibrevis = false;
    let mut prev_primitive: Option<Value> = None;

    let s = primitives.len();
    for i in 0..s {
        let primitive = primitives[i];
        let duration_log = rhythmic_head::duration_log(tx.host, primitive)?;

        let nr = event_cause(tx, primitive)?;

        // ugh. why not simply check for pitch?
        let Some(nr) = nr.filter(|&nr| in_event_class(tx.heap(), nr, "note-event")) else {
            let text = "cannot determine pitch of ligature primitive; skipping";
            match nr {
                Some(nr) => tx.event_warning(nr, text)?,
                None => tx.warning(text)?,
            }
            at_beginning = true;
            continue;
        };

        let pitch = cause_steps(tx, primitive)?;
        let mut prim = 0;
        let is_last = i == s - 1;

        if at_beginning {
            if is_last {
                // we can get here after invalid input
                tx.event_warning(nr, "single note ligature; skipping")?;
                break;
            }
            prev_semibrevis = false;
            prev_brevis_shape = false;
            prev_primitive = None;
        } else if pitch == prev_pitch {
            tx.event_warning(nr, "prime interval within ligature; skipping")?;
            at_beginning = true;
            continue;
        }

        if !(-3..=0).contains(&duration_log) {
            tx.event_warning(
                nr,
                "mensural ligature: duration none of maxima, longa, breve, or semibreve; skipping",
            )?;
            at_beginning = true;
            continue;
        }

        let mut general_case = true;
        let mut make_flexa = false;
        let mut allow_flexa = true;
        let is_brevis = duration_log == -1;

        // first check special cases
        // 1. initial longa or brevis
        if at_beginning && 0 > duration_log && duration_log > -3 {
            // descendens
            if cause_steps(tx, primitives[i + 1])? < pitch
                || (is_brevis && flag(tx, primitive, "left-down-stem")?)
            {
                let left_stem = if is_brevis { MLP_DOWN } else { 0 };
                prim = left_stem | MLP_BREVIS;
                general_case = false;
            }
        }
        // 2. semibrevis must not be followed by a brevis
        //    (theoretical sources require semibrevis, but
        //     longa and maxima can be denoted unambiguously,
        //     and in extremely rare cases these are used,
        //     see e.g. Wylkynson's five part Salve regina
        //     in the Eton Choirbook: fol. i2, Bassus part at words Et Jesum)
        else if prev_semibrevis && duration_log > -2 {
            prev_semibrevis = false;
            if duration_log == 0 {
                prim = MLP_BREVIS;
                general_case = false;
            } else {
                tx.event_warning(nr, "semibrevis must not be followed by a brevis; skipping")?;
                at_beginning = true;
                continue;
            }
        }
        // 3. semibreves are denoted by an upward tail on the left
        //    (theoretical sources require them at the beginning, but
        //     an upward tail is not used for anything else in the middle,
        //     and in extremely rare cases it's used to denote semibreves,
        //     see e.g. Fayrfax's Aeternae laudis lilium
        //     in the Lambeth Choirbook: fol. 58v, start of the fourth line)
        else if duration_log == 0 {
            prim = MLP_UP | MLP_BREVIS;
            general_case = false;
        }
        // 4. end, descendens
        else if is_last && pitch < prev_pitch {
            // brevis; previous note must be turned into flexa
            if is_brevis {
                if prev_brevis_shape {
                    make_flexa = true;
                    general_case = false;
                } else {
                    tx.event_warning(
                        nr,
                        "invalid ligature ending:\n\
                         when the last note is a descending brevis,\n\
                         the penultimate note must be another one,\n\
                         or the ligature must be longa-brevis or\n\
                         semibrevis-semibrevis-brevis",
                    )?;
                    break;
                }
            }
            // longa
            else if duration_log == -2
                && !prev_semibrevis
                && !flag(tx, primitive, "right-down-stem")?
            {
                prim = MLP_BREVIS;
                general_case = false;
                allow_flexa = false;
                if flag(tx, primitive, "right-up-stem")? {
                    prim |= MLP_JOIN_UP;
                }
            }
            // else maxima (or longa-after-semibrevis);
            // fall through to regular case below
        }

        if allow_flexa && flag(tx, primitive, "ligature-flexa")? {
            // flexa requested, check whether allowed:
            // - there should be a previous note
            // - both of the notes must be of brevis shape
            //   (i.e. can't be maxima or flexa;
            //   longa is forbidden as well - it's nonexistent anyway)
            // - there mustn't be a stem here
            // - no compulsory flexa for the next note,
            //   i.e. it's not an ultimate descending breve
            make_flexa =
                !at_beginning && prev_brevis_shape && duration_log > -2 && prim & MLP_STEM == 0;
            if make_flexa && i + 2 == s {
                // check last condition: look ahead to next note
                let next_primitive = primitives[i + 1];
                if rhythmic_head::duration_log(tx.host, next_primitive)? == -1 {
                    // breve: check whether descending
                    let next_pitch = cause_steps(tx, next_primitive)?;
                    if next_pitch < pitch {
                        // sorry, forbidden
                        make_flexa = false;
                    }
                }
            }
        }

        if general_case {
            const SHAPE: [i32; 3] = [MLP_MAXIMA, MLP_LONGA, MLP_BREVIS];

            prim = SHAPE[(duration_log + 3) as usize];
            if duration_log == -3 {
                if flag(tx, primitive, "right-down-stem")? {
                    prim |= MLP_JOIN_DOWN;
                } else if is_last && flag(tx, primitive, "right-up-stem")? {
                    prim |= MLP_JOIN_UP;
                }
            }
        }

        if make_flexa && let Some(prev) = prev_primitive {
            // turn the note with the previous one into a flexa
            let prev_prim = get_int(tx, prev, "primitive")?;
            set_int(
                tx,
                prev,
                "primitive",
                MLP_FLEXA_BEGIN | (prev_prim & MLP_STEM),
            );
            set_int(tx, prev, "flexa-interval", pitch - prev_pitch);
            prim = MLP_FLEXA_END;
            set_int(tx, primitive, "flexa-interval", pitch - prev_pitch);
        }

        // join_primitives replacement
        if !(at_beginning || make_flexa)
            && let Some(prev) = prev_primitive
        {
            grob::set_property(tx.heap(), prev, "add-join", Value::TRUE);
        }

        at_beginning = false;
        prev_primitive = Some(primitive);
        prev_pitch = pitch;
        set_int(tx, primitive, "primitive", prim);
        prev_brevis_shape = prim & MLP_BREVIS != 0;
        prev_semibrevis = prim & MLP_UP != 0;
    }
    Ok(())
}

/// The X extent's length of the glyph `name` in the grob's default font.
fn glyph_width(tx: &mut Tx<'_>, font: Value, name: &str) -> f64 {
    font::find_by_name(tx.heap(), font, name).extent(X).length()
}

/// `propagate_properties` (`:363–420`): the ligature's `thickness` (in
/// line thicknesses) and each head's width given to the heads; their
/// least length returned.
fn propagate_properties(tx: &mut Tx<'_>, ligature: Value, primitives: &[Value]) -> Run<f64> {
    let v = grob::property(tx.host, ligature, "thickness")?;
    let mut thickness = double_or(tx.host, v, 1.3);
    thickness *= grob::layout_dimension(tx.host, ligature, "line-thickness")?;

    let font = font_select::get_default_font(tx.host, ligature)?;
    let head_width = glyph_width(tx, font, "noteheads.sM1mensural");
    let maxima_head_width = glyph_width(tx, font, "noteheads.sM3ligmensural");

    let mut min_length = 0.0;
    let mut prev: Option<(Value, i32)> = None;
    for &primitive in primitives {
        let v = grob::property(tx.host, primitive, "primitive")?;
        let output = int_or(tx.heap(), v, 0);
        let t = tx.heap().flonum(thickness);
        grob::set_property(tx.heap(), primitive, "thickness", t);

        match output & MLP_ANY {
            MLP_BREVIS | MLP_LONGA => {
                min_length += head_width;
                let w = tx.heap().flonum(head_width);
                grob::set_property(tx.heap(), primitive, "head-width", w);
            }
            MLP_MAXIMA => {
                min_length += maxima_head_width;
                let w = tx.heap().flonum(maxima_head_width);
                grob::set_property(tx.heap(), primitive, "head-width", w);
            }
            MLP_FLEXA_BEGIN => {
                // the next note (should be MLP_FLEXA_END) will handle this one
            }
            MLP_FLEXA_END => {
                let flexa_scm = grob::property(tx.host, primitive, "flexa-width")?;
                let flexa_width = double_or(tx.host, flexa_scm, 2.0);
                min_length += flexa_width + thickness;
                let head_width = tx.heap().flonum(0.5 * (flexa_width + thickness));
                grob::set_property(tx.heap(), primitive, "head-width", head_width);
                if let Some((p, _)) = prev {
                    grob::set_property(tx.heap(), p, "head-width", head_width);
                    grob::set_property(tx.heap(), p, "flexa-width", flexa_scm);
                }
            }
            _ => tx.programming_error("unexpected case fall-through")?,
        }

        // join to the previous notehead is handled with the previous note.
        // let it know when this note has a left stem,
        // as it mustn't be hidden by the join
        if let Some((p, prev_output)) = prev
            && output & MLP_UP != 0
        {
            set_int(tx, p, "primitive", prev_output | MLP_JOIN_UP);
        }

        prev = Some((primitive, output));
    }
    Ok(min_length)
}

/// `fold_up_primitives` (`:422–476`): each head moved onto the first's
/// column after the ones before it, its dots above or behind it.
fn fold_up_primitives(tx: &mut Tx<'_>, primitives: &[Value], min_length: &mut f64) -> Run<()> {
    let Some(&first) = primitives.first() else {
        return Ok(());
    };
    let mut distance = 0.0;
    let staff_space = staff_symbol_referencer::staff_space(tx.host, first)?;
    let v = grob::property(tx.host, first, "thickness")?;
    let thickness = grob::to_double(tx.heap(), v)?;
    let first_column = item::get_column(tx.heap(), first);

    for (i, &current) in primitives.iter().enumerate() {
        if let Some(column) = first_column {
            move_related_items_to_column(tx, current, column, distance)?;
        }

        let v = grob::property(tx.host, current, "head-width")?;
        let head_width = double_or(tx.host, v, 0.0);
        distance += head_width - thickness;

        let dot_count = rhythmic_head::dot_count(tx.host, current)?;
        if dot_count == 0 {
            continue;
        }
        // Move dots above/behind the ligature.
        // dots should also avoid staff lines.
        let Some(dot_gr) = rhythmic_head::get_dots(tx.host, current)? else {
            continue;
        };

        let v = grob::property(tx.host, current, "staff-position")?;
        let pos = int_or(tx.heap(), v, 0);
        let on_line = staff_symbol_referencer::on_line(tx.host, current, pos)?;
        let mut vert_shift = if on_line { staff_space * 0.5 } else { 0.0 };
        let flexa_begin = get_int(tx, current, "primitive")? & MLP_FLEXA_BEGIN != 0;

        if i + 1 < primitives.len() {
            // dot in the midst => avoid next note;
            // what to avoid and where depends on
            // being on a line or between lines
            let delta = get_int(tx, current, "delta-position")?;
            if flexa_begin {
                vert_shift += if delta < 0 {
                    staff_space
                } else {
                    (if on_line { -2.0 } else { -1.0 }) * staff_space
                };
            } else if on_line {
                if 0 < delta && delta < 3 {
                    vert_shift -= staff_space;
                }
            } else if delta == 1 || delta == -1 {
                vert_shift -= f64::from(delta) * staff_space;
            }
        } else {
            *min_length += head_width * f64::from(dot_count);
        }

        translate(tx, dot_gr, vert_shift, Y)?;

        // move all dots behind head
        //
        // This is ugly and should probably be handled by configuring
        // the DotColumn appropriately.  Note that these dots will
        // be disconnected from their dot column.  See
        // move_related_items_to_column.
        //
        // This also means the padding isn't configurable as DotColumn.padding is.
        let stil = grob::property(tx.host, dot_gr, "dot-stencil")?;
        let dot_width = tx
            .heap()
            .foreign::<Stencil>(stil)
            .map_or(0.0, |s| s.extent(X).length());
        let by = (if flexa_begin {
            staff_space * 0.6
        } else {
            head_width
        }) - 2.0 * thickness
            + dot_width;
        translate(tx, dot_gr, by, X)?;
    }
    Ok(())
}

// --- the callbacks ------------------------------------------------------------

fn sign(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// The properties `brew_flexa` reads, read ahead of the drawing.
struct Flexa {
    staff_space: f64,
    interval: f64,
    blotdiameter: f64,
}

/// `brew_flexa` (`:39–118`): one half of a flexa, a single note's portion,
/// so that the two notes' colorations are handled apart.
fn brew_flexa(
    host: &mut dyn Host,
    f: &Flexa,
    solid: bool,
    mut width: f64,
    thickness: f64,
    begin: bool,
) -> Stencil {
    let heap = host.heap();
    let staff_space = f.staff_space;

    // The thickness of the horizontal lines of the flexa shape
    // should be equal to that of the horizontal lines of the
    // neomensural brevis note head (see mf/parmesan-heads.mf).
    let horizontal_line_thickness = staff_space * 0.35;

    // URGH!  vertical_line_thickness is adjustable (via thickness
    // property), while horizontal_line_thickness is constant.
    // Maybe both should be adjustable independently?

    let height = staff_space - horizontal_line_thickness;
    let interval = f.interval;
    let slope = (interval / 2.0 * staff_space) / width;

    // Compensate optical illusion regarding vertical position of left
    // and right endings due to slope.
    let ypos_correction = -0.1 * staff_space * sign(slope);
    let slope_correction = 0.2 * staff_space * sign(slope);
    let corrected_slope = slope + slope_correction / width;
    let blotdiameter = f.blotdiameter;
    width += 2.0 * blotdiameter;

    let mut stencil;
    if solid {
        // colorated flexae
        stencil = lookup::beam(
            heap,
            corrected_slope,
            width * 0.5,
            staff_space,
            blotdiameter,
        );
        heap.root(stencil.expr);
    } else {
        // outline
        stencil = lookup::beam(heap, corrected_slope, thickness, height, blotdiameter);
        heap.root(stencil.expr);
        if !begin {
            stencil.translate_axis(heap, width * 0.5 - thickness, X);
            heap.root(stencil.expr);
            stencil.translate_axis(heap, corrected_slope * (width * 0.5 - thickness), Y);
            heap.root(stencil.expr);
        }

        let mut bottom_edge = lookup::beam(
            heap,
            corrected_slope,
            width * 0.5,
            horizontal_line_thickness,
            blotdiameter,
        );
        heap.root(bottom_edge.expr);
        bottom_edge.translate_axis(heap, -0.5 * height, Y);
        heap.root(bottom_edge.expr);
        stencil.add_stencil(heap, &bottom_edge);
        heap.root(stencil.expr);

        let mut top_edge = lookup::beam(
            heap,
            corrected_slope,
            width * 0.5,
            horizontal_line_thickness,
            blotdiameter,
        );
        heap.root(top_edge.expr);
        top_edge.translate_axis(heap, 0.5 * height, Y);
        heap.root(top_edge.expr);
        stencil.add_stencil(heap, &top_edge);
        heap.root(stencil.expr);
    }

    if begin {
        stencil.translate_axis(heap, ypos_correction, Y);
    } else {
        stencil.translate_axis(heap, 0.5 * thickness - blotdiameter, X);
        heap.root(stencil.expr);
        stencil.translate_axis(heap, interval / -4.0 * staff_space, Y);
    }
    heap.root(stencil.expr);

    stencil.translate_axis(heap, -thickness, X);
    heap.root(stencil.expr);

    stencil
}

fn blank(host: &mut dyn Host) -> Stencil {
    let i = Interval::new(0.0, 0.0);
    lookup::blank(host.heap(), Box::new(i, i))
}

/// `internal_brew_primitive` (`:120–295`), its expressions rooted.
fn internal_brew_primitive(host: &mut dyn Host, me: Value) -> Run<Stencil> {
    let primitive_scm = grob::property(host, me, "primitive")?;
    if primitive_scm == Value::NULL {
        grob::unlocated_programming_error(
            host,
            "Mensural_ligature: undefined primitive -> ignoring grob",
        )?;
        return Ok(blank(host));
    }
    let primitive = to_int(host.heap(), primitive_scm)?;

    let mut thickness = 0.0;
    let mut width = 0.0;
    let mut flexa_width = 0.0;
    let staff_space = staff_symbol_referencer::staff_space(host, me)?;

    let style = grob::property(host, me, "style")?;
    let style = host.heap().name(style).map(String::from);
    let black = style.as_deref() == Some("blackpetrucci");
    let semi = style.as_deref() == Some("semipetrucci");

    if primitive & MLP_ANY != 0 {
        let v = grob::property(host, me, "thickness")?;
        thickness = double_or(host, v, 0.13);
        let v = grob::property(host, me, "head-width")?;
        width = double_or(host, v, staff_space) - thickness;
    }
    if primitive & MLP_FLEXA != 0 {
        let v = grob::property(host, me, "flexa-width")?;
        flexa_width = double_or(host, v, 2.0) * staff_space;
    }

    let note_shape = primitive & MLP_ANY;
    let fm = font_select::get_default_font(host, me)?;
    let color = if black {
        "black"
    } else if semi {
        "semi"
    } else {
        ""
    };

    let mut out = match note_shape {
        MLP_NONE => return Ok(blank(host)),
        MLP_MAXIMA | MLP_LONGA | MLP_BREVIS => {
            let duration_log = match note_shape {
                MLP_MAXIMA => -3,
                MLP_LONGA => -2,
                _ => -1,
            };
            let lig = if duration_log < -1 { "lig" } else { "" };
            let suffix = format!("{duration_log}{color}{lig}mensural");
            let mut index = String::from("noteheads.s");
            let rest = font::find_by_name(host.heap(), fm, &format!("{index}r{suffix}"));
            if !rest.is_empty() {
                let v = grob::property(host, me, "staff-position")?;
                let pos = int_or(host.heap(), v, 0);
                if !staff_symbol_referencer::on_line(host, me, pos)? {
                    index.push('r');
                }
            }
            font::find_by_name(host.heap(), fm, &format!("{index}{suffix}"))
        }
        MLP_FLEXA_BEGIN | MLP_FLEXA_END => {
            let v = grob::property(host, me, "flexa-interval")?;
            let interval = double_or(host, v, 0.0);
            let blotdiameter = grob::layout_dimension(host, me, "blot-diameter")?;
            let f = Flexa {
                staff_space,
                interval,
                blotdiameter,
            };
            brew_flexa(
                host,
                &f,
                black,
                flexa_width,
                thickness,
                note_shape == MLP_FLEXA_BEGIN,
            )
        }
        _ => {
            grob::unlocated_programming_error(
                host,
                "Mensural_ligature: unexpected case fall-through",
            )?;
            return Ok(blank(host));
        }
    };
    host.heap().root(out.expr);

    // we use thickness because the stem end of the glyph
    // "noteheads.sM2ligmensural" is round.
    let blotdiameter = thickness;
    // instead of 5.0 the length of a longa stem should be used:
    // Font_interface::get_default_font (???)->find_by_name
    // ("noteheads.sM2ligmensural???").extent (Y_AXIS).length ()
    let longa_stem_length = 5;
    let stem_length = f64::from(longa_stem_length) * 0.5 * staff_space;
    let mut warnings = Vec::new();

    if primitive & MLP_STEM != 0 {
        // assume MLP_UP
        let (mut y_bottom, mut y_top) = (0.0, stem_length);

        if primitive & MLP_DOWN != 0 {
            y_bottom = -y_top;
            y_top = 0.0;
        }

        let join_box = Box::new(
            Interval::new(-thickness, 0.0),
            Interval::new(y_bottom, y_top),
        );
        let join = lookup::round_filled_box(host.heap(), join_box, blotdiameter, &mut warnings);
        out.add_stencil(host.heap(), &join);
        host.heap().root(out.expr);
    }

    let has_join = grob::property(host, me, "add-join")? == Value::TRUE;
    let has_right_stem = primitive & MLP_RIGHT_STEM != 0;
    if has_join || has_right_stem {
        let join_right = if has_join {
            let v = grob::property(host, me, "delta-position")?;
            to_int(host.heap(), v)?
        } else if primitive & MLP_JOIN_UP != 0 {
            longa_stem_length
        } else {
            -longa_stem_length
        };
        if join_right != 0 {
            let mut y_top = f64::from(join_right) * 0.5 * staff_space;
            let mut y_bottom = 0.0;

            if y_top < 0.0 {
                y_bottom = y_top;
                y_top = 0.0;

                if has_join {
                    // if the previous note is longa-shaped,
                    // the joining line may hide the stem,
                    // so make join longer to serve as stem as well
                    if primitive & (MLP_LONGA | MLP_JOIN_DOWN) != 0 {
                        y_bottom -= stem_length + 0.25 * blotdiameter;
                    }

                    // if next note has a left upward stem,
                    // the joining line may hide that,
                    // so make join longer to serve as stem as well
                    if primitive & MLP_JOIN_UP != 0 {
                        y_top = stem_length + 0.25 * blotdiameter;
                    }
                }
            }

            let join_box = Box::new(
                Interval::new(width - thickness, width),
                Interval::new(y_bottom, y_top),
            );
            let join = lookup::round_filled_box(host.heap(), join_box, blotdiameter, &mut warnings);
            out.add_stencil(host.heap(), &join);
            host.heap().root(out.expr);
        } else {
            grob::unlocated_programming_error(host, "Mensural_ligature: (join_right == 0)")?;
        }
    }
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }

    Ok(out)
}

/// `Mensural_ligature::brew_ligature_primitive`, as above.
pub fn brew_ligature_primitive(host: &mut dyn Host, me: Value) -> Run<Value> {
    let depth = host.heap().root_depth();
    let brewed = internal_brew_primitive(host, me).map(|s| host.heap().wrap(s));
    host.heap().unroot_to(depth);
    brewed
}

/// `Mensural_ligature::print`: `'()`.
pub fn print(_: &mut dyn Host, _: Value) -> Run<Value> {
    Ok(Value::NULL)
}

/// What `%host-call` runs for the `ly:mensural-ligature::` callbacks
/// (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "mensural-ligature",
    f: host_mensural_ligature,
}];

/// The `ly:mensural-ligature::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("brew-ligature-primitive", brew_ligature_primitive),
    ("print", print),
];

fn host_mensural_ligature(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such mensural-ligature callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_brew_ligature_primitive(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "mensural-ligature",
        "ly:mensural-ligature::",
        "brew-ligature-primitive",
        args,
    )
}

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "mensural-ligature",
        "ly:mensural-ligature::",
        "print",
        args,
    )
}
