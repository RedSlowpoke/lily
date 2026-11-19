//! `directional-element-interface.cc` (part 3's, MS3 step 38), 31 lines:
//! `internal_get_grob_direction` (`:25–39`), `get_grob_direction`
//! (`:41–45`, layout's since MS3 step 31, in `grob.rs` until now),
//! `get_strict_grob_direction` (`:61–65`) and `set_grob_direction`
//! (`:67–72`). The note head's Scheme asks the strict direction as
//! `%strict-grob-direction` (`note_head.rs`, MS3 step 37), the same
//! function in the image.

use alloc::format;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::geometry::Direction;
use crate::grob;

const UP: Direction = 1;

/// `internal_get_grob_direction`: the grob's `direction` as
/// `from_scm<Direction>` reads it (an exact -1, 0 or 1, else 0); strict,
/// one of 0 is LilyPond's warning, and the grob set up.
fn internal_get_grob_direction(
    host: &mut dyn Host,
    me: Value,
    strict: bool,
) -> Result<Direction, Exception> {
    let d = grob::property(host, me, "direction")?;
    let dir = crate::dynamic_align_engraver::direction(d);
    if strict && dir == 0 {
        let name = grob::grob_name(host.heap(), me);
        grob::warning(
            host,
            me,
            &format!("direction of grob {name} must be UP or DOWN; using UP"),
        )?;
        set_grob_direction(host, me, UP);
        return Ok(UP);
    }
    Ok(dir)
}

/// `get_grob_direction`: the grob's `direction`, 0 for none.
pub fn get_grob_direction(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    internal_get_grob_direction(host, me, false)
}

/// `get_strict_grob_direction`: the grob's `direction` if up or down;
/// else LilyPond's warning, and the grob set up.
pub fn get_strict_grob_direction(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    internal_get_grob_direction(host, me, true)
}

/// `set_grob_direction`: the grob's `direction` set to `d`, unchecked.
pub fn set_grob_direction(host: &mut dyn Host, me: Value, d: Direction) {
    let heap = host.heap();
    let v = heap.integer(i64::from(d));
    grob::set_property(heap, me, "direction", v);
}
