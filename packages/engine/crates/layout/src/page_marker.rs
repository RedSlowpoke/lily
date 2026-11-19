//! LilyPond's `Page_marker` (`page-marker.cc`, `page-marker-scheme.cc`,
//! part 5's, MS3 step 64): what a toplevel `\label`, `\pageBreak`,
//! `\noPageBreak`, `\pageTurn` and their kin leave among a book's scores
//! (`lily-library.scm`'s `collect-music-aux` hands it to the score
//! handler). `Paper_book::get_system_specs` (`paper_book.rs`) reads it: a
//! permission sets the spec before it, a label the spec after it.

use lily_engine_scheme::{Cx, Foreign, Tag, Tracer, Value};

use crate::R;

/// A `Page_marker`: a permission's symbol (`symbol_`, `page-break-permission`,
/// `page-turn-permission` or `line-break-permission`) and value
/// (`permission_`: `force`, `allow` or `'()`), or a label (`label_`, a
/// symbol); `'()` where it has none.
pub struct PageMarker {
    pub symbol: Value,
    pub permission: Value,
    pub label: Value,
}

impl Foreign for PageMarker {
    fn type_name(&self) -> &'static str {
        "Page_marker"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in [self.symbol, self.permission, self.label] {
            tracer.trace(v);
        }
    }
}

/// `(ly:make-page-permission-marker symbol permission)`
/// (`page-marker-scheme.cc:22–32`): a marker whose permission `symbol`
/// is `permission`.
pub(crate) fn make_permission_marker(cx: &mut Cx, args: &[Value]) -> R {
    if args[0].tag() != Tag::Symbol {
        return Err(cx.wrong_type("ly:make-page-permission-marker", 1, args[0], "symbol"));
    }
    Ok(cx.heap_mut().wrap(PageMarker {
        symbol: args[0],
        permission: args[1],
        label: Value::NULL,
    }))
}

/// `(ly:make-page-label-marker label)` (`page-marker-scheme.cc:34–44`): a
/// marker of the label `label`.
pub(crate) fn make_label_marker(cx: &mut Cx, args: &[Value]) -> R {
    if args[0].tag() != Tag::Symbol {
        return Err(cx.wrong_type("ly:make-page-label-marker", 1, args[0], "symbol"));
    }
    Ok(cx.heap_mut().wrap(PageMarker {
        symbol: Value::NULL,
        permission: Value::NULL,
        label: args[0],
    }))
}

/// `(ly:page-marker? x)` (`Page_marker::type_p_name_`).
pub(crate) fn is_page_marker(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap().foreign::<PageMarker>(args[0]).is_some(),
    ))
}
