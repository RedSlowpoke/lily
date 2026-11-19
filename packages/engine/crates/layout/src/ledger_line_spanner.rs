//! `ledger-line-spanner.cc` (part 3's, MS3 step 39): the ledger line
//! spanner's callbacks `ly:ledger-line-spanner::set-spacing-rods`
//! (`ledger-line-spanner.cc:63–140`, 78 lines) with `set_rods` (`:39–61`,
//! 23) and `::print` (`:183–411`, 229) with its `Head_data` and
//! `Ledger_request` (`:142–177`, 36), the music crate's placeholders until
//! then (D67), and `Staff_symbol::get_ledger_line_thickness`
//! (`staff-symbol.cc:337–344`, 8), which only the print asks. Both are
//! Rust under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), as the stem's are.
//!
//! The ledgers' positions are `Staff_symbol::ledger_positions`
//! (`staff_symbol.rs`, ported first at MS3 step 37); a head's position,
//! the staff symbol, its staff space and line thickness are the
//! referencer's Scheme (`%rounded-staff-position`, `%staff-symbol`,
//! `%staff-symbol-staff-space`, `%staff-symbol-line-thickness`); the
//! merged ledgers `Interval_set::interval_union` (`axis_group.rs`'). An
//! accidental's `ledger-shortening-range` is its glyph's `LILC` entry, as
//! `Open_type_font::ledger_shortening_range` reads it, scaled by the
//! font's magnification; `(0 . 0)` of a Pango font (`Font_metric`'s).
//!
//! LilyPond 2.26.0's clang fuses the ledger line thickness's sum of
//! products (otool), `x × line thickness + y × staff space` rounded once,
//! and `set_rods`' `2 × min_length - left` (exact either way); the print's
//! other sums are not fused. Where LilyPond reads through a null, a head
//! without a column has none (no rod, the rank of no column).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::axis_group::interval_union;
use crate::callbacks::Callback;
use crate::font;
use crate::font_select;
use crate::geometry::{Box, Direction, Interval, X, Y, fma, max};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::lookup;
use crate::paper_column;
use crate::rod::Rod;
use crate::spacing_options::double_or;
use crate::staff_symbol;
use crate::stem::to_int;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 for `DOWN`, 1 for `UP`.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `Interval::contains`.
fn contains(iv: Interval, x: f64) -> bool {
    iv.left <= x && x <= iv.right
}

/// `Interval::operator *=`: both ends times `r`, swapped for a negative
/// `r`; an empty interval stays as it is.
fn scaled(mut iv: Interval, r: f64) -> Interval {
    if !iv.is_empty() {
        iv = Interval::new(iv.left * r, iv.right * r);
        if r < 0.0 {
            iv = Interval::new(iv.right, iv.left);
        }
    }
    iv
}

/// `Interval::widen`.
fn widen(iv: &mut Interval, w: f64) {
    iv.left -= w;
    iv.right += w;
}

/// `int sign (int)`.
fn sign(x: i32) -> i32 {
    x.signum()
}

/// `Staff_symbol_referencer::get_staff_symbol`: `%staff-symbol`'s grob.
fn staff_symbol_of(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    crate::staff_symbol_referencer::staff_symbol(host, me)
}

/// A real that a Scheme helper of `staff_symbol.rs` gives of `staff`.
fn staff_real(host: &mut dyn Host, name: &str, staff: Value) -> Result<f64, Exception> {
    let v = crate::call_lily(host, name, &[staff])?;
    grob::to_double(host.heap(), v)
}

/// `Staff_symbol_referencer::get_rounded_position`: `%rounded-staff-position`.
fn rounded_position(host: &mut dyn Host, h: Value) -> Result<i32, Exception> {
    let v = crate::staff_symbol_referencer::rounded_position(host, h, false)?;
    to_int(host.heap(), v)
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `Staff_symbol::get_ledger_line_thickness`: the staff symbol's
/// `ledger-line-thickness`, a pair of numbers `(x . y)` (else `(1 . 0.1)`),
/// as `x` line thicknesses and `y` staff spaces, the sum rounded once.
pub fn get_ledger_line_thickness(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let lt_pair = grob::property(host, me, "ledger-line-thickness")?;
    let heap = host.heap();
    let mut z = (1.0, 0.1);
    if let (Some(a), Some(b)) = (heap.car(lt_pair), heap.cdr(lt_pair))
        && heap.is_number(a)
        && heap.is_number(b)
    {
        z = (grob::to_double(heap, a)?, grob::to_double(heap, b)?);
    }
    let line_thickness = staff_real(host, "%staff-symbol-line-thickness", me)?;
    let staff_space = staff_real(host, "%staff-symbol-staff-space", me)?;
    Ok(fma(z.0, line_thickness, z.1 * staff_space))
}

// --- set-spacing-rods -------------------------------------------------------

/// `set_rods`: on each side of the staff where both columns have
/// ledgered heads, a rod from the current column to the previous one (to
/// its right: the heads are walked from right to left) keeping their
/// ledgers twice `min_length` apart.
fn set_rods(
    host: &mut dyn Host,
    current_extents: &[Interval; 2],
    previous_extents: &[Interval; 2],
    current_column: Option<Value>,
    previous_column: Option<Value>,
    min_length: f64,
) -> Result<(), Exception> {
    for d in [UP, DOWN] {
        let (current, previous) = (current_extents[ix(d)], previous_extents[ix(d)]);
        if !current.is_empty() && !previous.is_empty() {
            let mut rod = Rod {
                item_drul: [current_column, previous_column],
                distance: fma(2.0, min_length, -previous.left) + current.right,
            };
            rod.add_to_cols(host)?;
        }
    }
    Ok(())
}

/// `set_spacing_rods`: rods between the columns of consecutive heads
/// with ledgers (not an ambitus's), so that the ledgers on each side of
/// the staff keep `minimum-length-fraction` (0.15 unset) of the widest
/// head apart; the spanner killed if it has no staff symbol.
pub fn set_spacing_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let Some(staff) = staff_symbol_of(host, me)? else {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    };
    let min_length_fraction = real_or(host, me, "minimum-length-fraction", 0.15)?;
    let mut current_extents = [Interval::EMPTY; 2];
    let mut previous_extents = [Interval::EMPTY; 2];
    let mut current_head_width = 0.0;
    let mut previous_column = None;
    let mut current_column = None;
    // What LilyPond computes here and does not use: the staff's extent.
    grob::extent(host, staff, staff, Y)?;

    let heads = grob_array::extract_item_array(host, me, "note-heads")?;
    for &h in heads.iter().rev() {
        let pos = rounded_position(host, h)?;
        if staff_symbol::ledger_positions(host, staff, pos, None)?.is_empty() {
            continue;
        }
        // Ambitus heads can appear out of order in the heads, but as
        // prefatory matter they need no rods.
        if grob::has_interface(host.heap(), h, "ambitus-interface") {
            continue;
        }
        let column = item::get_column(host.heap(), h);
        if current_column != column {
            set_rods(
                host,
                &current_extents,
                &previous_extents,
                current_column,
                previous_column,
                current_head_width * min_length_fraction,
            )?;
            previous_column = current_column;
            current_column = column;
            previous_extents = current_extents;
            current_extents = [Interval::EMPTY; 2];
            current_head_width = 0.0;
        }
        let head_extent = grob::extent(host, h, column.unwrap_or(Value::NULL), X)?;
        let vdir = sign(pos);
        if vdir == 0 {
            continue;
        }
        current_extents[ix(vdir)].unite(head_extent);
        current_head_width = max(current_head_width, head_extent.length());
    }
    if previous_column.is_some() && current_column.is_some() {
        set_rods(
            host,
            &current_extents,
            &previous_extents,
            current_column,
            previous_column,
            current_head_width * min_length_fraction,
        )?;
    }
    Ok(Value::UNSPECIFIED)
}

// --- print ------------------------------------------------------------------

/// `Head_data`: a ledgered head's position, ledgers and extents.
struct HeadData {
    position: i32,
    ledger_positions: Vec<f64>,
    head_extent: Interval,
    ledger_extent: Interval,
    accidental_extent: Interval,
    ledger_shortening_range: Interval,
}

/// `Ledger_request`: one side of a column's ledgered heads, and the
/// ledgers' extents by position, in order (`std::map<Real, …>`).
struct LedgerRequest {
    max_ledger_extent: Interval,
    max_head_extent: Interval,
    max_position: i32,
    heads: Vec<HeadData>,
    ledger_extents: Vec<(f64, Vec<Interval>)>,
}

impl Default for LedgerRequest {
    fn default() -> LedgerRequest {
        LedgerRequest {
            max_ledger_extent: Interval::EMPTY,
            max_head_extent: Interval::EMPTY,
            max_position: 0,
            heads: Vec::new(),
            ledger_extents: Vec::new(),
        }
    }
}

impl LedgerRequest {
    /// `ledger_extents_[lpos].push_back (x)`.
    fn push_extent(&mut self, lpos: f64, x: Interval) {
        let at = self.ledger_extents.partition_point(|(p, _)| *p < lpos);
        match self.ledger_extents.get_mut(at) {
            Some((p, extents)) if *p == lpos => extents.push(x),
            _ => self.ledger_extents.insert(at, (lpos, Vec::from([x]))),
        }
    }
}

/// `Font_metric::ledger_shortening_range` of the default font of the
/// accidental `g` for `glyph`: its `LILC` entry's range in points times
/// the font's magnification, empty for a glyph without one, `(0 . 0)` of
/// a font that is no OpenType font.
fn ledger_shortening_range(
    host: &mut dyn Host,
    g: Value,
    glyph: &str,
) -> Result<Interval, Exception> {
    let fm = font_select::get_default_font(host, g)?;
    let Some(m) = font::metric(host.heap(), fm) else {
        return Ok(Interval::new(0.0, 0.0));
    };
    let range = match m
        .font
        .metrics(glyph)
        .and_then(|e| e.ledger_shortening_range)
    {
        Some([l, r]) => Interval::new(l, r),
        None => Interval::EMPTY,
    };
    Ok(scaled(
        scaled(range, lily_engine_fonts::POINT),
        m.magnification,
    ))
}

/// `print`: the ledger lines of the spanner's heads, each head's as wide
/// as it widened by `length-fraction` (0.25 unset) of its width, on each
/// side of the staff (a head off the staff without ledgers included);
/// the ledgers of neighbouring columns shortened to the middle between
/// their heads, `gap` (0.1 unset) apart where both reach past the first
/// space beyond the staff; a ledger in the head's accidental's glyph's
/// shortening range shortened to the middle between the accidental and
/// the head; at each position the ledgers merged where they meet, each
/// a box rounded by the ledger line thickness, which is its height.
/// Nothing without heads, a staff symbol or a ledgered head.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let heads = grob_array::extract_grob_array(host, me, "note-heads")?;
    if heads.is_empty() {
        return Ok(Value::NULL);
    }
    let Some(staff) = staff_symbol_of(host, me)? else {
        return Ok(Value::NULL);
    };
    let halfspace = staff_real(host, "%staff-symbol-staff-space", staff)? / 2.0;
    let staff_extent = grob::extent(host, staff, staff, Y)?;
    let staff_extent = scaled(staff_extent, 1.0 / halfspace);
    let length_fraction = real_or(host, me, "length-fraction", 0.25)?;

    let mut common_x = grob::common_refpoint_of_array(host.heap(), &heads, Some(me), X);
    let accidental_grob = host.heap().intern("accidental-grob");
    for &h in heads.iter().rev() {
        let g = grob_array::object(host, h, accidental_grob)?;
        if grob_of(host.heap(), g).is_some() {
            common_x = common_x.and_then(|c| grob::common_refpoint(host.heap(), c, g, X));
        }
    }
    let common_x = common_x.unwrap_or(Value::NULL);

    let mut reqs: BTreeMap<u64, [LedgerRequest; 2]> = BTreeMap::new();
    for &h in heads.iter().rev() {
        let pos = rounded_position(host, h)?;
        let ledger_positions = staff_symbol::ledger_positions(host, staff, pos, Some(h))?;
        // The heads that make ledgers and those outside the staff that
        // make none, as in the first space beyond it.
        if ledger_positions.is_empty() && contains(staff_extent, f64::from(pos)) {
            continue;
        }
        let head_extent = grob::extent(host, h, common_x, X)?;
        let mut ledger_extent = head_extent;
        widen(&mut ledger_extent, length_fraction * head_extent.length());
        let vdir = sign(if pos != 0 { pos } else { 1 });
        let column = item::get_column(host.heap(), h);
        let rank = paper_column::rank(host.heap(), column.unwrap_or(Value::NULL));
        let mut hd = HeadData {
            position: pos,
            ledger_positions,
            head_extent,
            ledger_extent,
            accidental_extent: Interval::EMPTY,
            ledger_shortening_range: Interval::EMPTY,
        };
        let g = grob_array::object(host, h, accidental_grob)?;
        if grob_of(host.heap(), g).is_some() {
            hd.accidental_extent = grob::extent(host, g, common_x, X)?;
            let glyph = if grob::property(host, g, "parenthesized")? == Value::TRUE {
                Some(alloc::string::String::from("accidentals.rightparen"))
            } else {
                let v = grob::property(host, g, "glyph-name")?;
                let heap = host.heap();
                heap.str(v).map(|s| alloc::string::String::from(s.as_str()))
            };
            if let Some(glyph) = glyph.filter(|s| !s.is_empty()) {
                let range = ledger_shortening_range(host, g, &glyph)?;
                hd.ledger_shortening_range = scaled(range, 1.0 / halfspace);
                // Compensate for rounding errors.
                widen(&mut hd.ledger_shortening_range, 1e-3);
            }
        }
        let req = &mut reqs.entry(rank).or_default()[ix(vdir)];
        req.max_ledger_extent.unite(ledger_extent);
        req.max_head_extent.unite(head_extent);
        req.max_position = vdir * (vdir * req.max_position).max(vdir * pos);
        req.heads.push(hd);
    }
    if reqs.is_empty() {
        return Ok(Value::NULL);
    }

    // Where the ledgers of neighbouring columns would come too close,
    // each column's widest ledger shortened to make room between them.
    let gap = real_or(host, me, "gap", 0.1)?;
    let mut reqs: Vec<[LedgerRequest; 2]> = reqs.into_values().collect();
    for i in 1..reqs.len() {
        let (before, after) = reqs.split_at_mut(i);
        let (last, this) = (&mut before[i - 1], &mut after[0]);
        for d in [DOWN, UP] {
            let (last, this) = (&mut last[ix(d)], &mut this[ix(d)]);
            // A column has heads on one side of the staff only, often.
            let outside = |p: i32| !contains(staff_extent, f64::from(p));
            if !outside(last.max_position) || !outside(this.max_position) {
                continue;
            }
            // Midway between the furthest bounds of the two heads.
            let center = (last.max_head_extent.right + this.max_head_extent.left) / 2.0;
            // Both further than the first space beyond the staff (the
            // tilt of quarter note heads)?
            let both = outside(last.max_position - sign(last.max_position))
                && outside(this.max_position - sign(this.max_position));
            for (which, lr) in [(LEFT, &mut *last), (RIGHT, &mut *this)] {
                let w = f64::from(which);
                let limit = center + if both { w * gap / 2.0 } else { 0.0 };
                let end = lr.max_ledger_extent.at_mut(-which);
                *end = w * max(w * *end, w * limit);
            }
        }
    }

    // Each head's ledgers, as wide as its column's allow, shortened by
    // its accidental.
    for dir_reqs in &mut reqs {
        for d in [DOWN, UP] {
            let lr = &mut dir_reqs[ix(d)];
            let mut extents = Vec::new();
            for hd in &mut lr.heads {
                if !lr.max_ledger_extent.is_empty() {
                    hd.ledger_extent.intersect(lr.max_ledger_extent);
                }
                for &lpos in &hd.ledger_positions {
                    let mut x_extent = hd.ledger_extent;
                    if contains(hd.ledger_shortening_range, lpos - f64::from(hd.position))
                        && !hd.accidental_extent.is_empty()
                    {
                        let dist = (hd.accidental_extent.right + hd.head_extent.left) / 2.0;
                        let left_shorten = max(-hd.ledger_extent.left + dist, 0.0);
                        x_extent.left += left_shorten;
                    }
                    if !x_extent.is_empty() {
                        extents.push((lpos, x_extent));
                    }
                }
            }
            for (lpos, x) in extents {
                lr.push_extent(lpos, x);
            }
        }
    }

    // Read before the stencil is made, which nothing roots.
    let thickness = get_ledger_line_thickness(host, staff)?;
    let me_x = grob::relative_coordinate(host, me, common_x, X)?;
    let half_thickness = thickness * 0.5;
    let y_extent = Interval::new(-half_thickness, half_thickness);
    let mut warnings = Vec::new();
    let heap = host.heap();
    let mut ledgers = Stencil::default();
    for dir_reqs in &reqs {
        for d in [DOWN, UP] {
            for (lpos, extents) in &dir_reqs[ix(d)].ledger_extents {
                // Ledgers at one position that overlap are one stencil.
                let ivs = extents.iter().map(|x| (x.left, x.right)).collect();
                for (l, r) in interval_union(ivs) {
                    let b = Box::new(Interval::new(l, r), y_extent);
                    let mut line = lookup::round_filled_box(heap, b, thickness, &mut warnings);
                    line.translate_axis(heap, lpos * halfspace, Y);
                    ledgers.add_stencil(heap, &line);
                }
            }
        }
    }
    ledgers.translate_axis(heap, -me_x, X);
    let ledgers = heap.wrap(ledgers);
    // What a warning could free: the stencil.
    let depth = heap.root_depth();
    heap.root(ledgers);
    let warned = warnings
        .iter()
        .try_for_each(|text| grob::unlocated_warning(host, text));
    host.heap().unroot_to(depth);
    warned.map(|()| ledgers)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:ledger-line-spanner::` primitives
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "ledger-line-spanner",
    f: host_ledger_line_spanner,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 2] =
    [("print", print), ("set-spacing-rods", set_spacing_rods)];

fn host_ledger_line_spanner(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such ledger line spanner callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:ledger-line-spanner::… grob)`: LilyPond's subrs, primitives here
/// so that they print as LilyPond's. Each checks its grob and tail-calls
/// `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let spanner = cx.heap_mut().intern("ledger-line-spanner");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[spanner, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:ledger-line-spanner::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_print "print",
    ly_set_spacing_rods "set-spacing-rods",
}
