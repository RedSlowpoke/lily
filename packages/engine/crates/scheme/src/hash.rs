//! Hash tables, object and procedure properties, and procedures with
//! setters (chapter 03 §2.6): Guile's `make-hash-table` and the `hash`,
//! `hashq`, `hashv` and `hashx` families, `make-weak-key-hash-table`,
//! `make-object-property`, `procedure-property`, `make-procedure-with-setter`
//! and `setter`.
//!
//! A table is a foreign object whose entries keep the order they were added
//! in, under an index from a hash of the key to the entries. Which equality
//! a lookup uses is the procedure's, as in Guile: `hashq-ref` of a table
//! filled by `hash-set!` hashes and compares with `eq?`. A strong table
//! holds each entry as a handle, the pair `(key . value)` that
//! `hash-get-handle` returns; a weak-key table holds keys weakly
//! ([`Heap::wrap_weak`]) and its values strongly, and has no handles, as
//! Guile 3's weak tables have none. `hashx-` tables keep the user's buckets,
//! alists indexed by what the user's hash procedure gives.
//!
//! The procedures that call Scheme (`hash-fold`, `hash-for-each`, the
//! `hashx-` family, properties) are [`PRELUDE`], over the primitives here.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::SIZE_MAX;
use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

/// The equality a lookup uses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Eq,
    Eqv,
    Equal,
}

#[derive(Clone)]
struct Entry {
    hash: u64,
    key: Value,
    /// The handle `(key . value)` in a strong table, the value in a weak one.
    value: Value,
}

/// The size a table reports to a `hashx-` procedure's hash, and prints.
const BUCKETS: u64 = 31;

#[derive(Clone)]
pub(crate) struct Table {
    weak: bool,
    entries: Vec<Option<Entry>>,
    /// Hash to the indices of the live entries with it.
    index: BTreeMap<u64, Vec<u32>>,
    count: usize,
    /// `hashx-` buckets: the user's hash to the bucket's alist.
    buckets: BTreeMap<u64, Value>,
}

impl Foreign for Table {
    fn type_name(&self) -> &'static str {
        "hash-table"
    }

    /// `#<hash-table 2/31>`, without the address Guile prints.
    fn print(&self, _: &Heap, out: &mut String) {
        let kind = if self.weak {
            "weak-table"
        } else {
            "hash-table"
        };
        let _ =
            core::fmt::Write::write_fmt(out, format_args!("#<{kind} {}/{BUCKETS}>", self.len()));
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for entry in self.entries.iter().flatten() {
            if !self.weak {
                tracer.trace(entry.key);
            }
            tracer.trace(entry.value);
        }
        for &bucket in self.buckets.values() {
            tracer.trace(bucket);
        }
    }

    fn prune(&mut self, live: &dyn Fn(Value) -> bool) {
        let dead: Vec<u32> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().filter(|e| !live(e.key)).map(|_| i as u32))
            .collect();
        for i in dead {
            self.unlink(i);
        }
        self.compact();
    }

    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        Some(Box::new(self.clone()))
    }

    /// The live entries in their order, with their hashes, and the
    /// buckets; the index is made again when it is read.
    fn save(&self, out: &mut ImageWriter) -> bool {
        out.bool(self.weak);
        out.count(self.count);
        for e in self.entries.iter().flatten() {
            out.u64(e.hash);
            out.value(e.key);
            out.value(e.value);
        }
        out.count(self.buckets.len());
        for (&hash, &bucket) in &self.buckets {
            out.u64(hash);
            out.value(bucket);
        }
        true
    }
}

/// A table as [`Table::save`] wrote it, its index rebuilt.
pub(crate) fn load_table(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let mut table = Table::new(r.bool()?);
    for _ in 0..r.count()? {
        let (hash, key, value) = (r.u64()?, r.value()?, r.value()?);
        table.insert(hash, key, value);
    }
    for _ in 0..r.count()? {
        let hash = r.u64()?;
        table.buckets.insert(hash, r.value()?);
    }
    Some(Box::new(table))
}

impl Table {
    fn new(weak: bool) -> Table {
        Table {
            weak,
            entries: Vec::new(),
            index: BTreeMap::new(),
            count: 0,
            buckets: BTreeMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.count + self.buckets.len()
    }

    fn find(&self, heap: &Heap, kind: Kind, hash: u64, key: Value) -> Option<u32> {
        let candidates = self.index.get(&hash)?;
        candidates.iter().copied().find(|&i| {
            let entry = self.entries[i as usize].as_ref();
            entry.is_some_and(|e| same(heap, kind, e.key, key))
        })
    }

    fn insert(&mut self, hash: u64, key: Value, value: Value) {
        let i = self.entries.len() as u32;
        self.entries.push(Some(Entry { hash, key, value }));
        self.index.entry(hash).or_default().push(i);
        self.count += 1;
    }

    fn remove_at(&mut self, i: u32) {
        self.unlink(i);
        self.compact();
    }

    fn unlink(&mut self, i: u32) {
        let Some(entry) = self.entries[i as usize].take() else {
            return;
        };
        if let Some(list) = self.index.get_mut(&entry.hash) {
            list.retain(|&j| j != i);
            if list.is_empty() {
                self.index.remove(&entry.hash);
            }
        }
        self.count -= 1;
    }

    /// Renumbers the entries once half of them are gone.
    fn compact(&mut self) {
        if self.count * 2 < self.entries.len() && self.entries.len() > 16 {
            let entries = core::mem::take(&mut self.entries);
            self.index.clear();
            self.count = 0;
            for e in entries.into_iter().flatten() {
                self.insert(e.hash, e.key, e.value);
            }
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
        self.buckets.clear();
        self.count = 0;
    }
}

fn same(heap: &Heap, kind: Kind, a: Value, b: Value) -> bool {
    match kind {
        Kind::Eq => a == b,
        Kind::Eqv => heap.eqv(a, b),
        Kind::Equal => heap.equal(a, b),
    }
}

// --- hashing --------------------------------------------------------------------------

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x0100_0000_01b3)
}

fn hash_str(text: &str) -> u64 {
    text.bytes()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| mix(h, b as u64))
}

/// A hash that agrees with `kind`'s equality: `eq?` by the word, `eqv?`
/// numbers by their value, `equal?` strings, pairs and vectors by their
/// contents, as far as `budget` items.
pub(crate) fn hash(heap: &Heap, kind: Kind, v: Value) -> u64 {
    match kind {
        Kind::Eq => mix(0, v.to_bits()),
        Kind::Eqv => hash_eqv(heap, v),
        Kind::Equal => {
            let mut budget = 64;
            hash_equal(heap, v, &mut budget)
        }
    }
}

fn hash_eqv(heap: &Heap, v: Value) -> u64 {
    match v.tag() {
        Tag::Flonum => {
            let x = heap.as_flonum(v).unwrap_or(0.0);
            let bits = if x.is_nan() {
                f64::NAN.to_bits()
            } else {
                x.to_bits()
            };
            mix(1, bits)
        }
        Tag::Bignum => heap.as_bignum(v).map_or(2, |n| {
            mix(mix(2, n.bit_length()), n.count_ones() ^ n.signum() as u64)
        }),
        Tag::Ratio => heap.as_ratio(v).map_or(3, |(n, d)| {
            mix(mix(3, hash_eqv(heap, n)), hash_eqv(heap, d))
        }),
        Tag::Complex => heap
            .as_complex(v)
            .map_or(4, |(re, im)| mix(mix(4, re.to_bits()), im.to_bits())),
        _ => mix(0, v.to_bits()),
    }
}

fn hash_equal(heap: &Heap, v: Value, budget: &mut u32) -> u64 {
    if *budget == 0 {
        return 7;
    }
    *budget -= 1;
    match v.tag() {
        Tag::String => heap.str(v).map_or(5, |s| hash_str(s.as_str())),
        Tag::Pair => {
            let (car, cdr) = (heap.car(v).unwrap_or(v), heap.cdr(v).unwrap_or(v));
            let h = hash_equal(heap, car, budget);
            mix(mix(6, h), hash_equal(heap, cdr, budget))
        }
        Tag::Vector => heap.vector_items(v).map_or(8, |items| {
            items.iter().fold(mix(8, items.len() as u64), |h, &x| {
                mix(h, hash_equal(heap, x, budget))
            })
        }),
        // A foreign object may be `equal?` to another with other contents.
        Tag::Foreign => heap
            .foreign_object(v)
            .map_or(9, |o| hash_str(o.type_name())),
        _ => hash_eqv(heap, v),
    }
}

// --- primitives ---------------------------------------------------------------------------

fn table<'a>(cx: &'a mut Cx, subr: &str, position: usize, v: Value) -> Result<&'a Table, Thrown> {
    if cx.heap.foreign::<Table>(v).is_none() {
        return Err(cx.wrong_type(subr, position, v, "hash-table"));
    }
    Ok(cx
        .heap
        .foreign::<Table>(v)
        .unwrap_or_else(|| unreachable!()))
}

/// `(make-hash-table [size])` and `(make-weak-key-hash-table [size])`.
pub(crate) fn make_hash_table(cx: &mut Cx, args: &[Value]) -> R {
    if let Some(&size) = args.first() {
        cx.size(size, SIZE_MAX)?;
    }
    Ok(cx.heap.wrap(Table::new(false)))
}

pub(crate) fn make_weak_key_hash_table(cx: &mut Cx, args: &[Value]) -> R {
    if let Some(&size) = args.first() {
        cx.size(size, SIZE_MAX)?;
    }
    Ok(cx.heap.wrap_weak(Table::new(true)))
}

pub(crate) fn is_hash_table(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap.foreign::<Table>(args[0]).is_some()))
}

/// The entry of `key`, its table's weakness, and the key's hash.
fn lookup(
    cx: &mut Cx,
    subr: &str,
    kind: Kind,
    t: Value,
    key: Value,
) -> Result<(Option<u32>, bool, u64), Thrown> {
    table(cx, subr, 1, t)?;
    let h = hash(&cx.heap, kind, key);
    let table = cx
        .heap
        .foreign::<Table>(t)
        .unwrap_or_else(|| unreachable!());
    Ok((table.find(&cx.heap, kind, h, key), table.weak, h))
}

fn entry_value(cx: &Cx, t: Value, i: u32) -> Value {
    let table = cx
        .heap
        .foreign::<Table>(t)
        .unwrap_or_else(|| unreachable!());
    let entry = table.entries[i as usize]
        .as_ref()
        .unwrap_or_else(|| unreachable!());
    if table.weak {
        entry.value
    } else {
        cx.heap.cdr(entry.value).unwrap_or(Value::FALSE)
    }
}

fn reference(cx: &mut Cx, subr: &str, kind: Kind, args: &[Value]) -> R {
    let (found, _, _) = lookup(cx, subr, kind, args[0], args[1])?;
    Ok(match found {
        Some(i) => entry_value(cx, args[0], i),
        None => args.get(2).copied().unwrap_or(Value::FALSE),
    })
}

fn set(cx: &mut Cx, subr: &str, kind: Kind, args: &[Value]) -> R {
    let (t, key, value) = (args[0], args[1], args[2]);
    let (found, weak, h) = lookup(cx, subr, kind, t, key)?;
    match (found, weak) {
        (Some(i), true) => {
            let table = cx
                .heap
                .foreign_mut::<Table>(t)
                .unwrap_or_else(|| unreachable!());
            if let Some(e) = table.entries[i as usize].as_mut() {
                e.value = value;
            }
        }
        (Some(i), false) => {
            let handle = cx
                .heap
                .foreign::<Table>(t)
                .and_then(|t| t.entries[i as usize].as_ref())
                .map_or(Value::NULL, |e| e.value);
            cx.heap.set_cdr(handle, value);
        }
        (None, weak) => {
            let stored = if weak {
                value
            } else {
                cx.heap.cons(key, value)
            };
            let table = cx
                .heap
                .foreign_mut::<Table>(t)
                .unwrap_or_else(|| unreachable!());
            table.insert(h, key, stored);
        }
    }
    Ok(value)
}

fn remove(cx: &mut Cx, subr: &str, kind: Kind, args: &[Value]) -> R {
    let (found, _, _) = lookup(cx, subr, kind, args[0], args[1])?;
    let Some(i) = found else {
        return Ok(Value::FALSE);
    };
    let table = cx
        .heap
        .foreign_mut::<Table>(args[0])
        .unwrap_or_else(|| unreachable!());
    let removed = table.entries[i as usize]
        .as_ref()
        .map_or(Value::FALSE, |e| e.value);
    table.remove_at(i);
    Ok(removed)
}

/// Guile 3's weak tables have no handles: its C function says the table
/// is not a hash table.
fn no_handles(cx: &mut Cx, subr: &str, t: Value) -> Thrown {
    cx.wrong_type(subr, 1, t, "hash-table")
}

fn get_handle(cx: &mut Cx, subr: &str, kind: Kind, args: &[Value]) -> R {
    let (found, weak, _) = lookup(cx, subr, kind, args[0], args[1])?;
    if weak {
        return Err(no_handles(cx, subr, args[0]));
    }
    Ok(match found {
        Some(i) => {
            let table = cx
                .heap
                .foreign::<Table>(args[0])
                .unwrap_or_else(|| unreachable!());
            table.entries[i as usize]
                .as_ref()
                .map_or(Value::FALSE, |e| e.value)
        }
        None => Value::FALSE,
    })
}

fn create_handle(cx: &mut Cx, subr: &str, kind: Kind, args: &[Value]) -> R {
    let (found, weak, h) = lookup(cx, subr, kind, args[0], args[1])?;
    if weak {
        return Err(no_handles(cx, subr, args[0]));
    }
    if let Some(i) = found {
        let table = cx
            .heap
            .foreign::<Table>(args[0])
            .unwrap_or_else(|| unreachable!());
        return Ok(table.entries[i as usize]
            .as_ref()
            .map_or(Value::FALSE, |e| e.value));
    }
    let handle = cx.heap.cons(args[1], args[2]);
    let table = cx
        .heap
        .foreign_mut::<Table>(args[0])
        .unwrap_or_else(|| unreachable!());
    table.insert(h, args[1], handle);
    Ok(handle)
}

/// Guile's C functions under each family, which its errors name.
const GET: &str = "scm_hash_fn_get_handle";
const CREATE: &str = "scm_hash_fn_create_handle_x";
const REMOVE: &str = "hash_fn_remove_x";

macro_rules! family {
    ($kind:expr, $ref:ident, $set:ident, $rem:ident, $get:ident, $create:ident) => {
        pub(crate) fn $ref(cx: &mut Cx, args: &[Value]) -> R {
            reference(cx, GET, $kind, args)
        }
        pub(crate) fn $set(cx: &mut Cx, args: &[Value]) -> R {
            set(cx, CREATE, $kind, args)
        }
        pub(crate) fn $rem(cx: &mut Cx, args: &[Value]) -> R {
            remove(cx, REMOVE, $kind, args)
        }
        pub(crate) fn $get(cx: &mut Cx, args: &[Value]) -> R {
            get_handle(cx, GET, $kind, args)
        }
        pub(crate) fn $create(cx: &mut Cx, args: &[Value]) -> R {
            create_handle(cx, CREATE, $kind, args)
        }
    };
}

family!(
    Kind::Equal,
    hash_ref,
    hash_set,
    hash_remove,
    hash_get_handle,
    hash_create_handle
);
family!(
    Kind::Eq,
    hashq_ref,
    hashq_set,
    hashq_remove,
    hashq_get_handle,
    hashq_create_handle
);
family!(
    Kind::Eqv,
    hashv_ref,
    hashv_set,
    hashv_remove,
    hashv_get_handle,
    hashv_create_handle
);

pub(crate) fn hash_clear(cx: &mut Cx, args: &[Value]) -> R {
    table(cx, "hash-clear!", 1, args[0])?;
    if let Some(t) = cx.heap.foreign_mut::<Table>(args[0]) {
        t.clear();
    }
    Ok(Value::UNSPECIFIED)
}

/// `(%hash-entries table subr position)`: its entries as pairs `(key .
/// value)`, the last added first, and the `hashx-` buckets' after them; a
/// strong table's are its handles. Not a table is `subr`'s error.
pub(crate) fn hash_entries(cx: &mut Cx, args: &[Value]) -> R {
    if cx.heap.foreign::<Table>(args[0]).is_none() {
        let subr = String::from(cx.heap.name(args[1]).unwrap_or("hash-fold"));
        let position = args[2].as_fixnum().unwrap_or(1) as usize;
        return Err(cx.wrong_type(&subr, position, args[0], "hash-table"));
    }
    let t = table(cx, "hash-fold", 3, args[0])?;
    let weak = t.weak;
    let mut pairs: Vec<(Value, Value)> = t
        .entries
        .iter()
        .flatten()
        .map(|e| (e.key, e.value))
        .collect();
    let buckets: Vec<Value> = t.buckets.values().copied().collect();
    let mut out = Value::NULL;
    for bucket in buckets {
        let mut at = bucket;
        while let (Some(h), Some(next)) = (cx.heap.car(at), cx.heap.cdr(at)) {
            out = cx.heap.cons(h, out);
            at = next;
        }
    }
    for (key, value) in pairs.drain(..) {
        let handle = if weak {
            cx.heap.cons(key, value)
        } else {
            value
        };
        out = cx.heap.cons(handle, out);
    }
    Ok(out)
}

/// `(%hashx-bucket table index)` and `(%hashx-bucket-set! table index
/// alist)`: a `hashx-` table's buckets.
pub(crate) fn hashx_bucket(cx: &mut Cx, args: &[Value]) -> R {
    let i = cx.size(args[1], SIZE_MAX)? as u64;
    let t = table(cx, "hashx-ref", 3, args[0])?;
    Ok(t.buckets.get(&i).copied().unwrap_or(Value::NULL))
}

pub(crate) fn hashx_bucket_set(cx: &mut Cx, args: &[Value]) -> R {
    let i = cx.size(args[1], SIZE_MAX)? as u64;
    table(cx, "hashx-set!", 3, args[0])?;
    if let Some(t) = cx.heap.foreign_mut::<Table>(args[0]) {
        if args[2] == Value::NULL {
            t.buckets.remove(&i);
        } else {
            t.buckets.insert(i, args[2]);
        }
    }
    Ok(Value::UNSPECIFIED)
}

/// `(hash key size)`, `(hashq key size)`, `(hashv key size)`: a hash below
/// `size`, not Guile's numbers.
fn hash_below(cx: &mut Cx, kind: Kind, args: &[Value]) -> R {
    let size = cx.size(args[1], SIZE_MAX)?;
    if size == 0 {
        return Err(cx.out_of_range("hash", 2, args[1]));
    }
    let h = hash(&cx.heap, kind, args[0]) % size as u64;
    Ok(cx.heap.integer(h as i64))
}

pub(crate) fn hash_fn(cx: &mut Cx, args: &[Value]) -> R {
    hash_below(cx, Kind::Equal, args)
}
pub(crate) fn hashq_fn(cx: &mut Cx, args: &[Value]) -> R {
    hash_below(cx, Kind::Eq, args)
}
pub(crate) fn hashv_fn(cx: &mut Cx, args: &[Value]) -> R {
    hash_below(cx, Kind::Eqv, args)
}

// --- procedures with setters ------------------------------------------------------------

/// Guile's `<procedure-with-setter>`, which applies as its procedure.
pub(crate) struct WithSetter {
    pub procedure: Value,
    pub setter: Value,
}

impl Foreign for WithSetter {
    fn type_name(&self) -> &'static str {
        "procedure-with-setter"
    }

    /// Guile prints the struct with its addresses: `#<struct:… pwpw …
    /// proc: … setter: …>`; the engine leaves them out.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<procedure-with-setter proc: ");
        heap.write(self.procedure, out);
        out.push_str(" setter: ");
        heap.write(self.setter, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.procedure);
        tracer.trace(self.setter);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.procedure);
        out.value(self.setter);
        true
    }
}

pub(crate) fn load_with_setter(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(WithSetter {
        procedure: r.value()?,
        setter: r.value()?,
    }))
}

/// The procedure a procedure with a setter applies as.
pub(crate) fn with_setter_procedure(heap: &Heap, value: Value) -> Option<Value> {
    heap.foreign::<WithSetter>(value).map(|p| p.procedure)
}

pub(crate) fn make_procedure_with_setter(cx: &mut Cx, args: &[Value]) -> R {
    for (i, &p) in args.iter().enumerate() {
        if !crate::prims::procedure(&cx.heap, p) {
            return Err(cx.wrong_type_at("make-procedure-with-setter", i + 1, p));
        }
    }
    Ok(cx.heap.wrap(WithSetter {
        procedure: args[0],
        setter: args[1],
    }))
}

pub(crate) fn is_procedure_with_setter(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap.foreign::<WithSetter>(args[0]).is_some(),
    ))
}

pub(crate) fn setter(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<WithSetter>(args[0]) {
        Some(p) => Ok(p.setter),
        None => Err(cx.wrong_type_at("setter", 1, args[0])),
    }
}

/// What Guile's `boot-9.scm` and `procprop.c` define over the tables:
/// folds, object properties as weak-key tables, and procedure properties,
/// whose `name` is the procedure's own unless one was set.
pub(crate) const PRELUDE: &str = r#"
(define (hash-fold proc init table)
  (let loop ((entries (%hash-entries table 'hash-fold 3)) (acc init))
    (if (null? entries)
        acc
        (loop (cdr entries) (proc (caar entries) (cdar entries) acc)))))
(define (hash-for-each proc table)
  (for-each (lambda (h) (proc (car h) (cdr h))) (%hash-entries table 'hash-for-each 2)))
(define (hash-for-each-handle proc table)
  (for-each proc (%hash-entries table 'hash-for-each-handle 2)))
(define (hash-map->list proc table)
  (hash-fold (lambda (k v acc) (cons (proc k v) acc)) '() table))
(define (hash-count pred table)
  (let loop ((entries (%hash-entries table 'hash-count 2)) (n 0))
    (cond ((null? entries) n)
          ((pred (caar entries) (cdar entries)) (loop (cdr entries) (+ n 1)))
          (else (loop (cdr entries) n)))))
(define (hashx-get-handle hash assoc table key)
  (assoc key (%hashx-bucket table (hash key 31))))
(define (hashx-ref hash assoc table key . default)
  (let ((h (hashx-get-handle hash assoc table key)))
    (cond (h (cdr h)) ((pair? default) (car default)) (else #f))))
(define (hashx-create-handle! hash assoc table key init)
  (let* ((i (hash key 31)) (bucket (%hashx-bucket table i)) (h (assoc key bucket)))
    (or h
        (let ((h (cons key init)))
          (%hashx-bucket-set! table i (cons h bucket))
          h))))
(define (hashx-set! hash assoc table key value)
  (set-cdr! (hashx-create-handle! hash assoc table key value) value)
  value)
(define (hashx-remove! hash assoc table key)
  (let* ((i (hash key 31)) (bucket (%hashx-bucket table i)) (h (assoc key bucket)))
    (if h (%hashx-bucket-set! table i (delq h bucket)))
    h))
(define (make-object-property)
  (let ((table (make-weak-key-hash-table)))
    (make-procedure-with-setter
     (lambda (obj) (hashq-ref table obj #f))
     (lambda (obj val) (hashq-set! table obj val)))))
(define %object-properties (make-weak-key-hash-table))
(define (object-properties obj) (hashq-ref %object-properties obj '()))
(define (set-object-properties! obj alist) (hashq-set! %object-properties obj alist))
(define (object-property obj key) (assq-ref (object-properties obj) key))
(define (set-object-property! obj key value)
  (set-object-properties! obj (assq-set! (object-properties obj) key value))
  value)
(define %procedure-properties (make-weak-key-hash-table))
(define (%check-procedure subr p)
  (if (not (procedure? p))
      (scm-error 'wrong-type-arg subr "Wrong type argument in position ~A: ~S"
                 (list 1 p) (list p))))
(define (procedure-properties p)
  (%check-procedure "procedure-properties" p)
  (let ((props (hashq-ref %procedure-properties p '()))
        (name (%procedure-name p)))
    (if (and name (not (assq 'name props))) (acons 'name name props) props)))
(define (set-procedure-properties! p alist)
  (%check-procedure "set-procedure-properties!" p)
  (hashq-set! %procedure-properties p alist))
(define (procedure-property p key)
  (%check-procedure "procedure-property" p)
  (assq-ref (procedure-properties p) key))
(define (set-procedure-property! p key value)
  (%check-procedure "set-procedure-property!" p)
  (hashq-set! %procedure-properties p
              (assq-set! (hashq-ref %procedure-properties p '()) key value)))
(define (procedure-name p)
  (%check-procedure "procedure-name" p)
  (let ((props (hashq-ref %procedure-properties p '())))
    (if (assq 'name props) (assq-ref props 'name) (%procedure-name p))))
(define (procedure-documentation p)
  (%check-procedure "procedure-documentation" p)
  (assq-ref (hashq-ref %procedure-properties p '()) 'documentation))
"#;
