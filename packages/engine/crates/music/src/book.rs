//! LilyPond's `Score` and `Book` (`score.cc`, `book.cc` and their Scheme
//! faces), as far as the parser and `declarations-init.ly`'s book handlers
//! need them (MS2 step 16): a score's music, header and output definitions
//! (step 23), and a book's paper, header, scores and parts. The parser makes them (`\score`, `\book`,
//! `\bookpart`), `scorify-music` makes a score with `ly:make-score`, and
//! the handlers collect them. Their output definitions and processing are
//! layout's (`Book::process` and `Paper_book`, `paper_book.rs`), but for
//! the scaled paper of a `Paper_book` ([`paper_book`]) and the scaled
//! layout a score translates with ([`scaled_layout`]), MS3 step 14a; and
//! `ly:book-add-bookpart!`'s parts, given their parent as the book adds
//! them, the scores before each in a part of their own (MS3 step 35).

use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Thrown, Tracer, Value, Vm};

use crate::R;
use crate::music::Music;
use crate::output_def::{
    OutputDef, host_lookup_variable, real_variable, scale_output_def,
    set_parent as set_output_parent,
};

/// A score: its music (`'()` until it has some), its header, a module or
/// `#f`, its output definitions in the order they were added (`defs_`),
/// and whether its music had errors (`error_found_`).
pub struct Score {
    pub music: Value,
    pub header: Value,
    pub defs: Vec<Value>,
    pub error: bool,
}

impl Score {
    /// A score with no music and the header `header`.
    pub fn new(header: Value) -> Score {
        Score {
            music: Value::NULL,
            header,
            defs: Vec::new(),
            error: false,
        }
    }
}

impl Foreign for Score {
    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(Score {
            music: self.music,
            header: self.header,
            defs: self.defs.clone(),
            error: self.error,
        }))
    }

    fn type_name(&self) -> &'static str {
        "Score"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.music);
        tracer.trace(self.header);
        for &d in &self.defs {
            tracer.trace(d);
        }
    }
}

/// A book or a book's part: its `\paper`, an output definition, or `#f`
/// for a part that has none; its header, a module or `#f`; its scores and
/// its parts, the last added first.
pub struct Book {
    pub paper: Value,
    pub header: Value,
    pub scores: Value,
    pub parts: Value,
}

impl Book {
    pub fn new(paper: Value, header: Value) -> Book {
        Book {
            paper,
            header,
            scores: Value::NULL,
            parts: Value::NULL,
        }
    }
}

impl Foreign for Book {
    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(Book {
            paper: self.paper,
            header: self.header,
            scores: self.scores,
            parts: self.parts,
        }))
    }

    fn type_name(&self) -> &'static str {
        "Book"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.paper);
        tracer.trace(self.header);
        tracer.trace(self.scores);
        tracer.trace(self.parts);
    }
}

/// `Book::set_parent` (`book.cc:131–148`): `part`'s paper, or a new
/// output definition if it has none, its parent `book`'s paper; and its
/// header a new module with `book`'s variables and then its own, if
/// `book` has a header.
pub fn set_parent(cx: &mut dyn Books, part: Value, book: Value) {
    let Some(above) = cx.heap().foreign::<Book>(book).map(|b| (b.paper, b.header)) else {
        return;
    };
    let Some((paper, header)) = cx.heap().foreign::<Book>(part).map(|b| (b.paper, b.header)) else {
        return;
    };
    let paper = if cx.heap().foreign::<OutputDef>(paper).is_some() {
        paper
    } else {
        let scope = cx.make_module(&["lily"]).unwrap_or(Value::FALSE);
        cx.heap_mut().wrap(OutputDef::new(scope))
    };
    // A null `parent->paper_` is none.
    let parent = cx.heap().foreign::<OutputDef>(above.0).map(|_| above.0);
    if let Some(def) = cx.heap_mut().foreign_mut::<OutputDef>(paper) {
        def.parent = parent;
    }
    let mut header = header;
    if is_module(cx, above.1) {
        let copy = cx.make_module(&["lily"]).unwrap_or(Value::FALSE);
        cx.module_copy(copy, above.1);
        if is_module(cx, header) {
            cx.module_copy(copy, header);
        }
        header = copy;
    }
    if let Some(b) = cx.heap_mut().foreign_mut::<Book>(part) {
        b.paper = paper;
        b.header = header;
    }
}

/// `Book::add_scores_to_bookpart` (`book.cc:151–168`): the scores added
/// to `book` so far, if any, moved into a new part of it, whose parent
/// it is.
pub fn add_scores_to_bookpart(cx: &mut dyn Books, book: Value) {
    let Some(scores) = cx.heap().foreign::<Book>(book).map(|b| b.scores) else {
        return;
    };
    if cx.heap().car(scores).is_none() {
        return;
    }
    let mut part = Book::new(Value::FALSE, Value::FALSE);
    part.scores = scores;
    let part = cx.heap_mut().wrap(part);
    set_parent(cx, part, book);
    let parts = cx.heap().foreign::<Book>(book).expect("a book").parts;
    let parts = cx.heap_mut().cons(part, parts);
    let b = cx.heap_mut().foreign_mut::<Book>(book).expect("a book");
    b.parts = parts;
    b.scores = Value::NULL;
}

/// `Book::add_bookpart` (`book.cc:170–176`): the scores added to `book`
/// so far in a part of their own, then `part`, whose parent `book` is.
pub fn add_bookpart(cx: &mut dyn Books, book: Value, part: Value) {
    add_scores_to_bookpart(cx, book);
    set_parent(cx, part, book);
    if let Some(parts) = cx.heap().foreign::<Book>(book).map(|b| b.parts) {
        let parts = cx.heap_mut().cons(part, parts);
        cx.heap_mut()
            .foreign_mut::<Book>(book)
            .expect("a book")
            .parts = parts;
    }
}

/// Whether `v` is a module (`ly_is_module`).
fn is_module(cx: &dyn Books, v: Value) -> bool {
    cx.heap().foreign_object(v).map(|f| f.type_name()) == Some("module")
}

/// What a book's parts need of the interpreter: a primitive's (`Cx`), as
/// `ly:book-add-bookpart!` adds a part, or the VM's, as `Book::process`
/// adds the scores after the last part.
pub trait Books {
    fn heap(&self) -> &Heap;
    fn heap_mut(&mut self) -> &mut Heap;
    fn make_module(&mut self, uses: &[&str]) -> Option<Value>;
    fn module_copy(&mut self, to: Value, from: Value) -> bool;
}

impl Books for Cx {
    fn heap(&self) -> &Heap {
        Cx::heap(self)
    }
    fn heap_mut(&mut self) -> &mut Heap {
        Cx::heap_mut(self)
    }
    fn make_module(&mut self, uses: &[&str]) -> Option<Value> {
        Cx::make_module(self, uses)
    }
    fn module_copy(&mut self, to: Value, from: Value) -> bool {
        Cx::module_copy(self, to, from)
    }
}

impl Books for Vm {
    fn heap(&self) -> &Heap {
        Vm::heap(self)
    }
    fn heap_mut(&mut self) -> &mut Heap {
        Vm::heap_mut(self)
    }
    fn make_module(&mut self, uses: &[&str]) -> Option<Value> {
        Vm::make_module(self, uses)
    }
    fn module_copy(&mut self, to: Value, from: Value) -> bool {
        Vm::module_copy(self, to, from)
    }
}

/// The book `init.ly` makes of the file's toplevel parts and scores after
/// the file (`init.ly:66–86`, D110), for `toplevel-book-handler`: a book of
/// `paper` and `header` (`ly:make-book`) with the parts `parts`, in the
/// file's order, then the scores `scores` after the last part; or, with
/// no parts, of the scores; or none with neither.
pub fn toplevel_book(
    vm: &mut Vm,
    paper: Value,
    header: Value,
    parts: &[Value],
    scores: &[Value],
) -> Option<Value> {
    if parts.is_empty() && scores.is_empty() {
        return None;
    }
    let header = if is_module(vm, header) {
        header
    } else {
        Value::FALSE
    };
    let book = vm.heap_mut().wrap(Book::new(paper, header));
    for &part in parts {
        add_bookpart(vm, book, part);
    }
    let mut list = Value::NULL;
    for &score in scores {
        list = vm.heap_mut().cons(score, list);
    }
    if let Some(b) = vm.heap_mut().foreign_mut::<Book>(book) {
        b.scores = list;
    }
    Some(book)
}

/// The paper of `Paper_book`'s constructor (`paper-book.cc:61–67`):
/// `paper` scaled by its own `output-scale`, its parent `parent`, the
/// paper of the book's `Paper_book`, for a part.
pub fn paper_book(vm: &mut Vm, paper: Value, parent: Option<Value>) -> Result<Value, Exception> {
    let scale = real_variable(vm, paper, "output-scale")?;
    let scaled = scale_output_def(vm, paper, scale);
    if let Some(parent) = parent {
        set_output_parent(vm.heap_mut(), scaled, parent);
    }
    Ok(scaled)
}

/// The output definition `Score::book_rendering` translates a `\layout`
/// `def` with (`score.cc:112–142`): `def` scaled by `layoutbook`'s
/// `output-scale` if it is a paper, else by 1, its parent `layoutbook`.
pub fn scaled_layout(vm: &mut Vm, def: Value, layoutbook: Value) -> Result<Value, Exception> {
    let kind = vm.heap_mut().intern("output-def-kind");
    let paper = vm.heap_mut().intern("paper");
    let scale = if host_lookup_variable(vm, layoutbook, kind) == Some(paper) {
        real_variable(vm, layoutbook, "output-scale")?
    } else {
        1.0
    };
    let scaled = scale_output_def(vm, def, scale);
    set_output_parent(vm.heap_mut(), scaled, layoutbook);
    Ok(scaled)
}

/// The output definition `ly:score-embedded-format` translates a
/// `\layout` `def` with (`score-scheme.cc:127–133`): a clone of `def` if
/// it is scaled already (`cloned`), else `def` scaled by the
/// `output-scale` of `od`, the markup's layout; its parent `od`.
pub fn embedded_layout(vm: &mut Vm, def: Value, od: Value) -> Result<Value, Exception> {
    let cloned = vm.heap_mut().intern("cloned");
    let scaled = if host_lookup_variable(vm, def, cloned) == Some(Value::TRUE) {
        let old = vm
            .heap()
            .foreign::<OutputDef>(def)
            .map_or(Value::FALSE, |d| d.scope);
        let scope = vm.make_module(&["lily"]).unwrap_or(Value::FALSE);
        vm.module_copy(scope, old);
        vm.heap_mut().wrap(OutputDef::new(scope))
    } else {
        let scale = real_variable(vm, od, "output-scale")?;
        scale_output_def(vm, def, scale)
    };
    set_output_parent(vm.heap_mut(), scaled, od);
    Ok(scaled)
}

/// `LY_ASSERT_SMOB (T, v, at)`.
fn check<T: Foreign>(
    cx: &mut Cx,
    subr: &str,
    at: usize,
    v: Value,
    what: &str,
) -> Result<(), Thrown> {
    match cx.heap().foreign::<T>(v) {
        Some(_) => Ok(()),
        None => Err(cx.wrong_type(subr, at, v, what)),
    }
}

/// `(ly:make-score music)`.
pub(crate) fn ly_make_score(cx: &mut Cx, args: &[Value]) -> R {
    check::<Music>(cx, "ly:make-score", 1, args[0], "Music")?;
    let mut score = Score::new(Value::FALSE);
    score.music = args[0];
    Ok(cx.heap_mut().wrap(score))
}

/// `(ly:score? x)`.
pub(crate) fn is_score(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Score>(cx, args[0])
}

/// The score `v`, checked for `subr`.
fn score<'c>(cx: &'c mut Cx, subr: &str, v: Value) -> Result<&'c mut Score, Thrown> {
    check::<Score>(cx, subr, 1, v, "Score")?;
    Ok(cx.heap_mut().foreign_mut::<Score>(v).expect("checked"))
}

/// `(ly:score-music score)`.
pub(crate) fn ly_score_music(cx: &mut Cx, args: &[Value]) -> R {
    Ok(score(cx, "ly:score-music", args[0])?.music)
}

/// `(ly:score-header score)`.
pub(crate) fn ly_score_header(cx: &mut Cx, args: &[Value]) -> R {
    Ok(score(cx, "ly:score-header", args[0])?.header)
}

/// `(ly:score-set-header! score module)`.
pub(crate) fn ly_score_set_header(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:score-set-header!";
    check::<Score>(cx, SUBR, 1, args[0], "Score")?;
    if cx.heap().foreign_object(args[1]).map(|f| f.type_name()) != Some("module") {
        return Err(cx.wrong_type(SUBR, 2, args[1], "module"));
    }
    score(cx, SUBR, args[0])?.header = args[1];
    Ok(Value::UNSPECIFIED)
}

/// `(ly:score-error? score)`.
pub(crate) fn ly_score_error(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(score(cx, "ly:score-error?", args[0])?.error))
}

/// `(ly:score-output-defs score)`: a new list of them.
pub(crate) fn ly_score_output_defs(cx: &mut Cx, args: &[Value]) -> R {
    let defs = score(cx, "ly:score-output-defs", args[0])?.defs.clone();
    Ok(cx.heap_mut().list(&defs))
}

/// `(ly:score-add-output-def! score def)`.
pub(crate) fn ly_score_add_output_def(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:score-add-output-def!";
    check::<Score>(cx, SUBR, 1, args[0], "Score")?;
    check::<crate::OutputDef>(cx, SUBR, 2, args[1], "Output_def")?;
    score(cx, SUBR, args[0])?.defs.push(args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:book? x)`.
pub(crate) fn is_book(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Book>(cx, args[0])
}

/// The book `v`, checked for `subr` as its argument `at`.
fn book<'c>(cx: &'c mut Cx, subr: &str, at: usize, v: Value) -> Result<&'c mut Book, Thrown> {
    check::<Book>(cx, subr, at, v, "Book")?;
    Ok(cx.heap_mut().foreign_mut::<Book>(v).expect("checked"))
}

/// `(ly:book-add-score! book score)`: a score or a text, added.
pub(crate) fn ly_book_add_score(cx: &mut Cx, args: &[Value]) -> R {
    let scores = book(cx, "ly:book-add-score!", 1, args[0])?.scores;
    let scores = cx.heap_mut().cons(args[1], scores);
    book(cx, "ly:book-add-score!", 1, args[0])?.scores = scores;
    Ok(Value::UNSPECIFIED)
}

/// `(ly:book-add-bookpart! book part)`: [`add_bookpart`].
pub(crate) fn ly_book_add_bookpart(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:book-add-bookpart!";
    check::<Book>(cx, SUBR, 1, args[0], "Book")?;
    check::<Book>(cx, SUBR, 2, args[1], "Book")?;
    add_bookpart(cx, args[0], args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:book-scores book)`: its scores, in the order they were added.
pub(crate) fn ly_book_scores(cx: &mut Cx, args: &[Value]) -> R {
    let scores = book(cx, "ly:book-scores", 1, args[0])?.scores;
    Ok(reverse(cx, scores))
}

/// `(ly:book-book-parts book)`: its parts, in the order they were added.
pub(crate) fn ly_book_book_parts(cx: &mut Cx, args: &[Value]) -> R {
    let parts = book(cx, "ly:book-book-parts", 1, args[0])?.parts;
    Ok(reverse(cx, parts))
}

/// `(ly:make-book-part scores)`: a part with no paper and no header whose
/// scores are a copy of `scores` (`ly_append`), which holds the last added
/// first, as `collect-scores-for-book`'s `toplevel-scores` does.
pub(crate) fn ly_make_book_part(cx: &mut Cx, args: &[Value]) -> R {
    let mut part = Book::new(Value::FALSE, Value::FALSE);
    let reversed = reverse(cx, args[0]);
    part.scores = reverse(cx, reversed);
    Ok(cx.heap_mut().wrap(part))
}

fn reverse(cx: &mut Cx, mut list: Value) -> Value {
    let mut out = Value::NULL;
    while let (Some(item), Some(next)) = (cx.heap().car(list), cx.heap().cdr(list)) {
        out = cx.heap_mut().cons(item, out);
        list = next;
    }
    out
}
