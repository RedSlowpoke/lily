//! `item.cc` and `item-scheme.cc` (MS3 step 14): a grob's class, which
//! LilyPond's `Grob` subclasses are, and an item's break direction, its
//! column and the slots of its broken copies.
//!
//! - [`Class`]: `Item`, `Paper_column`, `Spanner` or `System`, kept with
//!   layout's part of the grob. A grob a Scheme engraver makes
//!   (`ly:engraver-make-grob`) takes the one class of its definition's
//!   `meta.classes`, as `Engraver::choose_grob_type<Grob>` does, the first
//!   time layout reaches it ([`class`]); layout's engravers set theirs
//!   ([`set_class`]), as `make_item`, `make_column` and `make_spanner` do.
//!   Either way the grob takes the interfaces its class's constructors add
//!   (`item-interface`, `paper-column-interface`, `spanner-interface`,
//!   `system-interface`).
//! - An item's break direction (`break_status_dir`), whether it is broken,
//!   its prebroken pieces and its column (`get_column`), with
//!   `ly:item?`, `ly:item-break-dir` and `ly:item-get-column`. Its broken
//!   copies and the original a copy was made from are the slots of
//!   layout's grob (`Grob::broken`, `Grob::original`), which
//!   `break_breakable_item` fills (MS3 step 32, pulled forward from step
//!   33 with `clone`, `handle_prebroken_dependencies` and `break_visible`,
//!   [`break_breakable_item`]); a traced compile makes the pieces
//!   (`dump.rs`), the per-edit compile does not yet.
//!
//! - `get_system` ([`get_system`]) and `spanned_column_rank_interval`
//!   ([`spanned_column_rank_interval`]), which `ly:grob-system` and
//!   `ly:grob-spanned-column-rank-interval` give (`grob.rs`): an item's
//!   through its parent on X and its column, a spanner's and a system's
//!   through its bounds (`spanner.rs`, MS3 step 15). A column's system and
//!   rank are kept with its grob (`paper_column.rs`): no system until
//!   breaking sets it, and the rank `VPOS` until `System::add_column` gives
//!   it one, as `Paper_column_engraver` does for each column it makes.
//! - `find_broken_piece` ([`find_broken_piece`], MS3 step 15): the item, or
//!   its broken copy, on a system; `spanned_system_rank_interval`
//!   ([`spanned_system_rank_interval`], MS3 step 33): the ranks of the
//!   systems it, or its pieces, are on.
//! - `is_non_musical` ([`is_non_musical`], MS3 step 15), which puts an item
//!   on its timestep's command or musical column.
//! - `pure_y_extent` and `cache_pure_height` (`item.cc:241–259`): an item's
//!   pure height kept relative to itself, `unpure.rs`' (MS3 step 14) over
//!   `Grob::pure_height`. A broken copy keeps none of its original's, as
//!   the copy constructor leaves `cached_pure_height_valid_` false
//!   ([`clone`]; checked over several lines at MS3 step 60, OPEN B4).

use core::cell::RefCell;

use lily_engine_music::{Arena, Host, clone_grob, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, Tag, Value};

use crate::R;
use crate::break_substitution::{self, Criterion};
use crate::geometry::{Direction, X};
use crate::grob::{self, grobs};
use crate::paper_column;
use crate::spanner;
use crate::system;

/// LilyPond's grob classes: `Item`, its `Paper_column`, `Spanner` and its
/// `System`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Item,
    PaperColumn,
    Spanner,
    System,
}

impl Class {
    /// `class_name`, as `meta.classes` names it.
    pub fn name(self) -> &'static str {
        match self {
            Class::Item => "Item",
            Class::PaperColumn => "Paper_column",
            Class::Spanner => "Spanner",
            Class::System => "System",
        }
    }

    fn named(name: &str) -> Option<Class> {
        [
            Class::Item,
            Class::PaperColumn,
            Class::Spanner,
            Class::System,
        ]
        .into_iter()
        .find(|c| c.name() == name)
    }

    /// The interfaces its constructors add, in the order they add them.
    pub(crate) fn interfaces(self) -> &'static [&'static str] {
        match self {
            Class::Item => &["item-interface"],
            Class::PaperColumn => &["item-interface", "paper-column-interface"],
            Class::Spanner => &["spanner-interface"],
            Class::System => &["spanner-interface", "system-interface"],
        }
    }

    /// Whether it is an `Item` (`dynamic_cast<Item *>`).
    pub fn is_item(self) -> bool {
        matches!(self, Class::Item | Class::PaperColumn)
    }

    /// Whether it is a `Spanner` (`dynamic_cast<Spanner *>`).
    pub fn is_spanner(self) -> bool {
        matches!(self, Class::Spanner | Class::System)
    }
}

/// The class of the grob `id` of `arena`, whose handle is `grob`: the one
/// it was given, else, the first time it is asked for, the one class of
/// its definition's `meta.classes`, which it then takes with its
/// interfaces. `None` for none, or more than one (LilyPond's error for an
/// engraver that does not name the class it makes).
pub(crate) fn classify(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    id: u32,
    grob: Value,
) -> Option<Class> {
    {
        let mut a = arena.borrow_mut();
        let slot = grobs(&mut a);
        // Read first: a write copies a row N8's snapshots share (D259), and
        // the whole score comes through here in one call as a translation
        // ends (`want_fonts`).
        let g = slot.get(id)?;
        if g.classified {
            return g.class;
        }
        slot.get_mut(id)?.classified = true;
    }
    let meta = heap.intern("meta");
    let meta = grob::property_data(heap, arena, id, grob, meta);
    let key = heap.intern("classes");
    let classes = grob::assq_ref(heap, key, meta).unwrap_or(Value::NULL);
    let only = match (heap.car(classes), heap.cdr(classes)) {
        (Some(c), Some(rest)) if rest == Value::NULL && c.tag() == Tag::Symbol => {
            heap.name(c).and_then(Class::named)
        }
        _ => None,
    };
    if let Some(class) = only {
        set(heap, arena, id, class);
    }
    only
}

/// `class` given to the grob `id`, with its interfaces.
fn set(heap: &mut Heap, arena: &RefCell<Arena>, id: u32, class: Class) {
    if let Some(g) = grobs(&mut arena.borrow_mut()).get_mut(id) {
        (g.class, g.classified) = (Some(class), true);
    }
    for name in class.interfaces() {
        let sym = heap.intern(name);
        grob::add_interface(arena, id, sym);
    }
}

/// The class of the grob `grob`, if it is a grob and has one ([`classify`]).
pub fn class(heap: &mut Heap, grob: Value) -> Option<Class> {
    let (arena, id) = grob_of(heap, grob)?;
    classify(heap, &arena, id, grob)
}

/// The grob `grob` made a `class`, as LilyPond's engravers make an item, a
/// column or a spanner: its class and the interfaces its constructors add.
pub fn set_class(heap: &mut Heap, grob: Value, class: Class) {
    if let Some((arena, id)) = grob_of(heap, grob) {
        set(heap, &arena, id, class);
    }
}

/// Whether `grob` is an `Item` (`unsmob<Item>`): an item or a column.
pub fn is_item(heap: &mut Heap, grob: Value) -> bool {
    class(heap, grob).is_some_and(Class::is_item)
}

/// What `f` reads of layout's part of the grob `grob`.
fn read<T>(heap: &Heap, grob: Value, f: impl FnOnce(&grob::Grob) -> T) -> Option<T> {
    let (arena, id) = grob_of(heap, grob)?;
    let mut a = arena.borrow_mut();
    grobs(&mut a).get(id).map(f)
}

/// `break_status_dir`: of a broken copy, -1 if it is its original's left
/// piece (the end of a line) and 1 if it is not (the beginning of one); 0
/// for an item that is no copy.
pub fn break_status_dir(heap: &Heap, item: Value) -> Direction {
    let Some(original) = read(heap, item, |g| g.original).flatten() else {
        return 0;
    };
    if read(heap, original, |g| g.broken[0]).flatten() == Some(item) {
        -1
    } else {
        1
    }
}

/// `is_broken`: whether the item has a broken copy.
pub fn is_broken(heap: &Heap, item: Value) -> bool {
    read(heap, item, |g| g.broken.iter().any(Option::is_some)).unwrap_or(false)
}

/// `find_prebroken_piece`: the item itself for 0, else its broken copy at
/// `d`, if it has one.
pub fn find_prebroken_piece(heap: &Heap, item: Value, d: Direction) -> Option<Value> {
    match d {
        0 => Some(item),
        _ => read(heap, item, |g| g.broken[usize::from(d > 0)]).flatten(),
    }
}

/// `get_column`: a column itself, else its parent's on X while that is an
/// item; `None` for none.
pub fn get_column(heap: &mut Heap, item: Value) -> Option<Value> {
    let mut g = item;
    loop {
        if class(heap, g) == Some(Class::PaperColumn) {
            return Some(g);
        }
        let parent = grob::get_parent(heap, g, X)?;
        if !is_item(heap, parent) {
            return None;
        }
        g = parent;
    }
}

/// `get_system`: an item's parent's on X, if it has one; a column's
/// `system_` (`paper_column.rs`), none until breaking sets it; a spanner's
/// or a system's, its bounds' (`spanner.rs`); none for a grob of no class.
pub fn get_system(heap: &mut Heap, grob: Value) -> Option<Value> {
    let mut g = grob;
    loop {
        match class(heap, g) {
            Some(Class::Item) => g = grob::get_parent(heap, g, X)?,
            Some(Class::PaperColumn) => return paper_column::system(heap, g),
            Some(Class::Spanner | Class::System) => return spanner::get_system(heap, g),
            None => return None,
        }
    }
}

/// `find_broken_piece`: the item if it is on the system `l`, else its
/// broken copy that is, if one is.
pub fn find_broken_piece(heap: &mut Heap, item: Value, l: Value) -> Option<Value> {
    if get_system(heap, item) == Some(l) {
        return Some(item);
    }
    let broken = read(heap, item, |g| g.broken)?;
    broken
        .into_iter()
        .flatten()
        .find(|&s| get_system(heap, s) == Some(l))
}

/// `is_non_musical`: its parent's on X while that is an item, else its own
/// `non-musical`.
pub fn is_non_musical(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let mut g = me;
    while let Some(parent) = grob::get_parent(host.heap(), g, X) {
        if !is_item(host.heap(), parent) {
            break;
        }
        g = parent;
    }
    Ok(grob::property(host, g, "non-musical")? == Value::TRUE)
}

// --- the pieces before line breaking (MS3 step 32) -------------------------

/// `clone` of the item `item`: the copy `Item::Item (Item const &)` makes
/// over `Grob`'s ([`grob::copied`]), with no pieces and no pure height of
/// its own, and `Paper_column`'s for a column ([`paper_column::copied`]).
/// The item is made first ([`grob::make`]), as LilyPond's is when it is
/// copied. `None` for what is not a grob.
pub fn clone(host: &mut dyn Host, item: Value) -> Option<Value> {
    grob::make(host, item);
    let heap = host.heap();
    let copy = clone_grob(heap, item)?;
    grob::copied(heap, copy, item);
    if class(heap, item) == Some(Class::PaperColumn) {
        paper_column::copied(heap, copy, item);
    }
    Some(copy)
}

/// `break_breakable_item` (`item.cc:74–101`; `Grob`'s, a spanner's, does
/// nothing): a non-musical item copied for the end of a line and for the
/// beginning of the next, each copy typeset in `sys`, and the copies
/// made its pieces; LilyPond's programming errors for an item broken
/// already and for a copy.
pub fn break_breakable_item(host: &mut dyn Host, me: Value, sys: Value) -> Result<(), Exception> {
    let heap = host.heap();
    if !is_item(heap, me) {
        return Ok(());
    }
    if is_broken(heap, me) {
        return grob::programming_error(host, me, "item is already broken");
    }
    if read(heap, me, |g| g.original).flatten().is_some() {
        return grob::programming_error(host, me, "item is a clone; refusing to break");
    }
    if is_non_musical(host, me)? {
        let mut new_copies = [None; 2];
        for copy in &mut new_copies {
            let Some(item) = clone(host, me) else {
                return Ok(());
            };
            system::typeset_grob(host, sys, item)?;
            *copy = Some(item);
        }
        grob::with_grob_mut(host.heap(), me, |g| g.broken = new_copies);
    }
    Ok(())
}

/// `handle_prebroken_dependencies` of an item, `Item`'s
/// (`item.cc:137–143`) over `Grob`'s (`grob.cc:341–352`): a piece's
/// objects made its original's with each grob in them replaced by its
/// piece on the piece's side (`substitute_object_links`,
/// `break_substitution.rs`); then the item killed if it is not visible
/// on its side of a break ([`break_visible`]). `Grob`'s alone, a
/// spanner's, does nothing before line breaking, when no spanner is a
/// copy.
pub fn handle_prebroken_dependencies(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let heap = host.heap();
    if !is_item(heap, me) {
        return Ok(());
    }
    if let Some(original) = read(heap, me, |g| g.original).flatten() {
        let crit = Criterion::Direction(break_status_dir(heap, me));
        let orig = read(heap, original, |g| g.objects).unwrap_or(Value::NULL);
        break_substitution::substitute_object_links(heap, me, crit, orig);
    }
    if !break_visible(host, me)? {
        grob::suicide(host.heap(), me);
    }
    Ok(())
}

/// `break_visible` (`item.cc:145–155`): if the item's `break-visibility`
/// is a vector, its entry for the item's break direction (-1, 0, 1 the
/// first, second and third), true only for `#t` (`from_scm<bool>`), and
/// false for a vector too short, whose entry Guile does not have; else
/// true.
pub fn break_visible(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let vis = grob::property(host, me, "break-visibility")?;
    let heap = host.heap();
    let index = (break_status_dir(heap, me) + 1) as usize;
    Ok(match heap.vector_items(vis) {
        Some(entries) => entries.get(index) == Some(&Value::TRUE),
        None => true,
    })
}

/// `VPOS`: a column's rank before `add_column` gives it one.
pub const VPOS: u64 = u64::MAX;

/// `spanned_column_rank_interval`: an item's column's rank twice, or, for
/// an item without a column, the empty `Interval_t<vsize>`, `(VPOS . 0)`;
/// a spanner's bounds' columns' ranks (`spanner.rs`). A column's rank is
/// [`VPOS`] until it is added to a system. A grob of no class, which
/// LilyPond does not have, gives the empty interval.
pub fn spanned_column_rank_interval(heap: &mut Heap, grob: Value) -> (u64, u64) {
    match class(heap, grob) {
        Some(Class::Spanner | Class::System) => spanner::spanned_column_rank_interval(heap, grob),
        Some(_) => match get_column(heap, grob) {
            Some(col) => {
                let rank = paper_column::rank(heap, col);
                (rank, rank)
            }
            None => (VPOS, 0),
        },
        None => (VPOS, 0),
    }
}

/// `spanned_system_rank_interval`: an item's system's rank twice; else
/// the ranks of its prebroken pieces' systems, the empty
/// `System_rank_interval`, `(VPOS . 0)`, if none has one (MS3 step 33).
/// A spanner's and a system's are its own or its pieces'
/// (`spanner.rs`); a grob of no class gives the empty interval.
pub fn spanned_system_rank_interval(heap: &mut Heap, grob: Value) -> (u64, u64) {
    match class(heap, grob) {
        Some(Class::Spanner | Class::System) => spanner::spanned_system_rank_interval(heap, grob),
        Some(_) => {
            if let Some(st) = get_system(heap, grob) {
                let rank = system::get_rank(heap, st);
                return (rank, rank);
            }
            let mut sr = (VPOS, 0);
            for d in [-1, 1] {
                let bi = find_prebroken_piece(heap, grob, d);
                if let Some(st) = bi.and_then(|bi| get_system(heap, bi)) {
                    let rank = system::get_rank(heap, st);
                    sr = (sr.0.min(rank), sr.1.max(rank));
                }
            }
            sr
        }
        None => (VPOS, 0),
    }
}

/// `LY_ASSERT_SMOB (Item, v, position)` of `subr`.
pub(crate) fn item_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<(), lily_engine_scheme::Thrown> {
    if grob_of(cx.heap(), v).is_some() {
        grob::grob_arg(cx, subr, position, v)?;
        if is_item(cx.heap_mut(), v) {
            return Ok(());
        }
    }
    Err(cx.wrong_type(subr, position, v, "Item"))
}

/// `(ly:item? g)`.
pub(crate) fn ly_item_p(cx: &mut Cx, args: &[Value]) -> R {
    if grob_of(cx.heap(), args[0]).is_none() {
        return Ok(Value::FALSE);
    }
    grob::grob_arg(cx, "ly:item?", 1, args[0])?;
    Ok(Value::bool(is_item(cx.heap_mut(), args[0])))
}

/// `(ly:item-break-dir it)`.
pub(crate) fn ly_item_break_dir(cx: &mut Cx, args: &[Value]) -> R {
    item_arg(cx, "ly:item-break-dir", 1, args[0])?;
    let d = break_status_dir(cx.heap(), args[0]);
    Ok(Value::fixnum(i64::from(d)).unwrap_or(Value::NULL))
}

/// `(%item-prebroken-piece it d)`: [`find_prebroken_piece`], or `#f`.
pub(crate) fn prebroken_piece(cx: &mut Cx, args: &[Value]) -> R {
    item_arg(cx, "%item-prebroken-piece", 1, args[0])?;
    let d = crate::direction_arg(cx, "%item-prebroken-piece", 2, args[1])?;
    Ok(find_prebroken_piece(cx.heap(), args[0], d).unwrap_or(Value::FALSE))
}

/// `(ly:item-get-column it)`: its column, or `'()`.
pub(crate) fn ly_item_get_column(cx: &mut Cx, args: &[Value]) -> R {
    item_arg(cx, "ly:item-get-column", 1, args[0])?;
    Ok(get_column(cx.heap_mut(), args[0]).unwrap_or(Value::NULL))
}
