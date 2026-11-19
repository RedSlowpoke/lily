//! Guile's list procedures and SRFI-1 (chapter 03 §2.6): the alist ones
//! (`acons`, `assq-ref`, `assq-set!` …), `iota`, `make-list`, `list-head`,
//! the destructive `reverse!`, `append!` and `delq!` families, `sort` and
//! `merge`, in `(guile)` as Guile has them; and the `(srfi srfi-1)` module,
//! made with every VM, with SRFI-1's procedures and its versions of
//! `delete`, `member`, `assoc`, `map`, `for-each` and `list-index`.
//!
//! What calls a procedure is Scheme ([`PRELUDE`] in `(guile)` and
//! [`SRFI_1_PRELUDE`] in `(srfi srfi-1)`), written for the engine from
//! SRFI-1's specification; the rest is Rust here. [`SRFI_1`] names the
//! primitives that belong to `(srfi srfi-1)`.

use alloc::vec::Vec;

use crate::errors::SIZE_MAX;
use crate::heap::Heap;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

/// The primitives defined in `(srfi srfi-1)`, not `(guile)`.
pub(crate) const SRFI_1: &[&str] = &[
    "take",
    "drop",
    "take-right",
    "drop-right",
    "last",
    "length+",
    "concatenate",
    "append-reverse",
];

/// The items of a list up to a tail that is not a pair, and that tail.
fn walk(heap: &Heap, list: Value) -> (Vec<Value>, Value) {
    let mut out = Vec::new();
    let mut at = list;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        out.push(car);
        at = cdr;
    }
    (out, at)
}

fn build(heap: &mut Heap, items: &[Value], tail: Value) -> Value {
    items
        .iter()
        .rev()
        .fold(tail, |tail, &item| heap.cons(item, tail))
}

type Same = fn(&Heap, Value, Value) -> bool;

fn eq(_: &Heap, a: Value, b: Value) -> bool {
    a == b
}
fn eqv(heap: &Heap, a: Value, b: Value) -> bool {
    heap.eqv(a, b)
}
fn equal(heap: &Heap, a: Value, b: Value) -> bool {
    heap.equal(a, b)
}

// --- alists ------------------------------------------------------------------------

pub(crate) fn acons(cx: &mut Cx, args: &[Value]) -> R {
    let entry = cx.heap.cons(args[0], args[1]);
    Ok(cx.heap.cons(entry, args[2]))
}

/// The first pair of the alist whose car is `same` to `key`, skipping what
/// is not a pair, as Guile's `sloppy-assq` does.
fn sloppy(heap: &Heap, alist: Value, key: Value, same: Same) -> Option<Value> {
    let mut at = alist;
    while let (Some(entry), Some(next)) = (heap.car(at), heap.cdr(at)) {
        if heap.car(entry).is_some_and(|k| same(heap, k, key)) {
            return Some(entry);
        }
        at = next;
    }
    None
}

fn reference(cx: &mut Cx, args: &[Value], same: Same) -> R {
    Ok(sloppy(&cx.heap, args[0], args[1], same)
        .and_then(|entry| cx.heap.cdr(entry))
        .unwrap_or(Value::FALSE))
}

/// `assq-set!`: sets the entry's value, or puts a new entry in front.
fn set(cx: &mut Cx, args: &[Value], same: Same) -> R {
    match sloppy(&cx.heap, args[0], args[1], same) {
        Some(entry) => {
            cx.heap.set_cdr(entry, args[2]);
            Ok(args[0])
        }
        None => acons(cx, &[args[1], args[2], args[0]]),
    }
}

/// `assq-remove!`: unlinks the first entry of the key.
fn remove(cx: &mut Cx, args: &[Value], same: Same) -> R {
    let Some(entry) = sloppy(&cx.heap, args[0], args[1], same) else {
        return Ok(args[0]);
    };
    Ok(unlink(&mut cx.heap, args[0], |_, v| v == entry, true))
}

macro_rules! alist {
    ($same:ident, $r:ident, $s:ident, $rm:ident) => {
        pub(crate) fn $r(cx: &mut Cx, args: &[Value]) -> R {
            reference(cx, args, $same)
        }
        pub(crate) fn $s(cx: &mut Cx, args: &[Value]) -> R {
            set(cx, args, $same)
        }
        pub(crate) fn $rm(cx: &mut Cx, args: &[Value]) -> R {
            remove(cx, args, $same)
        }
    };
}

alist!(eq, assq_ref, assq_set, assq_remove);
alist!(eqv, assv_ref, assv_set, assv_remove);
alist!(equal, assoc_ref, assoc_set, assoc_remove);

// --- deleting --------------------------------------------------------------------------

/// Unlinks the pairs of `list` whose car `hit` says, by `set-cdr!`, all of
/// them or only the first; the list that is left.
fn unlink(heap: &mut Heap, list: Value, hit: impl Fn(&Heap, Value) -> bool, once: bool) -> Value {
    let mut head = list;
    let mut previous: Option<Value> = None;
    let mut at = list;
    while let (Some(car), Some(next)) = (heap.car(at), heap.cdr(at)) {
        if hit(heap, car) {
            match previous {
                Some(p) => {
                    heap.set_cdr(p, next);
                }
                None => head = next,
            }
            if once {
                break;
            }
        } else {
            previous = Some(at);
        }
        at = next;
    }
    head
}

/// `delq`: a new list without the items `same` to `x`, as Guile's.
fn delete_copy(cx: &mut Cx, subr: &str, args: &[Value], same: Same) -> R {
    let (items, tail) = walk(&cx.heap, args[1]);
    if tail != Value::NULL {
        return Err(cx.wrong_type(subr, 2, args[1], "list"));
    }
    let x = args[0];
    let kept: Vec<Value> = items
        .into_iter()
        .filter(|&v| !same(&cx.heap, x, v))
        .collect();
    Ok(cx.heap.list(&kept))
}

macro_rules! deleting {
    ($same:ident, $copy:ident, $cs:literal, $bang:ident, $once:ident) => {
        pub(crate) fn $copy(cx: &mut Cx, args: &[Value]) -> R {
            delete_copy(cx, $cs, args, $same)
        }
        pub(crate) fn $bang(cx: &mut Cx, args: &[Value]) -> R {
            let x = args[0];
            Ok(unlink(&mut cx.heap, args[1], |h, v| $same(h, x, v), false))
        }
        pub(crate) fn $once(cx: &mut Cx, args: &[Value]) -> R {
            let x = args[0];
            Ok(unlink(&mut cx.heap, args[1], |h, v| $same(h, x, v), true))
        }
    };
}

deleting!(eq, delq, "delq", delq_bang, delq1_bang);
deleting!(eqv, delv, "delv", delv_bang, delv1_bang);
deleting!(equal, delete, "delete", delete_bang, delete1_bang);

// --- building --------------------------------------------------------------------------

/// `(iota count [start step])`: `start` and `step` may be any numbers.
/// `iota`, as Guile 2.2's (`srfi-1.scm`): a count that is a non-negative
/// integer, exact or inexact (`3.0`, which `make-dotted-bar-line` gives
/// it), and the items made from the last to the first, each `(+ start (*
/// i step))` with `i` as exact as the count.
pub(crate) fn iota(cx: &mut Cx, args: &[Value]) -> R {
    let count = match cx.heap.to_i64(args[0]) {
        Some(n) if n >= 0 => Some((n as usize, false)),
        Some(_) => None,
        None => cx
            .heap
            .as_flonum(args[0])
            .filter(|&x| (0.0..9.0e15).contains(&x) && (x as u64) as f64 == x)
            .map(|x| (x as usize, true)),
    };
    let Some((n, inexact)) = count else {
        // Guile's check throws the count alone.
        let key = cx.heap.intern("wrong-type-arg");
        let args = cx.heap.list(&[args[0]]);
        cx.error = Some(crate::vm::Exception { key, args });
        return Err(Thrown);
    };
    let zero = Value::fixnum(0).unwrap_or(Value::NULL);
    let one = Value::fixnum(1).unwrap_or(Value::NULL);
    let start = args.get(1).copied().unwrap_or(zero);
    let step = args.get(2).copied().unwrap_or(one);
    let mut list = Value::NULL;
    for i in (0..n).rev() {
        let i = match inexact {
            true => cx.heap.flonum(i as f64),
            false => cx.heap.integer(i as i64),
        };
        let offset = cx
            .heap
            .mul(i, step)
            .map_err(|e| cx.num_error("*", e, &[i, step]))?;
        let item = cx
            .heap
            .add(start, offset)
            .map_err(|e| cx.num_error("+", e, &[start, offset]))?;
        list = cx.heap.cons(item, list);
    }
    Ok(list)
}

pub(crate) fn make_list(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[0], SIZE_MAX)?;
    let fill = args.get(1).copied().unwrap_or(Value::NULL);
    Ok((0..n).fold(Value::NULL, |tail, _| cx.heap.cons(fill, tail)))
}

/// The pair `k` pairs into `list`, or `subr`'s error about what ended it.
fn nth_pair(cx: &mut Cx, subr: &str, list: Value, k: Value) -> R {
    let n = cx.size(k, SIZE_MAX)?;
    let mut at = list;
    for _ in 0..n {
        match cx.heap.cdr(at) {
            Some(next) => at = next,
            None => return Err(cx.wrong_type(subr, 1, at, "pair")),
        }
    }
    Ok(at)
}

pub(crate) fn list_head(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[1], SIZE_MAX)?;
    let mut items = Vec::new();
    let mut at = args[0];
    for _ in 0..n {
        match (cx.heap.car(at), cx.heap.cdr(at)) {
            (Some(car), Some(cdr)) => {
                items.push(car);
                at = cdr;
            }
            _ => return Err(cx.wrong_type("list-head", 1, at, "pair")),
        }
    }
    Ok(cx.heap.list(&items))
}

pub(crate) fn list_set(cx: &mut Cx, args: &[Value]) -> R {
    let pair = nth_pair(cx, "list-set!", args[0], args[1])?;
    if !cx.heap.set_car(pair, args[2]) {
        return Err(cx.out_of_range("list-set!", 2, args[1]));
    }
    Ok(Value::UNSPECIFIED)
}

/// `(reverse! list [tail])`, reusing the pairs. An improper list is put
/// back as it was and reported as Guile's `scm_reverse_x` reports it.
pub(crate) fn reverse_bang(cx: &mut Cx, args: &[Value]) -> R {
    let tail = args.get(1).copied().unwrap_or(Value::NULL);
    let mut done = tail;
    let mut at = args[0];
    while let Some(next) = cx.heap.cdr(at) {
        cx.heap.set_cdr(at, done);
        done = at;
        at = next;
    }
    if at != Value::NULL {
        let mut rest = at;
        while done != tail {
            let Some(next) = cx.heap.cdr(done) else { break };
            cx.heap.set_cdr(done, rest);
            rest = done;
            done = next;
        }
        return Err(cx.wrong_type_at("reverse!", 1, args[0]));
    }
    Ok(done)
}

/// `append!`: each list's last pair points at the next non-empty one.
pub(crate) fn append_bang(cx: &mut Cx, args: &[Value]) -> R {
    let mut result = Value::NULL;
    let mut last: Option<Value> = None;
    for (i, &list) in args.iter().enumerate() {
        if list == Value::NULL {
            continue;
        }
        match last {
            Some(pair) => {
                cx.heap.set_cdr(pair, list);
            }
            None => result = list,
        }
        if i + 1 < args.len() {
            if list.tag() != Tag::Pair {
                return Err(cx.wrong_type("append!", i + 1, list, "list"));
            }
            let mut at = list;
            while let Some(next) = cx.heap.cdr(at).filter(|n| n.tag() == Tag::Pair) {
                at = next;
            }
            last = Some(at);
        }
    }
    Ok(result)
}

// --- SRFI-1 ------------------------------------------------------------------------------

pub(crate) fn take(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[1], SIZE_MAX)?;
    let mut items = Vec::new();
    let mut at = args[0];
    for _ in 0..n {
        match (cx.heap.car(at), cx.heap.cdr(at)) {
            (Some(car), Some(cdr)) => {
                items.push(car);
                at = cdr;
            }
            _ => return Err(cx.wrong_type("car", 1, at, "pair")),
        }
    }
    Ok(cx.heap.list(&items))
}

pub(crate) fn drop(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[1], SIZE_MAX)?;
    let mut at = args[0];
    for _ in 0..n {
        match cx.heap.cdr(at) {
            Some(next) => at = next,
            None => return Err(cx.wrong_type("cdr", 1, at, "pair")),
        }
    }
    Ok(at)
}

/// How many pairs `take-right` and `drop-right` leave: the length minus `k`.
fn from_right(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
) -> Result<(Vec<Value>, Value, usize), Thrown> {
    let k = cx.size(args[1], SIZE_MAX)?;
    let (items, tail) = walk(&cx.heap, args[0]);
    match items.len().checked_sub(k) {
        Some(n) => Ok((items, tail, n)),
        None => Err(cx.wrong_type(subr, 1, args[0], "list")),
    }
}

pub(crate) fn take_right(cx: &mut Cx, args: &[Value]) -> R {
    let (_, _, n) = from_right(cx, "take-right", args)?;
    let mut at = args[0];
    for _ in 0..n {
        at = cx.heap.cdr(at).unwrap_or(Value::NULL);
    }
    Ok(at)
}

pub(crate) fn drop_right(cx: &mut Cx, args: &[Value]) -> R {
    let (items, _, n) = from_right(cx, "drop-right", args)?;
    Ok(cx.heap.list(&items[..n]))
}

pub(crate) fn last(cx: &mut Cx, args: &[Value]) -> R {
    let (items, _) = walk(&cx.heap, args[0]);
    match items.last() {
        Some(&v) => Ok(v),
        None => Err(cx.wrong_type("car", 1, args[0], "pair")),
    }
}

/// `length+`: `#f` for a circular list.
pub(crate) fn length_plus(cx: &mut Cx, args: &[Value]) -> R {
    let (mut slow, mut fast) = (args[0], args[0]);
    let mut n = 0i64;
    loop {
        for _ in 0..2 {
            match cx.heap.cdr(fast) {
                Some(next) => {
                    fast = next;
                    n += 1;
                }
                None => return Ok(cx.heap.integer(n)),
            }
        }
        slow = cx.heap.cdr(slow).unwrap_or(Value::NULL);
        if slow == fast {
            return Ok(Value::FALSE);
        }
    }
}

pub(crate) fn concatenate(cx: &mut Cx, args: &[Value]) -> R {
    let (lists, _) = walk(&cx.heap, args[0]);
    let Some((&last, init)) = lists.split_last() else {
        return Ok(Value::NULL);
    };
    let mut all = Vec::new();
    for &l in init {
        all.extend(walk(&cx.heap, l).0);
    }
    Ok(build(&mut cx.heap, &all, last))
}

pub(crate) fn append_reverse(cx: &mut Cx, args: &[Value]) -> R {
    let (items, _) = walk(&cx.heap, args[0]);
    Ok(items
        .into_iter()
        .fold(args[1], |tail, item| cx.heap.cons(item, tail)))
}

/// Guile's list procedures that take a procedure, in `(guile)`. `sort` is a
/// merge sort, so stable, as Guile's is; a vector is sorted as a list, while
/// `sort-list` takes a list only.
pub(crate) const PRELUDE: &str = r#"
(define (filter pred l)
  (if (not (list? l))
      (scm-error 'wrong-type-arg "filter" "Wrong type argument in position ~A: ~S"
                 (list 2 l) (list l)))
  (let loop ((l l) (out '()))
    (cond ((null? l) (reverse! out))
          ((pred (car l)) (loop (cdr l) (cons (car l) out)))
          (else (loop (cdr l) out)))))
(define filter! filter)
(define map-in-order map)
(define (merge a b less)
  (let loop ((a a) (b b) (out '()))
    (cond ((null? a) (reverse! out b))
          ((null? b) (reverse! out a))
          ((less (car b) (car a)) (loop a (cdr b) (cons (car b) out)))
          (else (loop (cdr a) b (cons (car a) out))))))
(define merge! merge)
(define (%sort-list l less)
  (let sort ((l l) (n (length l)))
    (if (< n 2)
        (if (= n 0) '() (list (car l)))
        (let ((half (quotient n 2)))
          (merge (sort l half) (sort (list-tail l half) (- n half)) less)))))
(define (sort items less)
  (if (vector? items)
      (list->vector (%sort-list (vector->list items) less))
      (%sort-list items less)))
(define sort! sort)
(define (%sort-list-only subr items less)
  (if (not (list? items))
      (scm-error 'wrong-type-arg subr "Wrong type argument in position ~A: ~S"
                 (list 1 items) (list items)))
  (%sort-list items less))
(define (sort-list items less) (%sort-list-only "sort-list" items less))
(define (sort-list! items less) (%sort-list-only "sort-list!" items less))
(define stable-sort sort)
(define stable-sort! sort)
(define (sorted? items less)
  (let loop ((l (if (vector? items) (vector->list items) items)))
    (or (null? l) (null? (cdr l))
        (and (not (less (cadr l) (car l))) (loop (cdr l))))))
(define (%cars+cdrs lists)
  (let loop ((ls lists) (cars '()) (cdrs '()))
    (cond ((null? ls) (values (reverse! cars) (reverse! cdrs)))
          ((pair? (car ls)) (loop (cdr ls) (cons (caar ls) cars) (cons (cdar ls) cdrs)))
          (else (values #f #f)))))
(define (%lset-member? = x l)
  (let loop ((l l))
    (and (pair? l) (or (= (car l) x) (loop (cdr l))))))
"#;

/// SRFI-1's procedures that take a procedure, and the short ones, in
/// `(srfi srfi-1)`, which uses `(guile)`.
pub(crate) const SRFI_1_PRELUDE: &str = r#"
(define first car)
(define second cadr)
(define third caddr)
(define fourth cadddr)
(define (fifth x) (car (cddddr x)))
(define (sixth x) (cadr (cddddr x)))
(define (seventh x) (caddr (cddddr x)))
(define (eighth x) (cadddr (cddddr x)))
(define (ninth x) (car (cddddr (cddddr x))))
(define (tenth x) (cadr (cddddr (cddddr x))))
(define (xcons a b) (cons b a))
(define (car+cdr p) (values (car p) (cdr p)))
(define (not-pair? x) (not (pair? x)))
(define (proper-list? x) (list? x))
(define (circular-list? x) (and (pair? x) (not (length+ x))))
(define (dotted-list? x) (and (length+ x) (not (list? x))))
(define (null-list? l)
  (cond ((pair? l) #f)
        ((null? l) #t)
        (else (error "null-list?: argument out of domain" l))))
(define (circular-list x . rest)
  (let ((l (cons x rest)))
    (set-cdr! (last-pair l) l)
    l))
(define (list-tabulate n proc)
  (let loop ((i (- n 1)) (out '()))
    (if (< i 0) out (loop (- i 1) (cons (proc i) out)))))
(define (split-at l k) (values (take l k) (drop l k)))
(define split-at! split-at)
(define take! take)
(define drop-right! drop-right)
(define concatenate! concatenate)
(define (fold kons knil l . ls)
  (if (null? ls)
      (let loop ((l l) (acc knil))
        (if (null? l) acc (loop (cdr l) (kons (car l) acc))))
      (let loop ((ls (cons l ls)) (acc knil))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars
                (loop cdrs (apply kons (append cars (list acc))))
                acc))))))
(define (fold-right kons knil l . ls)
  (if (null? ls)
      (let loop ((l (reverse l)) (acc knil))
        (if (null? l) acc (loop (cdr l) (kons (car l) acc))))
      (let loop ((ls (cons l ls)))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars
                (apply kons (append cars (list (loop cdrs))))
                knil))))))
(define (pair-fold kons knil l)
  (let loop ((l l) (acc knil))
    (if (pair? l)
        (let ((next (cdr l))) (loop next (kons l acc)))
        acc)))
(define (pair-fold-right kons knil l)
  (let loop ((l l))
    (if (pair? l) (kons l (loop (cdr l))) knil)))
(define (pair-for-each proc l)
  (let loop ((l l))
    (if (pair? l)
        (let ((next (cdr l))) (proc l) (loop next)))))
(define (reduce f ridentity l)
  (if (null? l) ridentity (fold f (car l) (cdr l))))
(define (reduce-right f ridentity l)
  (if (null? l)
      ridentity
      (let loop ((l l))
        (if (null? (cdr l)) (car l) (f (car l) (loop (cdr l)))))))
(define (append-reverse! rev tail) (reverse! rev tail))
(define (unfold p f g seed . tail-gen)
  (let loop ((seed seed) (out '()))
    (if (p seed)
        (append-reverse! out (if (pair? tail-gen) ((car tail-gen) seed) '()))
        (loop (g seed) (cons (f seed) out)))))
(define (unfold-right p f g seed . tail)
  (let loop ((seed seed) (out (if (pair? tail) (car tail) '())))
    (if (p seed) out (loop (g seed) (cons (f seed) out)))))
(define (map f l . ls)
  (if (null? ls)
      (let loop ((l l) (out '()))
        (if (pair? l) (loop (cdr l) (cons (f (car l)) out)) (reverse! out)))
      (let loop ((ls (cons l ls)) (out '()))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars (loop cdrs (cons (apply f cars) out)) (reverse! out)))))))
(define map-in-order map)
(define map! map)
(define (for-each f l . ls)
  (if (null? ls)
      (let loop ((l l))
        (if (pair? l) (begin (f (car l)) (loop (cdr l)))))
      (let loop ((ls (cons l ls)))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars (begin (apply f cars) (loop cdrs))))))))
(define (append-map f l . ls)
  (apply append (apply map f l ls)))
(define append-map! append-map)
(define (filter-map f l . ls)
  (let loop ((l (apply map f l ls)) (out '()))
    (cond ((null? l) (reverse! out))
          ((car l) (loop (cdr l) (cons (car l) out)))
          (else (loop (cdr l) out)))))
(define (count pred l . ls)
  (if (null? ls)
      (let loop ((l l) (n 0))
        (if (null? l) n (loop (cdr l) (if (pred (car l)) (+ n 1) n))))
      (let loop ((ls (cons l ls)) (n 0))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars (loop cdrs (if (apply pred cars) (+ n 1) n)) n))))))
(define (remove pred l) (filter (lambda (x) (not (pred x))) l))
(define remove! remove)
(define (partition pred l)
  (let loop ((l l) (in '()) (out '()))
    (cond ((null? l) (values (reverse! in) (reverse! out)))
          ((pred (car l)) (loop (cdr l) (cons (car l) in) out))
          (else (loop (cdr l) in (cons (car l) out))))))
(define partition! partition)
(define (find pred l)
  (let ((tail (find-tail pred l))) (and tail (car tail))))
(define (find-tail pred l)
  (let loop ((l l))
    (cond ((null? l) #f) ((pred (car l)) l) (else (loop (cdr l))))))
(define (take-while pred l)
  (let loop ((l l) (out '()))
    (if (and (pair? l) (pred (car l)))
        (loop (cdr l) (cons (car l) out))
        (reverse! out))))
(define take-while! take-while)
(define (drop-while pred l)
  (let loop ((l l))
    (if (and (pair? l) (pred (car l))) (loop (cdr l)) l)))
(define (span pred l) (values (take-while pred l) (drop-while pred l)))
(define span! span)
(define (break pred l) (span (lambda (x) (not (pred x))) l))
(define break! break)
(define (any pred l . ls)
  (if (null? ls)
      (let loop ((l l))
        (cond ((null? l) #f)
              ((null? (cdr l)) (pred (car l)))
              (else (or (pred (car l)) (loop (cdr l))))))
      (let loop ((ls (cons l ls)))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (and cars (or (apply pred cars) (loop cdrs))))))))
(define (every pred l . ls)
  (if (null? ls)
      (let loop ((l l))
        (cond ((null? l) #t)
              ((null? (cdr l)) (pred (car l)))
              (else (and (pred (car l)) (loop (cdr l))))))
      (let loop ((ls (cons l ls)) (last #t))
        (call-with-values (lambda () (%cars+cdrs ls))
          (lambda (cars cdrs)
            (if cars
                (let ((r (apply pred cars))) (and r (loop cdrs r)))
                last))))))
(define (list-index pred l . ls)
  (let loop ((ls (cons l ls)) (i 0))
    (call-with-values (lambda () (%cars+cdrs ls))
      (lambda (cars cdrs)
        (and cars (if (apply pred cars) i (loop cdrs (+ i 1))))))))
(define (delete x l . same)
  (if (null? same)
      ((@ (guile) delete) x l)
      (let ((= (car same))) (filter (lambda (y) (not (= x y))) l))))
(define (delete! x l . same) (apply delete x l same))
(define (member x l . same)
  (if (null? same)
      ((@ (guile) member) x l)
      (let ((= (car same)))
        (let loop ((l l))
          (cond ((null? l) #f) ((= x (car l)) l) (else (loop (cdr l))))))))
(define (assoc key alist . same)
  (if (null? same)
      ((@ (guile) assoc) key alist)
      (let ((= (car same)))
        (find (lambda (entry) (= key (car entry))) alist))))
(define (delete-duplicates l . same)
  (let ((= (if (pair? same) (car same) equal?)))
    (let loop ((l l) (out '()))
      (cond ((null? l) (reverse! out))
            ((any (lambda (y) (= y (car l))) out) (loop (cdr l) out))
            (else (loop (cdr l) (cons (car l) out)))))))
(define delete-duplicates! delete-duplicates)
(define (alist-cons key value alist) (cons (cons key value) alist))
(define (alist-copy alist)
  (map (lambda (entry) (cons (car entry) (cdr entry))) alist))
(define (alist-delete key alist . same)
  (let ((= (if (pair? same) (car same) equal?)))
    (remove (lambda (entry) (= key (car entry))) alist)))
(define alist-delete! alist-delete)
(define (list-copy l)
  (let loop ((l l) (out '()))
    (if (pair? l) (loop (cdr l) (cons (car l) out)) (append-reverse! out l))))
(define (zip l . ls) (apply map list l ls))
(define (unzip1 l) (map car l))
(define (unzip2 l) (values (map car l) (map cadr l)))
(define (unzip3 l) (values (map car l) (map cadr l) (map caddr l)))
(define (list= = . lists)
  (or (null? lists)
      (let outer ((a (car lists)) (rest (cdr lists)))
        (or (null? rest)
            (let ((b (car rest)))
              (and (let inner ((a a) (b b))
                     (cond ((null? a) (null? b))
                           ((null? b) #f)
                           (else (and (= (car a) (car b)) (inner (cdr a) (cdr b))))))
                   (outer b (cdr rest))))))))
(define (lset<= = . lists)
  (or (null? lists)
      (let loop ((a (car lists)) (rest (cdr lists)))
        (or (null? rest)
            (and (every (lambda (x) (%lset-member? = x (car rest))) a)
                 (loop (car rest) (cdr rest)))))))
(define (lset= = . lists)
  (or (null? lists)
      (let loop ((a (car lists)) (rest (cdr lists)))
        (or (null? rest)
            (and (every (lambda (x) (%lset-member? = x (car rest))) a)
                 (every (lambda (x) (%lset-member? = x a)) (car rest))
                 (loop (car rest) (cdr rest)))))))
(define (lset-adjoin = l . items)
  (fold (lambda (x acc) (if (%lset-member? = x acc) acc (cons x acc))) l items))
(define (lset-union = . lists)
  (if (null? lists)
      '()
      (fold (lambda (l acc)
              (cond ((null? l) acc)
                    ((null? acc) l)
                    ((eq? l acc) acc)
                    (else (fold (lambda (x acc) (if (%lset-member? = x acc) acc (cons x acc)))
                                acc l))))
            (car lists) (cdr lists))))
(define (lset-intersection = l . lists)
  (filter (lambda (x) (every (lambda (other) (%lset-member? = x other)) lists)) l))
(define (lset-difference = l . lists)
  (filter (lambda (x) (not (any (lambda (other) (%lset-member? = x other)) lists))) l))
(define (lset-xor = . lists)
  (reduce (lambda (b a)
            (call-with-values (lambda () (lset-diff+intersection = a b))
              (lambda (a-b a-and-b)
                (cond ((null? a-b) (lset-difference = b a))
                      ((null? a-and-b) (append b a))
                      (else (fold (lambda (x acc) (if (%lset-member? = x a-and-b) acc (cons x acc)))
                                  a-b b))))))
          '() lists))
(define lset-union! lset-union)
(define lset-intersection! lset-intersection)
(define lset-difference! lset-difference)
(define lset-xor! lset-xor)
(define (lset-diff+intersection = l . lists)
  (partition (lambda (x) (not (any (lambda (other) (%lset-member? = x other)) lists))) l))
(define lset-diff+intersection! lset-diff+intersection)
"#;
