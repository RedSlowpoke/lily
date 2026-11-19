//! Embedded Scheme (02 §2.4, §5.4, §6's P4): `parse-scm.cc`'s reading and
//! evaluation of a `#` or `$` form, and what the grammar makes of a value.
//!
//! The lexer hands the text after `#` or `$` to the reader at that byte,
//! with LilyPond's `#{ … #}` given to [`LilyScan`], which finds where it
//! ends; `#@` and `$@` read the form after the `@` as `(apply values
//! form)`. A read error is Guile's, under `Guile signaled an error for the
//! expression beginning here` at the byte after `#` or `$`, and the lexer
//! goes on at that byte, as LilyPond's does. The end of input after `#`
//! fails the file with no message.
//!
//! A form runs in the innermost scope with `(*location*)` its [`Input`],
//! through the interpreter's `eval_at`, which places its syntax errors in
//! the file; any error is reported as LilyPond reports it, the form's value
//! is then unspecified and the file has failed. What it writes to the error
//! port comes among the diagnostics where Guile flushes it: before its own
//! message of an error, and at the end of the file.
//!
//! Bison's timing (§5.4): `$` runs when it is lexed. A `#` runs when the
//! grammar takes its token: Bison shifts a `SCM_TOKEN` and reduces it to
//! `embedded_scm_bare` or the toplevel's or a header's item at once, a
//! default reduction, so a `#` runs before the token after it is lexed, and
//! before a `$` there. Values after the first wait as extra tokens: each a
//! `SCM_IDENTIFIER` after `#`, classified after `$`.
//!
//! `ly:parser-include-string` (P11's, brought forward for T16) is Scheme in
//! `(lily)` ([`define_include_string`]) that keeps its strings in a box;
//! after each form runs, the parser takes them ([`take_included`]) and the
//! lexer reads them next (`Lexer::include_string`), as LilyPond's pushes
//! each on its lexer's include stack while the form runs.
//!
//! A `#{ … #}` evaluates to LilyPond's call of `read-lily-expression-internal`,
//! which parses the template with a clone of the parser (`embedded.rs`, MS2
//! step 33): a form of the template is read at its byte for its end, and the
//! closure the reader made of it there is called in its place.

use crate::Parser;
use crate::classify::{Env, Id, is_key};
use crate::diag::{At, Diagnostics};
use crate::lexer::{NO_FORM, Tok, Token};
use crate::source::{Counts, Source, Span};
use crate::syntax::Key;
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::Input;
use lily_engine_scheme::{Embedded, Heap, LilyScan, Position, Tag, Value, Vm, read};

/// The message LilyPond gives an error of Scheme read or run from a file,
/// Guile's own after the excerpt.
const GUILE_ERROR: &str = "Guile signaled an error for the expression beginning here";

/// The port LilyPond reads a form from: the file at the byte `at`, whose
/// counts are `c`, set to its line and to its column less one
/// (`internal_parse_embedded_scheme`).
fn port(src: &Source, at: usize, c: Counts) -> lily_engine_scheme::Source<'_> {
    lily_engine_scheme::Source {
        text: src.scheme_text(),
        file: Some(src.name()),
        offset: at,
        at: Position {
            line: c.line - 1,
            column: c.column.saturating_sub(1),
        },
    }
}

fn point(file: u16, at: usize) -> At {
    At::Span(Span {
        file,
        start: at as u32,
        end: at as u32,
    })
}

impl Env<'_> {
    /// `parse_embedded_scheme` at the byte `at` of `src`: the form's index
    /// in [`Env::forms`] and the byte after it ([`crate::classify::Scopes::read`]).
    pub(crate) fn read_form(
        &mut self,
        src: &Source,
        file: u16,
        at: usize,
        diags: &mut Diagnostics,
    ) -> (Option<u32>, usize) {
        let text = src.scheme_text();
        if at > text.len() {
            return (None, at);
        }
        let multiple = text.as_bytes().get(at) == Some(&b'@');
        let mut scan = LilyScan {
            file: Some(String::from(src.name())),
        };
        let heap = self.vm().heap_mut();
        let embedded: &mut dyn Embedded = &mut scan;
        match read(heap, text, at + usize::from(multiple), Some(embedded)) {
            Ok(r) if r.datum == Value::EOF => (None, r.end),
            Ok(mut r) => {
                let closure = self.closure(file, at);
                let heap = self.vm().heap_mut();
                if let Some(closure) = closure {
                    r.datum = closure;
                } else if multiple {
                    let apply = heap.intern("apply");
                    let values = heap.intern("values");
                    r.datum = heap.list(&[apply, values, r.datum]);
                }
                self.keep(r.datum);
                let end = r.end;
                self.forms.push(r);
                let form = self.forms.len() as u32 - 1;
                if closure.is_some() {
                    self.mark_thunk(form);
                }
                (Some(form), end)
            }
            Err(e) => {
                let exception = self
                    .vm()
                    .read_error(&e, &port(src, at, src.counts(at as u32)));
                let text = exception.text(self.vm().heap());
                diags.guile_error(point(file, at), GUILE_ERROR, &text);
                (None, at)
            }
        }
    }

    /// `evaluate_embedded_scheme` of the form `form`, read at the byte `at`
    /// of `src`: its value, rooted, or None after an error, reported.
    pub(crate) fn eval_form(
        &mut self,
        src: &Source,
        file: u16,
        form: u32,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<Value> {
        let c = src.counts(at as u32);
        // `(*location*)`: LilyPond's `Input` of the `#` or `$` token, stepped
        // forward (`eval_scm_token`): empty, at the byte after it, which a
        // template's music is placed at (MS2 step 33).
        let location = src.input(file, at as u32, at as u32);
        // Rooted by the fluid that `eval_at` sets to it.
        let location = self.vm().heap_mut().wrap(location);
        let module = self.innermost();
        let previous = self.vm().set_current_module(module);
        let result = match self.is_thunk(form) {
            // A template's closure, called (`internal_evaluate_embedded_scheme`).
            true => {
                let thunk = self.forms[form as usize].datum;
                let before = self.vm().set_location(location);
                self.keep(before);
                let result = self.vm().apply(thunk, &[]);
                self.vm().set_location(before);
                result
            }
            false => self.eval_kept(form, &port(src, at, c), location),
        };
        self.vm().set_current_module(previous);
        self.define_taken();
        // `here_input`: the `#` or `$` before the form.
        let here = Span {
            file,
            start: at.saturating_sub(1) as u32,
            end: at as u32,
        };
        take_scheme_output(self.vm(), diags, here);
        match result {
            Ok(value) => {
                self.keep(value);
                self.place_music(src, file, at, value);
                Some(value)
            }
            Err(e) => {
                let text = e.text(self.vm().heap());
                diags.guile_error(point(file, at), GUILE_ERROR, &text);
                None
            }
        }
    }

    /// What `ly:parser-define!` was given, defined in the innermost scope.
    pub(crate) fn define_taken(&mut self) {
        for (name, value) in take_defined(self.vm()) {
            self.define(&name, value);
        }
    }

    /// `eval_scm`: music among a form's values that has no `origin` is
    /// given the `#` or `$` before the form, the lexer's place then (N2).
    fn place_music(&mut self, src: &Source, file: u16, at: usize, value: Value) {
        let Some(token) = at.checked_sub(1) else {
            return;
        };
        for value in self.values_of(value) {
            let heap = self.vm().heap_mut();
            if heap.foreign::<lily_engine_music::Music>(value).is_none() {
                continue;
            }
            let key = heap.intern("origin");
            let origin = lily_engine_music::music_property(heap, value, key);
            if origin.is_some_and(|o| heap.foreign::<Input>(o).is_some()) {
                continue;
            }
            let input = match self.override_input {
                Some(input) => input,
                None => {
                    let input = src.input(file, token as u32, at as u32);
                    self.vm().heap_mut().wrap(input)
                }
            };
            lily_engine_music::set_origin(self.vm().heap_mut(), value, input);
        }
    }

    /// The values of `value`: itself, or a `values` object's.
    pub(crate) fn values_of(&self, value: Value) -> Vec<Value> {
        if value.tag() == Tag::Values {
            self.vm_ref().heap().values_items(value).to_vec()
        } else {
            alloc::vec![value]
        }
    }

    /// The `[crate::classify::Scopes::immediate]` of a `$` form.
    pub(crate) fn immediate_values(
        &mut self,
        src: &Source,
        file: u16,
        form: Option<u32>,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<Vec<Option<(Id, u32)>>> {
        let value = self.eval_form(src, file, form?, at, diags)?;
        let values = self.values_of(value);
        let mut tokens = Vec::with_capacity(values.len());
        for value in values {
            // `scan_scm_id` leaves an unspecified value without a token.
            tokens.push((value != Value::UNSPECIFIED).then(|| {
                let value = self.special_copy(value);
                let id = self.classify(value);
                (id, self.keep(value))
            }));
        }
        Some(tokens)
    }
}

/// A key of `value`: a symbol, an index, or a string as a symbol.
fn key(heap: &Heap, value: Value) -> Option<Key> {
    if let Some(text) = heap.str(value) {
        return Some(Key::Symbol(String::from(text.as_str())));
    }
    if !is_key(heap, value) {
        return None;
    }
    Some(match value.tag() {
        Tag::Symbol => Key::Symbol(String::from(heap.name(value).unwrap_or_default())),
        // An index past u64, as an `UNSIGNED` past it.
        _ => Key::Index(value.as_fixnum().map_or(u64::MAX, |n| n as u64)),
    })
}

/// `make_reverse_key_list` in order: the keys of a `#` form's or an
/// `SCM_IDENTIFIER`'s value in a property path, a key or a list of them;
/// None for anything else, which is `not a key`.
pub(crate) fn scheme_keys(heap: &Heap, value: Value) -> Option<Vec<Key>> {
    if let Some(key) = key(heap, value) {
        return Some(alloc::vec![key]);
    }
    let mut keys = Vec::new();
    let mut p = value;
    while p.tag() == Tag::Pair {
        keys.push(key(heap, heap.car(p)?)?);
        p = heap.cdr(p)?;
    }
    (p == Value::NULL).then_some(keys)
}

/// A count that is not negative, None for any other value; past u64, its
/// greatest.
fn index(heap: &Heap, value: Value) -> Option<u64> {
    match value.tag() {
        Tag::Fixnum => value.as_fixnum().and_then(|n| u64::try_from(n).ok()),
        Tag::Bignum => heap
            .as_bignum(value)
            .filter(|n| !n.is_negative())
            .map(|_| u64::MAX),
        _ => None,
    }
}

/// `scale?` and `scale->factor` of a multiplier's value (`multiplier_scm`):
/// an exact rational that is not negative, or a fraction `(n . d)` of
/// indices, as `n/d`; None for anything else, which is `not a multiplier`.
/// One past u64 is `(1, 1)`, which leaves the factor as it was. A moment is
/// not taken until the music model reaches the parser.
pub(crate) fn scale(heap: &Heap, value: Value) -> Option<(u64, u64)> {
    let fit = |n: u64, d: u64| {
        if n == u64::MAX || d == u64::MAX {
            (1, 1)
        } else {
            (n, d)
        }
    };
    if let Some((n, d)) = heap.as_ratio(value) {
        return Some(fit(index(heap, n)?, index(heap, d)?));
    }
    if value.tag() == Tag::Pair {
        let (n, d) = (heap.car(value)?, heap.cdr(value)?);
        return Some(fit(index(heap, n)?, index(heap, d)?));
    }
    index(heap, value).map(|n| fit(n, 1))
}

/// The box in `(lily)` that holds the strings `ly:parser-include-string`
/// was given, the last first.
const INCLUDED: &str = "%parser-included-strings";

/// The box in `(lily)` that holds what `ly:parser-define!` was given, each
/// `(id . val)`, the last first.
const DEFINED: &str = "%parser-defined";

/// The box in `(lily)` that holds the language `ly:parser-set-note-names`
/// was last given the pitch names of, or `#f`.
const NOTE_NAMES: &str = "%parser-note-names";

/// Defines `ly:parser-include-string`, `ly:parser-define!`,
/// `ly:parser-lookup` and `ly:parser-set-note-names` in `vm`'s `(lily)`:
/// each checks its argument as `LY_ASSERT_TYPE` does. The first two keep
/// it for the parser, which includes the string or defines the variable in
/// its innermost scope (`set_identifier`) when the Scheme it ran returns;
/// the third looks the symbol up in what `ly:parser-define!` kept since,
/// the last first, as if it were defined already, as LilyPond defines it
/// at once (MS3 step 35), then in the current module, which is the
/// parser's innermost scope while it runs Scheme, `'()` if it is unbound;
/// the last sets `pitchnames` and keeps the language whose names the alist
/// is, which the lexer takes up when the Scheme returns
/// (`note-names-language`, `\language`).
pub fn define_include_string(vm: &mut Vm) -> Result<(), String> {
    const TEMPLATES: &str = crate::embedded::DEFINITIONS;
    let text = alloc::format!(
        "(define {INCLUDED} (list '()))
         (define-public (ly:parser-include-string ly-code)
           (if (not (string? ly-code))
               (scm-error 'wrong-type-arg \"ly:parser-include-string\"
                          \"Wrong type argument in position ~A (expecting ~A): ~S\"
                          (list 1 \"string\" ly-code) (list ly-code)))
           (set-car! {INCLUDED} (cons ly-code (car {INCLUDED}))))
         (define {DEFINED} (list '()))
         (define-public (ly:parser-define! id val)
           (if (not (symbol? id))
               (scm-error 'wrong-type-arg \"ly:parser-define!\"
                          \"Wrong type argument in position ~A (expecting ~A): ~S\"
                          (list 1 \"symbol\" id) (list id)))
           (set-car! {DEFINED} (cons (cons id val) (car {DEFINED}))))
         (define-public (ly:parser-lookup symbol)
           (if (not (symbol? symbol))
               (scm-error 'wrong-type-arg \"ly:parser-lookup\"
                          \"Wrong type argument in position ~A (expecting ~A): ~S\"
                          (list 1 \"symbol\" symbol) (list symbol)))
           (let ((defined (assq symbol (car {DEFINED}))))
             (if defined (cdr defined) (module-ref (current-module) symbol '()))))
         {TEMPLATES}
         (define {NOTE_NAMES} (list #f))
         (define-public (ly:parser-set-note-names names)
           (set! pitchnames names)
           (let ((entry (find (lambda (e) (eq? (cdr e) names)) language-pitch-names)))
             (if entry (set-car! {NOTE_NAMES} (car entry)))))"
    );
    let lily = vm
        .eval_text("(resolve-module '(lily))", None)
        .map_err(|e| e.text(vm.heap()))?;
    let previous = vm.set_current_module(lily);
    let ran = vm.eval_text(&text, None).map(|_| ());
    vm.set_current_module(previous);
    ran.map_err(|e| e.text(vm.heap()))
}

/// The strings `ly:parser-include-string` was given since the last call,
/// in the order of its calls, taken out of its box; none without one.
pub(crate) fn take_included(vm: &mut Vm) -> Vec<String> {
    let Some(cell) = vm.slot("lily", INCLUDED).and_then(|s| vm.slot_value(s)) else {
        return Vec::new();
    };
    let heap = vm.heap_mut();
    let mut list = heap.car(cell).unwrap_or(Value::NULL);
    if list == Value::NULL {
        return Vec::new();
    }
    heap.set_car(cell, Value::NULL);
    let mut strings = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        strings.extend(heap.str(item).map(|s| String::from(s.as_str())));
        list = rest;
    }
    strings.reverse();
    strings
}

/// The language `ly:parser-set-note-names` was given the names of since
/// the last call, taken out of its box.
pub(crate) fn take_note_names(vm: &mut Vm) -> Option<String> {
    let cell = vm.slot("lily", NOTE_NAMES).and_then(|s| vm.slot_value(s))?;
    let heap = vm.heap_mut();
    let language = heap.car(cell)?;
    heap.set_car(cell, Value::FALSE);
    heap.name(language).map(String::from)
}

/// What `ly:parser-define!` was given since the last call, in the order
/// of its calls, taken out of its box: each name and value.
pub(crate) fn take_defined(vm: &mut Vm) -> Vec<(String, Value)> {
    let Some(cell) = vm.slot("lily", DEFINED).and_then(|s| vm.slot_value(s)) else {
        return Vec::new();
    };
    let heap = vm.heap_mut();
    let mut list = heap.car(cell).unwrap_or(Value::NULL);
    if list == Value::NULL {
        return Vec::new();
    }
    heap.set_car(cell, Value::NULL);
    let mut defined = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        if let (Some(id), Some(val)) = (heap.car(item), heap.cdr(item))
            && let Some(name) = heap.name(id)
        {
            defined.push((String::from(name), val));
        }
        list = rest;
    }
    defined.reverse();
    defined
}

/// The box in `(lily)` where the music crate's warnings keep what they
/// were given, the last first (`lily_engine_music`'s `warn.rs`).
pub(crate) const DIAGNOSTICS: &str = "%diagnostics";

/// Takes what Scheme wrote to the error port and what `ly:warning`,
/// `ly:input-warning`, `ly:parser-error` and the rest kept since the last
/// call into `diags`, in the order LilyPond prints them (08 §6): each
/// message after what `force-output` had flushed of the port's text when it
/// was given, and the rest of that text pending till the next flush. A
/// warning or an error is at its input location with the excerpt, or with
/// none, a warning nowhere and a parser error `here`, at the lexer's last
/// token (`Lily_parser::parser_error`); a programming error is LilyPond's
/// two lines, at its input location with the excerpt between them, or at
/// none. Messages are left out, as `--loglevel=WARNING` leaves them.
/// An error fails the file. `ly:expect-warning`'s are kept in their place.
pub(crate) fn take_scheme_output(vm: &mut Vm, diags: &mut Diagnostics, here: Span) {
    let (text, flushed) = vm.take_error_port();
    // Bytes of `text` put among the diagnostics so far.
    let mut done = 0;
    let flush_to = |diags: &mut Diagnostics, done: &mut usize, mark: usize| {
        if let Some(part) = text.get(*done..mark.min(flushed))
            && !part.is_empty()
        {
            diags.output(part);
            diags.flush();
            *done += part.len();
        }
    };
    let cell = vm.slot("lily", DIAGNOSTICS).and_then(|s| vm.slot_value(s));
    let heap = vm.heap_mut();
    let mut list = cell.and_then(|c| heap.car(c)).unwrap_or(Value::NULL);
    if let Some(cell) = cell
        && list != Value::NULL
    {
        heap.set_car(cell, Value::NULL);
    }
    let mut taken = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        taken.push(item);
        list = rest;
    }
    for item in taken.into_iter().rev() {
        let Some(&[kind, location, message, mark]) = heap.vector_items(item) else {
            continue;
        };
        if heap.name(kind) == Some("placed") {
            if let Some(placed) = crate::embedded::placed_message(heap, location) {
                diags.placed(placed);
            }
            continue;
        }
        let (Some(kind), Some(message)) = (heap.name(kind), heap.str(message)) else {
            continue;
        };
        let message = String::from(message.as_str());
        let mark = mark.as_fixnum().map_or(0, |m| m.max(0) as usize);
        let at = heap.foreign::<Input>(location).map(|i| diags.at_input(i));
        if kind != "message" && kind != "expect" {
            flush_to(diags, &mut done, mark);
        }
        match kind {
            "expect" => diags.expect(message),
            "check" => diags.check_expected(),
            "warning" => diags.warning(at.unwrap_or(At::Nowhere), message),
            "error" => diags.error(at.unwrap_or(At::Span(here)), message),
            "programming-error" => diags.programming_error(at, &message),
            _ => {}
        }
    }
    flush_to(diags, &mut done, flushed);
    diags.output(text.get(done..).unwrap_or_default());
}

impl Parser<'_, '_> {
    /// `eval_scm_token` of a `SCM_TOKEN` just taken: its first value, the
    /// others pushed as `SCM_IDENTIFIER`s; unspecified after an error or
    /// for no value, the file failed after an error. A string the form
    /// included is lexed next.
    pub(crate) fn scheme_value(&mut self, token: &Token) -> Value {
        let file = token.loc.file;
        let src = self.lexer.source_of(file);
        let at = token.loc.start as usize + 1;
        self.publish();
        let value = match token.value {
            NO_FORM => None,
            form => self
                .env
                .eval_form(src, file, form, at, &mut self.lexer.diags),
        };
        self.take_template_tokens();
        for text in take_included(self.env.vm()) {
            self.lexer.include_string(text);
        }
        self.take_note_names();
        let Some(value) = value else {
            self.lexer.error_level = true;
            return Value::UNSPECIFIED;
        };
        if value.tag() != Tag::Values {
            return value;
        }
        let values = self.env.values_of(value);
        for &extra in values.iter().skip(1).rev() {
            let index = self.env.keep(extra);
            self.lexer.push_extra(Tok::Identifier(Id::Scm), index);
        }
        values.first().copied().unwrap_or(Value::UNSPECIFIED)
    }

    /// The pitch names of the language `ly:parser-set-note-names` was
    /// given, if it was, for the tokens lexed from now on.
    pub(crate) fn take_note_names(&mut self) {
        if let Some(language) = take_note_names(self.env.vm())
            && let Some(table) = self.names.language(&language)
        {
            self.lexer.set_pitch_names(table);
        }
    }
}
