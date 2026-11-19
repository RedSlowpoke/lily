//! S1's values and heap (chapter 03 §5.1): the word's encoding, symbols,
//! strings, foreign objects, epochs, the collector and its roots, `equal?`,
//! and Guile 3's printer. The printed forms are Guile 3.0.11's; with
//! `guile` installed, each case is also printed by guile and compared.
use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;

use lily_engine_scheme::{CHUNK, Foreign, Heap, Tag, Tracer, Value};

#[test]
fn a_value_is_one_word() {
    assert_eq!(size_of::<Value>(), 8);
    for n in [0, 1, -1, 42, Value::FIXNUM_MAX, Value::FIXNUM_MIN] {
        let v = Value::fixnum(n).unwrap();
        assert_eq!((v.tag(), v.as_fixnum()), (Tag::Fixnum, Some(n)));
    }
    assert_eq!(
        Value::FIXNUM_MAX,
        2305843009213693951,
        "Guile's most-positive-fixnum"
    );
    assert_eq!(Value::fixnum(Value::FIXNUM_MAX + 1), None);
    assert_eq!(Value::fixnum(Value::FIXNUM_MIN - 1), None);
    for c in ['\0', 'a', 'λ', '😀', '\u{10ffff}'] {
        assert_eq!(
            (Value::char(c).tag(), Value::char(c).as_char()),
            (Tag::Char, Some(c))
        );
    }
    let immediates = [
        Value::NULL,
        Value::FALSE,
        Value::TRUE,
        Value::UNSPECIFIED,
        Value::UNDEFINED,
        Value::EOF,
        Value::ERROR,
    ];
    for (i, a) in immediates.iter().enumerate() {
        assert!(!a.is_heap() && a.as_fixnum().is_none() && a.as_char().is_none());
        assert!(immediates[i + 1..].iter().all(|b| a != b));
    }
    assert!(
        Value::FALSE.is_false() && !Value::NULL.is_false() && !Value::fixnum(0).unwrap().is_false()
    );
    assert_eq!(Value::bool(true), Value::TRUE);
}

#[test]
fn symbols_and_keywords_are_interned() {
    let mut heap = Heap::new();
    let symbol = heap.intern("foo");
    assert_eq!(symbol, heap.intern("foo"));
    assert_ne!(symbol, heap.intern("bar"));
    let key = heap.keyword("foo");
    assert_eq!((key.tag(), symbol.tag()), (Tag::Keyword, Tag::Symbol));
    assert_ne!(key, symbol);
    assert_eq!(heap.name(key), Some("foo"));
    let many: Vec<Value> = (0..5000).map(|i| heap.intern(&format!("s{i}"))).collect();
    for (i, &symbol) in many.iter().enumerate() {
        assert_eq!(heap.name(symbol), Some(format!("s{i}").as_str()));
        assert_eq!(heap.intern(&format!("s{i}")), symbol);
    }
    assert_eq!(heap.name(Value::NULL), None);
}

#[test]
fn pairs_and_vectors() {
    let mut heap = Heap::new();
    let [one, two] = [1, 2].map(|n| Value::fixnum(n).unwrap());
    let pair = heap.cons(one, two);
    assert_eq!((heap.car(pair), heap.cdr(pair)), (Some(one), Some(two)));
    assert!(heap.set_car(pair, two) && heap.set_cdr(pair, Value::NULL));
    assert_eq!(heap.car(pair), Some(two));
    assert!(!heap.set_car(one, two));
    assert_eq!(heap.car(Value::NULL), None);
    let vector = heap.vector(vec![one, pair]);
    assert_eq!(heap.vector_items(vector), Some(&[one, pair][..]));
    heap.vector_items_mut(vector).unwrap()[0] = two;
    assert_eq!(heap.vector_items(vector).unwrap()[0], two);
    assert_eq!(heap.vector_items(pair), None);
}

#[test]
fn strings_index_characters() {
    let mut heap = Heap::new();
    let ascii = heap.string("hello");
    let s = heap.str(ascii).unwrap();
    assert!(s.is_ascii());
    assert_eq!((s.len(), s.char_at(4), s.char_at(5)), (5, Some('o'), None));

    // Past the index's stride, with characters of every width.
    let text: String = (0..200)
        .map(|i| ['a', 'é', 'λ', '♩', '😀'][i % 5])
        .collect();
    let chars: Vec<char> = text.chars().collect();
    let string = heap.string(text.as_str());
    let s = heap.str(string).unwrap();
    assert!(!s.is_ascii());
    assert_eq!(s.len(), 200);
    for (k, &c) in chars.iter().enumerate() {
        assert_eq!(s.char_at(k), Some(c), "character {k}");
        assert_eq!(
            s.byte_offset(k),
            Some(text.char_indices().nth(k).unwrap().0)
        );
    }
    assert_eq!(
        (s.char_at(200), s.byte_offset(200), s.byte_offset(201)),
        (None, Some(text.len()), None)
    );

    // A change of width moves the bytes after it; one of the same width does not.
    let s = heap.str_mut(string).unwrap();
    assert!(s.set_char(1, 'e') && s.set_char(3, '♫') && s.set_char(150, 'Z'));
    assert!(!s.set_char(200, 'x'));
    let mut expected = chars.clone();
    (expected[1], expected[3], expected[150]) = ('e', '♫', 'Z');
    for (k, &c) in expected.iter().enumerate() {
        assert_eq!(s.char_at(k), Some(c), "character {k} after set_char");
    }
    assert_eq!(s.as_str(), expected.iter().collect::<String>());

    // An ASCII string that is given a wider character is no longer ASCII.
    let s = heap.str_mut(ascii).unwrap();
    assert!(s.set_char(0, 'ħ'));
    assert!(!s.is_ascii());
    assert_eq!((s.as_str(), s.char_at(1)), ("ħello", Some('e')));
}

/// A foreign object for the tests: counts its drops, holds a value.
struct Probe {
    value: Value,
    dropped: Rc<Cell<usize>>,
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

impl Foreign for Probe {
    fn type_name(&self) -> &'static str {
        "Probe"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Probe ");
        heap.write(self.value, out);
        out.push('>');
    }

    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        (other as &dyn std::any::Any)
            .downcast_ref::<Probe>()
            .is_some_and(|other| heap.equal(self.value, other.value))
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.value);
    }
}

struct Other;

impl Foreign for Other {
    fn type_name(&self) -> &'static str {
        "Other"
    }
}

#[test]
fn foreign_objects() {
    let mut heap = Heap::new();
    let dropped = Rc::new(Cell::new(0));
    let held = heap.string("held");
    let probe = heap.wrap(Probe {
        value: held,
        dropped: dropped.clone(),
    });
    let other = heap.wrap(Other);
    assert_eq!(probe.tag(), Tag::Foreign);
    assert!(heap.foreign::<Probe>(probe).is_some() && heap.foreign::<Other>(probe).is_none());
    assert!(heap.foreign::<Probe>(other).is_none() && heap.foreign::<Probe>(held).is_none());
    assert_eq!(heap.foreign_object(other).unwrap().type_name(), "Other");

    let mut out = String::new();
    let list = heap.list(&[probe, other]);
    heap.write(list, &mut out);
    assert_eq!(out, r#"(#<Probe "held"> #<Other>)"#);

    // equal? asks the object; eq? objects are equal? without asking.
    let same = heap.string("held");
    let twin = heap.wrap(Probe {
        value: same,
        dropped: dropped.clone(),
    });
    assert!(heap.equal(probe, twin) && !heap.equal(probe, other) && heap.equal(other, other));
    let another = heap.wrap(Other);
    assert!(!heap.equal(other, another));
    heap.foreign_mut::<Probe>(twin).unwrap().value = Value::NULL;
    assert!(!heap.equal(probe, twin));

    // What the object traces lives as long as it does; the heap drops it.
    let root = heap.root(probe);
    heap.collect(&[]);
    assert_eq!(dropped.get(), 1, "the twin was dropped");
    assert_eq!(heap.str(held).map(|s| s.as_str()), Some("held"));
    assert_eq!(heap.rooted(root), probe);
    heap.unroot_to(0);
    heap.collect(&[]);
    assert_eq!((dropped.get(), heap.objects()), (2, 0));
}

#[test]
fn the_collector_frees_what_no_root_reaches() {
    let mut heap = Heap::new();
    let first = heap.cons(Value::NULL, Value::NULL);
    for _ in 0..999 {
        heap.cons(Value::NULL, Value::NULL);
    }
    let (a, b) = (
        heap.cons(Value::NULL, Value::NULL),
        heap.cons(Value::NULL, Value::NULL),
    );
    heap.set_cdr(a, b);
    heap.set_cdr(b, a);
    heap.collect(&[]);
    assert_eq!(
        (heap.objects(), heap.stats().freed, heap.stats().collections),
        (0, 1002, 1)
    );
    // Freed slots are used again, the lowest first.
    assert_eq!(heap.cons(Value::NULL, Value::NULL), first);

    // The root stack, a cell updated as a list grows, and the caller's roots.
    let mut heap = Heap::new();
    let depth = heap.root_depth();
    let list = heap.root(Value::NULL);
    for n in 0..10_000 {
        let item = heap.cons(Value::fixnum(n).unwrap(), heap.rooted(list));
        heap.set_rooted(list, item);
        heap.string(format!("garbage {n}"));
    }
    let names = [heap.string("kept"), heap.vector(vec![Value::TRUE])];
    heap.collect(&[&names]);
    assert_eq!((heap.objects(), heap.stats().live), (10_002, 10_002));
    let (mut sum, mut at) = (0, heap.rooted(list));
    while let Some(n) = heap.car(at) {
        sum += n.as_fixnum().unwrap();
        at = heap.cdr(at).unwrap();
    }
    assert_eq!(sum, (0..10_000).sum::<i64>());
    assert_eq!(heap.str(names[0]).unwrap().as_str(), "kept");
    heap.unroot_to(depth);
    heap.collect(&[]);
    assert_eq!(heap.objects(), 0);
}

#[test]
fn safe_points_collect_when_enough_was_allocated() {
    let mut heap = Heap::new();
    let string = heap.string("kept");
    let kept = heap.root(string);
    heap.collect(&[]);
    assert!(!heap.safe_point(&[]));
    // One pair short of 4 MiB in pairs, then 4 MiB.
    for _ in 0..(4 << 20) / 16 - 1 {
        heap.cons(Value::NULL, Value::NULL);
    }
    assert!(!heap.safe_point(&[]));
    heap.cons(Value::NULL, Value::NULL);
    assert!(heap.safe_point(&[]));
    assert_eq!(
        (
            heap.objects(),
            heap.str(heap.rooted(kept)).unwrap().as_str()
        ),
        (1, "kept")
    );
    assert!(!heap.safe_point(&[]));
}

#[test]
fn chunks_carry_epochs() {
    let mut heap = Heap::new();
    let pair = || (Value::NULL, Value::NULL);
    // Epoch 0: two chunks of pairs, of which the first object is kept.
    let kept: Vec<Value> = (0..2 * CHUNK)
        .map(|_| heap.cons(pair().0, pair().1))
        .collect();
    heap.root(kept[0]);
    assert_eq!(heap.begin_epoch(), 1);
    assert_eq!(
        (
            heap.epoch(),
            heap.epoch_of(kept[0]),
            heap.epoch_of(Value::NULL)
        ),
        (1, Some(0), None)
    );
    heap.collect(&[]);

    // The empty second chunk is opened again, in epoch 1.
    let fresh: Vec<Value> = (0..CHUNK).map(|_| heap.cons(pair().0, pair().1)).collect();
    assert!(fresh.iter().all(|&v| heap.epoch_of(v) == Some(1)));
    assert_eq!(fresh[0], kept[CHUNK]);
    // Then the free slots of the first chunk: since MS6 step 3 they report
    // the epoch they were put in, 1, not the chunk's (D235); the kept
    // object still reports 0.
    let reused: Vec<Value> = (1..CHUNK).map(|_| heap.cons(pair().0, pair().1)).collect();
    assert_eq!(reused[0], kept[1]);
    assert!(reused.iter().all(|&v| heap.epoch_of(v) == Some(1)));
    assert_eq!(heap.epoch_of(kept[0]), Some(0));
    // Then a new chunk, in epoch 1.
    let new = heap.cons(pair().0, pair().1);
    assert_eq!(heap.epoch_of(new), Some(1));

    // Other arenas have their own chunks.
    let string = heap.string("s");
    let vector = heap.vector(vec![]);
    let foreign = heap.wrap(Other);
    assert_eq!(
        [string, vector, foreign].map(|v| heap.epoch_of(v)),
        [Some(1); 3]
    );
}

#[test]
fn deep_structures_use_no_native_stack() {
    // 64 KiB of stack, where recursion over a million pairs would overflow.
    std::thread::Builder::new()
        .stack_size(64 << 10)
        .spawn(|| {
            let mut heap = Heap::new();
            let (mut list, mut tree) = (Value::NULL, Value::NULL);
            for n in 0..1_000_000 {
                list = heap.cons(Value::fixnum(n).unwrap(), list);
            }
            for _ in 0..200_000 {
                tree = heap.cons(tree, Value::NULL);
            }
            let mut twin = Value::NULL;
            for _ in 0..200_000 {
                twin = heap.cons(twin, Value::NULL);
            }
            heap.collect(&[&[list, tree, twin]]);
            assert_eq!(heap.objects(), 1_400_000);
            assert!(heap.equal(tree, twin) && heap.equal(list, list));
            heap.set_car(twin, Value::TRUE);
            assert!(!heap.equal(tree, twin));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn equal_compares_contents() {
    let mut heap = Heap::new();
    let n = |k| Value::fixnum(k).unwrap();
    let a = heap.string("abc");
    let b = heap.string("abc");
    let c = heap.string("abd");
    assert!(heap.equal(a, b) && !heap.equal(a, c) && !heap.eqv(a, b));
    let x = heap.vector(vec![n(1), a]);
    let y = heap.vector(vec![n(1), b]);
    let z = heap.vector(vec![n(1)]);
    let l1 = heap.list(&[x, Value::char('q'), n(3)]);
    let l2 = heap.list(&[y, Value::char('q'), n(3)]);
    let l3 = heap.list(&[y, Value::char('q')]);
    assert!(heap.equal(l1, l2) && !heap.equal(l1, l3) && !heap.equal(x, z));
    let dotted = heap.cons(n(1), n(2));
    let other = heap.cons(n(1), n(3));
    assert!(!heap.equal(dotted, other) && !heap.equal(a, x) && !heap.equal(n(1), Value::char('1')));
    let (s, t) = (heap.intern("s"), heap.intern("s"));
    assert!(heap.equal(s, t));
}

// --- Guile 3's printer ---------------------------------------------------------

/// Guile's output for `(write expr)` or `(display expr)`, if guile is installed.
fn guile(display: bool, expr: &str) -> Option<String> {
    let program = format!("({} {expr})", if display { "display" } else { "write" });
    let out = Command::new("guile")
        .args(["--no-auto-compile", "-c", &program])
        .output()
        .ok()?;
    assert!(
        out.status.success(),
        "guile failed on {expr}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8(out.stdout).unwrap())
}

struct Case {
    /// The same value as a Guile expression.
    expr: &'static str,
    build: fn(&mut Heap) -> Value,
    display: bool,
    /// What Guile 3.0.11 prints.
    printed: &'static str,
}

fn symbols(heap: &mut Heap, names: &[&str]) -> Value {
    let symbols: Vec<Value> = names.iter().map(|name| heap.intern(name)).collect();
    heap.list(&symbols)
}

const SYMBOLS: [&str; 36] = [
    "abc", "a b", "", "1", "#foo", "a|b", ".", "+a", "-", "a(b", "#:x", "x;y", "x\"y", "a\\b",
    "a}b", "x:", ":x", "1+", "+5", "+.", "-.5e", "+inf.0", "-i", "+1/2", ".5", "..", "-1x", "a#",
    "a\tb", "'a", "@a", "é", "+2i", "-1.5e3", "...", "a,b",
];

const CASES: [Case; 7] = [
    Case {
        expr: r"(list #\nul #\x1 #\alarm #\tab #\newline #\space #\a #\( #\delete #\x80 #\xa0 #\é #\λ #\x200b #\xe000 #\xfeff #\xfffe #\x10ffff #\x1f600 #\x3000)",
        build: |heap| {
            let chars = "\0\x01\x07\t\n a(\x7f\u{80}\u{a0}éλ\u{200b}\u{e000}\u{feff}\u{fffe}\u{10ffff}😀\u{3000}";
            let chars: Vec<Value> = chars.chars().map(Value::char).collect();
            heap.list(&chars)
        },
        display: false,
        printed: r"(#\nul #\soh #\alarm #\tab #\newline #\space #\a #\( #\delete #\200 #\240 #\é #\λ #\20013 #\160000 #\177377 #\177776 #\4177777 #\😀 #\30000)",
    },
    Case {
        expr: r"(string #\x22 #\\ #\nul #\alarm #\backspace #\tab #\newline #\vtab #\page #\return #\esc #\space #\delete #\x80 #\xa0 #\é #\x2028 #\xe000 #\x10ffff #\λ #\x1f600)",
        build: |heap| {
            heap.string(
                "\"\\\0\x07\x08\t\n\x0b\x0c\r\x1b \x7f\u{80}\u{a0}é\u{2028}\u{e000}\u{10ffff}λ😀",
            )
        },
        display: false,
        printed: r#""\"\\\x00\a\b\t\n\v\f\r\x1b \x7f\x80\xa0é\u2028\ue000\U10ffffλ😀""#,
    },
    Case {
        expr: r##"(map string->symbol '("abc" "a b" "" "1" "#foo" "a|b" "." "+a" "-" "a(b" "#:x" "x;y" "x\"y" "a\\b" "a}b" "x:" ":x" "1+" "+5" "+." "-.5e" "+inf.0" "-i" "+1/2" ".5" ".." "-1x" "a#" "a\tb" "'a" "@a" "é" "+2i" "-1.5e3" "..." "a,b"))"##,
        build: |heap| symbols(heap, &SYMBOLS),
        display: false,
        printed: r#"(abc #{a b}# #{}# #{1}# #{#foo}# a|b #{.}# +a - #{a\x28;b}# #{#:x}# #{x;y}# #{x"y}# a\b #{a\x7d;b}# x: :x #{1+}# #{+5}# +. -.5e #{+inf.0}# #{-i}# #{+1/2}# #{.5}# .. -1x #{a#}# #{a\x9;b}# #{'a}# @a é #{+2i}# #{-1.5e3}# ... a,b)"#,
    },
    Case {
        expr: r#"(list #:foo (symbol->keyword (string->symbol "a b")) (symbol->keyword (string->symbol "1")))"#,
        build: |heap| {
            let keywords = [heap.keyword("foo"), heap.keyword("a b"), heap.keyword("1")];
            heap.list(&keywords)
        },
        display: false,
        printed: "(#:foo #:#{a b}# #:#{1}#)",
    },
    Case {
        expr: r#"(list 1 -2 (cons 1 2) (list 1 (cons 2 3)) (vector) (vector 1 "a" #\b) ''x '() #t #f (if #f #f) most-positive-fixnum most-negative-fixnum (read (open-input-string "")))"#,
        build: |heap| {
            let n = |k| Value::fixnum(k).unwrap();
            let dotted = heap.cons(n(1), n(2));
            let inner = heap.cons(n(2), n(3));
            let nested = heap.list(&[n(1), inner]);
            let empty = heap.vector(vec![]);
            let a = heap.string("a");
            let vector = heap.vector(vec![n(1), a, Value::char('b')]);
            let quote = heap.intern("quote");
            let x = heap.intern("x");
            let quoted = heap.list(&[quote, x]);
            let max = n(Value::FIXNUM_MAX);
            let min = n(Value::FIXNUM_MIN);
            heap.list(&[
                n(1),
                n(-2),
                dotted,
                nested,
                empty,
                vector,
                quoted,
                Value::NULL,
                Value::TRUE,
                Value::FALSE,
                Value::UNSPECIFIED,
                max,
                min,
                Value::EOF,
            ])
        },
        display: false,
        printed: r#"(1 -2 (1 . 2) (1 (2 . 3)) #() #(1 "a" #\b) (quote x) () #t #f #<unspecified> 2305843009213693951 -2305843009213693952 #<eof>)"#,
    },
    Case {
        expr: r#"(list "a\nb" #\x #:foo (string->symbol "a b") (vector "s" #\c) (string->symbol ""))"#,
        build: |heap| {
            let s = heap.string("a\nb");
            let keyword = heap.keyword("foo");
            let ab = heap.intern("a b");
            let inner = heap.string("s");
            let vector = heap.vector(vec![inner, Value::char('c')]);
            let empty = heap.intern("");
            heap.list(&[s, Value::char('x'), keyword, ab, vector, empty])
        },
        display: true,
        printed: "(a\nb x #:foo #{a b}# #(s c) #{}#)",
    },
    Case {
        expr: r#"(cons "x" (cons #\y "z"))"#,
        build: |heap| {
            let x = heap.string("x");
            let z = heap.string("z");
            let tail = heap.cons(Value::char('y'), z);
            heap.cons(x, tail)
        },
        display: true,
        printed: "(x y . z)",
    },
];

#[test]
fn values_print_as_guile_prints_them() {
    let mut heap = Heap::new();
    let mut compared = 0;
    for case in &CASES {
        let value = (case.build)(&mut heap);
        let mut out = String::new();
        if case.display {
            heap.display(value, &mut out)
        } else {
            heap.write(value, &mut out)
        }
        assert_eq!(out, case.printed, "{}", case.expr);
        if let Some(printed) = guile(case.display, case.expr) {
            assert_eq!(printed, case.printed, "guile on {}", case.expr);
            compared += 1;
        }
    }
    if compared == 0 {
        eprintln!("guile is not installed: the printed forms were not compared with it");
    }
}
