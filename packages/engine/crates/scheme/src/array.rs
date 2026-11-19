//! Guile's arrays as LilyPond's Scheme calls them (MS4 step 17, D229):
//! `make-array`, `array-ref`, `array-set!`, `array->list` and `array?`,
//! which `translation-functions.scm`'s `determine-frets` calls for a
//! FretBoard's barres (`barre-list`); since MS8 step 11 also those
//! `qr-code.scm` calls: `array-shape`, `array-dimensions`, `array-rank`,
//! `make-shared-array`, `transpose-array`, `array-cell-ref`,
//! `array-index-map!` and `array-copy!`, whose shared arrays are views of
//! another's elements ([`Shared`]). An array of one dimension whose
//! indices start at 0 is a vector, as Guile's `make-array` gives it; any
//! other is the reader's [`Array`]. Strings and bytevectors, which Guile's
//! arrays include, are arrays to `array?` alone.

use alloc::vec::Vec;

use crate::errors::{INDEX_OUT_OF_BOUNDS, MAX_VECTOR_LENGTH};
use crate::reader::{Array, Bytevector};
use crate::value::Value;
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

/// A bound of `make-array`: a length `n`, the indices 0 to `n - 1`, or a
/// list `(lower upper)`, with Guile's `bad array bound` for one whose upper
/// bound is below its lower bound less one; its lower bound and length.
fn bound(cx: &mut Cx, v: Value) -> Result<(i64, usize), Thrown> {
    let heap = &cx.heap;
    if let (Some(lower), Some(rest)) = (heap.car(v), heap.cdr(v)) {
        let upper = heap.car(rest);
        let end = heap.cdr(rest);
        let int = |x: Value| heap.is_exact_integer(x).then(|| heap.to_i64(x)).flatten();
        if let (Some(lower), Some(upper), Some(Value::NULL)) =
            (int(lower), upper.and_then(int), end)
            && upper >= lower - 1
            && ((upper - lower + 1) as u64) <= MAX_VECTOR_LENGTH
        {
            return Ok((lower, (upper - lower + 1) as usize));
        }
        return Err(cx.misc_error(None, "bad array bound ~a", &[v]));
    }
    Ok((0, cx.size(v, MAX_VECTOR_LENGTH)?))
}

/// `(make-array fill bound …)`: an array of `fill`, of a dimension for
/// each bound; a vector for one dimension from 0.
pub(crate) fn make_array(cx: &mut Cx, args: &[Value]) -> R {
    let fill = args[0];
    let mut shape = Vec::with_capacity(args.len() - 1);
    let mut total: usize = 1;
    for (i, &b) in args[1..].iter().enumerate() {
        let (lower, length) = bound(cx, b)?;
        total = match total.checked_mul(length) {
            Some(t) if t as u64 <= MAX_VECTOR_LENGTH => t,
            _ => return Err(cx.out_of_range("make-array", i + 2, b)),
        };
        shape.push((lower, length));
    }
    if let [(0, length)] = shape[..] {
        return Ok(cx.heap.vector(alloc::vec![fill; length]));
    }
    Ok(cx.heap.wrap(Array {
        shape,
        items: alloc::vec![fill; total],
    }))
}

/// What an array's elements are and where: its root (a vector or an
/// [`Array`]), the position of its first element there, and each
/// dimension's lower bound, length and stride. A vector and an [`Array`]
/// are their own roots; a [`Shared`] array is a view of one.
struct View {
    root: Value,
    vector: bool,
    offset: isize,
    shape: Vec<(i64, usize)>,
    strides: Vec<isize>,
}

/// The strides of `shape`'s elements in order, the last dimension's the
/// fastest.
fn row_major(shape: &[(i64, usize)]) -> Vec<isize> {
    let mut strides = alloc::vec![0; shape.len()];
    let mut size: isize = 1;
    for k in (0..shape.len()).rev() {
        strides[k] = size;
        size *= shape[k].1 as isize;
    }
    strides
}

/// The view of the array `a`; Guile's error, which names no procedure, for
/// what is none.
fn view(cx: &mut Cx, a: Value) -> Result<View, Thrown> {
    if let Some(items) = cx.heap.vector_items(a) {
        return Ok(View {
            root: a,
            vector: true,
            offset: 0,
            shape: alloc::vec![(0, items.len())],
            strides: alloc::vec![1],
        });
    }
    if let Some(array) = cx.heap.foreign::<Array>(a) {
        return Ok(View {
            root: a,
            vector: false,
            offset: 0,
            strides: row_major(&array.shape),
            shape: array.shape.clone(),
        });
    }
    if let Some(shared) = cx.heap.foreign::<Shared>(a) {
        return Ok(View {
            root: shared.root,
            vector: cx.heap.vector_items(shared.root).is_some(),
            offset: shared.offset,
            shape: shared.shape.clone(),
            strides: shared.strides.clone(),
        });
    }
    Err(cx.wrong_type_vm(None, a, "array"))
}

impl View {
    fn get(&self, heap: &crate::heap::Heap, at: isize) -> Value {
        let at = at as usize;
        match self.vector {
            true => heap
                .vector_items(self.root)
                .and_then(|v| v.get(at).copied()),
            false => heap
                .foreign::<Array>(self.root)
                .and_then(|a| a.items.get(at).copied()),
        }
        .unwrap_or(Value::UNSPECIFIED)
    }

    fn set(&self, heap: &mut crate::heap::Heap, at: isize, value: Value) {
        let at = at as usize;
        if self.vector {
            if let Some(slot) = heap.vector_items_mut(self.root).and_then(|v| v.get_mut(at)) {
                *slot = value;
            }
        } else if let Some(slot) = heap
            .foreign_mut::<Array>(self.root)
            .and_then(|a| a.items.get_mut(at))
        {
            *slot = value;
        }
    }

    /// Each element's position in the root, in order.
    fn positions(&self) -> Vec<isize> {
        let mut out = alloc::vec![self.offset];
        for (k, &(_, length)) in self.shape.iter().enumerate() {
            out = out
                .into_iter()
                .flat_map(|at| (0..length as isize).map(move |i| at + i * self.strides[k]))
                .collect();
        }
        out
    }
}

/// The position in its root of the element at the indices `idx` of the
/// array `v`, with Guile's errors, which name no procedure: the number of
/// indices, one that is no exact integer, and one out of its dimension's
/// bounds.
fn position(cx: &mut Cx, v: &View, idx: &[Value]) -> Result<isize, Thrown> {
    if idx.len() != v.shape.len() {
        let rank = cx.heap.integer(v.shape.len() as i64);
        return Err(cx.misc_error(None, "wrong number of indices, expecting ~A", &[rank]));
    }
    let mut at = v.offset;
    for ((&i, &(lower, length)), &stride) in idx.iter().zip(&v.shape).zip(&v.strides) {
        let n = match cx.heap.to_i64(i) {
            Some(n) if cx.heap.is_exact_integer(i) => n,
            _ => return Err(cx.wrong_type_vm(None, i, "exact integer")),
        };
        let offset = n.wrapping_sub(lower);
        if n < lower || offset as u64 >= length as u64 {
            let low = cx.heap.integer(lower);
            let high = cx.heap.integer(lower + length as i64 - 1);
            return Err(cx.raise(&INDEX_OUT_OF_BOUNDS, None, &[low, high, i]));
        }
        at += offset as isize * stride;
    }
    Ok(at)
}

/// `(array-ref a index …)`.
pub(crate) fn array_ref(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let at = position(cx, &v, &args[1..])?;
    Ok(v.get(&cx.heap, at))
}

/// `(array-set! a value index …)`.
pub(crate) fn array_set(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let at = position(cx, &v, &args[2..])?;
    v.set(&mut cx.heap, at, args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(array->list a)`: its elements in nested lists, a list a dimension;
/// the element of an array of none.
pub(crate) fn array_to_list(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let items: Vec<Value> = v
        .positions()
        .into_iter()
        .map(|at| v.get(&cx.heap, at))
        .collect();
    Ok(nest(cx, &v.shape, &items))
}

/// The lists of `items`, of the dimensions `shape`.
fn nest(cx: &mut Cx, shape: &[(i64, usize)], items: &[Value]) -> Value {
    let Some((&(_, length), rest)) = shape.split_first() else {
        return items.first().copied().unwrap_or(Value::UNSPECIFIED);
    };
    let size = rest.iter().map(|&(_, l)| l).product::<usize>();
    let rows: Vec<Value> = (0..length)
        .map(|i| nest(cx, rest, &items[i * size..(i + 1) * size]))
        .collect();
    cx.heap.list(&rows)
}

/// A view of another array's elements (Guile's shared array), which
/// `make-shared-array`, `transpose-array` and `array-cell-ref` make (MS8
/// step 11, `qr-code.scm`): what is set through it is set in its root.
/// Never in an image.
pub(crate) struct Shared {
    root: Value,
    offset: isize,
    shape: Vec<(i64, usize)>,
    strides: Vec<isize>,
}

impl crate::heap::Foreign for Shared {
    fn type_name(&self) -> &'static str {
        "shared-array"
    }

    /// As the array of its elements prints.
    fn print(&self, heap: &crate::heap::Heap, out: &mut alloc::string::String) {
        let v = View {
            root: self.root,
            vector: heap.vector_items(self.root).is_some(),
            offset: self.offset,
            shape: self.shape.clone(),
            strides: self.strides.clone(),
        };
        let items = v
            .positions()
            .into_iter()
            .map(|at| v.get(heap, at))
            .collect();
        Array {
            shape: self.shape.clone(),
            items,
        }
        .print(heap, out);
    }

    fn trace(&self, tracer: &mut crate::heap::Tracer<'_>) {
        tracer.trace(self.root);
    }
}

fn shared(cx: &mut Cx, v: View) -> Value {
    cx.heap.wrap(Shared {
        root: v.root,
        offset: v.offset,
        shape: v.shape,
        strides: v.strides,
    })
}

/// The list of `(lower upper)` of each dimension of `v`.
fn shape_list(cx: &mut Cx, v: &View) -> Value {
    let bounds: Vec<Value> = v
        .shape
        .iter()
        .map(|&(lower, length)| {
            let low = cx.heap.integer(lower);
            let high = cx.heap.integer(lower + length as i64 - 1);
            cx.heap.list(&[low, high])
        })
        .collect();
    cx.heap.list(&bounds)
}

/// `(array-shape a)`.
pub(crate) fn array_shape(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    Ok(shape_list(cx, &v))
}

/// `(array-dimensions a)`: each dimension's length, or its `(lower
/// upper)` where its lower bound is not 0.
pub(crate) fn array_dimensions(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let dims: Vec<Value> = v
        .shape
        .iter()
        .map(|&(lower, length)| match lower {
            0 => cx.heap.integer(length as i64),
            _ => {
                let low = cx.heap.integer(lower);
                let high = cx.heap.integer(lower + length as i64 - 1);
                cx.heap.list(&[low, high])
            }
        })
        .collect();
    Ok(cx.heap.list(&dims))
}

/// `(array-rank a)`.
pub(crate) fn array_rank(cx: &mut Cx, args: &[Value]) -> R {
    let rank = match view(cx, args[0]) {
        Ok(v) => v.shape.len(),
        Err(_) => {
            cx.error = None;
            0
        }
    };
    Ok(cx.heap.integer(rank as i64))
}

/// `(%array-position a (index …))`: where in its root the element is, for
/// `make-shared-array`'s Scheme.
pub(crate) fn array_position(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let idx = list_items(cx, args[1]);
    let at = position(cx, &v, &idx)?;
    Ok(cx.heap.integer(at as i64))
}

fn list_items(cx: &Cx, mut list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(car), Some(cdr)) = (cx.heap.car(list), cx.heap.cdr(list)) {
        out.push(car);
        list = cdr;
    }
    out
}

/// `(%make-shared-array a offset (stride …) ((lower upper) …))`: a view
/// of `a`'s root, as `make-shared-array`'s Scheme measured it.
pub(crate) fn make_shared(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let int = |cx: &Cx, x: Value| cx.heap.to_i64(x).unwrap_or(0);
    let offset = int(cx, args[1]) as isize;
    let strides = list_items(cx, args[2])
        .into_iter()
        .map(|s| int(cx, s) as isize)
        .collect();
    let shape = list_items(cx, args[3])
        .into_iter()
        .map(|b| {
            let bounds = list_items(cx, b);
            let (lower, upper) = (int(cx, bounds[0]), int(cx, bounds[1]));
            (lower, (upper - lower + 1).max(0) as usize)
        })
        .collect();
    Ok(shared(
        cx,
        View {
            offset,
            strides,
            shape,
            ..v
        },
    ))
}

/// `(transpose-array a dim …)`: dimension `i` of `a` is the result's
/// dimension `dim` of the `i`th, each once.
pub(crate) fn transpose_array(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let dims: Vec<usize> = args[1..]
        .iter()
        .map(|&d| cx.heap.to_i64(d).unwrap_or(-1) as usize)
        .collect();
    let rank = v.shape.len();
    let mut seen = alloc::vec![false; rank];
    if dims.len() != rank
        || dims
            .iter()
            .any(|&d| d >= rank || core::mem::replace(&mut seen[d], true))
    {
        return Err(cx.misc_error(Some("transpose-array"), "bad argument list", &[]));
    }
    let (mut shape, mut strides) = (v.shape.clone(), v.strides.clone());
    for (i, &d) in dims.iter().enumerate() {
        shape[d] = v.shape[i];
        strides[d] = v.strides[i];
    }
    Ok(shared(
        cx,
        View {
            shape,
            strides,
            ..v
        },
    ))
}

/// `(array-cell-ref a index …)`: the element, or the array of the
/// dimensions after the indices given.
pub(crate) fn array_cell_ref(cx: &mut Cx, args: &[Value]) -> R {
    let v = view(cx, args[0])?;
    let k = args.len() - 1;
    if k > v.shape.len() {
        return Err(cx.misc_error(Some("array-cell-ref"), "bad argument list", &[]));
    }
    let leading = View {
        root: v.root,
        vector: v.vector,
        offset: v.offset,
        shape: v.shape[..k].to_vec(),
        strides: v.strides[..k].to_vec(),
    };
    let at = position(cx, &leading, &args[1..])?;
    if k == v.shape.len() {
        return Ok(v.get(&cx.heap, at));
    }
    Ok(shared(
        cx,
        View {
            offset: at,
            shape: v.shape[k..].to_vec(),
            strides: v.strides[k..].to_vec(),
            ..v
        },
    ))
}

/// `(array-copy! src dst)`: each element of `src` into `dst`, in order.
pub(crate) fn array_copy_into(cx: &mut Cx, args: &[Value]) -> R {
    let (src, dst) = (view(cx, args[0])?, view(cx, args[1])?);
    let lengths = |v: &View| v.shape.iter().map(|&(_, l)| l).collect::<Vec<_>>();
    if lengths(&src) != lengths(&dst) {
        return Err(cx.misc_error(Some("array-copy!"), "arrays of different shapes", &[]));
    }
    let values: Vec<Value> = src
        .positions()
        .into_iter()
        .map(|at| src.get(&cx.heap, at))
        .collect();
    for (at, value) in dst.positions().into_iter().zip(values) {
        dst.set(&mut cx.heap, at, value);
    }
    Ok(Value::UNSPECIFIED)
}

/// A new array of the elements of a shared one, as `array-copy` gives it.
pub(crate) fn copy_shared(cx: &mut Cx, a: Value) -> Option<Value> {
    cx.heap.foreign::<Shared>(a)?;
    let v = view(cx, a).ok()?;
    let items: Vec<Value> = v
        .positions()
        .into_iter()
        .map(|at| v.get(&cx.heap, at))
        .collect();
    Some(match v.shape[..] {
        [(0, _)] => cx.heap.vector(items),
        _ => cx.heap.wrap(Array {
            shape: v.shape,
            items,
        }),
    })
}

/// The primitives of `(rnrs bytevectors)` that `qr-code.scm` calls.
pub(crate) const BYTEVECTORS: &[&str] = &[
    "bytevector?",
    "bytevector-length",
    "bytevector-u8-ref",
    "bytevector->u8-list",
];

fn bytes(cx: &mut Cx, subr: &str, v: Value) -> Result<Vec<u8>, Thrown> {
    match cx.heap.foreign::<Bytevector>(v) {
        Some(b) => Ok(b.0.clone()),
        None => Err(cx.wrong_type(subr, 1, v, "bytevector")),
    }
}

/// `(bytevector? x)`.
pub(crate) fn is_bytevector(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap.foreign::<Bytevector>(args[0]).is_some(),
    ))
}

/// `(bytevector-length bv)`.
pub(crate) fn bytevector_length(cx: &mut Cx, args: &[Value]) -> R {
    let n = bytes(cx, "bytevector-length", args[0])?.len();
    Ok(cx.heap.integer(n as i64))
}

/// `(bytevector-u8-ref bv k)`.
pub(crate) fn bytevector_u8_ref(cx: &mut Cx, args: &[Value]) -> R {
    let b = bytes(cx, "bytevector-u8-ref", args[0])?;
    match cx
        .heap
        .to_i64(args[1])
        .and_then(|k| usize::try_from(k).ok())
        .and_then(|k| b.get(k))
    {
        Some(&byte) => Ok(cx.heap.integer(byte as i64)),
        None => Err(cx.out_of_range("bytevector-u8-ref", 2, args[1])),
    }
}

/// `(bytevector->u8-list bv)`.
pub(crate) fn bytevector_to_u8_list(cx: &mut Cx, args: &[Value]) -> R {
    let b = bytes(cx, "bytevector->u8-list", args[0])?;
    let items: Vec<Value> = b.iter().map(|&byte| cx.heap.integer(byte as i64)).collect();
    Ok(cx.heap.list(&items))
}

/// `(ice-9 iconv)`'s `(string->bytevector string encoding)`, for the
/// encodings `qr-code.scm` and a score may name: Latin-1, whose `encoding-error`
/// a character past it throws, and UTF-8.
pub(crate) fn string_to_bytevector(cx: &mut Cx, args: &[Value]) -> R {
    let Some(text) = cx
        .heap
        .str(args[0])
        .map(|s| alloc::string::String::from(s.as_str()))
    else {
        return Err(cx.wrong_type("string->bytevector", 1, args[0], "string"));
    };
    let Some(encoding) = cx
        .heap
        .str(args[1])
        .map(|s| s.as_str().to_ascii_lowercase())
    else {
        return Err(cx.wrong_type("string->bytevector", 2, args[1], "string"));
    };
    let bytes = match encoding.as_str() {
        "utf-8" | "utf8" => text.into_bytes(),
        "latin1" | "latin-1" | "iso-8859-1" | "iso8859-1" => {
            let mut out = Vec::new();
            for c in text.chars() {
                match u8::try_from(c as u32) {
                    Ok(byte) => out.push(byte),
                    Err(_) => {
                        let key = cx.heap.intern("encoding-error");
                        let subr = cx.heap.string("scm_to_stringn");
                        let message = cx
                            .heap
                            .string("cannot convert wide string to output locale");
                        let args = cx.heap.list(&[subr, message, Value::NULL, args[0]]);
                        cx.error = Some(crate::vm::Exception { key, args });
                        return Err(Thrown);
                    }
                }
            }
            out
        }
        _ => {
            return Err(cx.misc_error(
                Some("string->bytevector"),
                "unknown encoding ~S",
                &[args[1]],
            ));
        }
    };
    Ok(cx.heap.wrap(Bytevector(bytes)))
}

/// Guile's array procedures over those above, in `(guile)`.
pub(crate) const PRELUDE: &str = r#"
(define (make-shared-array array mapfunc . bounds)
  (let* ((shape (map (lambda (b) (if (pair? b) b (list 0 (- b 1)))) bounds))
         (lows (map car shape))
         (base (%array-position array (apply mapfunc lows)))
         (strides
          (let loop ((k 0) (dims shape) (out '()))
            (if (null? dims)
                (reverse out)
                (loop (+ k 1) (cdr dims)
                      (cons (if (> (cadar dims) (caar dims))
                                (- (%array-position
                                     array
                                     (apply mapfunc
                                            (let bump ((i 0) (l lows))
                                              (if (null? l) '()
                                                  (cons (if (= i k) (+ (car l) 1) (car l))
                                                        (bump (+ i 1) (cdr l)))))))
                                   base)
                                0)
                            out))))))
    (%make-shared-array array base strides shape)))
;; One array's elements in order (`array->list`'s), several arrays' together.
(define (array-for-each proc array . arrays)
  (define (flat a)
    (let loop ((l (array->list a)) (k (array-rank a)))
      (if (<= k 1) (if (= k 0) (list l) l) (apply append (map (lambda (x) (loop x (- k 1))) l)))))
  (apply for-each proc (flat array) (map flat arrays)))
(define (array-index-map! array proc)
  (let loop ((dims (array-shape array)) (idx '()))
    (if (null? dims)
        (let ((idx (reverse idx)))
          (apply array-set! array (apply proc idx) idx))
        (do ((i (caar dims) (+ i 1)))
            ((> i (cadar dims)))
          (loop (cdr dims) (cons i idx))))))
"#;

/// `(array? x)`: an array, a vector, a string or a bytevector.
pub(crate) fn is_array(cx: &mut Cx, args: &[Value]) -> R {
    let heap = &cx.heap;
    let v = args[0];
    Ok(Value::bool(
        heap.vector_items(v).is_some()
            || heap.str(v).is_some()
            || heap.foreign::<Array>(v).is_some()
            || heap.foreign::<Bytevector>(v).is_some(),
    ))
}
