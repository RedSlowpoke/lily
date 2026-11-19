//! 05 §7's queue (MS6 step 9, D251): the order a compile lays its pages
//! out in, and the points between its steps.
//!
//! - **The visible page** ([`set_visible_page`]) is the page the host shows,
//!   given with each edit (`lily_compile`'s `visible_page` less one, the
//!   first by default), counted over the compile's books.
//!   A book's pages are drawn ([`crate::page::make_pages`]) in [`order`]: the
//!   visible page, the pages after it, then those before it, the last page
//!   last, as it is spaced by the force of the page before it. What a page
//!   says is put back in the pages' order ([`Said`]), so a compile says what
//!   it says from the first page whatever the page shown. The pages' stencils
//!   and their SVG stay in the pages' order: though the engine returns between
//!   steps (H3, D254), a page is handed over with the others when the compile
//!   ends, and 05 §7's hand-over of the visible page first is not built.
//! - **A step** ends at a [`check`], which measures the time spent. The
//!   returns to the host (`lily_engine_music::returns`) are in the loops that
//!   hold their state, a subset of these: each grob's prebroken pieces, its
//!   `before-line-breaking`, `springs-and-rods`, break and substitution and
//!   `after-line-breaking`; each line given back, each column's springs,
//!   each box of a column's skylines, each pure offset and part of a line's
//!   heights, each line the alignment places, each outside-staff grob
//!   placed; the sub-steps of a cell (05 §7.3), as the callbacks of
//!   [`CALLBACKS`] end; line breaking, each page drawn, each grob's stencil
//!   of a system, each page's stencil and SVG. [`longest`] is the longest
//!   time between two checks of a compile, the collector's pauses in it left
//!   out (the collector's own step), against 05 §9's 20 ms, and what ended
//!   it.
//! - **A return (D261)** needs no Rust frame between the check and the step's
//!   loop. So a spacing spanner's read goes column by column
//!   ([`crate::spacing_spanner::Spacing`]), each line's pure heights are read
//!   before `springs-and-rods`, grob by grob and measure by measure, top line
//!   first ([`crate::axis_group::Heights`]), and a system's stencils grob by
//!   grob ([`crate::system::PaperSystem`]); and `%queue-check` in Scheme called
//!   from a call in steps returns from inside it (`Vm::begin_steps`), which
//!   divides the page's spacing problem between systems and staves. The check
//!   in a callback a read's Rust frames are under still only measures.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

static CLOCK: AtomicUsize = AtomicUsize::new(0);
static VISIBLE: AtomicU32 = AtomicU32::new(0);
/// The pages of the compile's books drawn before the book being drawn.
static BEFORE: AtomicU32 = AtomicU32::new(0);
static LAST: AtomicU64 = AtomicU64::new(0);
/// What the collector had spent at the last check, and the longest step with
/// the collections in it.
static COLLECTED: AtomicU64 = AtomicU64::new(0);
static LONGEST_WALL: AtomicU64 = AtomicU64::new(0);
static LONGEST: AtomicU64 = AtomicU64::new(0);
static WHAT: AtomicU32 = AtomicU32::new(0);
/// What ended the step before the one running, and before the longest.
static PREVIOUS: AtomicU32 = AtomicU32::new(0);
static FROM: AtomicU32 = AtomicU32::new(0);
static STEPS: AtomicU32 = AtomicU32::new(0);
static VISIBLE_AT: AtomicU64 = AtomicU64::new(0);
static BEGUN: AtomicU64 = AtomicU64::new(0);

/// What ended a step, as [`longest`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Step {
    /// Nothing has: the time from [`begin`].
    Begun,
    BeforeLineBreaking,
    SpringsAndRods,
    LineBreaking,
    AfterLineBreaking,
    Page,
    Stencil,
    Pages,
    Written,
    /// A callback of [`CALLBACKS`] has ended.
    Positioning,
    ControlPoints,
    Positions,
    HorizontalSkylines,
    VerticalSkylines,
    Heights,
    /// An outside-staff grob is placed (`%add-grobs-of-one-priority`).
    OutsideStaff,
    /// A grob broken or substituted, or the cells given back.
    Substitution,
    Cells,
    /// A line the alignment places, purely or not (`%align-minimum-translations`).
    Aligned,
    /// A grob's prebroken pieces made, or a line given back before the breaks.
    Pieces,
    /// A grob's box of a column's skylines, or its part of a line's heights.
    Box,
}

/// The callbacks a cell's sub-steps end with (05 §7.3): collisions,
/// accidentals, ties and dots placed; slurs; beams; a column's and a line's
/// skylines, the latter with its outside-staff grobs placed; a line's pure
/// heights.
const CALLBACKS: [(&str, Step); 6] = [
    ("positioning-done", Step::Positioning),
    ("control-points", Step::ControlPoints),
    ("positions", Step::Positions),
    ("horizontal-skylines", Step::HorizontalSkylines),
    ("vertical-skylines", Step::VerticalSkylines),
    ("adjacent-pure-heights", Step::Heights),
];

static SYMBOLS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

const STEP_NAMES: [&str; 21] = [
    "begun",
    "before-line-breaking",
    "springs-and-rods",
    "line breaking",
    "after-line-breaking",
    "a page drawn",
    "a grob's stencil",
    "a page's stencil",
    "a page written",
    "positioning-done",
    "control-points",
    "positions",
    "horizontal-skylines",
    "vertical-skylines",
    "adjacent-pure-heights",
    "an outside-staff grob placed",
    "a grob broken or substituted",
    "the cells given back",
    "a line aligned",
    "a grob's pieces or a line given back",
    "a grob's box or height",
];

impl Step {
    pub fn name(self) -> &'static str {
        STEP_NAMES[self as usize]
    }
}

/// The clock checks read, in milliseconds: the core's.
pub fn set_clock(clock: fn() -> f64) {
    CLOCK.store(clock as usize, Ordering::Relaxed);
}

fn now() -> f64 {
    match CLOCK.load(Ordering::Relaxed) {
        0 => 0.0,
        // Only `set_clock` stores it, from a `fn() -> f64`.
        f => unsafe { core::mem::transmute::<usize, fn() -> f64>(f)() },
    }
}

/// The page the host shows, for the compile about to start: 0 the first.
pub fn set_visible_page(page: u32) {
    VISIBLE.store(page, Ordering::Relaxed);
}

/// A compile's layout begins: no page drawn, no step ended; `heap`'s
/// symbols of [`CALLBACKS`] found.
pub fn begin(heap: &mut lily_engine_scheme::Heap) {
    for (s, (name, _)) in SYMBOLS.iter().zip(CALLBACKS) {
        s.store(heap.intern(name).to_bits(), Ordering::Relaxed);
    }
    let t = now();
    BEFORE.store(0, Ordering::Relaxed);
    for a in [&LAST, &BEGUN] {
        a.store(t.to_bits(), Ordering::Relaxed);
    }
    for a in [&LONGEST, &LONGEST_WALL, &VISIBLE_AT] {
        a.store(0, Ordering::Relaxed);
    }
    COLLECTED.store(
        lily_engine_scheme::collecting_ms().to_bits(),
        Ordering::Relaxed,
    );
    for a in [&WHAT, &PREVIOUS, &FROM] {
        a.store(Step::Begun as u32, Ordering::Relaxed);
    }
    STEPS.store(0, Ordering::Relaxed);
}

/// A step ended with `what`.
pub fn check(what: Step) {
    let t = now();
    let last = f64::from_bits(LAST.swap(t.to_bits(), Ordering::Relaxed));
    let collected = lily_engine_scheme::collecting_ms();
    let before = f64::from_bits(COLLECTED.swap(collected.to_bits(), Ordering::Relaxed));
    STEPS.fetch_add(1, Ordering::Relaxed);
    let previous = PREVIOUS.swap(what as u32, Ordering::Relaxed);
    if t - last > f64::from_bits(LONGEST_WALL.load(Ordering::Relaxed)) {
        LONGEST_WALL.store((t - last).to_bits(), Ordering::Relaxed);
    }
    // A collection is the collector's step, not layout's.
    let spent = t - last - (collected - before);
    if spent > f64::from_bits(LONGEST.load(Ordering::Relaxed)) {
        LONGEST.store(spent.to_bits(), Ordering::Relaxed);
        WHAT.store(what as u32, Ordering::Relaxed);
        FROM.store(previous, Ordering::Relaxed);
    }
}

/// The callback for `sym` has ended: a step if it is one of [`CALLBACKS`].
#[inline]
pub fn after_callback(sym: Value) {
    let bits = sym.to_bits();
    if let Some(i) = SYMBOLS
        .iter()
        .position(|s| s.load(Ordering::Relaxed) == bits)
    {
        check(CALLBACKS[i].1);
    }
}

/// `(%queue-check n)`: a step's end in the image's Scheme, `n` the
/// [`Step`]'s number: an outside-staff grob placed, a line the alignment
/// places.
pub(crate) fn scheme_check(cx: &mut lily_engine_scheme::Cx, args: &[Value]) -> crate::R {
    let n = args[0].as_fixnum().unwrap_or(0);
    check(step(u32::try_from(n).unwrap_or(0)));
    // A call in steps returns here if its time is spent (B23, D261).
    if lily_engine_music::returns::time_up() {
        cx.yield_step();
    }
    Ok(Value::UNSPECIFIED)
}

/// The time spent outside the steps, as between a compile's layout and its
/// writing, which does not count: the next step begins now.
pub fn pause() {
    LAST.store(now().to_bits(), Ordering::Relaxed);
    COLLECTED.store(
        lily_engine_scheme::collecting_ms().to_bits(),
        Ordering::Relaxed,
    );
    PREVIOUS.store(Step::Begun as u32, Ordering::Relaxed);
}

/// The longest step with the collections in it, in milliseconds.
pub fn longest_wall() -> f64 {
    f64::from_bits(LONGEST_WALL.load(Ordering::Relaxed))
}

/// The longest step since [`begin`], the collections in it left out, in
/// milliseconds, what ended it and what ended the step before it, the
/// steps, and the milliseconds from [`begin`] until the visible page was
/// drawn (0 if it was not).
pub fn longest() -> (f64, Step, Step, u32, f64) {
    let (what, from) = (
        step(WHAT.load(Ordering::Relaxed)),
        step(FROM.load(Ordering::Relaxed)),
    );
    let at = f64::from_bits(VISIBLE_AT.load(Ordering::Relaxed));
    let begun = f64::from_bits(BEGUN.load(Ordering::Relaxed));
    (
        f64::from_bits(LONGEST.load(Ordering::Relaxed)),
        what,
        from,
        STEPS.load(Ordering::Relaxed),
        if at > 0.0 { at - begun } else { 0.0 },
    )
}

fn step(n: u32) -> Step {
    match n {
        1 => Step::BeforeLineBreaking,
        2 => Step::SpringsAndRods,
        3 => Step::LineBreaking,
        4 => Step::AfterLineBreaking,
        5 => Step::Page,
        6 => Step::Stencil,
        7 => Step::Pages,
        8 => Step::Written,
        9 => Step::Positioning,
        10 => Step::ControlPoints,
        11 => Step::Positions,
        12 => Step::HorizontalSkylines,
        13 => Step::VerticalSkylines,
        14 => Step::Heights,
        15 => Step::OutsideStaff,
        16 => Step::Substitution,
        17 => Step::Cells,
        18 => Step::Aligned,
        19 => Step::Pieces,
        20 => Step::Box,
        _ => Step::Begun,
    }
}

/// The order a book of `pages` pages draws them in: the visible page if it
/// is this book's and not its last, the pages after it, then those before
/// it, and the last page last; else the pages' order.
pub fn order(pages: usize) -> Vec<usize> {
    let before = BEFORE.fetch_add(pages as u32, Ordering::Relaxed) as usize;
    let visible = (VISIBLE.load(Ordering::Relaxed) as usize).checked_sub(before);
    match visible {
        Some(v) if v + 1 < pages => {
            let mut order: Vec<usize> = (v..pages - 1).collect();
            order.extend((0..v).rev());
            order.push(pages - 1);
            order
        }
        _ => (0..pages).collect(),
    }
}

/// The page `page` of [`order`] has been drawn: the first of the order, the
/// visible one or the book's first, gives the visible page's time.
pub fn drawn(page: usize, order: &[usize]) {
    if order.first() == Some(&page) && VISIBLE_AT.load(Ordering::Relaxed) == 0 {
        VISIBLE_AT.store(now().to_bits(), Ordering::Relaxed);
    }
    check(Step::Page);
}

/// What a step said: the error port's text and `(lily)`'s diagnostics,
/// taken off so that they can be said again in another order.
pub(crate) struct Said {
    from: usize,
    errors: alloc::string::String,
    items: Vec<Value>,
}

/// Where what is said stands: the error port's length and the diagnostics.
pub(crate) fn mark(host: &mut dyn Host) -> (usize, Value) {
    let cell = host.lily("%diagnostics").unwrap_or(Value::NULL);
    let head = host.heap().car(cell).unwrap_or(Value::NULL);
    (host.error_len(), head)
}

/// What was said since `mark`, taken off.
pub(crate) fn take_since(host: &mut dyn Host, mark: (usize, Value)) -> Said {
    let errors = host.errors_since(mark.0);
    host.cut_errors(mark.0);
    let cell = host.lily("%diagnostics").unwrap_or(Value::NULL);
    let heap = host.heap();
    let mut items = Vec::new();
    let mut list = heap.car(cell).unwrap_or(Value::NULL);
    while list != mark.1 {
        let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) else {
            break;
        };
        items.push(item);
        list = rest;
    }
    if heap.car(cell).is_some() {
        heap.set_car(cell, mark.1);
    }
    items.reverse();
    Said {
        from: mark.0,
        errors,
        items,
    }
}

/// `said` said again, in order: each step's text and its diagnostics, whose
/// places in the text move with it.
pub(crate) fn say_again(host: &mut dyn Host, said: Vec<Said>) -> Result<(), Exception> {
    let cell = host.lily("%diagnostics").unwrap_or(Value::NULL);
    for s in said {
        let start = host.error_len();
        host.write_errors(&s.errors);
        let heap = host.heap();
        for item in s.items {
            // A diagnostic's mark is its place in the error port's text.
            if let Some(items) = heap.vector_items_mut(item)
                && let Some(mark) = items.get(3).and_then(|m| m.as_fixnum())
            {
                let shifted = mark - s.from as i64 + start as i64;
                items[3] = Value::fixnum(shifted).unwrap_or(items[3]);
            }
            let head = heap.car(cell).unwrap_or(Value::NULL);
            let list = heap.cons(item, head);
            heap.set_car(cell, list);
        }
    }
    Ok(())
}
