//! How `\name` and shorthands become tokens (02 §2.3, §5.1): the keyword
//! table and `lookup_identifier_symbol` (`lily-lexer.cc`), `scan_escaped_word`,
//! `scan_shorthand` and `scan_scm_id` (`lexer.ll`), and
//! `try_special_identifiers` (`parser.yy`), as one classification over the
//! scopes of an [`Env`]: the file's module, which uses the interpreter's
//! `(lily)`. The lexer looks keywords up itself and asks [`Scopes`] for the
//! rest.
//!
//! What the grammar builds that LilyPond keeps as a smob is the music
//! crate's: music (frozen, N2), post-events as music, and pitches, which
//! assignments define into the module as any other value and `\name`
//! classifies by their kind, as Scheme reaches them through `#x`. A music
//! function is that crate's `MusicFunction`, by the predicate of its
//! result. A context modification and a context definition (P6) are
//! copied as `try_special_identifiers` copies them.

use crate::diag::Diagnostics;
use crate::embedded::LilyParser;
use crate::source::Source;
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::{Maker, Music as MusicValue, Pitch as PitchValue};
use lily_engine_scheme::{Exception, Heap, Hosted, Read, Root, Tag, Value, Vm};

/// What a `\name` that is no keyword is, as `parser.yy`'s token for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Book,
    Number,
    /// Music that is a post-event.
    Event,
    Music,
    Pitch,
    Duration,
    /// An alist, for `\name.key`.
    Lookup,
    Module,
    /// Any other value.
    Scm,
    MusicFunction,
    EventFunction,
    ScmFunction,
    /// A markup command, in markup mode (P7): its value is the procedure.
    MarkupFunction,
    MarkupListFunction,
}

impl Id {
    /// Its name in Bison's messages.
    pub fn display(self) -> &'static str {
        match self {
            Id::Book => "BOOK_IDENTIFIER",
            Id::Number => "NUMBER_IDENTIFIER",
            Id::Event => "EVENT_IDENTIFIER",
            Id::Music => "MUSIC_IDENTIFIER",
            Id::Pitch => "PITCH_IDENTIFIER",
            Id::Duration => "DURATION_IDENTIFIER",
            Id::Lookup => "LOOKUP_IDENTIFIER",
            Id::Module => "MODULE_IDENTIFIER",
            Id::Scm => "SCM_IDENTIFIER",
            Id::MusicFunction => "MUSIC_FUNCTION",
            Id::EventFunction => "EVENT_FUNCTION",
            Id::ScmFunction => "SCM_FUNCTION",
            Id::MarkupFunction => "MARKUP_FUNCTION",
            Id::MarkupListFunction => "MARKUP_LIST_FUNCTION",
        }
    }
}

/// What the lexer looks a name up in, and reads and runs Scheme with:
/// `Lily_lexer`'s scopes and its parser's interpreter.
pub trait Scopes {
    /// The tokens of the templates the last Scheme run parsed, indented
    /// (`embedded.rs`); none without them.
    fn template_tokens(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// `lookup_identifier_symbol`, then `scan_scm_id` on what it found: the
    /// kind of `name`'s value and the index of the value the token carries;
    /// None when `name` is unbound in every scope.
    fn classify(&mut self, name: &str) -> Option<(Id, u32)>;

    /// `parse_embedded_scheme` at the byte `at` of `src`, the one after `#`
    /// or `$`: the index of the form read and the byte after it. The index
    /// is None after a read error, which is reported, and at the end of
    /// input. None for scopes that read no Scheme.
    fn read(
        &mut self,
        src: &Source,
        file: u16,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<(Option<u32>, usize)>;

    /// `eval_scm` of a `$` form read at `at`: each of its values as
    /// `scan_scm_id` classifies it, None for one that is unspecified; no
    /// values for none. None when there was no form or it failed, which is
    /// reported.
    fn immediate(
        &mut self,
        src: &Source,
        file: u16,
        form: Option<u32>,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<Vec<Option<(Id, u32)>>>;

    /// Whether each parameter of the function a token's value `index` is
    /// is optional, for the `EXPECT_SCM` and `EXPECT_OPTIONAL` tokens that
    /// `scan_scm_id` pushes; None for no function.
    fn parameters(&mut self, _index: u32) -> Option<Vec<bool>> {
        None
    }

    /// `lookup_markup_command`, then `lookup_markup_list_command`, of
    /// `name` in markup mode: the kind of command and the index of the
    /// procedure; None for no markup command (P7). A part's stub is loaded
    /// first (D74); a part that fails to load is written to `diags`.
    fn markup_command(&mut self, _name: &str, _diags: &mut Diagnostics) -> Option<(Id, u32)> {
        None
    }

    /// Whether the value of index `index` is a markup command, as `$`
    /// classifies a procedure in markup mode.
    fn markup_function(&mut self, _index: u32) -> Option<Id> {
        None
    }

    /// The tokens `push_markup_predicates` pushes for the markup command of
    /// index `index`, in the order they are read, for the tokens recorded.
    fn markup_expects(&mut self, _index: u32) -> Vec<&'static str> {
        Vec::new()
    }

    /// `scm_module_define` of the string `value` as `name` into the
    /// innermost scope (`\\sourcefilename`'s `input-file-name`).
    fn define_string(&mut self, _name: &str, _value: &str) {}

    /// The strings `ly:parser-include-string` was given since the last
    /// call, in the order of its calls; none for scopes that run no Scheme.
    fn included_strings(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// What the host has of the file `name`, which `\include` looks for:
    /// None for scopes that give no files.
    fn include_file(&mut self, _name: &str) -> Option<Hosted<Vec<u8>>> {
        None
    }

    /// `\include \name`'s file name: the string `name` is bound to, or
    /// what `display` writes of its value, `#<undefined>` unbound. None for
    /// scopes that give no files.
    fn include_name(&mut self, _name: &str) -> Option<Result<String, String>> {
        None
    }

    /// `\include #form` and `$form`'s file name: the string the form read
    /// at `at` gives, or what `display` writes of its value, None when it
    /// failed, which is reported. None for scopes that give no files.
    fn include_form(
        &mut self,
        _src: &Source,
        _file: u16,
        _form: Option<u32>,
        _at: usize,
        _diags: &mut Diagnostics,
    ) -> Option<Result<String, Option<String>>> {
        None
    }

    /// Whether the parse is to stop, for a file the host must be asked for
    /// that Scheme needed ([`Vm::waiting`]).
    fn waiting(&mut self) -> bool {
        false
    }

    /// The procedure `name` names in `chordmodifiers`, in chord mode
    /// (`chordmodifier_tab_`): its index; None if it names none (P10).
    fn chord_modifier(&mut self, _name: &str) -> Option<u32> {
        None
    }

    /// The drum type `name` names in `drumPitchNames`, in drum mode: the
    /// index of its symbol; None if it names none or no symbol.
    fn drum_pitch(&mut self, _name: &str) -> Option<u32> {
        None
    }
}

/// No scopes: every name is unbound, as in an empty module, and Scheme is
/// not read.
pub struct NoScopes;

impl Scopes for NoScopes {
    fn classify(&mut self, _: &str) -> Option<(Id, u32)> {
        None
    }

    fn read(
        &mut self,
        _: &Source,
        _: u16,
        _: usize,
        _: &mut Diagnostics,
    ) -> Option<(Option<u32>, usize)> {
        None
    }

    fn immediate(
        &mut self,
        _: &Source,
        _: u16,
        _: Option<u32>,
        _: usize,
        _: &mut Diagnostics,
    ) -> Option<Vec<Option<(Id, u32)>>> {
        None
    }
}

/// `key?`: a symbol or an index.
pub(crate) fn is_key(heap: &Heap, value: Value) -> bool {
    match value.tag() {
        Tag::Symbol => true,
        Tag::Fixnum => value.as_fixnum().is_some_and(|n| n >= 0),
        Tag::Bignum => heap.as_bignum(value).is_some_and(|n| !n.is_negative()),
        _ => false,
    }
}

/// `scan_scm_id` and `try_special_identifiers`: a music function by the
/// predicate of its result, `music_p` (`ly:music?`) or `event_p`
/// (`ly:event?`) or another, then the value's kind; anything else is an
/// `SCM_IDENTIFIER`.
fn classify(heap: &Heap, (music_p, event_p): (Value, Value), value: Value) -> Id {
    if let Some(result) = crate::function::result_predicate(heap, value) {
        return match result {
            _ if result == music_p => Id::MusicFunction,
            _ if result == event_p => Id::EventFunction,
            _ => Id::ScmFunction,
        };
    }
    match value.tag() {
        Tag::Fixnum | Tag::Bignum | Tag::Flonum | Tag::Ratio | Tag::Complex => Id::Number,
        Tag::Foreign => match heap.foreign_object(value).map(|f| f.type_name()) {
            _ if heap.foreign::<MusicValue>(value).is_some() => {
                if lily_engine_music::is_music_type(heap, value, "post-event") {
                    Id::Event
                } else {
                    Id::Music
                }
            }
            _ if heap.foreign::<PitchValue>(value).is_some() => Id::Pitch,
            _ if heap.foreign::<lily_engine_music::Duration>(value).is_some() => Id::Duration,
            Some("module") => Id::Module,
            _ if heap.foreign::<lily_engine_music::Book>(value).is_some() => Id::Book,
            _ => Id::Scm,
        },
        Tag::Pair => {
            let first = heap.car(value).unwrap_or(Value::FALSE);
            match heap.car(first) {
                Some(key) if is_key(heap, key) => Id::Lookup,
                _ => Id::Scm,
            }
        }
        _ => Id::Scm,
    }
}

/// The scopes of one file, and the values of the tokens lexed from it.
pub struct Env<'v> {
    vm: &'v mut Vm,
    /// `scopes_`, the file's module first and the innermost last.
    scopes: Vec<Value>,
    /// The values identifier tokens carry, rooted until [`Env::release`],
    /// and their roots.
    values: Vec<Value>,
    roots: Vec<Root>,
    /// The values before this one are released.
    released: usize,
    /// The forms `#` and `$` read, their data among `values`.
    pub(crate) forms: Vec<Read>,
    /// The root stack's depth before them.
    depth: usize,
    /// `ly:music?` and `ly:event?`, which classify a music function.
    predicates: (Value, Value),
    /// What makes music, of `(lily)`'s `%music-properties`.
    pub(crate) maker: Maker,
    /// The files `\include` reads, if any.
    pub(crate) include: Option<Include<'v>>,
    /// A template's: whether `\include` reads the VM's files
    /// (`Vm::host_file`), as the parse running it reads the host's (MS6
    /// step 3, OPEN F).
    vm_files: bool,
    /// Its [`LilyParser`], `(lily)`'s `%parser` while it lives, and the one
    /// that was before it (`embedded.rs`).
    pub(crate) parser: Value,
    previous_parser: Value,
    /// A template's (`ly:parser-clone`'s): the closures of its forms by
    /// byte, taken in the file `closures_file`; the forms read so, whose
    /// data is a closure; and the location its music is placed at.
    closures: Value,
    closures_file: u16,
    thunks: Vec<u32>,
    pub(crate) override_input: Option<Value>,
}

/// What the host has of a file by its name (`source::include_candidates`).
pub type Include<'v> = &'v mut dyn FnMut(&str) -> Hosted<Vec<u8>>;

impl<'v> Env<'v> {
    /// A file's scopes: a module that uses `(guile)` and `(lily)`, as
    /// `ly_make_module` makes one, and `(lily curried-definitions)`, as
    /// `init.ly`'s first line has it use, and the init files' modules
    /// (`session::replay`); with what `init.ly` defines before the file is
    /// read that the parser and the book handlers read, and its
    /// `(use-modules (srfi srfi-1))` (its `(lily clip-region)` and `(ice-9
    /// pretty-print)` are not the engine's yet). That is used before the
    /// init files' modules, which are searched first: their variables are
    /// the file's own in LilyPond, and `\break` is not srfi-1's `break`.
    pub fn new(vm: &'v mut Vm) -> Env<'v> {
        let mut env = Env::bare(vm);
        let module = env.module();
        let previous = env.vm.set_current_module(module);
        let _ = env.vm.eval_text("(use-modules (srfi srfi-1))", None);
        env.vm.set_current_module(previous);
        crate::session::replay(env.vm, module);
        for name in ["toplevel-scores", "toplevel-bookparts"] {
            env.define(name, Value::NULL);
        }
        for name in ["$defaultheader", "$current-book", "$current-bookpart"] {
            env.define(name, Value::FALSE);
        }
        env
    }

    /// The scopes of an init file: a module like a file's, without the
    /// session's variables, which are what init files define.
    pub fn bare(vm: &'v mut Vm) -> Env<'v> {
        let module = vm
            .make_module(&["lily", "lily curried-definitions"])
            .unwrap_or(Value::FALSE);
        let depth = vm.heap().root_depth();
        // Rooted while the parse lasts: since K2, a module is collected
        // when nothing reaches it.
        vm.heap_mut().root(module);
        let parser = vm.heap_mut().wrap(LilyParser::new(alloc::vec![module]));
        Env::with_parser(vm, depth, alloc::vec![module], parser)
    }

    /// The scopes of a file parsed again from a boundary of its last parse
    /// (02 §5.7, `reuse.rs`): `module`, which [`Env::new`] made for it and
    /// the items before the boundary defined in.
    pub(crate) fn resume(vm: &'v mut Vm, module: Value) -> Env<'v> {
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(module);
        let parser = vm.heap_mut().wrap(LilyParser::new(alloc::vec![module]));
        Env::with_parser(vm, depth, alloc::vec![module], parser)
    }

    /// The scopes of a template's parse (`ly:parse-string-expression`):
    /// those of `clone`, a [`LilyParser`] that `ly:parser-clone` made, its
    /// closures taken in the file `file`, its music placed at its location.
    pub(crate) fn cloned(vm: &'v mut Vm, clone: Value, file: u16) -> Env<'v> {
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(clone);
        let (scopes, closures, location) = match vm.heap().foreign::<LilyParser>(clone) {
            Some(p) => (p.scopes.clone(), p.closures, p.location),
            None => (Vec::new(), Value::NULL, Value::FALSE),
        };
        let mut env = Env::with_parser(vm, depth, scopes, clone);
        env.closures = closures;
        env.closures_file = file;
        env.override_input = (location != Value::FALSE).then_some(location);
        env.vm_files = env.vm.has_file_source();
        env
    }

    /// An Env of `scopes`, with `parser` its [`LilyParser`], rooted, made
    /// `%parser`, and `%host-call`'s host the parser's.
    fn with_parser(vm: &'v mut Vm, depth: usize, scopes: Vec<Value>, parser: Value) -> Env<'v> {
        vm.heap_mut().root(parser);
        vm.set_host(Some(crate::embedded::host));
        let previous_parser = crate::embedded::install(vm, parser);
        let module = scopes.first().copied().unwrap_or(Value::FALSE);
        let mut predicate = |name| vm.module_value(module, name).unwrap_or(Value::FALSE);
        let predicates = (predicate("ly:music?"), predicate("ly:event?"));
        let maker = Maker::new(predicate("%music-properties"));
        Env {
            vm,
            scopes,
            values: Vec::new(),
            roots: Vec::new(),
            released: 0,
            forms: Vec::new(),
            depth,
            predicates,
            maker,
            include: None,
            vm_files: false,
            parser,
            previous_parser,
            closures: Value::NULL,
            closures_file: 0,
            thunks: Vec::new(),
            override_input: None,
        }
    }

    /// Its [`LilyParser`], which Scheme sees as `%parser`.
    pub(crate) fn lily_parser(&mut self) -> Option<&mut LilyParser> {
        self.vm.heap_mut().foreign_mut::<LilyParser>(self.parser)
    }

    /// The tokens of the templates the Scheme it ran parsed, indented, to
    /// follow its own; none if it records none.
    pub(crate) fn template_tokens(&mut self) -> Vec<String> {
        self.lily_parser()
            .map(|p| core::mem::take(&mut p.tokens))
            .unwrap_or_default()
    }

    /// The closure of a template's form read at the byte `at` of `file`,
    /// which is called in its place (`internal_parse_embedded_scheme`):
    /// only in the template's own text, the lexer's top input.
    pub(crate) fn closure(&self, file: u16, at: usize) -> Option<Value> {
        if self.closures == Value::NULL || file != self.closures_file {
            return None;
        }
        crate::embedded::closure_at(self.vm.heap(), self.closures, at)
    }

    /// Marks the form `form` as a closure's, which is called, not
    /// evaluated.
    pub(crate) fn mark_thunk(&mut self, form: u32) {
        self.thunks.push(form);
    }

    /// Whether the form `form` is a closure's.
    pub(crate) fn is_thunk(&self, form: u32) -> bool {
        self.thunks.contains(&form)
    }

    /// Whether `\include` reads files: a file's parse given them, or a
    /// template's when the VM has them.
    fn has_files(&self) -> bool {
        self.include.is_some() || self.vm_files
    }

    /// The file's module, where its assignments define.
    pub fn module(&self) -> Value {
        self.scopes[0]
    }

    pub fn vm(&mut self) -> &mut Vm {
        self.vm
    }

    pub(crate) fn vm_ref(&self) -> &Vm {
        self.vm
    }

    /// `eval_at` of the form at `form` in [`Env::forms`].
    pub(crate) fn eval_kept(
        &mut self,
        form: u32,
        source: &lily_engine_scheme::Source,
        location: Value,
    ) -> Result<Value, Exception> {
        self.vm
            .eval_at(&self.forms[form as usize], source, location)
    }

    /// `scan_scm_id`'s kind of `value`.
    pub fn classify(&self, value: Value) -> Id {
        classify(self.vm.heap(), self.predicates, value)
    }

    /// `try_special_identifiers`' copies: of a context modification, which
    /// then adds to its own mods; any other value as it is. A context
    /// definition, which LilyPond clones, is immutable (N3), and music is
    /// copied where the parser places it.
    pub(crate) fn special_copy(&mut self, value: Value) -> Value {
        lily_engine_music::copy_context_mod(self.vm.heap_mut(), value).unwrap_or(value)
    }

    /// A file name `\\include` takes, a string's text; else what `display`
    /// writes of `value`, None for no value.
    fn include_value(&self, value: Option<Value>) -> Result<String, Option<String>> {
        let value = value.ok_or(None)?;
        let heap = self.vm.heap();
        if let Some(text) = heap.str(value) {
            return Ok(String::from(text.as_str()));
        }
        let mut shown = String::new();
        heap.display(value, &mut shown);
        Err(Some(shown))
    }

    /// The value of a token that [`Scopes::classify`] gave `index`.
    pub fn value(&self, index: u32) -> Value {
        self.values[index as usize]
    }

    /// Keeps `value` for a token, rooted, and gives its index.
    pub(crate) fn keep(&mut self, value: Value) -> u32 {
        self.roots.push(self.vm.heap_mut().root(value));
        self.values.push(value);
        (self.values.len() - 1) as u32
    }

    /// How many values were kept: what [`Env::release`] takes.
    pub(crate) fn kept(&self) -> usize {
        self.values.len()
    }

    /// Unroots the values kept before the `mark`th, which no token to come
    /// carries: the parser releases them after each toplevel item (K2), so
    /// that a long file does not keep every value it made. Their indexes
    /// give `#f` after.
    pub(crate) fn release(&mut self, mark: usize) {
        let mark = mark.min(self.values.len());
        for i in self.released..mark {
            self.vm.heap_mut().set_rooted(self.roots[i], Value::FALSE);
            self.values[i] = Value::FALSE;
        }
        self.released = self.released.max(mark);
    }

    /// `lookup_identifier_symbol`: `name`'s value in the innermost scope
    /// that has it.
    pub fn lookup(&mut self, name: &str) -> Option<Value> {
        let scopes = &self.scopes;
        scopes
            .iter()
            .rev()
            .find_map(|&scope| self.vm.module_value(scope, name))
    }

    /// What `name` is bound to in the alist the variable `table` holds,
    /// as `alist_to_hashq_table` finds it: the first entry of its symbol.
    fn table_entry(&mut self, table: &str, name: &str) -> Option<Value> {
        let mut list = self.lookup(table)?;
        let heap = self.vm.heap_mut();
        let key = heap.intern(name);
        while let (Some(entry), Some(rest)) = (heap.car(list), heap.cdr(list)) {
            if heap.car(entry) == Some(key) {
                return heap.cdr(entry);
            }
            list = rest;
        }
        None
    }

    /// The innermost scope, where assignments define.
    pub fn innermost(&self) -> Value {
        self.scopes[self.scopes.len() - 1]
    }

    /// `ly_make_module`: a module of no name that uses `(guile)` and
    /// `(lily)`, kept as a value is ([`Env::keep`]).
    pub(crate) fn make_module(&mut self) -> Value {
        let module = self.vm.make_module(&["lily"]).unwrap_or(Value::FALSE);
        self.keep(module);
        module
    }

    /// `add_scope`: `module` uses the scopes there are, the innermost
    /// searched first, and becomes the innermost, where assignments define
    /// and forms are evaluated.
    pub(crate) fn add_scope(&mut self, module: Value) {
        for i in 0..self.scopes.len() {
            let scope = self.scopes[i];
            self.vm.module_use(module, scope);
        }
        self.scopes.push(module);
        self.publish_scopes();
    }

    /// `remove_scope`: the innermost scope, removed. The file's module is
    /// never removed.
    pub(crate) fn remove_scope(&mut self) -> Value {
        let removed = match self.scopes.len() {
            1 => self.scopes[0],
            _ => self.scopes.pop().unwrap_or(Value::FALSE),
        };
        self.publish_scopes();
        removed
    }

    /// Its scopes, for `ly:parser-clone`.
    fn publish_scopes(&mut self) {
        let scopes = self.scopes.clone();
        if let Some(p) = self.lily_parser() {
            p.scopes = scopes;
        }
    }

    /// `set_identifier`'s `scm_module_define` into the innermost scope.
    pub(crate) fn define(&mut self, name: &str, value: Value) {
        let scope = self.innermost();
        self.vm.module_define(scope, name, value);
    }
}

impl Scopes for Env<'_> {
    fn define_string(&mut self, name: &str, value: &str) {
        let value = self.vm.heap_mut().string(value);
        self.define(name, value);
    }

    fn template_tokens(&mut self) -> Vec<String> {
        Env::template_tokens(self)
    }

    fn classify(&mut self, name: &str) -> Option<(Id, u32)> {
        let value = self.lookup(name)?;
        let value = self.special_copy(value);
        Some((Env::classify(self, value), self.keep(value)))
    }

    fn parameters(&mut self, index: u32) -> Option<Vec<bool>> {
        crate::function::optional_params(self.vm.heap(), self.value(index))
    }

    fn markup_command(&mut self, name: &str, diags: &mut Diagnostics) -> Option<(Id, u32)> {
        crate::markup::lookup_command(self, name, diags)
    }

    fn markup_function(&mut self, index: u32) -> Option<Id> {
        crate::markup::command_kind(self, self.value(index))
    }

    fn markup_expects(&mut self, index: u32) -> Vec<&'static str> {
        let value = self.value(index);
        crate::markup::signature(self, value)
            .iter()
            .rev()
            .map(|expect| expect.display())
            .collect()
    }

    fn included_strings(&mut self) -> Vec<String> {
        crate::scheme::take_included(self.vm)
    }

    fn include_file(&mut self, name: &str) -> Option<Hosted<Vec<u8>>> {
        match &mut self.include {
            Some(include) => Some(include(name)),
            None if self.vm_files => self.vm.host_file(name),
            None => None,
        }
    }

    fn include_name(&mut self, name: &str) -> Option<Result<String, String>> {
        self.has_files().then_some(())?;
        let value = self.lookup(name);
        Some(
            self.include_value(value)
                .map_err(|shown| shown.unwrap_or_else(|| String::from("#<undefined>"))),
        )
    }

    fn include_form(
        &mut self,
        src: &Source,
        file: u16,
        form: Option<u32>,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<Result<String, Option<String>>> {
        self.has_files().then_some(())?;
        let value = form.and_then(|form| self.eval_form(src, file, form, at, diags));
        Some(self.include_value(value))
    }

    fn waiting(&mut self) -> bool {
        self.vm.waiting()
    }

    fn chord_modifier(&mut self, name: &str) -> Option<u32> {
        let value = self.table_entry("chordmodifiers", name)?;
        Some(self.keep(value))
    }

    fn drum_pitch(&mut self, name: &str) -> Option<u32> {
        let value = self.table_entry("drumPitchNames", name)?;
        (value.tag() == Tag::Symbol).then(|| self.keep(value))
    }

    fn read(
        &mut self,
        src: &Source,
        file: u16,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<(Option<u32>, usize)> {
        Some(self.read_form(src, file, at, diags))
    }

    fn immediate(
        &mut self,
        src: &Source,
        file: u16,
        form: Option<u32>,
        at: usize,
        diags: &mut Diagnostics,
    ) -> Option<Vec<Option<(Id, u32)>>> {
        self.immediate_values(src, file, form, at, diags)
    }
}

impl Drop for Env<'_> {
    fn drop(&mut self) {
        crate::embedded::install(self.vm, self.previous_parser);
        self.vm.heap_mut().unroot_to(self.depth);
    }
}
