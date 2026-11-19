//! `rhythmic-head.cc` (part 3's, MS3): `get_stem` (`rhythmic-head.cc:35–40`)
//! and `set_dots` (`:50–54`), 11 lines, which `Stem_engraver` and
//! `Dots_engraver` call, ported first (MS3 step 16); and the rest of the
//! file (MS3 step 37), `get_dots` (`:28–33`, 6), `dot_count` (`:42–48`, 7)
//! and `duration_log` (`:57–63`, 7), which the dot column, the rests and
//! the stems read.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array;
use crate::item;
use crate::stem::to_int;

/// `get_dots`: `me`'s `dot` object, if it is an item.
pub fn get_dots(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("dot");
    let s = grob_array::object(host, me, sym)?;
    Ok(item::is_item(host.heap(), s).then_some(s))
}

/// `get_stem`: `me`'s `stem` object, if it is an item.
pub fn get_stem(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("stem");
    let s = grob_array::object(host, me, sym)?;
    Ok(item::is_item(host.heap(), s).then_some(s))
}

/// `dot_count`: the `dot-count` of `me`'s dots as `from_scm<int> (…, 0)`
/// reads it, 0 without dots.
pub fn dot_count(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    let Some(dots) = get_dots(host, me)? else {
        return Ok(0);
    };
    let v = grob::property(host, dots, "dot-count")?;
    let heap = host.heap();
    Ok(heap
        .is_exact_integer(v)
        .then(|| heap.to_i64(v).and_then(|n| i32::try_from(n).ok()))
        .flatten()
        .unwrap_or(0))
}

/// `set_dots`: the dots `dot` set as `me`'s `dot` object.
pub fn set_dots(host: &mut dyn Host, me: Value, dot: Value) {
    let heap = host.heap();
    let sym = heap.intern("dot");
    grob::set_object(heap, me, sym, dot);
}

/// `duration_log`: `me`'s `duration-log`, converted as `scm_to_int`
/// converts it, if it is a number, else 0.
pub fn duration_log(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    let s = grob::property(host, me, "duration-log")?;
    let heap = host.heap();
    if heap.is_number(s) {
        to_int(heap, s)
    } else {
        Ok(0)
    }
}
