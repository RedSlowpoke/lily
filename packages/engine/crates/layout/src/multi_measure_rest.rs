//! `multi-measure-rest.cc` (part 3's, MS3 step 42): the multi-measure
//! rest's callbacks `ly:multi-measure-rest::print` (`:63–85`, 16 lines
//! from `SCM`) over `bar_width` (`:44–61`, 18), `::height` (`:87–100`,
//! 10), `::set-spacing-rods` (`:393–403`, 8) and `::set-text-rods`
//! (`:405–420`, 12), which the rest's `MultiMeasureRestNumber` has, over
//! `calculate_spacing_rods` (`:340–391`, 50); the symbol, `symbol_stencil`
//! (`:165–189`, 25) with `calc_measure_duration_log` (`:102–163`, 59),
//! `big_rest` (`:194–220`, 25) and `church_rest` (`:225–332`, 106); and
//! `add_column` (`:334–338`), which `Multi_measure_rest_engraver` calls.
//! The music crate's placeholders until then (D67). All four callbacks are
//! Rust under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), as the stem's are.
//!
//! The rests' glyphs, positions and names are `rest.rs`' (`%rest-glyph-name`,
//! `%rest-staff-position`), the glyphs the accidental's `find_by_name` in
//! the rest's default font; the columns' break-align widths and minimum
//! distances `paper_column.rs`', the durations' space `spacing_options.rs`'.
//! The text rods read the number's stencil, `ly:text-interface::print`'s
//! (`text.rs`), whose Emmentaler text is kerned since MS3 step 51 (W6).
//!
//! Powers of two are `ldexp`'s and `exp2`'s, as LilyPond's binary computes
//! `pow (2.0, …)`. LilyPond's clang fuses three sums of these (2.26.0's
//! binary):
//! `church_rest`'s Y of a rest longer than a whole and its left offset, and
//! the space a measure count adds to the rods; [`fma`] here. (`big_rest`'s
//! `width - 2 * ythick` is fused too, and is the same unfused.)

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, Moment, Rational};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::accidental::find_by_name;
use crate::geometry::{Box, Direction, Interval, X, Y, fma};
use crate::rod::Rod;
use crate::spacing_options::{SpacingOptions, double_or};
use crate::stencil::Stencil;
use crate::{direction, font_select, grob, item, lookup, paper_column, spanner, stem};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `get_bound (d)`, which LilyPond reads through: a spanner without one
/// throws `misc-error`.
fn bound(host: &mut dyn Host, me: Value, d: Direction) -> Result<Value, Exception> {
    spanner::get_bound(host, me, d)?.ok_or_else(|| {
        crate::misc_error(
            host.heap(),
            "ly:multi-measure-rest",
            "multi-measure rest without a bound",
        )
    })
}

/// `get_bound (d)->get_column ()`.
fn bound_column(host: &mut dyn Host, me: Value, d: Direction) -> Result<Value, Exception> {
    let b = bound(host, me, d)?;
    item::get_column(host.heap(), b).ok_or_else(|| {
        crate::misc_error(
            host.heap(),
            "ly:multi-measure-rest",
            "multi-measure rest bound in no column",
        )
    })
}

/// A real number of `v` (`scm_to_double`), Guile's error if it is none.
fn real(host: &mut dyn Host, v: Value) -> Result<f64, Exception> {
    let v = crate::call_lily(host, "%scm->double", &[v])?;
    Ok(host.heap().to_f64(v).unwrap_or(0.0))
}

/// `from_scm<int> (v)` (`scm_to_int`), Guile's error if `v` is no exact
/// integer of an `int`'s range.
fn int(host: &mut dyn Host, v: Value) -> Result<i32, Exception> {
    let v = crate::call_lily(host, "%scm->int", &[v])?;
    Ok(host
        .heap()
        .to_i64(v)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(0))
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// The stencil `s`, its expression rooted until the caller unroots it.
fn kept(host: &mut dyn Host, s: Stencil) -> Stencil {
    host.heap().root(s.expr);
    s
}

/// `Lookup::round_filled_box`, its warning given at once, as LilyPond's.
fn round_filled_box(host: &mut dyn Host, b: Box, blot: f64) -> Result<Stencil, Exception> {
    let mut warnings = Vec::new();
    let s = lookup::round_filled_box(host.heap(), b, blot, &mut warnings);
    let s = kept(host, s);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    Ok(s)
}

/// `bar_width`: from the left bound's column to the right one's, each
/// column's break-align width of the side of `spacing-pair` facing the
/// rest (`staff-bar` if it is no pair), its edge facing the rest.
pub fn bar_width(host: &mut dyn Host, me: Value) -> Result<Interval, Exception> {
    let spacing_pair = grob::property(host, me, "spacing-pair")?;
    let mut iv = Interval::EMPTY;
    for d in [LEFT, RIGHT] {
        let col = bound_column(host, me, d)?;
        let heap = host.heap();
        let align_sym = match (heap.car(spacing_pair), heap.cdr(spacing_pair)) {
            (Some(l), Some(r)) => {
                if d == LEFT {
                    l
                } else {
                    r
                }
            }
            _ => heap.intern("staff-bar"),
        };
        let coldim = crate::call_lily(host, "%break-align-width", &[col, align_sym])?;
        let coldim = grob::number_pair(host.heap(), coldim).unwrap_or(Interval::EMPTY);
        *iv.at_mut(d) = coldim.at(-d);
    }
    Ok(iv)
}

/// `print`: the symbol as wide as the space between the bounds' columns'
/// bar lines, moved right where the left bound's column is left of its
/// bar line's right edge.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sp_iv = bar_width(host, me)?;
    let space = sp_iv.length();

    let left = bound(host, me, LEFT)?;
    let rx = grob::relative_coordinate(host, left, Value::NULL, X)?;
    // we gotta stay clear of sp_iv, so move a bit to the right if
    // needed.
    let x_off = f64::max(sp_iv.at(LEFT) - rx, 0.0);

    let mut mol = Stencil::default();
    let symbol = symbol_stencil(host, me, space)?;
    mol.add_stencil(host.heap(), &symbol);

    mol.translate_axis(host.heap(), x_off, X);
    Ok(host.heap().wrap(mol))
}

/// `height`: the Y extent of the symbol as wide as can be.
pub fn height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let extent = height_rooted(host, me);
    host.heap().unroot_to(depth);
    extent
}

fn height_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let space = 1000000.0; // something very large...

    let mut mol = Stencil::default();
    let symbol = symbol_stencil(host, me, space)?;
    mol.add_stencil(host.heap(), &symbol);

    let y = mol.extent(Y);
    Ok(crate::offset(host.heap(), (y.left, y.right)))
}

/// `calc_measure_duration_log`: the duration log, of the rest's
/// `usable-duration-logs`, closest to the left bound's `measure-length`
/// (1 if it is no moment): the longest not longer, or with
/// `round-up-to-longer-rest` or a length among `round-up-exceptions`, the
/// shortest not shorter; else the shortest of them, or the longest. A
/// `usable-duration-logs` that is empty or no list is LilyPond's warning
/// and a whole rest's.
fn calc_measure_duration_log(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    // TODO: Getting the measure length from a Paper_column, which is engraved in
    // Score context, is bogus in polymetric scores, where the Timing_translator
    // operates in Staff context rather than Score context.  See issue #4633.
    let left = bound(host, me, LEFT)?;
    let sml = grob::property(host, left, "measure-length")?;
    let ml = host
        .heap()
        .foreign::<Moment>(sml)
        .map_or(Rational::int(1), |m| m.main);
    let duration = ml.to_f64();
    let fraction = {
        let heap = host.heap();
        let num = heap.integer(ml.numerator());
        let den = heap.integer(ml.denominator());
        heap.cons(num, den)
    };
    let exceptions = grob::property(host, me, "round-up-exceptions")?;
    let found = crate::call_lily(host, "member", &[fraction, exceptions])?;
    let round_up = crate::call_lily(host, "list?", &[found])? == Value::TRUE
        || grob::property(host, me, "round-up-to-longer-rest")? == Value::TRUE;

    // Out of range initial values.
    let mut closest_usable_duration_log = if round_up {
        -15 // high value
    } else {
        15 // low value
    };
    let mut minimum_usable_duration_log = -15;
    let mut maximum_usable_duration_log = 15;

    let duration_logs_list = grob::property(host, me, "usable-duration-logs")?;
    let is_list = crate::call_lily(host, "list?", &[duration_logs_list])? == Value::TRUE;
    if duration_logs_list == Value::NULL || !is_list {
        grob::unlocated_warning(
            host,
            "usable-duration-logs must be a non-empty list.  Falling back to whole rests.",
        )?;
        closest_usable_duration_log = 0;
    } else {
        let mut at = duration_logs_list;
        while let (Some(v), Some(rest)) = (host.heap().car(at), host.heap().cdr(at)) {
            at = rest;
            let dur_log = int(host, v)?;
            if dur_log > minimum_usable_duration_log {
                minimum_usable_duration_log = dur_log;
            }
            if dur_log < maximum_usable_duration_log {
                maximum_usable_duration_log = dur_log;
            }
            let dur = libm::ldexp(1.0, -dur_log);
            if round_up {
                if duration <= dur && dur_log > closest_usable_duration_log {
                    closest_usable_duration_log = dur_log;
                }
            } else if duration >= dur && dur_log < closest_usable_duration_log {
                closest_usable_duration_log = dur_log;
            }
        }
    }

    if closest_usable_duration_log == 15 {
        closest_usable_duration_log = minimum_usable_duration_log;
    }
    if closest_usable_duration_log == -15 {
        closest_usable_duration_log = maximum_usable_duration_log;
    }

    Ok(closest_usable_duration_log)
}

/// `symbol_stencil`: nothing for a `measure-count` that is no number or
/// not positive; over `expand-limit` measures the big rest across 70 % of
/// `space`; else the church rests of the measures, each measure the rest
/// of [`calc_measure_duration_log`]. The stencil's expression is rooted
/// until the caller unroots it.
fn symbol_stencil(host: &mut dyn Host, me: Value, space: f64) -> Result<Stencil, Exception> {
    let mut measure_count = 0;
    let m = grob::property(host, me, "measure-count")?;
    if host.heap().is_number(m) {
        measure_count = int(host, m)?;
    }
    if measure_count <= 0 {
        return Ok(Stencil::default());
    }

    let limit = grob::property(host, me, "expand-limit")?;
    if measure_count > int(host, limit)? {
        let padding = 0.15;
        let mut s = big_rest(host, me, (1.0 - 2.0 * padding) * space)?;
        s.translate_axis(host.heap(), padding * space, X);
        Ok(kept(host, s))
    } else {
        let musfont = font_select::get_default_font(host, me)?;
        host.heap().root(musfont);
        let mdl = calc_measure_duration_log(host, me)?;
        church_rest(host, me, musfont, measure_count, mdl, space)
    }
}

/// `big_rest`: a thick bar `width` wide less two hair lines, at each end
/// a hair line two staff spaces high; `width` 0 its least, unrounded.
///
/// WIDTH can also be 0 to determine the minimum size of the object.
fn big_rest(host: &mut dyn Host, me: Value, width: f64) -> Result<Stencil, Exception> {
    let thick_thick = real_or(host, me, "thick-thickness", 1.0)?;
    let hair_thick = real_or(host, me, "hair-thickness", 0.1)?;

    let ss = staff_space(host, me)?;
    let slt = grob::layout_dimension(host, me, "line-thickness")?;
    let y = slt * thick_thick / 2.0 * ss;
    let ythick = hair_thick * slt * ss;
    let b = Box::new(
        Interval::new(0.0, f64::max(0.0, fma(ythick, -2.0, width))),
        Interval::new(-y, y),
    );

    let blot = if width != 0.0 {
        0.8 * f64::min(y, ythick)
    } else {
        0.0
    };

    let mut m = round_filled_box(host, b, 0.0)?;
    let half = Interval::new(-0.5 * ythick, 0.5 * ythick);
    let half = if ythick < 0.0 {
        Interval::new(half.right, half.left)
    } else {
        half
    };
    let yb = round_filled_box(host, Box::new(half, Interval::new(-ss, ss)), blot)?;

    let heap = host.heap();
    m.add_at_edge(heap, X, RIGHT, &yb, 0.0);
    m.add_at_edge(heap, X, LEFT, &yb, 0.0);

    m.align_to(heap, X, f64::from(LEFT));

    Ok(kept(host, m))
}

/// `Staff_symbol_referencer::staff_space`.
fn staff_space(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    crate::staff_symbol_referencer::staff_space(host, me)
}

/// `Rest::staff_position_internal (me, duration_log, dir)`.
fn rest_staff_position(
    host: &mut dyn Host,
    me: Value,
    duration_log: i32,
    dir: Direction,
) -> Result<f64, Exception> {
    let args = [
        me,
        Value::fixnum(i64::from(duration_log)).unwrap_or(Value::NULL),
        Value::fixnum(i64::from(dir)).unwrap_or(Value::NULL),
    ];
    let pos = crate::call_lily(host, "%rest-staff-position", &args)?;
    Ok(host.heap().to_f64(pos).unwrap_or(0.0))
}

/// `Rest::glyph_name (me, durlog, "", true, offset)`.
fn rest_glyph_name(
    host: &mut dyn Host,
    me: Value,
    durlog: i32,
    offset: f64,
) -> Result<String, Exception> {
    let heap = host.heap();
    let args = [
        me,
        Value::fixnum(i64::from(durlog)).unwrap_or(Value::NULL),
        heap.string(""),
        Value::TRUE,
        heap.flonum(offset),
    ];
    let name = crate::call_lily(host, "%rest-glyph-name", &args)?;
    Ok(host
        .heap()
        .str(name)
        .map(|s| String::from(s.as_str()))
        .unwrap_or_default())
}

/// `church_rest` (Kirchenpause): the rests of `measure_count` measures of
/// the duration log `mdl`, the longest first, centred in `space` with at
/// most `max-symbol-separation` (at least 1) between them, or 1 if they
/// do not fit. The rest's `staff-position`, when unset, is set: a whole
/// rest's (less 2 on a staff of several lines) when the longest is a
/// whole one, else a breve's. On one line (or none) a whole rest, and a
/// longer one of no direction, hangs from the line below its own.
fn church_rest(
    host: &mut dyn Host,
    me: Value,
    musfont: Value,
    measure_count: i32,
    mdl: i32,
    space: f64,
) -> Result<Stencil, Exception> {
    // using double here is not less exact than rationals because
    // only simple, unscaled durations are used for representation
    // even if you have \time 10/6
    let mut displayed_duration = libm::ldexp(1.0, -mdl) * f64::from(measure_count);
    let mut mols: Vec<Stencil> = Vec::new();
    let mut symbol_count = 0;
    let mut symbols_width = 0.0;
    let dir = direction::get_grob_direction(host, me)?;

    let sp = grob::property(host, me, "staff-position")?;

    let staff = crate::staff_symbol_referencer::staff_symbol(host, me)?.unwrap_or(Value::FALSE);
    let oneline = if staff != Value::FALSE {
        let line_positions = grob::property(host, staff, "line-positions")?;
        let n = crate::call_lily(host, "length", &[line_positions])?;
        host.heap().to_i64(n).unwrap_or(0) < 2
    } else {
        // If there is no StaffSymbol, print MMrests on one (invisible) line.
        true
    };

    let pos = if sp == Value::NULL {
        let pos = if (1.0..2.0).contains(&displayed_duration) {
            // i. e. longest rest symbol is semibreve
            rest_staff_position(host, me, 0, dir)? - if oneline { 0.0 } else { 2.0 }
        } else {
            rest_staff_position(host, me, 1, dir)?
        };
        let v = host.heap().flonum(pos);
        grob::set_property(host.heap(), me, "staff-position", v);
        pos
    } else {
        real(host, sp)?
    };

    let mut dl = -3;
    while displayed_duration > 0.0 {
        let duration = libm::ldexp(1.0, -dl);

        if displayed_duration < duration {
            dl += 1;
            continue;
        }

        displayed_duration -= duration;

        let ss = staff_space(host, me)?;
        let mut spi = rest_staff_position(host, me, dl, dir)?;
        let mut r = if oneline && (dl == 0 || (dl < 0 && dir == 0)) {
            spi -= 2.0;
            let name = rest_glyph_name(host, me, dl, if dl == 0 { 0.0 } else { -2.0 })?;
            find_by_name(host, musfont, &name)?
        } else {
            let name = rest_glyph_name(host, me, dl, if dl == 0 { 2.0 } else { 0.0 })?;
            find_by_name(host, musfont, &name)?
        };
        if dl < 0 {
            let font_size = real_or(host, me, "font-size", 0.0)?;
            let fs = libm::exp2(font_size / 6.0);
            r.translate_axis(host.heap(), fma(ss * 0.5, spi - pos, ss - fs), Y);
        } else {
            r.translate_axis(host.heap(), ss * 0.5 * (spi - pos), Y);
        }

        symbols_width += r.extent(X).length();
        mols.push(kept(host, r));
        symbol_count += 1;
    }

    // When symbols spread to fullest extent, outer padding is this much
    // bigger.
    let outer_padding_factor = 1.5;
    // Widest gap between symbols; to be limited by max-symbol-separation
    let mut inner_padding =
        (space - symbols_width) / (2.0 * outer_padding_factor + f64::from(symbol_count - 1));
    if inner_padding < 0.0 {
        inner_padding = 1.0;
    }

    let max_separation = f64::max(real_or(host, me, "max-symbol-separation", 8.0)?, 1.0);

    inner_padding = f64::min(inner_padding, max_separation);
    let left_offset = fma(
        -inner_padding,
        f64::from(symbol_count - 1),
        space - symbols_width,
    ) / 2.0;

    let heap = host.heap();
    let mut mol = Stencil::default();
    // The list LilyPond conses up: the last made first.
    for s in mols.iter().rev() {
        mol.add_at_edge(heap, X, LEFT, s, inner_padding);
    }
    mol.align_to(heap, X, f64::from(LEFT));
    mol.translate_axis(heap, left_offset, X);

    Ok(kept(host, mol))
}

/// `add_column`: `c` the rest's left bound if it has none, else its right.
pub fn add_column(host: &mut dyn Host, me: Value, c: Value) -> Result<(), Exception> {
    spanner::add_bound_item(host, me, c)
}

/// `calculate_spacing_rods`: rods from the left bound's column (and its
/// piece at the end of a line) to the right bound's (and its piece at the
/// start of one), each the columns' minimum distance plus `length`, at
/// least `minimum-length`: `length` grown by twice `bound-padding` and,
/// if either column has a `spacing`, by the left column's
/// `full-measure-extra-space`, the space of its `measure-length` (1 if no
/// moment) as the rest's spacing options give it, and `space-increment`
/// for each doubling of `measure-count` (1 if no `int`). Without both
/// bounds, LilyPond's programming error and no rod.
pub fn calculate_spacing_rods(
    host: &mut dyn Host,
    me: Value,
    mut length: f64,
) -> Result<(), Exception> {
    let bounds = spanner::bounds(host.heap(), me);
    let (Some(left), Some(right)) = (bounds[0], bounds[1]) else {
        return grob::unlocated_programming_error(host, "Multi measure rest seems misplaced.");
    };

    let column = |host: &mut dyn Host, b: Value| {
        item::get_column(host.heap(), b).ok_or_else(|| {
            crate::misc_error(
                host.heap(),
                "ly:multi-measure-rest",
                "multi-measure rest bound in no column",
            )
        })
    };
    let lc = column(host, left)?;
    let rc = column(host, right)?;

    let spacing_sym = host.heap().intern("spacing");
    let mut spacing = grob::object(host, lc, spacing_sym)?;
    if lily_engine_music::grob_of(host.heap(), spacing).is_none() {
        spacing = grob::object(host, rc, spacing_sym)?;
    }
    if lily_engine_music::grob_of(host.heap(), spacing).is_some() {
        let mut options = SpacingOptions::default();
        options.init_from_grob(host, me)?;
        let v = grob::property(host, lc, "measure-length")?;
        let mlen = host
            .heap()
            .foreign::<Moment>(v)
            .copied()
            .unwrap_or(Moment::new(Rational::int(1), Rational::int(0)));
        let extra = real_or(host, lc, "full-measure-extra-space", 0.0)?;
        let duration_space = options.get_duration_space(host, mlen.main)?;
        let space_increment = real_or(host, me, "space-increment", 0.0)?;
        let v = grob::property(host, me, "measure-count")?;
        let count = stem::int_or(host.heap(), v, 1);
        length += fma(
            space_increment,
            libm::log2(f64::from(count)),
            extra + duration_space,
        );
    }

    length = fma(real_or(host, me, "bound-padding", 0.0)?, 2.0, length);

    let minlen = real_or(host, me, "minimum-length", 0.0)?;

    let lb = item::find_prebroken_piece(host.heap(), lc, RIGHT);
    let rb = item::find_prebroken_piece(host.heap(), rc, LEFT);
    for li in [Some(lc), lb].into_iter().flatten() {
        for ri in [Some(rc), rb].into_iter().flatten() {
            let mut rod = Rod {
                item_drul: [Some(li), Some(ri)],
                distance: f64::max(
                    paper_column::minimum_distance(host, li, ri)? + length,
                    minlen,
                ),
            };
            rod.add_to_cols(host)?;
        }
    }
    Ok(())
}

/// `set_spacing_rods`: the rods of the symbol's least width.
pub fn set_spacing_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let set = symbol_stencil(host, me, 0.0)
        .and_then(|s| calculate_spacing_rods(host, me, s.extent(X).length()));
    host.heap().unroot_to(depth);
    set.map(|()| Value::UNSPECIFIED)
}

/// `set_text_rods`: the rods of the grob's stencil's width (0 for none or
/// an empty one), for the rest's number and text.
pub fn set_text_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stil = grob::property(host, me, "stencil")?;
    // FIXME uncached
    let len = match host.heap().foreign::<Stencil>(stil) {
        Some(s) if !s.extent(X).is_empty() => s.extent(X).length(),
        _ => 0.0,
    };
    calculate_spacing_rods(host, me, len)?;
    Ok(Value::UNSPECIFIED)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:multi-measure-rest::` primitives
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "multi-measure-rest",
    f: host_multi_measure_rest,
}];

type Callback = fn(&mut dyn Host, Value) -> Result<Value, Exception>;

fn host_multi_measure_rest(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let f: Callback = match vm.heap().name(args[1]) {
        Some("height") => height,
        Some("print") => print,
        Some("set-spacing-rods") => set_spacing_rods,
        Some("set-text-rods") => set_text_rods,
        _ => {
            return Err(crate::misc_error(
                vm.heap(),
                "%host-call",
                "no such multi-measure rest callback",
            ));
        }
    };
    f(vm, args[2])
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:multi-measure-rest::… spanner)`: LilyPond's subrs, primitives here
/// so that they print as LilyPond's. Each checks its spanner and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    spanner::spanner_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let rest = cx.heap_mut().intern("multi-measure-rest");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[rest, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:multi-measure-rest::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_height "height",
    ly_print "print",
    ly_set_spacing_rods "set-spacing-rods",
    ly_set_text_rods "set-text-rods",
}
