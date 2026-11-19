//! `note-column.cc` (part 3's, MS3): so far `set_stem`
//! (`note-column.cc:120–125`) and `add_head` (`:133–156`), 30 lines, which
//! `Rhythmic_column_engraver` calls, ported first (MS3 step 16); and what
//! the note spacing reads of it (MS3 step 31): `get_stem` (`:63–68`, 6),
//! `first_head` (`:158–163`, 6) and `accidentals` (`:209–230`, 22). And
//! the column's main extent (MS3 step 37), `ly:note-column::calc-main-extent`
//! (`:184–204`, 21), the music crate's placeholder until then (D67): Scheme
//! over `%host-call` (D127, [`HOSTS`]), as its first head is the stem's
//! ([`stem::first_head`]). And what the note collision reads of it (MS3
//! step 41): `has_rests` (`:46–50`, 5), `shift_less` (`:52–61`, 10),
//! `head_positions_interval` (`:82–98`, 17), `dir` (`:100–118`, 19),
//! `support_head` (`:172–177`, 6) and `dot_column` (`:232–243`, 12). The
//! slurs (MS4 step 7, borrowed for the slur scoring) read `get_flag`
//! (`:70–80`, 11), `get_rest` (`:127–131`, 5) and `extremal_heads`
//! (`:165–170`, 6). The rest of the file, the cross-staff extent, and
//! note-column-scheme.cc's two `ly:note-column-` functions, which no grob
//! of the engine reaches yet, go with their callers.

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::axis_group;
use crate::direction;
use crate::geometry::{Direction, X};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::stem;

/// `has_rests`: whether the note column `me`'s `rest` is a grob.
pub fn has_rests(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let sym = host.heap().intern("rest");
    let rest = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), rest).is_some())
}

/// What `shift_less` compares of a note column: its `horizontal-shift`,
/// converted as `scm_to_int` converts it, if it is a number, else 0.
pub fn horizontal_shift(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    let s = grob::property(host, me, "horizontal-shift")?;
    let heap = host.heap();
    if heap.is_number(s) {
        stem::to_int(heap, s)
    } else {
        Ok(0)
    }
}

/// `head_positions_interval`: the lowest and highest rounded staff
/// positions of `me`'s `note-heads`; without heads the empty `Slice`,
/// `(INT_MAX, -INT_MAX)`.
pub fn head_positions_interval(host: &mut dyn Host, me: Value) -> Result<[i32; 2], Exception> {
    let mut iv = [i32::MAX, -i32::MAX];
    for se in grob_array::extract_grob_array(host, me, "note-heads")? {
        let j = stem::rounded_position(host, se)?;
        iv = [iv[0].min(j), iv[1].max(j)];
    }
    Ok(iv)
}

/// `dir`: the strict direction of `me`'s stem; without one, the sign of
/// the centre of its heads' positions; without either, LilyPond's
/// programming error and 0.
pub fn dir(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    let sym = host.heap().intern("stem");
    let stem = grob_array::object(host, me, sym)?;
    if grob::has_interface(host.heap(), stem, "stem-interface") {
        return direction::get_strict_grob_direction(host, stem);
    }
    if !grob_array::extract_grob_array(host, me, "note-heads")?.is_empty() {
        let [l, r] = head_positions_interval(host, me)?;
        return Ok(((i64::from(l) + i64::from(r)) / 2).signum() as Direction);
    }
    let text = if grob::has_interface(host.heap(), me, "note-column-interface") {
        "Note_column without heads and stem"
    } else {
        "dir() given grob without Note_column interface"
    };
    grob::unlocated_programming_error(host, text)?;
    Ok(0)
}

/// `set_stem`: `stem` set as the note column `me`'s `stem` object and made
/// its element.
pub fn set_stem(host: &mut dyn Host, me: Value, stem: Value) -> Result<(), Exception> {
    let heap = host.heap();
    let sym = heap.intern("stem");
    grob::set_object(heap, me, sym, stem);
    axis_group::add_element(host, me, stem)
}

/// `add_head`: a rest, `me`'s `rest` unless it has note heads; a note
/// head, added to its `note-heads`. Either is made its element, or, where
/// a rest and note heads meet, LilyPond's warning at `me` instead. A grob
/// of neither interface is made its element.
pub fn add_head(host: &mut dyn Host, me: Value, h: Value) -> Result<(), Exception> {
    let mut both = false;
    if grob::has_interface(host.heap(), h, "rest-interface") {
        let heads = grob_array::extract_grob_array(host, me, "note-heads")?;
        if !heads.is_empty() {
            both = true;
        } else {
            let heap = host.heap();
            let sym = heap.intern("rest");
            grob::set_object(heap, me, sym, h);
        }
    } else if grob::has_interface(host.heap(), h, "note-head-interface") {
        let sym = host.heap().intern("rest");
        let rest = grob_array::object(host, me, sym)?;
        if grob_of(host.heap(), rest).is_some() {
            both = true;
        }
        let sym = host.heap().intern("note-heads");
        grob_array::add_grob(host, me, sym, h)?;
    }
    if both {
        grob::warning(
            host,
            me,
            "cannot have note heads and rests together on a stem",
        )
    } else {
        axis_group::add_element(host, me, h)
    }
}

/// `get_stem`: the note column `me`'s `stem`, if it is an item.
pub fn get_stem(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("stem");
    let s = grob_array::object(host, me, sym)?;
    Ok(item::is_item(host.heap(), s).then_some(s))
}

/// `get_flag`: the `flag` of its stem, if it is an item; none without a
/// stem.
pub fn get_flag(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let Some(stem) = get_stem(host, me)? else {
        return Ok(None);
    };
    let sym = host.heap().intern("flag");
    let s = grob_array::object(host, stem, sym)?;
    Ok(item::is_item(host.heap(), s).then_some(s))
}

/// `get_rest`: the note column `me`'s `rest`, if it is a grob.
pub fn get_rest(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("rest");
    let r = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), r).is_some().then_some(r))
}

/// `extremal_heads`: [`stem::extremal_heads`] of the column itself, as
/// LilyPond's is ("weird; see the implementation").
pub fn extremal_heads(host: &mut dyn Host, me: Value) -> Result<[Option<Value>; 2], Exception> {
    stem::extremal_heads(host, me)
}

/// `first_head`: its stem's first head ([`stem::first_head`]); none
/// without a stem.
pub fn first_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    match get_stem(host, me)? {
        Some(st) => stem::first_head(host, st),
        None => Ok(None),
    }
}

/// `support_head`: its stem's support head ([`stem::support_head`]);
/// none without a stem.
pub fn support_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    match get_stem(host, me)? {
        Some(st) => stem::support_head(host, st),
        None => Ok(None),
    }
}

/// `dot_column`: the X parent of the dots of the first of `me`'s
/// `note-heads` that has dots; none if no head has any.
pub fn dot_column(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("dot");
    for h in grob_array::extract_grob_array(host, me, "note-heads")? {
        let dots = grob_array::object(host, h, sym)?;
        if grob_of(host.heap(), dots).is_some() {
            return Ok(grob::get_parent(host.heap(), dots, X));
        }
    }
    Ok(None)
}

/// `accidentals`: the `accidental-grob` of the first of its `note-heads`
/// that has one, or its X parent if that is an accidental placement;
/// none if no head has one.
pub fn accidentals(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("accidental-grob");
    let mut acc = None;
    for h in grob_array::extract_grob_array(host, me, "note-heads")? {
        let a = grob_array::object(host, h, sym)?;
        if grob_of(host.heap(), a).is_some() {
            acc = Some(a);
            break;
        }
    }
    let Some(acc) = acc else {
        return Ok(None);
    };
    let heap = host.heap();
    match grob::get_parent(heap, acc, crate::geometry::X) {
        Some(p) if grob::has_interface(heap, p, "accidental-placement-interface") => Ok(Some(p)),
        _ => Ok(Some(acc)),
    }
}

/// `calc_main_extent`: the X extent, in `me`, of the column's main head
/// (with a stem its first head, else its first head of all; suspended
/// heads aside), else of its rest, else `(0 . 0)`.
pub fn calc_main_extent(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let main_head = if get_stem(host, me)?.is_some() {
        first_head(host, me)?
    } else {
        grob_array::extract_grob_array(host, me, "note-heads")?
            .first()
            .copied()
    };
    let main_item = match main_head {
        Some(h) => Some(h),
        None => {
            let sym = host.heap().intern("rest");
            let rest = grob_array::object(host, me, sym)?;
            grob_of(host.heap(), rest).is_some().then_some(rest)
        }
    };
    let e = match main_item {
        Some(g) => grob::extent(host, g, me, X)?,
        None => crate::geometry::Interval::new(0.0, 0.0),
    };
    Ok(crate::offset(host.heap(), (e.left, e.right)))
}

/// What `%host-call` runs for [`PRELUDE`] (D127): [`calc_main_extent`]
/// of the note column given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "note-column-calc-main-extent",
    f: host_calc_main_extent,
}];

fn host_calc_main_extent(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    calc_main_extent(vm, args[1])
}

/// `ly:note-column::calc-main-extent`, which checks its grob and calls
/// [`calc_main_extent`] ([`HOSTS`]).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:note-column::calc-main-extent grob)
  (%check-type "ly:note-column::calc-main-extent" 1 ly:grob? "Grob" grob)
  (%host-call 'note-column-calc-main-extent grob))
"#;
