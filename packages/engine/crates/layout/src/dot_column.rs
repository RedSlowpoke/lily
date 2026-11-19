//! `dot-column.cc` (part 3's, MS3): `add_head` (`dot-column.cc:236–264`,
//! 29 lines), which `Dot_column_engraver` calls, ported first (MS3 step
//! 16), with `Side_position_interface::add_support` (`side_position.rs`),
//! which it calls; and the dots' positioning (MS3 step 37),
//! `ly:dot-column::calc-positioning-done` (`:41–234`, 194), the music
//! crate's placeholder until then (D67), with `dot-configuration.cc` (137)
//! and `dot-formatting-problem.cc` (28): Rust over `%host-call` (D127,
//! [`HOSTS`]), as it sorts, shifts and scores configurations of dots over
//! a skyline of the heads.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction as Sky, Skyline};

use crate::axis_group;
use crate::direction;
use crate::geometry::{Direction, Interval, X, Y};
use crate::grob;
use crate::grob_array;
use crate::side_position;
use crate::stem;

/// `add_head`: where the rhythmic head `head` has dots, `head` a support of
/// the dot column `me`, the dots added to its `dots` and made its element,
/// their `Y-offset` `ly:grob::x-parent-positioning`. A note head's dots
/// take it as their `X-offset`; a rest's are moved right by the rest's
/// width and `me`'s `padding` (a number, else Guile's type error), which
/// `calc_positioning_done` undoes.
pub fn add_head(host: &mut dyn Host, me: Value, head: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("dot");
    let d = grob_array::object(host, head, sym)?;
    if grob_of(host.heap(), d).is_none() {
        return Ok(());
    }
    side_position::add_support(host, me, head)?;
    let sym = host.heap().intern("dots");
    grob_array::add_grob(host, me, sym, d)?;
    let proc = crate::lily_procedure(host, "ly:grob::x-parent-positioning")?;
    grob::set_property(host.heap(), d, "Y-offset", proc);
    if grob::has_interface(host.heap(), head, "rest-interface") {
        let width = grob::extent(host, head, head, X)?.length();
        let padding = grob::property(host, me, "padding")?;
        let padding = grob::to_double(host.heap(), padding)?;
        if let Err(text) = grob::translate_axis(host.heap(), d, width + padding, X) {
            grob::programming_error(host, d, text)?;
        }
    } else {
        grob::set_property(host.heap(), d, "X-offset", proc);
    }
    axis_group::add_element(host, me, d)
}

/// `Dot_position`: a dot at a staff position, and the direction it would
/// rather move in, its own `direction` if its head is a note head.
#[derive(Clone, Copy)]
struct DotPosition {
    pos: i32,
    dir: Direction,
    dot: Value,
}

/// `Dot_configuration`: the dots by the staff position each is put at.
type Configuration = BTreeMap<i32, DotPosition>;

/// `Dot_configuration::badness`: for each dot, twice the square of how
/// far it is from its own position, and 2 more moved against its
/// direction, else 1 more moved down or not at all.
fn badness(cfg: &Configuration) -> i32 {
    let mut t = 0;
    for (&p, ent) in cfg {
        let mut demerit = (p - ent.pos) * (p - ent.pos) * 2;
        let dot_move_dir = (p - ent.pos).signum();
        if ent.dir != 0 && dot_move_dir != ent.dir {
            demerit += 2;
        } else if dot_move_dir != 1 {
            demerit += 1;
        }
        t += demerit;
    }
    t
}

/// `Staff_symbol_referencer::on_line`: `%staff-on-line?`, the ledgers
/// asked.
fn on_line(host: &mut dyn Host, me: Value, pos: i32) -> Result<bool, Exception> {
    let p = host.heap().integer(i64::from(pos));
    Ok(crate::call_lily(host, "%staff-on-line?", &[me, p])? != Value::FALSE)
}

/// `Dot_configuration::shifted`: `cfg` with the dot at `k` moved off the
/// staff lines towards `d`, one position from a line and two from a
/// space, and the dots after it that way moved as far as they would
/// collide.
fn shifted(
    host: &mut dyn Host,
    cfg: &Configuration,
    k: i32,
    d: Direction,
) -> Result<Configuration, Exception> {
    let mut new_cfg = Configuration::new();
    let mut offset = 0;
    let mut process_entry = |host: &mut dyn Host, p: i32, ent: DotPosition| {
        if p == k {
            let p = if on_line(host, ent.dot, p)? {
                p + d
            } else {
                p + 2 * d
            };
            offset = 2 * d;
            new_cfg.insert(p, ent);
        } else {
            if !new_cfg.contains_key(&p) {
                offset = 0;
            }
            new_cfg.insert(p + offset, ent);
        }
        Ok::<(), Exception>(())
    };
    if d > 0 {
        for (&p, &ent) in cfg {
            process_entry(host, p, ent)?;
        }
    } else {
        for (&p, &ent) in cfg.iter().rev() {
            process_entry(host, p, ent)?;
        }
    }
    Ok(new_cfg)
}

/// `Dot_configuration::remove_collision`: if a dot is at `p`, `cfg`
/// shifted up or down from it, whichever is less bad, down at a tie.
fn remove_collision(host: &mut dyn Host, cfg: &mut Configuration, p: i32) -> Result<(), Exception> {
    if cfg.contains_key(&p) {
        let cfg_up = shifted(host, cfg, p, 1)?;
        let cfg_down = shifted(host, cfg, p, -1)?;
        *cfg = if badness(&cfg_up) < badness(&cfg_down) {
            cfg_up
        } else {
            cfg_down
        };
    }
    Ok(())
}

/// `Dot_configuration::x_offset`: how far right the heads reach at the
/// dots' positions, at least 0 (`Dot_formatting_problem`'s skyline).
fn x_offset(cfg: &Configuration, head_skyline: &Skyline) -> f64 {
    let mut off = 0.0;
    for &p in cfg.keys() {
        let h = head_skyline.height(f64::from(p));
        // `std::max (off, h)`.
        off = if off < h { h } else { off };
    }
    off
}

/// `common_refpoint` on X of `a` and `b`, none for no `b`.
fn common_x(host: &mut dyn Host, a: Value, b: Option<Value>) -> Option<Value> {
    b.and_then(|b| grob::common_refpoint(host.heap(), a, b, X))
}

/// `extent` on X of `g` relative to `commonx`, its root for none.
fn x_extent(host: &mut dyn Host, g: Value, commonx: Option<Value>) -> Result<Interval, Exception> {
    grob::extent(host, g, commonx.unwrap_or(Value::NULL), X)
}

/// `Box`, in a skyline's form.
fn sky_box(x: Interval, y: Interval) -> lily_engine_fonts::Box {
    let i = |i: Interval| lily_engine_fonts::Interval::new(i.left, i.right);
    lily_engine_fonts::Box { x: i(x), y: i(y) }
}

/// `%staff-position` of `g`, pure or not.
fn staff_position(host: &mut dyn Host, g: Value, pure: bool) -> Result<f64, Exception> {
    crate::staff_symbol_referencer::get_position(host, g, pure)
}

/// `calc_positioning_done`: the dots of the dot column `me` put each at a
/// staff position off the staff lines, apart from each other and as near
/// its head's as may be, and `me` moved right of the heads there (its
/// supports, the stems' first heads and their flags) by its `padding`;
/// the note collision positioned first, which may kill dots, and of a
/// stem's dots beyond its `chord-dots-limit` those at the ends killed.
///
/// A dot of no Y parent is passed over where LilyPond would crash. The
/// stems whose flags are boxes are taken in the order they are met, where
/// LilyPond's `std::unordered_set` takes them in its own: that matters
/// only where two flags' boxes tie in the skyline's sort.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("note-collision");
    let collision = grob_array::object(host, me, sym)?;
    if grob_of(host.heap(), collision).is_some() {
        grob::property(host, collision, "positioning-done")?;
    }
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);
    let mut dots = grob_array::extract_grob_array(host, me, "dots")?;
    let mut parent_stems = Vec::new();
    let mut ss = 0.0;
    let mut commonx = Some(me);
    let stem_sym = host.heap().intern("stem");
    for &d in &dots {
        let Some(n) = grob::get_parent(host.heap(), d, Y) else {
            continue;
        };
        commonx = common_x(host, n, commonx);
        let s = grob_array::object(host, n, stem_sym)?;
        if grob_of(host.heap(), s).is_some() {
            commonx = common_x(host, s, commonx);
            if stem::first_head(host, s)? == Some(n) {
                parent_stems.push(s);
            }
        }
    }
    let mut boxes = Vec::new();
    let mut stems: Vec<Value> = Vec::new();
    let support = grob_array::extract_grob_array(host, me, "side-support-elements")?;
    let mut base_x = Interval::EMPTY;
    for &st in &parent_stems {
        if let Some(h) = stem::first_head(host, st)? {
            base_x.unite(x_extent(host, h, commonx)?);
        }
    }
    for &s in &support {
        if ss == 0.0 {
            ss = crate::staff_symbol_referencer::staff_space(host, s)?;
        }
        // A rest's Y extent waits for the line breaking.
        let mut y = Interval::EMPTY;
        if grob::has_interface(host.heap(), s, "rest-interface") {
            base_x.unite(x_extent(host, s, commonx)?);
            continue;
        } else if grob::has_interface(host.heap(), s, "stem-interface") {
            let d = direction::get_grob_direction(host, s)?;
            let y1 = stem::head_positions(host, s)?.at(-d);
            let y2 = y1 + f64::from(d) * 7.0;
            y.add_point(y1);
            y.add_point(y2);
            if !stems.contains(&s) {
                stems.push(s);
            }
        } else if grob::has_interface(host.heap(), s, "note-head-interface") {
            y = Interval::new(-1.1, 1.1);
        } else {
            grob::unlocated_programming_error(host, "unknown grob in dot col support")?;
            continue;
        }
        y.translate(staff_position(host, s, false)?);
        let b = sky_box(x_extent(host, s, commonx)?, y);
        boxes.push(b);
        let st = grob_array::object(host, s, stem_sym)?;
        if grob_of(host.heap(), st).is_some() && !stems.contains(&st) {
            stems.push(st);
        }
    }
    let flag_sym = host.heap().intern("flag");
    for &st in &stems {
        let flag = grob_array::object(host, st, flag_sym)?;
        if grob_of(host.heap(), flag).is_some() {
            let commony = grob::common_refpoint(host.heap(), st, flag, Y);
            let mut y = grob::extent(host, flag, commony.unwrap_or(Value::NULL), Y)?;
            y.scale(2.0 / ss);
            let x = x_extent(host, flag, commonx)?;
            boxes.push(sky_box(x, y));
        }
    }
    // By their pure positions: before the line breaking a dot's Y may
    // follow a rest's, which a beam would move.
    if dots.len() > 1 {
        let mut keyed = Vec::with_capacity(dots.len());
        for &d in &dots {
            keyed.push((staff_position(host, d, true)?, d));
        }
        lily_engine_skyline::sort(&mut keyed, &|a: &(f64, Value), b: &(f64, Value)| a.0 < b.0);
        dots = keyed.into_iter().map(|(_, d)| d).collect();
    }
    let chord_dots_limit = grob::property(host, me, "chord-dots-limit")?;
    if host.heap().is_number(chord_dots_limit) {
        // The dots by stem, then those over the limit of each.
        let mut dots_each_stem: Vec<Vec<Value>> = alloc::vec![Vec::new(); parent_stems.len()];
        for &d in &dots {
            let Some(n) = grob::get_parent(host.heap(), d, Y) else {
                continue;
            };
            let st = grob_array::object(host, n, stem_sym)?;
            if grob_of(host.heap(), st).is_some()
                && let Some(j) = parent_stems.iter().position(|&p| p == st)
            {
                dots_each_stem[j].push(d);
            }
        }
        let limit = crate::call_lily(host, "%scm->vsize", &[chord_dots_limit])?;
        let limit = host.heap().to_i64(limit).map_or(u64::MAX, |n| n as u64);
        for (j, &st) in parent_stems.iter().enumerate() {
            let chord = stem::head_positions(host, st)?;
            let total_room = (chord.length() as u64).wrapping_add(2).wrapping_add(limit) / 2;
            let mut total_dots = dots_each_stem[j].len() as u64;
            // The excess removed from the ends of the stem.
            let mut first_dot = 0;
            while total_dots > total_room {
                if (total_dots - total_room).is_multiple_of(2) {
                    grob::suicide(host.heap(), dots_each_stem[j][first_dot]);
                    first_dot += 1;
                } else {
                    let last = first_dot + total_dots as usize - 1;
                    grob::suicide(host.heap(), dots_each_stem[j][last]);
                }
                total_dots -= 1;
            }
        }
    }
    let mut i = dots.len();
    while i > 0 {
        i -= 1;
        if !grob::is_live_grob(host.heap(), dots[i]) {
            dots.remove(i);
        } else {
            // add_head's translation undone.
            let x = grob::relative_coordinate(host, dots[i], me, X)?;
            if let Err(text) = grob::translate_axis(host.heap(), dots[i], -x, X) {
                grob::programming_error(host, dots[i], text)?;
            }
        }
    }
    // `Dot_formatting_problem`: the heads' skyline, right of them by Y.
    let mut head_skyline = Skyline::from_boxes(&boxes, Horizon::Y, Sky::Up);
    head_skyline.set_minimum_height(base_x.right);
    let mut cfg = Configuration::new();
    let style_sym = host.heap().intern("kievan");
    for &dot in &dots {
        let mut dp = DotPosition {
            pos: 0,
            dir: 0,
            dot,
        };
        if let Some(note) = grob::get_parent(host.heap(), dot, Y) {
            if grob::has_interface(host.heap(), note, "note-head-interface") {
                dp.dir = get_dir(host, dot)?;
            }
            x_extent(host, note, commonx)?;
        }
        let v = crate::staff_symbol_referencer::rounded_position(host, dot, true)?;
        let mut p = stem::to_int(host.heap(), v)?;
        // Not by the staff-position's offset callback: a dot's Y-offset is
        // its column's positioning.
        let offset = grob::property(host, dot, "staff-position")?;
        let offset = if host.heap().is_number(offset) {
            grob::to_double(host.heap(), offset)?
        } else {
            0.0
        };
        p = p.wrapping_add(offset as i32);
        dp.pos = p;
        remove_collision(host, &mut cfg, p)?;
        cfg.insert(p, dp);
        if on_line(host, dot, p)? && grob::property(host, dot, "style")? != style_sym {
            remove_collision(host, &mut cfg, p)?;
        }
    }
    for (&p, ent) in &cfg {
        crate::staff_symbol_referencer::set_position(host, ent.dot, f64::from(p), true)?;
    }
    let off = x_offset(&cfg, &head_skyline);
    let rel = grob::relative_coordinate(host, me, commonx.unwrap_or(Value::NULL), X)?;
    let padding = grob::property(host, me, "padding")?;
    let padding = if host.heap().is_number(padding) {
        grob::to_double(host.heap(), padding)?
    } else {
        0.0
    };
    if let Err(text) = grob::translate_axis(host.heap(), me, off - rel + padding, X) {
        grob::programming_error(host, me, text)?;
    }
    Ok(Value::TRUE)
}

/// `from_scm<Direction> (get_property (g, "direction"))`: -1, 0 or 1, 0
/// for anything else.
fn get_dir(host: &mut dyn Host, g: Value) -> Result<Direction, Exception> {
    let d = grob::property(host, g, "direction")?;
    Ok(crate::dynamic_align_engraver::direction(d))
}

/// What `%host-call` runs for [`PRELUDE`] (D127): [`calc_positioning_done`]
/// of the dot column given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "dot-column-calc-positioning-done",
    f: host_calc_positioning_done,
}];

fn host_calc_positioning_done(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    calc_positioning_done(vm, args[1])
}

/// `ly:dot-column::calc-positioning-done`, which checks its grob and calls
/// [`calc_positioning_done`] ([`HOSTS`]).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:dot-column::calc-positioning-done grob)
  (%check-type "ly:dot-column::calc-positioning-done" 1 ly:grob? "Grob" grob)
  (%host-call 'dot-column-calc-positioning-done grob))
"#;
