//! `(lily)`'s own definitions (chapter 03 §5.4): what `lily.scm` defines
//! before it loads LilyPond's Scheme files and what those files call of the
//! C++ at load time, so that `lily-library.scm` and `music-functions.scm`
//! load from the installed LilyPond's source.
//!
//! In Rust, LilyPond's `ly:assoc-get`, `ly:chain-assoc-get`, `ly:version`,
//! `ly:dir?`, `ly:dimension?`, `ly:error` and the lengths
//! (`ly:pt` and the rest of [`UNITS`]), with their errors as LilyPond
//! 2.26.0 throws them; and the type predicates of LilyPond's C++ types that the init
//! data names (`ly:grob?`, `ly:context?` and the rest of [`NEVER`]), false of
//! every value until the engine has the type. In Scheme ([`PRELUDE`]),
//! `lily.scm`'s own `format`, `simple-format` where a score's is `(ice-9
//! format)`'s, `is-absolute?`, `ly-getcwd` (empty: the host's names start
//! at its root), `ly:string-substitute`,
//! `ly:string-percent-encode` and `ly:parser-output-name`, the name the
//! compile gives (MS2 step 26, `timing.ly`'s), its `%location` and
//! `*location*`, `_i`, `G_`, and its session variables: `define-session`,
//! `define-session-public` and `call-after-session`, which record what a
//! later session restores, and the session's start, save and end; the
//! options' defaults (`ly:get-option`, and `ly:set-option`) and
//! `ly:debug`; and the tables the
//! init data fills at load:
//! `ly:add-interface`'s grob interfaces and
//! `ly:register-translator`'s translators. Stencils, their heads and
//! `ly:make-stencil` are layout's (`lily_engine_layout`, MS3 step 2). The
//! regular expressions are `regex.rs`'.

use alloc::boxed::Box;
use alloc::string::String;

use crate::ports;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown};

type R = Result<Value, Thrown>;

/// LilyPond's version, as `ly:version` gives it.
const VERSION: [i64; 3] = [2, 26, 0];

/// What the host has of a file it is asked for by name (D74): its text,
/// none, or what it has not said yet, for which the run stops and waits
/// ([`crate::Vm::waiting`]) until the host is asked.
pub enum Hosted<T> {
    Text(T),
    Missing,
    Ask,
}

/// The host's source of a part's files: what it has of the file named
/// ([`crate::Vm::set_part_source`]).
pub type PartSource = Box<dyn FnMut(&str) -> Hosted<String>>;

/// The host's source of the files `\include` reads, for the parses Scheme
/// runs, a template's (D92; [`crate::Vm::set_file_source`]).
pub type FileSource = Box<dyn FnMut(&str) -> Hosted<alloc::vec::Vec<u8>>>;

/// `(%part-source file)`: the text of a part's file from the host, or `#f`.
/// A file the host must be asked for cancels the run, past every `catch`,
/// and leaves the VM waiting.
pub(crate) fn part_source(cx: &mut Cx, args: &[Value]) -> R {
    match hosted_text(cx, "%part-source", 1, args[0])? {
        Some(text) => Ok(text),
        None => Err(wait(cx)),
    }
}

/// `(%part-sources files)`: [`part_source`]'s text of each name of `files`,
/// and `#f` of a `#f`, in order. Every file the host must be asked for is
/// asked for before the run is cancelled, so that a part's files are asked
/// for at one stop (MS2 step 30).
pub(crate) fn part_sources(cx: &mut Cx, args: &[Value]) -> R {
    let mut texts = alloc::vec::Vec::new();
    let mut asked = false;
    let mut list = args[0];
    while let (Some(file), Some(rest)) = (cx.heap.car(list), cx.heap.cdr(list)) {
        if file == Value::FALSE {
            texts.push(Value::FALSE);
        } else {
            let text = hosted_text(cx, "%part-sources", 1, file)?;
            asked |= text.is_none();
            texts.push(text.unwrap_or(Value::FALSE));
        }
        list = rest;
    }
    if list != Value::NULL {
        return Err(cx.wrong_type("%part-sources", 1, args[0], "list"));
    }
    if asked {
        return Err(wait(cx));
    }
    Ok(cx.heap.list(&texts))
}

/// What the host has of the file `name` names: its text, `#f` when it has
/// none, or None when it must be asked for it.
fn hosted_text(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    name: Value,
) -> Result<Option<Value>, Thrown> {
    let Some(name) = cx.heap.str(name).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type(subr, position, name, "string"));
    };
    Ok(match cx.part_source.as_mut().map(|source| source(&name)) {
        Some(Hosted::Text(text)) => Some(cx.heap.string(&text)),
        Some(Hosted::Ask) => None,
        Some(Hosted::Missing) | None => Some(Value::FALSE),
    })
}

/// `(%undo-suspend!)`: R11's logging stopped, in the heap and the slots,
/// until `%undo-resume!`, so that a part a stub loads stays loaded when
/// the item that loaded it is undone, as the image's parts are (D74); true
/// if an item was running.
pub(crate) fn undo_suspend(cx: &mut Cx, _: &[Value]) -> R {
    let running = cx.heap.suspend_undo();
    cx.globals.logging = false;
    Ok(Value::bool(running))
}

/// `(%undo-resume! running)`: logging again after `%undo-suspend!` gave
/// `running`, in a new epoch.
pub(crate) fn undo_resume(cx: &mut Cx, args: &[Value]) -> R {
    if !args[0].is_false() {
        cx.heap.resume_undo(true);
        cx.globals.begin_item();
    }
    Ok(Value::UNSPECIFIED)
}

/// What a host does for `%host-call` ([`crate::Vm::set_host`]): it is
/// given the whole VM, which it may run again, and the call's arguments.
pub type HostFn = fn(&mut crate::Vm, &[Value]) -> Result<Value, Exception>;

/// A crate's own [`HostFn`] for the `%host-call`s whose `what` is the
/// symbol `what` (D127), which [`crate::Vm::add_host_handlers`] or
/// [`crate::Extension::hosts`] give a VM: so another crate's callbacks
/// reach its Rust that needs the whole VM, which a primitive does not get
/// (D70).
#[derive(Clone, Copy)]
pub struct HostHandler {
    pub what: &'static str,
    pub f: HostFn,
}

/// `(%host-call what arg …)`: the handler for `what`, else the host's
/// [`HostFn`], called with the VM and the arguments, once this primitive
/// has returned to the loop, which then takes its value as the call's
/// (MS2 step 33, D92, D127). So the parser can parse inside a running
/// procedure, as LilyPond's `ly:parse-string-expression` does. An error
/// without either.
pub(crate) fn host_call(cx: &mut Cx, args: &[Value]) -> R {
    let what = args.first().and_then(|&a| cx.heap.name(a));
    let handler = what.and_then(|w| cx.handlers.iter().find(|h| h.what == w));
    let Some(f) = handler.map(|h| h.f).or(cx.host) else {
        return Err(cx.misc_error(Some("%host-call"), "no host to call", &[]));
    };
    cx.hosting = Some(f);
    Ok(Value::UNSPECIFIED)
}

/// Cancels the run, past every `catch`, and leaves the VM waiting for the
/// host.
fn wait(cx: &mut Cx) -> Thrown {
    cx.waiting = true;
    cx.cancelled = true;
    Thrown
}

/// A music type's display method before the `display` part loads (OPEN
/// C6, D263): what LilyPond's `display-lily.scm`, loaded with
/// `music-functions.scm`, puts in each type's `display-methods`, which a
/// written music object prints. It prints as LilyPond's compiled method
/// does, `#<procedure method (note)>`, and a call of it calls `call`, which
/// loads the part and calls the method of the type at its index.
pub struct DisplayStub {
    name: Value,
    param: Value,
    music_type: Value,
    index: Value,
    call: Value,
}

impl crate::heap::Foreign for DisplayStub {
    fn type_name(&self) -> &'static str {
        "display-stub"
    }

    fn print(&self, heap: &crate::heap::Heap, out: &mut String) {
        out.push_str("#<procedure ");
        heap.display(self.name, out);
        out.push_str(" (");
        heap.display(self.param, out);
        out.push_str(")>");
    }

    fn trace(&self, tracer: &mut crate::heap::Tracer<'_>) {
        for v in [
            self.name,
            self.param,
            self.music_type,
            self.index,
            self.call,
        ] {
            tracer.trace(v);
        }
    }

    fn save(&self, out: &mut crate::image::ImageWriter) -> bool {
        out.values(&[
            self.name,
            self.param,
            self.music_type,
            self.index,
            self.call,
        ]);
        true
    }

    fn applicable(&self) -> Option<Value> {
        Some(self.call)
    }
}

pub(crate) fn load_display_stub(
    r: &mut crate::image::ImageReader<'_>,
) -> Option<Box<dyn crate::heap::Foreign>> {
    let [name, param, music_type, index, call] = r.values()?[..] else {
        return None;
    };
    Some(Box::new(DisplayStub {
        name,
        param,
        music_type,
        index,
        call,
    }))
}

/// `(%named-formals setting)`: which procedures the compiler keeps the
/// formals of to print from now on (`Globals::named`): `#f` none,
/// `toplevel` those a toplevel form defines, `#t` every named one; the
/// setting before.
pub(crate) fn named_formals(cx: &mut Cx, args: &[Value]) -> R {
    use crate::module::{NAMED_ALL, NAMED_NONE, NAMED_TOPLEVEL};
    let new = match args[0] {
        Value::FALSE => NAMED_NONE,
        v if cx.heap.name(v) == Some("toplevel") => NAMED_TOPLEVEL,
        _ => NAMED_ALL,
    };
    let old = core::mem::replace(&mut cx.globals.named, new);
    Ok(match old {
        NAMED_NONE => Value::FALSE,
        NAMED_TOPLEVEL => cx.heap.intern("toplevel"),
        _ => Value::TRUE,
    })
}

/// `(%make-display-stub name param type index call)`.
pub(crate) fn make_display_stub(cx: &mut Cx, args: &[Value]) -> R {
    Ok(cx.heap.wrap(DisplayStub {
        name: args[0],
        param: args[1],
        music_type: args[2],
        index: args[3],
        call: args[4],
    }))
}

/// `(%display-stub-target stub)`: `(type . index)`, or `#f` for no stub.
pub(crate) fn display_stub_target(cx: &mut Cx, args: &[Value]) -> R {
    let Some((t, i)) = cx
        .heap
        .foreign::<DisplayStub>(args[0])
        .map(|s| (s.music_type, s.index))
    else {
        return Ok(Value::FALSE);
    };
    Ok(cx.heap.cons(t, i))
}

/// `(ly:version)`: `(2 26 0)`, a new list each time.
pub(crate) fn version(cx: &mut Cx, _: &[Value]) -> R {
    let items = VERSION.map(|n| cx.heap.integer(n));
    Ok(cx.heap.list(&items))
}

/// The entry of `alist` whose key is `equal?` to `key`, as Guile's
/// `scm_assoc` finds it: an item that is not a pair, or an improper end,
/// is its error, which Guile 3 throws from `assq`.
fn assoc(cx: &mut Cx, key: Value, alist: Value) -> Result<Option<Value>, Thrown> {
    let mut at = alist;
    while let (Some(entry), Some(next)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        match cx.heap.car(entry) {
            Some(k) if cx.heap.equal(k, key) => return Ok(cx.heap.cdr(entry)),
            Some(_) => at = next,
            None => break,
        }
    }
    if at == Value::NULL {
        return Ok(None);
    }
    Err(cx.wrong_type("assq", 2, alist, "association list"))
}

/// `(ly:assoc-get key alist [default [strict-checking]])`: the value of
/// `key` in `alist`, else `default` or `#f`; with `strict-checking`, a
/// missing key is LilyPond's programming error, on the error port.
pub(crate) fn assoc_get(cx: &mut Cx, args: &[Value]) -> R {
    let (key, alist) = (args[0], args[1]);
    if !matches!(alist.tag(), Tag::Pair | Tag::Null) {
        return Err(cx.wrong_type("ly:assoc-get", 2, alist, "list"));
    }
    if let Some(value) = assoc(cx, key, alist)? {
        return Ok(value);
    }
    let default = args.get(2).copied().unwrap_or(Value::FALSE);
    if args.get(3).is_some_and(|&strict| strict != Value::FALSE) {
        let mut text = String::from("programming error: Cannot find key `");
        cx.heap.write(key, &mut text);
        text.push_str("' in alist, setting to `");
        cx.heap.write(default, &mut text);
        text.push_str("'.\ncontinuing, cross fingers\n");
        let port = ports::current_error(cx);
        ports::emit(cx, port, &text);
    }
    Ok(default)
}

/// `(ly:chain-assoc-get key chain [default [strict-checking]])`: the value
/// of `key` in the first alist of `chain` that has it, else `default` or
/// `#f`; a `chain` that is not a list is an empty one, as in LilyPond.
pub(crate) fn chain_assoc_get(cx: &mut Cx, args: &[Value]) -> R {
    let mut at = args[1];
    while let (Some(alist), Some(next)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        if let Some(value) = assoc(cx, args[0], alist)? {
            return Ok(value);
        }
        at = next;
    }
    Ok(args.get(2).copied().unwrap_or(Value::FALSE))
}

/// `(ly:dir? x)`: whether `x` is a direction, an exact -1, 0 or 1.
pub(crate) fn is_dir(_: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        args[0].as_fixnum().is_some_and(|n| (-1..=1).contains(&n)),
    ))
}

/// `(ly:dimension? x)`: whether `x` is a number, complex ones included.
pub(crate) fn is_dimension(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap.is_number(args[0])))
}

/// LilyPond's lengths in its internal unit, the millimetre, computed as
/// `dimensions.hh` computes them: a unit's points times `PT_TO_MM`.
const INCH_TO_PT: f64 = 72.270;
const CM_TO_PT: f64 = INCH_TO_PT / 2.54;
const MM_TO_PT: f64 = CM_TO_PT / 10.0;
const PT_TO_MM: f64 = 1.0 / MM_TO_PT;

/// `ly:pt`, `ly:mm`, `ly:cm`, `ly:inch` and `ly:bp`, each with its unit in
/// millimetres.
pub(crate) const UNITS: [(&str, f64); 5] = [
    ("ly:pt", PT_TO_MM),
    ("ly:mm", MM_TO_PT * PT_TO_MM),
    ("ly:cm", CM_TO_PT * PT_TO_MM),
    ("ly:inch", INCH_TO_PT * PT_TO_MM),
    ("ly:bp", INCH_TO_PT / 72.0 * PT_TO_MM),
];

/// `(ly:pt x)` and the rest of [`UNITS`]: `x` of the unit, in millimetres.
fn length(cx: &mut Cx, args: &[Value], at: usize) -> R {
    let (name, unit) = UNITS[at];
    match cx.heap.to_f64(args[0]) {
        Some(x) => Ok(cx.heap.flonum(x * unit)),
        None => Err(cx.wrong_type(name, 1, args[0], "number")),
    }
}

pub(crate) fn pt(cx: &mut Cx, args: &[Value]) -> R {
    length(cx, args, 0)
}

pub(crate) fn mm(cx: &mut Cx, args: &[Value]) -> R {
    length(cx, args, 1)
}

pub(crate) fn cm(cx: &mut Cx, args: &[Value]) -> R {
    length(cx, args, 2)
}

pub(crate) fn inch(cx: &mut Cx, args: &[Value]) -> R {
    length(cx, args, 3)
}

pub(crate) fn bp(cx: &mut Cx, args: &[Value]) -> R {
    length(cx, args, 4)
}

/// The predicates of LilyPond's C++ types that `lily.scm`'s init data
/// names and the engine does not have yet: none of its values is one.
pub(crate) const NEVER: [&str; 7] = [
    "ly:context?",
    "ly:grob-array?",
    "ly:grob-properties?",
    "ly:grob?",
    "ly:music-output?",
    "ly:stream-event?",
    "ly:tuplet-description?",
];

/// One of [`NEVER`]: false.
pub(crate) fn never(_: &mut Cx, _: &[Value]) -> R {
    Ok(Value::FALSE)
}

/// `(gettext message [domain [category]])`, in `(guile)`: the message, as
/// Guile gives it with no catalogue.
pub(crate) fn gettext(cx: &mut Cx, args: &[Value]) -> R {
    match args[0].tag() {
        Tag::String => Ok(args[0]),
        _ => Err(cx.wrong_type("gettext", 1, args[0], "string")),
    }
}

/// `(ly:error message arg ...)`. LilyPond prints `fatal error:` and the
/// message and exits; the engine throws it as a `misc-error` from no
/// procedure, which ends what is running.
pub(crate) fn error(cx: &mut Cx, args: &[Value]) -> R {
    if args[0].tag() != Tag::String {
        return Err(cx.wrong_type("ly:error", 1, args[0], "string"));
    }
    let key = cx.heap.intern("misc-error");
    let rest = cx.heap.list(&args[1..]);
    let args = cx.heap.list(&[Value::FALSE, args[0], rest, Value::FALSE]);
    cx.error = Some(Exception { key, args });
    Err(Thrown)
}

/// `(%bound-locally? symbol)`: whether the current module itself has a
/// value for `symbol`, as `lily.scm` asks `variable-bound?` of the variable
/// `module-make-local-var!` gives.
pub(crate) fn bound_locally(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.globals.current_module(&cx.heap);
    let bound = args[0].tag() == Tag::Symbol
        && cx
            .globals
            .local_slot(module, args[0])
            .is_some_and(|slot| cx.globals.values[slot as usize] != Value::UNDEFINED);
    Ok(Value::bool(bound))
}

/// `String_convert::percent_encode`: each byte of `s` but `-./:_`, digits
/// and ASCII letters as `%xx`, as LilyPond 2.26.0 encodes.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-./:_".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&alloc::format!("%{b:02x}"));
        }
    }
    out
}

/// `(ly:string-percent-encode str)`.
pub(crate) fn string_percent_encode(cx: &mut Cx, args: &[Value]) -> R {
    let Some(text) = cx.heap.str(args[0]).map(|s| percent_encode(s.as_str())) else {
        return Err(cx.wrong_type("ly:string-percent-encode", 1, args[0], "string"));
    };
    Ok(cx.heap.string(&text))
}

/// The primitives of `(lily)`.
pub(crate) const NAMES: &[&str] = &[
    "ly:assoc-get",
    "ly:string-percent-encode",
    "ly:chain-assoc-get",
    "ly:version",
    "ly:dir?",
    "ly:dimension?",
    "ly:error",
    "%bound-locally?",
    "ly:pt",
    "ly:mm",
    "ly:cm",
    "ly:inch",
    "ly:bp",
    "ly:context?",
    "ly:grob-array?",
    "ly:grob-properties?",
    "ly:grob?",
    "ly:music-output?",
    "ly:stream-event?",
    "ly:tuplet-description?",
    "%part-source",
    "%part-sources",
    "%named-formals",
    "%make-display-stub",
    "%display-stub-target",
    "%undo-suspend!",
    "%undo-resume!",
];

/// `lily.scm`'s definitions that `(lily)` starts with, after the export of
/// the `ly:` primitives above, as `LY_DEFINE` exports LilyPond's. `%location` is
/// defined first, so [`crate::Vm::set_location`] finds its slot; `lily.scm`
/// defines it again into the same slot.
///
/// A session variable is recorded as `(name is-parser? module . value)`,
/// where `lily.scm` records its variable object: the engine has none yet,
/// and the module and the name find the same slot. `declarations-init.ly`
/// calls `session-start-record` and `session-save`: a file's module uses
/// the init file's, which gives it the exports of `define-session-public`
/// too, and the parser records the init file's variables when it ends
/// (`parse_init`), so `session-save` records only those exports, as
/// `define-session`'s, and ends the first session. `session-terminate`
/// gives each recorded variable its value again and runs the thunks of
/// `call-after-session`, after each file (`lily_engine_core`), and then
/// gives the options back the values `session-save` found, as
/// `lilypond-all`'s `ly:reset-options` gives each file those it began with
/// (MS4 step 14, C2.2: a score's `point-and-click #f` ends with it). `ly:debug`
/// prints nothing at LilyPond's `WARNING` level, and `ly:get-option` gives
/// LilyPond's defaults of the options the init files read, and of
/// `point-and-click`, which the SVG writer's `grob-cause` reads, and of
/// `use-paper-size-for-page`, which `page.scm`'s page stencils read. The
/// `backend` is `svg`, as `main.cc:662–663` sets it when a run names no
/// backend and SVG is its only format, and `lily.scm:562–566` then sets
/// `music-strings-to-paths`. `(ice-9 format)`'s `format` is
/// `ice9-format`, as `lily.scm:21` imports it, for `number-format`.
pub(crate) const PRELUDE: &str = r#"
(use-modules (lily curried-definitions))
(use-modules ((ice-9 format) #:select ((format . ice9-format))))
(export ly:assoc-get ly:chain-assoc-get ly:string-percent-encode ly:version ly:make-regex ly:regex-quote ly:dir? ly:dimension?
        ly:error ly:pt ly:mm ly:cm ly:inch ly:bp ly:context? ly:grob-array? ly:grob-properties? ly:grob?
        ly:music-output? ly:stream-event? ly:tuplet-description?)
(define format simple-format)
;; `general-scheme.cc`'s `ly:string-substitute`, and `lily.scm`'s
;; `is-absolute?` of a file's name, as on any system but Windows.
(define-public (ly:string-substitute a b s)
  (%check-type "ly:string-substitute" 1 string? "string" a)
  (%check-type "ly:string-substitute" 2 string? "string" b)
  (%check-type "ly:string-substitute" 3 string? "string" s)
  (if (string-null? a)
      s
      (let loop ((start 0) (parts '()))
        (let ((i (string-contains s a start)))
          (if i
              (loop (+ i (string-length a)) (cons* b (substring s start i) parts))
              (apply string-append (reverse (cons (substring s start) parts))))))))
(define-public (is-absolute? file-name)
  (and (> (string-length file-name) 0) (eqv? (string-ref file-name 0) #\/)))
;; The engine has no current directory: the host's names start at its root.
(define-public (ly-getcwd) "")
;; The output name a compile gives LilyPond's files, which the core sets
;; (`ly:parser-output-name`).
(define %output-name (list #f))
(define-public (ly:parser-output-name) (car %output-name))
(define %location (make-fluid))
(define-public (*location*) (fluid-ref %location))
(define-syntax-rule-public (_i x) x)
(define-public G_ gettext)
(define-public (ly:wide-char->utf-8 wc) (string (integer->char wc)))
(define %options '((backend . svg) (paper-size . "a4") (staff-size . 20) (deterministic . #f)
                   (point-and-click . #t) (use-paper-size-for-page . #t) (midi-extension . "midi")))
(define-public (ly:get-option var) (assq-ref %options var))
;; `program-option-scheme.cc`'s, without its table: an option it does not
;; know is set without `no such program option`, and a value is not checked
;; against the option's type (MS2 step 21).
(define*-public (ly:set-option var #:optional (val #t))
  (%check-type "ly:set-option" 1 symbol? "symbol" var)
  (let ((name (symbol->string var)))
    (if (string-prefix? "no-" name)
        (begin (set! var (string->symbol (substring name 3)))
               (set! val (not val)))))
  (set! %options (acons var val %options))
  (if #f #f))
(define lilypond-declarations '())
(define lilypond-exports '())
(define after-session-thunks '())
(define first-session-done? #f)
(define %session-options #f)
(define-public (call-after-session thunk)
  (if first-session-done?
      (ly:error (G_ "call-after-session used after session start")))
  (set! after-session-thunks (append after-session-thunks (list thunk))))
(define (%session-check name)
  (if first-session-done?
      (ly:error (G_ "define-session used after session start")))
  (if (%bound-locally? name)
      (ly:error (G_ "symbol ~S redefined") name)))
(define-public (session-start-record) (if #f #f))
(define-public (session-save)
  (for-each (lambda (e)
              (set! lilypond-declarations
                    (cons (cons* (car e) #f (cdr e) (module-ref (cdr e) (car e)))
                          lilypond-declarations)))
            lilypond-exports)
  (set! %session-options %options)
  (set! first-session-done? #t))
(define (session-terminate)
  (for-each (lambda (record)
              (let ((module (caddr record)) (value (cdddr record)))
                (if (not (eq? (module-ref module (car record) #f) value))
                    (module-define! module (car record) value))))
            lilypond-declarations)
  (for-each (lambda (thunk) (thunk)) after-session-thunks)
  (if %session-options (set! %options %session-options)))
(define-syntax-rule (define-session name value)
  (begin
    (%session-check 'name)
    (define name value)
    (set! lilypond-declarations
          (cons (cons* 'name #f (current-module) name) lilypond-declarations))))
(define-public (ly:debug str . rest)
  (%check-type "ly:debug" 1 string? "string" str)
  (if #f #f))
(define %grob-interfaces (make-hash-table 59))
(define (%check-type subr position ok? what x)
  (if (not (ok? x))
      (scm-error 'wrong-type-arg subr
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list position what x) (list x))))
;; `lily.scm`'s: the `svg` backend draws music strings as paths.
(define music-string-to-path-backends
  '(svg))
(if (memq (ly:get-option 'backend) music-string-to-path-backends)
    (ly:set-option 'music-strings-to-paths #t))
(define-public (ly:add-interface iface desc props)
  (%check-type "ly:add-interface" 1 symbol? "symbol" iface)
  (%check-type "ly:add-interface" 2 string? "string" desc)
  (%check-type "ly:add-interface" 3 list? "list" props)
  (hashq-set! %grob-interfaces iface (list iface desc props))
  (if #f #f))
(define-public (ly:all-grob-interfaces) %grob-interfaces)
(define %translators (make-hash-table 31))
(define*-public (ly:register-translator creator name #:optional (description '()))
  (%check-type "ly_register_translator" 1 (lambda (c) (or (procedure? c) (pair? c)))
               "translator creator" creator)
  (%check-type "ly:register-translator" 2 symbol? "symbol" name)
  (%check-type "ly:register-translator" 3 list? "list" description)
  (hashq-set! %translators name (list name creator description))
  (if #f #f))
(define %parts '())
(define %parts-loaded '())
;; The parts the image loaded for its init and bound to their stubs again
;; after it (D78): when one loads again, a `define` of a name the image
;; holds a value of, which is no stub, keeps that value (D82), so that what
;; the init stored stays `eq?` to what the part's procedures compare it to
;; (`chordmodifiers`' `sus-modifier`), and a `add-new-clef` is not run a second
;; time: tablature.scm's `moderntab`, the one toplevel call of the kept parts,
;; which `set!`s `supported-clefs` onto what the image already holds (OPEN C9).
(define %parts-kept '())
(define %stubs (make-weak-key-hash-table 61))
(define (%part-stub part module name)
  (define (stub . args)
    (%load-part! part)
    (let ((value (module-ref (resolve-module module) name)))
      (if (eq? value stub)
          (scm-error 'misc-error (symbol->string name) "part ~A does not define ~A"
                     (list part name) #f))
      (apply value args)))
  (set-procedure-property! stub 'name name)
  (hashq-set! %stubs stub (list part module name))
  stub)
;; The image's markup commands whose procedures call a part's stub when
;; they are interpreted, in the layout, which asks the host for nothing
;; (D228): their part loads at their lookup, as 03 §6 has `qr` load on
;; `\qr-code`.
(define %markup-parts '((qr-code-markup . qr)))
;; What `symbol` names in `module`, or #f, a part's stub after its part
;; loaded: markup mode's lookup of a command by its name is its trigger,
;; and that of a command of `%markup-parts`.
(define-public (%markup-procedure module symbol)
  (let* ((proc (module-ref module symbol #f))
         (stub (and (procedure? proc) (hashq-ref %stubs proc)))
         (part (assq-ref %markup-parts symbol)))
    (cond
      (stub (%load-part! (car stub)) (module-ref module symbol #f))
      (part (%load-part! part) proc)
      (else proc))))
;; Every file's text is asked for before any of them runs: a host that must
;; be asked for some of them stops the run once it is asked for them all,
;; and the part loads whole when the run starts again. What the load changes
;; is not logged (R11): an undo of the item that loaded it leaves it loaded.
(define (%load-part! part)
  (if (not (memq part %parts-loaded))
      (let* ((files (assq-ref %parts part))
             (texts (%part-sources
                      (map (lambda (file) (and (not (member file %files-loaded)) file))
                           files)))
             (running (%undo-suspend!)))
        (dynamic-wind
          (lambda () #f)
          (lambda ()
            (set! %parts-loaded (cons part %parts-loaded))
            (for-each (lambda (file text)
                        (save-module-excursion
                          (lambda ()
                            (set-current-module (resolve-module '(lily)))
                            (%load-text file text (memq part %parts-kept)))))
                      files texts))
          (lambda () (%undo-resume! running))))))
;; Each part a value of the alists `alists` is a stub of, loaded: a
;; translation's grobs' definitions, before their layout, which asks the
;; host for no file (A46, D228).
(define (%load-stubbed-parts! alists)
  (for-each
    (lambda (alist)
      (for-each
        (lambda (entry)
          (let ((stub (and (pair? entry) (procedure? (cdr entry)) (hashq-ref %stubs (cdr entry)))))
            (if stub (%load-part! (car stub)))))
        alist))
    alists))
;; Whether `form` defines a name the current module holds a value of that
;; is no part's stub.
(define (%kept-definition? form)
  (and (pair? form) (eq? (car form) 'define) (pair? (cdr form))
       (let* ((name (let head ((h (cadr form))) (if (pair? h) (head (car h)) h)))
              (value (and (symbol? name) (module-ref (current-module) name #f))))
         (and value (not (and (procedure? value) (hashq-ref %stubs value)))))))
(define (%kept-form? form)
  (or (%kept-definition? form)
      (and (pair? form) (eq? (car form) 'add-new-clef))))
(define %files-loaded '())
(define (%load-source file)
  (if (not (member file %files-loaded))
      (%load-text file (%part-source file))))
(define* (%load-text file text #:optional kept)
  (if (not (member file %files-loaded))
      (begin
        (if (not text)
            (scm-error 'misc-error "%load-part!" "needs ~A, which the host has not given"
                       (list file) #f))
        (set! %files-loaded (cons file %files-loaded))
        (save-module-excursion
          (lambda ()
            ;; LilyPond's own Scheme, which Guile would have compiled:
            ;; its procedures print their formals (OPEN C6).
            (let ((port (open-input-string text))
                  (named #f))
              (dynamic-wind
                ;; A part loaded at run time keeps every named
                ;; procedure's; the image keeps its toplevel ones'.
                (lambda ()
                  (set! named (%named-formals #t))
                  (if (eq? named 'toplevel) (%named-formals 'toplevel)))
                (lambda ()
                  (let loop ((form (read port)))
                    (if (not (eof-object? form))
                        (begin
                          (if (not (and kept (%kept-form? form)))
                              (primitive-eval form))
                          (loop (read port))))))
                (lambda () (%named-formals named)))))))))
(define-public (ly:load x) (%load-source (string-append x ".scm")));; LilyPond's `display-methods` of each music type, as `display-lily.scm`
;; makes them, before the `display` part loads (OPEN C6): `table` is
;; `((type (name . param) …) …)`, each type's methods as the part defines
;; them, the first called first.
(define (%display-method-stubs! table)
  (for-each
    (lambda (entry)
      (let* ((type (car entry))
             (props (hashq-ref music-name-to-property-table type '()))
             (stubs (let loop ((methods (cdr entry)) (i 0))
                      (if (null? methods)
                          '()
                          (cons (%make-display-stub (caar methods) (cdar methods) type i
                                                    %display-stub-call)
                                (loop (cdr methods) (+ i 1)))))))
        (let ((props (assoc-set! props 'display-methods stubs))
              (entry (assq type %music-properties)))
          ;; `make-music` reads `%music-properties`, which shares each
          ;; type's list with the table.
          (hashq-set! music-name-to-property-table type props)
          (if entry (set-cdr! entry props)))))
    table))
(define (%display-stub-call stub music)
  (%load-part! 'display)
  (let* ((target (%display-stub-target stub))
         (props (hashq-ref music-name-to-property-table (car target) '())))
    ((list-ref (assoc-ref props 'display-methods) (cdr target)) music)))
;; A module's own file that a part's data holds, which a `use-modules` of
;; the module loads as a part loads (D273): `(lily accreg)`'s.
(define (%load-module-source file)
  (if (not (member file %files-loaded))
      (let ((text (%part-source file))
            (running (%undo-suspend!)))
        (dynamic-wind
          (lambda () #f)
          (lambda ()
            (save-module-excursion
              (lambda ()
                (set-current-module (resolve-module '(lily)))
                (%load-text file text))))
          (lambda () (%undo-resume! running))))))
(define-syntax-rule (define-session-public name value)
  (begin
    (%session-check 'name)
    (define name value)
    (set! lilypond-exports
          (acons 'name (current-module) lilypond-exports))
    (export name)))
"#;
