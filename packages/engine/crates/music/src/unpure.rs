//! LilyPond's unpure-pure containers (`unpure-pure-container.cc`): a grob
//! property's value with a part for the pure calculations of line
//! breaking. Layout calls them from MS3; music functions make them now
//! (`\alterBroken`, `\shape`), so the value, its predicate and its parts
//! are here.

use alloc::string::String;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Tracer, Value};

use crate::R;

/// Its unpure part, and its pure part if it was given one.
pub struct UnpurePure {
    unpure: Value,
    pure: Option<Value>,
}

impl UnpurePure {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<UnpurePure> {
        let unpure = r.value()?;
        let pure = r.value()?;
        Some(UnpurePure {
            unpure,
            pure: (pure != Value::UNDEFINED).then_some(pure),
        })
    }

    /// `is_unchanging`: no pure part of its own.
    fn unchanging(&self) -> bool {
        self.pure.is_none()
    }
}

/// A part as the container prints it: a procedure by its name if it has
/// one.
fn display_part(heap: &Heap, part: Value, out: &mut String) {
    match heap.procedure_name(part) {
        Some(name) if heap.is_procedure(part) => out.push_str(name),
        _ => heap.display(part, out),
    }
}

impl UnpurePure {
    /// Its procedures, unpure and pure: what N8's digests take it as
    /// (`frozen::digest`, `checkpoint.rs`).
    pub(crate) fn parts(&self) -> [Value; 2] {
        [self.unpure, self.pure.unwrap_or(Value::FALSE)]
    }
}

impl Foreign for UnpurePure {
    fn type_name(&self) -> &'static str {
        "Unpure_pure_container"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<unpure-pure-container ");
        display_part(heap, self.unpure, out);
        if let Some(pure) = self.pure.filter(|_| !self.unchanging()) {
            out.push(' ');
            display_part(heap, pure, out);
        }
        out.push_str(" >");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.unpure);
        if let Some(pure) = self.pure {
            tracer.trace(pure);
        }
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.unpure);
        out.value(self.pure.unwrap_or(Value::UNDEFINED));
        true
    }
}

fn container(
    cx: &mut Cx,
    subr: &str,
    v: Value,
) -> Result<(Value, Option<Value>), lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<UnpurePure>(v) {
        Some(c) => Ok((c.unpure, c.pure)),
        None => Err(cx.wrong_type(subr, 1, v, "Unpure_pure_container")),
    }
}

/// `(ly:make-unpure-pure-container unpure [pure])`: a value that is no
/// procedure is its own pure part, as LilyPond's prints `3 3`.
pub(crate) fn make(cx: &mut Cx, args: &[Value]) -> R {
    let pure = match args.get(1) {
        Some(&pure) => Some(pure),
        None if !cx.heap().is_procedure(args[0]) => Some(args[0]),
        None => None,
    };
    Ok(cx.heap_mut().wrap(UnpurePure {
        unpure: args[0],
        pure,
    }))
}

pub(crate) fn is_container(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<UnpurePure>(cx, args[0])
}

/// The unpure part of `v` if it is a container: what layout's property
/// lookup calls in its place (`Grob::internal_get_property`).
pub fn unpure_of(heap: &Heap, v: Value) -> Option<Value> {
    heap.foreign::<UnpurePure>(v).map(|c| c.unpure)
}

/// The unpure part of `v` and its pure part, none for a container
/// without one of its own (`is_unchanging`), if `v` is a container: what
/// layout's pure lookups call (`Grob::call_pure_function`).
pub fn parts_of(heap: &Heap, v: Value) -> Option<(Value, Option<Value>)> {
    heap.foreign::<UnpurePure>(v).map(|c| (c.unpure, c.pure))
}

pub(crate) fn unpure_part(cx: &mut Cx, args: &[Value]) -> R {
    Ok(container(cx, "ly:unpure-pure-container-unpure-part", args[0])?.0)
}

/// `(%unpure-pure-unchanging? pc)`: whether `pc` has no pure part of its
/// own (`is_unchanging`).
pub(crate) fn is_unchanging(cx: &mut Cx, args: &[Value]) -> R {
    let (_, pure) = container(cx, "%unpure-pure-unchanging?", args[0])?;
    Ok(Value::bool(pure.is_none()))
}

/// Its pure part; without one, its unpure part called without the two
/// arguments a pure call adds (`Unpure_pure_call`), by [`PRELUDE`]'s
/// `%unpure-pure-call`.
pub(crate) fn pure_part(cx: &mut Cx, args: &[Value]) -> R {
    match container(cx, "ly:unpure-pure-container-pure-part", args[0])? {
        (_, Some(pure)) => Ok(pure),
        (unpure, None) => match cx.global("lily", "%unpure-pure-call") {
            Some(f) => Ok(cx.tail_call(f, &[unpure])),
            None => Ok(unpure),
        },
    }
}

/// `Unpure_pure_call`, and `ly:pure-call` and `ly:unpure-call`, which
/// call a part of a container, or a procedure, or give a value.
pub(crate) const PRELUDE: &str = r#"
(define (%unpure-pure-call unpure)
  (lambda (grob start end . rest) (apply unpure grob rest)))
(define-public (ly:pure-call data grob start end . rest)
  (if (and (ly:unpure-pure-container? data)
           (not (%unpure-pure-unchanging? data)))
      (let ((pure (ly:unpure-pure-container-pure-part data)))
        (if (procedure? pure) (apply pure grob start end rest) pure))
      (let ((data (if (ly:unpure-pure-container? data)
                      (ly:unpure-pure-container-unpure-part data)
                      data)))
        (if (procedure? data) (apply data grob rest) data))))
(define-public (ly:unpure-call data grob . rest)
  (let ((data (if (ly:unpure-pure-container? data)
                  (ly:unpure-pure-container-unpure-part data)
                  data)))
    (if (procedure? data) (apply data grob rest) data)))
"#;
