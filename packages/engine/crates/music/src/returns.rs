//! H3 (README §5, 09 §8's P3, D254): when a compile in steps gives the
//! worker back. A worker cannot be interrupted during a call (08 §4.3), so
//! a compile the host runs in steps has a deadline, and the loops that can
//! stop ask [`due`] between what they do: the global loop between timesteps,
//! layout between grobs, lines, systems and pages, and columns, a line's
//! heights and the page's spacing problem (D261). What they have done stays in
//! their state, and the host's next `lily_step` goes on from it. Whatever is
//! held in that state must be rooted: since D261 a collection can come between
//! two steps, not only at a safe point inside one.
//!
//! No deadline ([`begin`] with `f64::INFINITY`) never returns, which is a
//! compile that is traced, or a nested one, or native.
//!
//! **A compile near the memory's cap (D259)** returns as if its time were
//! spent: wasm has no way back from an allocation that fails, so the core
//! asks [`short`] where a return can stop it, and gives a diagnostic there
//! before an allocation traps.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// `f64::INFINITY`'s bits.
const NEVER: u64 = 0x7FF0_0000_0000_0000;

/// Bytes a grob costs as its translation ends, with no place to stop (D259;
/// measured on staves of 8,000 to 12,000 notes and T29 × 40).
pub const GROB_END: usize = 2 << 10;

static CLOCK: AtomicUsize = AtomicUsize::new(0);
static DEADLINE: AtomicU64 = AtomicU64::new(NEVER);
static ROOM: AtomicUsize = AtomicUsize::new(0);

/// The clock [`due`] reads, in milliseconds: the core's.
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

/// What [`short`] asks: whether the memory left is too little to go on, with
/// so many bytes more allocated.
pub fn set_room(short_of_memory: fn(usize) -> bool) {
    ROOM.store(short_of_memory as usize, Ordering::Relaxed);
}

/// Whether the memory left is too little to go on, if the core says.
pub fn short() -> bool {
    short_by(0)
}

/// Whether it would be, with `extra` bytes more allocated: for what allocates
/// that much with no place to stop in it.
pub fn short_by(extra: usize) -> bool {
    match ROOM.load(Ordering::Relaxed) {
        0 => false,
        // Only `set_room` stores it, from a `fn(usize) -> bool`.
        f => unsafe { core::mem::transmute::<usize, fn(usize) -> bool>(f)(extra) },
    }
}

/// The step that begins now ends at `deadline`, by the clock.
pub fn begin(deadline: f64) {
    DEADLINE.store(deadline.to_bits(), Ordering::Relaxed);
}

/// Whether the step's time is spent, or the memory.
#[inline]
pub fn due() -> bool {
    let deadline = DEADLINE.load(Ordering::Relaxed);
    deadline != NEVER && (now() >= f64::from_bits(deadline) || short())
}

/// Whether the step's time is spent, whatever the memory: for the loops of a
/// column, a grob of a line or a system, whose returns would use up the asks
/// the memory's check waits on a collection with (D261; T29 × 8).
#[inline]
pub fn time_up() -> bool {
    let deadline = DEADLINE.load(Ordering::Relaxed);
    deadline != NEVER && now() >= f64::from_bits(deadline)
}

static AHEAD: AtomicU64 = AtomicU64::new(0);

/// A translation's start returned while it asked ahead for `create_top`
/// (B23, D272).
pub(crate) fn returned_ahead() {
    AHEAD.fetch_add(1, Ordering::Relaxed);
}

/// How many times a translation's start has returned while it asked ahead
/// for `create_top` (B23, D272), for the tests.
pub fn returns_ahead() -> u64 {
    AHEAD.load(Ordering::Relaxed)
}

static MID: AtomicU64 = AtomicU64::new(0);

/// The global loop returned in the middle of a timestep (B23, D272).
pub(crate) fn returned_mid() {
    MID.fetch_add(1, Ordering::Relaxed);
}

/// How many times the global loop has returned in the middle of a timestep
/// (B23, D272), for the tests.
pub fn returns_mid() -> u64 {
    MID.load(Ordering::Relaxed)
}
