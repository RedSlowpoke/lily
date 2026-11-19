//! S5's write barrier and undo log (03 §5.1 and §5.4, R10, R11; MS6 step
//! 2): what an item changes of the objects and globals older than it is
//! undone, every mutating primitive's change, the latest first; what it
//! allocated is not logged; its reads are recorded with their versions and
//! its writes in order; the barrier is off outside items; the log keeps
//! what it will give back through a collection; and a part's load
//! (`%undo-suspend!`, D74) stays.
//!
//! The redo half (MS6 step 3, 02 §5.7): items undone the latest first,
//! each keeping what it changed as it was at its end, are done again the
//! oldest first in new items, which are undone and kept again as theirs;
//! what is kept lives through collections until it is forgotten.

use lily_engine_scheme::{Value, Vm};

/// The value of `source`'s last form as `write` writes it, or the error.
fn run(vm: &mut Vm, source: &str) -> String {
    match vm.eval_text(source, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            out
        }
        Err(e) => format!("error: {}", e.text(vm.heap())),
    }
}

/// Objects of every mutable kind and globals, made before any item, and a
/// picture of all of them.
const OLD: &str = "
(define-module (other))
(define-public shared 'shared)
(define-module (guile-user))
(use-modules (other))
(define p (cons 'a 'b))
(define boxed (let ((n 0)) (lambda () (set! n (+ n 1)) n)))
(define v (vector 1 2 3))
(define s (string #\\a #\\b #\\c))
(define h (make-hash-table))
(hashq-set! h 'k 1)
(hashq-set! h 'gone 2)
(define w (make-weak-key-hash-table))
(define key (list 'key))
(hashq-set! w key 'held)
(define o (list 'o))
(set-object-property! o 'colour 'red)
(define f (make-fluid 'outer))
(define d (delay (list 'forced)))
(define a (make-array 0 2 2))
(define x 1)
(define al (list (cons 'k 1)))
(define (picture)
  (list p v s (hash-map->list cons h) (hashq-ref w key) (object-property o 'colour)
        (fluid-ref f) (promise? d) (array->list a) x (defined? 'y)
        (module-ref (resolve-module '(other)) 'shared)
        (module-ref (resolve-module '(other)) 'added #f) al))
";

/// What `OLD`'s item changes, through each mutating primitive.
const CHANGES: &str = "
(set-car! p 'changed)
(set-cdr! p (list 'new))
(boxed)
(vector-set! v 0 'one)
(vector-fill! v 9)
(string-set! s 0 #\\z)
(hashq-set! h 'k 'changed)
(hashq-set! h 'added 3)
(hashq-remove! h 'gone)
(hashq-set! w key 'changed)
(set-object-property! o 'colour 'blue)
(fluid-set! f 'inner)
(force d)
(array-set! a 'set 1 1)
(set! x 2)
(define y 'new)
(module-set! (resolve-module '(other)) 'shared 'changed)
(module-define! (resolve-module '(other)) 'added 'new)
(set! al (assq-set! al 'k 2))
(set-cdr! (assq 'k al) 3)
";

fn old() -> Vm {
    let mut vm = Vm::new();
    let ran = run(&mut vm, OLD);
    assert!(!ran.starts_with("error"), "{ran}");
    vm
}

#[test]
fn an_items_changes_are_undone_each_primitives() {
    let mut vm = old();
    let before = run(&mut vm, "(picture)");
    let mark = vm.begin_item();
    let ran = run(&mut vm, CHANGES);
    assert!(!ran.starts_with("error"), "{ran}");
    vm.end_item(mark);
    let changed = run(&mut vm, "(picture)");
    assert_ne!(changed, before);
    assert!(changed.contains("(changed new)"), "{changed}");
    vm.undo(mark);
    assert_eq!(run(&mut vm, "(picture)"), before);
    assert_eq!(vm.undo_len(), (0, 0));
    // The closure's boxed variable too, and `y` is no variable again.
    assert_eq!(run(&mut vm, "(boxed)"), "1");
    assert_eq!(run(&mut vm, "y"), "error: Unbound variable: y");
}

#[test]
fn what_an_item_allocates_is_not_logged() {
    let mut vm = old();
    let mark = vm.begin_item();
    // More than a chunk of each, so that the last is in a chunk the item
    // opened: an object in the free slot of an older chunk reports its
    // epoch, and its change is logged though it need not be.
    run(
        &mut vm,
        "(define pairs (make-list 300 0))
         (define vectors (map (lambda (i) (make-vector 1 0)) (iota 300)))
         (define tables (map (lambda (i) (make-hash-table)) (iota 300)))",
    );
    let logged = vm.undo_len().0;
    run(
        &mut vm,
        "(set-car! pairs 1) (vector-set! (car (last-pair vectors)) 0 1)
         (hashq-set! (car (last-pair tables)) 'a 1)",
    );
    assert_eq!(vm.undo_len().0, logged);
    // Each object older than the item is logged once, at its first change.
    run(
        &mut vm,
        "(set-car! p 1) (set-car! p 2) (set-cdr! p 3) (vector-set! v 0 1) (vector-set! v 1 1)",
    );
    assert_eq!(vm.undo_len().0, 2);
    vm.end_item(mark);
    vm.undo(mark);
    assert_eq!(run(&mut vm, "(list p v)"), "((a . b) #(1 2 3))");
}

#[test]
fn the_barrier_is_off_outside_items() {
    let mut vm = old();
    run(&mut vm, CHANGES);
    assert_eq!(vm.undo_len(), (0, 0));
    assert!(!vm.in_item());
    let mark = vm.begin_item();
    assert!(vm.in_item());
    vm.end_item(mark);
    run(&mut vm, "(set-car! p 'after) (set! x 3)");
    assert_eq!(vm.undo_len(), (0, 0));
    // Nothing logged, nothing undone.
    vm.undo(mark);
    assert_eq!(run(&mut vm, "(list (car p) x)"), "(after 3)");
}

#[test]
fn reads_and_writes_are_recorded_with_versions() {
    let mut vm = old();
    run(&mut vm, "(define r 10) (define (get) r)");
    let r = vm.slot("guile-user", "r").unwrap();
    let x = vm.slot("guile-user", "x").unwrap();
    let version = vm.slot_version(r);
    run(&mut vm, "(get)");
    let mark = vm.begin_item();
    run(&mut vm, "(get) (set! x (+ x r)) (define z 1)");
    let item = vm.end_item(mark);
    assert!(item.reads.contains(&(r, version)), "{:?}", item.reads);
    let z = vm.slot("guile-user", "z").unwrap();
    assert_eq!(item.writes, vec![x, z]);
    // Reads before the item and after it are not its.
    assert!(vm.take_reads().is_empty());
    // Undone, `x` has its value again under a version it never had.
    let set = vm.slot_version(x);
    vm.undo(mark);
    assert_eq!(vm.slot_value(x), Some(Value::fixnum(1).unwrap()));
    assert!(vm.slot_version(x) > set);
    assert_eq!(vm.slot_value(z), None);
}

#[test]
fn items_are_undone_latest_first() {
    let mut vm = old();
    let first = vm.begin_item();
    run(&mut vm, "(set! x 2) (define made (list 'first))");
    vm.end_item(first);
    let second = vm.begin_item();
    // `made` is older than the second item: its change is logged.
    run(
        &mut vm,
        "(set! x 3) (set-car! made 'second) (set-car! p 'second)",
    );
    vm.end_item(second);
    assert_eq!(run(&mut vm, "(list x made (car p))"), "(3 (second) second)");
    vm.undo(second);
    assert_eq!(run(&mut vm, "(list x made (car p))"), "(2 (first) a)");
    vm.undo(first);
    assert_eq!(
        run(&mut vm, "(list x (defined? 'made) (car p))"),
        "(1 #f a)"
    );
}

#[test]
fn the_log_keeps_what_it_gives_back() {
    let mut vm = old();
    run(&mut vm, "(define only (list (string-copy \"kept\")))");
    let mark = vm.begin_item();
    // The old car's only reference is the log's, and `o`'s too, whose
    // property lives in a weak table.
    run(&mut vm, "(set-car! only #f) (set! o #f) (set! key #f)");
    for _ in 0..3 {
        vm.collect();
    }
    run(&mut vm, "(make-list 100000 'garbage)");
    vm.collect();
    vm.end_item(mark);
    vm.undo(mark);
    assert_eq!(
        run(
            &mut vm,
            "(list only (object-property o 'colour) (hashq-ref w key))"
        ),
        "((\"kept\") red held)"
    );
}

#[test]
fn a_parts_load_is_not_undone() {
    let mut vm = old();
    let mark = vm.begin_item();
    run(
        &mut vm,
        "(set! x 2)
         (let ((running ((@@ (lily) %undo-suspend!))))
           (set-car! p 'loaded) (module-define! (current-module) 'part-defined 'part)
           ((@@ (lily) %undo-resume! ) running))
         (define after (list 'after))
         (set! v 'after)",
    );
    vm.end_item(mark);
    vm.undo(mark);
    assert_eq!(
        run(&mut vm, "(list x (car p) part-defined (defined? 'after) v)"),
        "(1 loaded part #f #(1 2 3))"
    );
}

/// Items undone, keeping their ends, then done again, the oldest first,
/// each in a new item of its own; the state each leaves is the first run's.
#[test]
fn items_undone_keeping_their_ends_are_done_again() {
    let mut vm = old();
    let before = run(&mut vm, "(picture)");
    let first = vm.begin_item();
    run(
        &mut vm,
        "(set! x 2) (define made (list 'first)) (set-car! p 'first)",
    );
    vm.end_item(first);
    let second = vm.begin_item();
    let ran = run(&mut vm, CHANGES);
    assert!(!ran.starts_with("error"), "{ran}");
    run(&mut vm, "(set-car! made 'second)");
    vm.end_item(second);
    let after_first = "(2 (first) first)";
    let after = run(&mut vm, "(picture)");
    let made = run(&mut vm, "made");
    let second_kept = vm.undo_keeping(second);
    assert_eq!(run(&mut vm, "(list x made (car p))"), after_first);
    let first_kept = vm.undo_keeping(first);
    assert_eq!(run(&mut vm, "(picture)"), before);
    // What is kept lives through collections.
    run(&mut vm, "(make-list 100000 'garbage)");
    vm.collect();
    // Done again, the oldest first, each in an item of its own.
    let again = vm.begin_item();
    vm.redo(&first_kept);
    vm.end_item(again);
    assert_eq!(run(&mut vm, "(list x made (car p))"), after_first);
    let again_second = vm.begin_item();
    vm.redo(&second_kept);
    vm.end_item(again_second);
    assert_eq!(run(&mut vm, "(picture)"), after);
    assert_eq!(run(&mut vm, "made"), made);
    vm.forget_redo();
    // The new items are undone as the first were, and kept again.
    let kept = vm.undo_keeping(again_second);
    vm.undo(again);
    assert_eq!(run(&mut vm, "(picture)"), before);
    let last = vm.begin_item();
    vm.redo(&kept);
    vm.end_item(last);
    vm.undo(last);
    assert_eq!(run(&mut vm, "(picture)"), before);
    assert_eq!(vm.undo_len(), (0, 0));
}

/// A variable Rust sets and gives back itself (the parser's `%parser`) is
/// defined unlogged: an undo leaves what it was set to last, not a value an
/// item replaced, which may have gone since (MS6 step 3).
#[test]
fn an_unlogged_define_is_not_undone() {
    let mut vm = old();
    let module = vm.eval_text("(current-module)", None).expect("a module");
    let mark = vm.begin_item();
    let seven = vm.heap_mut().integer(7);
    assert!(vm.module_define_unlogged(module, "x", seven));
    run(&mut vm, "(set-car! p 'changed)");
    vm.end_item(mark);
    vm.undo(mark);
    // The pair is given back; `x` keeps what it was set to, unlogged.
    assert_eq!(run(&mut vm, "(list x (car p))"), "(7 a)");
}
