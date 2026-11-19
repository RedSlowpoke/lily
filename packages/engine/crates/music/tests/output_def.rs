//! An output definition's parent (MS3 step 14a): `Output_def::lookup_variable`
//! through the parents (`output-def.cc:109–120`) in `ly:output-def-lookup`,
//! `find_context_def` and `ly:output-def-parent`; `scale_output_def` with
//! `paper.scm`'s `scale-layout` in Rust (D110); a book's parts given their
//! parent as `ly:book-add-bookpart!` adds them (`book.cc:131–176`), the
//! scaled paper of a `Paper_book` (`paper-book.cc:61–67`, whose
//! `Book::process` is layout's), and the scaled layout a score is translated with
//! (`score.cc:112–142`); and `Output_def::normalize`
//! (`output-def.cc:134–243`, MS3 step 32, D129), which `Book::process`
//! calls on a `Paper_book`'s paper. In a VM with only the music crate.
use lily_engine_music::{
    Book, ContextDef, OutputDef, host_lookup_variable, make_context_def, normalize, paper_book,
    root, scale_output_def, scaled_layout, set_parent,
};
use lily_engine_scheme::{Value, Vm};

/// A VM with the music crate's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

fn show(vm: &mut Vm, text: &str) -> String {
    let v = eval(vm, text);
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

/// Each of `calls` and what it gives, written.
fn check(vm: &mut Vm, calls: &[(&str, &str)]) {
    for &(call, want) in calls {
        assert_eq!(show(vm, call), want, "{call}");
    }
}

/// `(lily)`'s binding of `name`.
fn global(vm: &mut Vm, name: &str) -> Value {
    eval(vm, name)
}

/// Defines `name` in `(lily)` as `value`.
fn define(vm: &mut Vm, name: &str, value: Value) {
    vm.define(name, value);
}

/// Output definitions `child`, `mid` and `top`, each the parent of the one
/// before, made by `ly:make-output-def` and defined in `(lily)`.
fn chain(vm: &mut Vm) -> [Value; 3] {
    eval(
        vm,
        "(define-public child (ly:make-output-def))
         (define-public mid (ly:make-output-def))
         (define-public top (ly:make-output-def))
         (ly:output-def-set-variable! child 'own 1)
         (ly:output-def-set-variable! mid 'own 2)
         (ly:output-def-set-variable! mid 'middle 'm)
         (ly:output-def-set-variable! top 'middle 't)
         (ly:output-def-set-variable! top 'deep 'd)",
    );
    let [child, mid, top] = ["child", "mid", "top"].map(|name| global(vm, name));
    set_parent(vm.heap_mut(), child, mid);
    set_parent(vm.heap_mut(), mid, top);
    [child, mid, top]
}

#[test]
fn a_lookup_reads_the_scope_then_each_parent_up_to_the_root() {
    let mut vm = vm();
    let [child, mid, top] = chain(&mut vm);
    check(
        &mut vm,
        &[
            // The definition's own first, then its parent's, then the root's.
            ("(ly:output-def-lookup child 'own)", "1"),
            ("(ly:output-def-lookup child 'middle)", "m"),
            ("(ly:output-def-lookup child 'deep)", "d"),
            ("(ly:output-def-lookup mid 'middle)", "m"),
            ("(ly:output-def-lookup top 'own 'none)", "none"),
            ("(ly:output-def-lookup child 'nowhere)", "()"),
            ("(ly:output-def-lookup child 'nowhere 0)", "0"),
            // What the scope imports, before the parent: `(lily)`'s.
            ("(define-public deep 'lily)", "#<unspecified>"),
            ("(ly:output-def-lookup child 'deep)", "lily"),
            // Its parent, else the default, else `()`.
            ("(eq? (ly:output-def-parent child) mid)", "#t"),
            ("(eq? (ly:output-def-parent mid 'x) top)", "#t"),
            ("(ly:output-def-parent top)", "()"),
            ("(ly:output-def-parent top 'x)", "x"),
            // A clone has none (`Output_def`'s copy constructor).
            ("(ly:output-def-parent (ly:output-def-clone child))", "()"),
            (
                "(ly:output-def-lookup (ly:output-def-clone child) 'middle)",
                "()",
            ),
            // The scope is the definition's own.
            ("(module-ref (ly:output-def-scope child) 'middle #f)", "#f"),
        ],
    );
    let middle = vm.heap_mut().intern("middle");
    let m = vm.heap_mut().intern("m");
    assert_eq!(host_lookup_variable(&mut vm, child, middle), Some(m));
    let nowhere = vm.heap_mut().intern("nowhere");
    assert_eq!(host_lookup_variable(&mut vm, child, nowhere), None);
    assert_eq!(root(vm.heap(), child), top);
    assert_eq!(root(vm.heap(), mid), top);
    assert_eq!(root(vm.heap(), top), top);
}

#[test]
fn a_context_definition_is_found_in_the_parents_too() {
    let mut vm = vm();
    let [child, mid, _] = chain(&mut vm);
    let mods = eval(&mut vm, "'((context-name Staff))");
    let items = vec![vm.heap().car(mods).unwrap()];
    let (def, unknown) = make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
    assert_eq!(unknown, 0);
    let scope = vm.heap().foreign::<OutputDef>(mid).unwrap().scope;
    vm.module_define(scope, "Staff", def);
    // `find_context_def` of a translation (`context.rs`' `find_def`) reads
    // through the Host's lookup.
    let staff = vm.heap_mut().intern("Staff");
    assert_eq!(host_lookup_variable(&mut vm, child, staff), Some(def));
    assert!(vm.heap().foreign::<ContextDef>(def).is_some());
    check(
        &mut vm,
        &[
            (
                "(ly:context-def? (ly:output-def-lookup child 'Staff))",
                "#t",
            ),
            // `ly:output-find-context-def` lists the scope's own, as
            // LilyPond's `ly_module_2_alist` of it.
            ("(ly:output-find-context-def child)", "()"),
            ("(length (ly:output-find-context-def mid))", "1"),
        ],
    );
}

#[test]
fn scaling_divides_the_dimension_variables_its_scope_binds_to_numbers() {
    let mut vm = vm();
    eval(
        &mut vm,
        "(define-public unit 3)
         (define-public paper (ly:make-output-def))
         (define-public layout (ly:make-output-def))
         (ly:output-def-set-variable! paper 'dimension-variables
           '(mm in line-width unit paper-width staff-space nowhere))
         (ly:output-def-set-variable! paper 'line-width 100)
         (ly:output-def-set-variable! layout 'mm 1)
         (ly:output-def-set-variable! layout 'in 25.4)
         (ly:output-def-set-variable! layout 'paper-width \"wide\")
         (ly:output-def-set-variable! layout 'staff-space 3/2)
         (ly:output-def-set-variable! layout 'other 7)",
    );
    let [paper, layout] = ["paper", "layout"].map(|name| global(&mut vm, name));
    set_parent(vm.heap_mut(), layout, paper);
    let scaled = scale_output_def(&mut vm, layout, 2.0);
    define(&mut vm, "scaled", scaled);
    check(
        &mut vm,
        &[
            // `dimension-variables` through the parent; each number the
            // scope binds, divided by the scale with `/`.
            ("(ly:output-def-lookup scaled 'mm)", "0.5"),
            ("(ly:output-def-lookup scaled 'in)", "12.7"),
            ("(ly:output-def-lookup scaled 'staff-space)", "0.75"),
            // What `(lily)` binds, which `module-variable` finds, is
            // defined in the clone.
            ("(ly:output-def-lookup scaled 'unit)", "1.5"),
            ("(module-ref (ly:output-def-scope scaled) 'unit #f)", "1.5"),
            // Not a number: copied as it is.
            ("(ly:output-def-lookup scaled 'paper-width)", "\"wide\""),
            // The parent's own is not the scope's: not scaled, not found.
            ("(ly:output-def-lookup scaled 'line-width)", "()"),
            ("(ly:output-def-lookup scaled 'other)", "7"),
            ("(ly:output-def-lookup scaled 'nowhere 'none)", "none"),
            ("(ly:output-def-lookup scaled 'cloned)", "#t"),
            ("(ly:output-def-parent scaled)", "()"),
            // The definition scaled is unchanged.
            ("(ly:output-def-lookup layout 'mm)", "1"),
            ("(ly:output-def-lookup layout 'cloned 'no)", "no"),
            ("(eq? (ly:output-def-parent layout) paper)", "#t"),
        ],
    );
    // No `dimension-variables`: a clone, marked.
    let bare = eval(&mut vm, "(ly:make-output-def)");
    define(&mut vm, "bare", bare);
    let scaled = scale_output_def(&mut vm, bare, 4.0);
    define(&mut vm, "bare-scaled", scaled);
    check(
        &mut vm,
        &[("(ly:output-def-lookup bare-scaled 'cloned)", "#t")],
    );
}

/// A paper as `$defaultpaper` has its variables: its kind, `output-scale`,
/// `mm` and `dimension-variables`.
fn paper(vm: &mut Vm, name: &str, scale: &str) -> Value {
    eval(
        vm,
        &format!(
            "(define-public {name} (ly:make-output-def))
             (ly:output-def-set-variable! {name} 'output-def-kind 'paper)
             (ly:output-def-set-variable! {name} 'output-scale {scale})
             (ly:output-def-set-variable! {name} 'mm 1)
             (ly:output-def-set-variable! {name} 'dimension-variables '(mm))"
        ),
    );
    global(vm, name)
}

#[test]
fn a_part_is_given_its_book_as_parent_and_the_scores_before_it_a_part_of_their_own() {
    let mut vm = vm();
    let book_paper = paper(&mut vm, "book-paper", "2.0");
    let part_paper = paper(&mut vm, "part-paper", "4");
    // Two headers, modules as the parser makes them.
    let header = vm.make_module(&["lily"]).unwrap();
    define(&mut vm, "book-header", header);
    let part_header = vm.make_module(&["lily"]).unwrap();
    define(&mut vm, "part-header", part_header);
    for (module, name, text) in [
        (header, "title", "T"),
        (header, "composer", "C"),
        (part_header, "title", "P"),
    ] {
        let text = vm.heap_mut().string(text);
        vm.module_define(module, name, text);
    }
    let book = vm.heap_mut().wrap(Book::new(book_paper, header));
    define(&mut vm, "book", book);
    let plain = vm.heap_mut().wrap(Book::new(Value::FALSE, Value::FALSE));
    define(&mut vm, "plain-part", plain);
    let own = vm.heap_mut().wrap(Book::new(part_paper, part_header));
    define(&mut vm, "own-part", own);
    eval(
        &mut vm,
        "(ly:book-add-score! book 's1)
         (ly:book-add-score! book 's2)
         (ly:book-add-bookpart! book plain-part)
         (ly:book-add-score! book 's3)
         (ly:book-add-bookpart! book own-part)
         (ly:book-add-score! book 's4)",
    );
    check(
        &mut vm,
        &[
            // `add_scores_to_bookpart` at each `add_bookpart`: the scores
            // before a part in a part of their own, the parts in order;
            // the scores after the last part stay the book's until
            // `Book::process`.
            ("(length (ly:book-book-parts book))", "4"),
            (
                "(ly:book-scores (car (ly:book-book-parts book)))",
                "(s1 s2)",
            ),
            ("(eq? (cadr (ly:book-book-parts book)) plain-part)", "#t"),
            ("(ly:book-scores (caddr (ly:book-book-parts book)))", "(s3)"),
            ("(eq? (cadddr (ly:book-book-parts book)) own-part)", "#t"),
            ("(ly:book-scores book)", "(s4)"),
        ],
    );
    // `Book::set_parent`: a part of no paper is given a new one, and a
    // part's paper its book's as parent; its header a copy of the book's
    // with its own over it.
    let parts: Vec<Value> = {
        let b = vm.heap().foreign::<Book>(book).unwrap();
        let mut at = b.parts;
        let mut parts = Vec::new();
        while let (Some(p), Some(next)) = (vm.heap().car(at), vm.heap().cdr(at)) {
            parts.push(p);
            at = next;
        }
        parts
    };
    for part in parts {
        let paper = vm.heap().foreign::<Book>(part).unwrap().paper;
        let paper = vm.heap().foreign::<OutputDef>(paper).expect("a paper");
        assert_eq!(paper.parent, Some(book_paper));
    }
    assert_eq!(vm.heap().foreign::<Book>(own).unwrap().paper, part_paper);
    let made = vm.heap().foreign::<Book>(plain).unwrap().header;
    define(&mut vm, "plain-header", made);
    let mixed = vm.heap().foreign::<Book>(own).unwrap().header;
    define(&mut vm, "own-header", mixed);
    check(
        &mut vm,
        &[
            ("(module-ref plain-header 'title)", "\"T\""),
            ("(module-ref own-header 'title)", "\"P\""),
            ("(module-ref own-header 'composer)", "\"C\""),
            ("(eq? own-header part-header)", "#f"),
            ("(module-ref part-header 'composer #f)", "#f"),
        ],
    );
}

#[test]
fn a_layout_is_scaled_by_its_papers_output_scale_and_parented_to_it() {
    let mut vm = vm();
    let book_paper = paper(&mut vm, "book-paper", "2.0");
    let printed = paper_book(&mut vm, book_paper, None).unwrap();
    define(&mut vm, "printed", printed);
    eval(
        &mut vm,
        "(define-public layout (ly:make-output-def))
         (ly:output-def-set-variable! layout 'output-def-kind 'layout)
         (ly:output-def-set-variable! layout 'mm 1)
         (ly:output-def-set-variable! layout 'dimension-variables '(mm))",
    );
    let layout = global(&mut vm, "layout");
    let scaled = scaled_layout(&mut vm, layout, printed).unwrap();
    define(&mut vm, "scaled", scaled);
    // Of a layoutbook that is no paper, the scale is 1.
    let unscaled = scaled_layout(&mut vm, layout, layout).unwrap();
    define(&mut vm, "unscaled", unscaled);
    check(
        &mut vm,
        &[
            ("(ly:output-def-lookup scaled 'mm)", "0.5"),
            ("(ly:output-def-lookup scaled 'output-def-kind)", "layout"),
            ("(ly:output-def-lookup scaled 'output-scale)", "2.0"),
            ("(eq? (ly:output-def-parent scaled) printed)", "#t"),
            ("(ly:output-def-lookup unscaled 'mm)", "1.0"),
            ("(eq? (ly:output-def-parent unscaled) layout)", "#t"),
        ],
    );
    // `from_scm<double>` of an `output-scale` that is no real.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! book-paper 'output-scale 'big)",
    );
    let e = paper_book(&mut vm, book_paper, None).unwrap_err();
    assert_eq!(e.text(vm.heap()), "Wrong type (expecting real number): big");
}

#[test]
fn an_image_holds_no_output_definition_with_a_parent() {
    let mut vm = vm();
    chain(&mut vm);
    assert!(
        vm.write_image().is_err(),
        "a parented definition fails the write"
    );
    eval(
        &mut vm,
        "(set! child (ly:output-def-clone child))
         (set! mid #f)",
    );
    assert!(vm.write_image().is_ok(), "one with none is written");
}

/// The `left-margin`, `right-margin` and `line-width` [`normalize`] sets,
/// reals,
/// on a paper 100 wide, of default margins 10 and 5 (outer 12 and inner 8
/// two-sided), with the variables `set` given, and the warnings it gives.
fn normalized(vm: &mut Vm, set: &str) -> (String, String) {
    eval(
        vm,
        &format!(
            "(define-public p (ly:make-output-def))
             (for-each (lambda (e) (ly:output-def-set-variable! p (car e) (cdr e)))
                       '((paper-width . 100) (left-margin-default-scaled . 10)
                         (right-margin-default-scaled . 5) (outer-margin-default-scaled . 12)
                         (inner-margin-default-scaled . 8) (check-consistency . #t)
                         {set}))
             (set-car! %diagnostics '())"
        ),
    );
    let p = global(vm, "p");
    normalize(vm, p).unwrap();
    let set = show(
        vm,
        "(map (lambda (k) (module-ref (ly:output-def-scope p) k #f)) '(left-margin right-margin line-width))",
    );
    let said = show(vm, "(map (lambda (v) (vector-ref v 2)) (car %diagnostics))");
    (set, said)
}

#[test]
fn normalize_sets_the_margins_and_the_line_width_from_the_paper_width() {
    let mut vm = vm();
    let none = String::from("()");
    let cases = [
        // The default margins; given margins; a line width alone centred,
        // with one margin the other from it.
        ("", "(10.0 5.0 85.0)"),
        ("(left-margin . 20) (right-margin . 30)", "(20.0 30.0 50.0)"),
        ("(line-width . 60)", "(20.0 20.0 60.0)"),
        ("(line-width . 60) (right-margin . 10)", "(30.0 10.0 60.0)"),
        ("(line-width . 60) (left-margin . 10)", "(10.0 30.0 60.0)"),
        // Two-sided: the outer and inner margins, the binding offset added
        // to the inner one.
        ("(two-sided . #t) (binding-offset . 2)", "(12.0 10.0 78.0)"),
        ("(two-sided . #t) (binding-offset . x)", "(12.0 8.0 80.0)"),
    ];
    for (set, want) in cases {
        assert_eq!(
            normalized(&mut vm, set),
            (want.into(), none.clone()),
            "{set}"
        );
    }
    // Margins that do not add up, or a negative one: the defaults, with
    // LilyPond's warnings; not checked without check-consistency.
    assert_eq!(
        normalized(
            &mut vm,
            "(line-width . 60) (left-margin . 10) (right-margin . 10)"
        ),
        (
            "(10.0 5.0 85.0)".into(),
            "(\"margins do not fit with line-width, setting default values\")".into()
        )
    );
    assert_eq!(
        normalized(&mut vm, "(line-width . 120)"),
        (
            "(10.0 5.0 85.0)".into(),
            "(\"systems run off the page due to improper paper settings, setting default values\")"
                .into()
        )
    );
    assert_eq!(
        normalized(&mut vm, "(line-width . 120) (check-consistency . #f)").0,
        "(-10.0 -10.0 120.0)"
    );
    // No paper width: LilyPond's programming error, nothing set.
    eval(
        &mut vm,
        "(define-public bare (ly:make-output-def)) (set-car! %diagnostics '())",
    );
    let bare = global(&mut vm, "bare");
    normalize(&mut vm, bare).unwrap();
    assert_eq!(
        show(
            &mut vm,
            "(module-ref (ly:output-def-scope bare) 'line-width #f)"
        ),
        "#f"
    );
    assert_eq!(
        show(
            &mut vm,
            "(map (lambda (v) (vector-ref v 2)) (car %diagnostics))"
        ),
        "(\"called normalize () on paper with missing settings\")"
    );
}
