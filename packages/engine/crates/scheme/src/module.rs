//! Globals and modules (chapter 03 §5.4, R6, R10).
//!
//! A global is a slot: a value, a version that every `define` and `set!`
//! increments, the module it belongs to, and flags. Compiled code names a
//! slot by its index, so reading a global is one load from [`Globals::values`]
//! (R10's versions and owners are beside it, not in the way).
//!
//! A module is a table from symbols to slots, the interfaces it uses, and its
//! public interface, as Guile 3's are. The compiler resolves a toplevel
//! variable to the slot of its symbol in the module being compiled, which it
//! makes if there is none: a *cell* that belongs to the module's table but is
//! not one of its variables until something defines it ([`DEFINED`]). The
//! first time a cell that nothing defined is read or set, the VM looks the
//! symbol up among the module's imports and points the instruction at the
//! slot it finds. That is what Guile's evaluator does when it memoizes a
//! toplevel reference on its first run: a procedure that has not run yet sees
//! a later local `define` of an imported name, one that has run keeps the
//! imported variable. Imports are searched from the last used to the first,
//! which is Guile's default duplicate handling (`last`, and core bindings
//! overridden).
//!
//! The modules every VM has: `(guile)`, which holds the primitives and the
//! preludes and exports all of them; `(guile-user)`, the current module at
//! the start, as in Guile; `(lily)`, with `%location`, the fluid the host
//! sets around each `#` form, and `*location*`. A module whose code the host
//! provides as Scheme text ([`crate::Vm::provide_module`]) is loaded when it
//! is first used, as Guile autoloads a module from its file.
//!
//! R10's read tracking is built in and off: while it is on, the first read of
//! a slot sets its [`READ`] flag and records it with its version.
//!
//! R11's half for the slots (MS6 step 2): while an item runs
//! (`Vm::begin_item`), the first `define`, `set!` or `export` of each slot
//! in it logs the slot's value and flags, which [`Globals::undo`] gives
//! back, the latest first, with a version the slot never had, so that no
//! record of a later one takes the value for its own. The slots the log
//! names, and the values it holds, are kept by the collector
//! (`Vm::collect`).
//!
//! The redo half (MS6 step 3): [`Globals::undo_keeping`] keeps each slot's
//! value and flags at the end of the item it undoes, which
//! [`Globals::redo`] gives back in a later item, logged, with a new version.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::reader::read;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown};

/// The slot is a variable of its module: something defined it, or `export`
/// made it. A cell without it only stands for a reference.
pub(crate) const DEFINED: u8 = 1;
/// The slot was read while read tracking was on (R10).
pub(crate) const READ: u8 = 2;

/// The modules every VM makes, by their index.
pub(crate) const GUILE: u32 = 0;
pub(crate) const GUILE_USER: u32 = 1;
pub(crate) const LILY: u32 = 2;

/// A module as a Scheme value: what `current-module` returns and `eval`
/// takes. Its tables are in [`Globals`], at `index`.
pub(crate) struct ModuleObject {
    pub index: u32,
    /// Its name, a list of symbols.
    pub name: Value,
    kind: &'static str,
}

impl Foreign for ModuleObject {
    fn type_name(&self) -> &'static str {
        "module"
    }

    /// `#<directory (guile-user)>`, without the address Guile prints.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<");
        out.push_str(self.kind);
        out.push(' ');
        heap.write(self.name, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.name);
        tracer.module(self.index);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.u32(self.index);
        out.value(self.name);
        out.bool(self.kind == "module");
        true
    }
}

pub(crate) fn load_module_object(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(ModuleObject {
        index: r.u32()?,
        name: r.value()?,
        kind: if r.bool()? { "module" } else { "directory" },
    }))
}

fn save_names(w: &mut ImageWriter, names: &BTreeMap<u32, u32>) {
    w.count(names.len());
    for (&key, &slot) in names {
        w.u32(key);
        w.u32(slot);
    }
}

/// Read in a plain loop, as the image's readers are (D90), then built at
/// once from the pairs, which are written in their keys' order (MS3 step
/// 56).
fn load_names(r: &mut ImageReader<'_>) -> Option<BTreeMap<u32, u32>> {
    let n = r.count()?;
    let mut names = alloc::vec![(0, 0); n];
    let mut i = 0;
    while i < n {
        names[i] = (r.u32()?, r.u32()?);
        i += 1;
    }
    Some(names.into_iter().collect())
}

fn save_u32s(w: &mut ImageWriter, items: &[u32]) {
    w.count(items.len());
    for &n in items {
        w.u32(n);
    }
}

/// Read in a plain loop, as the image's readers are (D90, MS3 step 56).
fn load_u32s(r: &mut ImageReader<'_>) -> Option<Vec<u32>> {
    let n = r.count()?;
    let mut items = alloc::vec![0; n];
    let mut i = 0;
    while i < n {
        items[i] = r.u32()?;
        i += 1;
    }
    Some(items)
}

/// An interface a module uses.
pub(crate) enum Use {
    /// A module's public interface, as it is when a name is looked up.
    Module(u32),
    /// `#:select`, `#:prefix` or `#:hide`: the names it gives and their
    /// slots, as they were when the module was used.
    Custom(BTreeMap<u32, u32>),
}

pub(crate) struct Module {
    /// Its name's symbols, by their index.
    path: Vec<u32>,
    pub object: Value,
    /// Symbols to slots: its variables, and the cells of its references.
    table: BTreeMap<u32, u32>,
    uses: Vec<Use>,
    /// What it exports, by the name it exports it as; `None` until it has a
    /// public interface (`define-module`), which `use-modules` needs.
    public: Option<BTreeMap<u32, u32>>,
    /// `(guile)`: every variable is public.
    all_public: bool,
    /// Made by [`Globals::anonymous`], which no name finds: collected when
    /// nothing reaches it ([`Globals::keep`]).
    anonymous: bool,
}

/// An identifier a macro's expansion introduced (`expand.rs`): an
/// uninterned symbol that stands for `name` as the macro's definition sees
/// it, in `module` and the first `depth` lexical bindings of the expander's
/// scope there (0 for a macro defined at toplevel).
pub(crate) struct Alias {
    pub name: Value,
    pub module: u32,
    pub depth: u32,
}

/// [`Globals::named`]'s settings.
pub(crate) const NAMED_NONE: u8 = 0;
pub(crate) const NAMED_TOPLEVEL: u8 = 1;
pub(crate) const NAMED_ALL: u8 = 2;

/// The global slots and the modules.
#[derive(Default)]
pub(crate) struct Globals {
    /// The slots' values, `Value::UNDEFINED` when unbound: what compiled
    /// code reads.
    pub values: Vec<Value>,
    pub versions: Vec<u32>,
    /// The last version given ([`Globals::bump`]).
    last_version: u32,
    /// The module each slot belongs to.
    pub owners: Vec<u32>,
    /// Each slot's symbol.
    pub names: Vec<Value>,
    pub flags: Vec<u8>,
    /// Whether reads are tracked (R10).
    pub tracking: bool,
    /// Which procedures the compiler keeps the formals of to print
    /// ([`crate::compile::Template::names`]): while LilyPond's own Scheme
    /// loads, which Guile would have compiled (`%named-formals`, OPEN C6,
    /// D274): [`NAMED_NONE`], [`NAMED_TOPLEVEL`] (those a toplevel form
    /// defines, while the image is made) or [`NAMED_ALL`] (every named one,
    /// while a part loads).
    pub named: u8,
    /// The slots read since the host last took them, while tracking, each
    /// with its version at the first read.
    pub reads: Vec<(u32, u32)>,
    /// Whether changes of slots are logged (R11).
    pub logging: bool,
    /// Each slot's value and flags before its first change in an item.
    pub log: Vec<(u32, Value, u8)>,
    /// The changes made with logging on, each counted ([`Globals::written`]).
    written: u64,
    /// The slots the current item logged.
    logged: BTreeSet<u32>,
    /// Each slot's value, flags and version at the end of an item undone
    /// by [`Globals::undo_keeping`], oldest first.
    pub redo: Vec<(u32, Value, u8, u32)>,
    pub modules: Vec<Module>,
    /// The fluid that holds the current module.
    pub current: Value,
    /// Modules not loaded yet, by their name's symbols, and their code.
    sources: Vec<(Vec<u32>, String)>,
    /// The expander's aliases, by their symbol's index.
    pub aliases: BTreeMap<u32, Alias>,
    /// The slots and the anonymous modules a collection freed, for
    /// [`Globals::cell`] and [`Globals::anonymous`] to use again. A freed
    /// slot has no owner ([`FREE`]), a freed module no object.
    free_slots: Vec<u32>,
    free_modules: Vec<u32>,
}

/// The owner of a slot that belongs to no module.
const FREE: u32 = u32::MAX;

impl Globals {
    /// The slots and the three modules every VM has, `(guile-user)` current.
    pub(crate) fn new(heap: &mut Heap) -> Globals {
        let mut globals = Globals::default();
        for (name, kind) in [
            ("guile", "module"),
            ("guile-user", "directory"),
            ("lily", "directory"),
        ] {
            let path = [heap.intern(name).payload()];
            let index = globals.make(heap, &path, kind);
            globals.modules[index as usize].public = Some(BTreeMap::new());
            if index != GUILE {
                globals.modules[index as usize]
                    .uses
                    .push(Use::Module(GUILE));
            }
        }
        globals.modules[GUILE as usize].all_public = true;
        let user = globals.modules[GUILE_USER as usize].object;
        globals.current = heap.wrap(crate::control::Fluid { value: user });
        globals
    }

    // --- slots

    /// The slot of `symbol` in `module`, a cell that is not one of its
    /// variables if it has none.
    pub(crate) fn cell(&mut self, module: u32, symbol: Value) -> u32 {
        let key = symbol.payload();
        if let Some(&slot) = self.modules[module as usize].table.get(&key) {
            return slot;
        }
        let slot = match self.free_slots.pop() {
            // Its version goes on counting, so that no record of an earlier
            // version takes it for its old variable.
            Some(slot) => {
                let at = slot as usize;
                self.values[at] = Value::UNDEFINED;
                self.owners[at] = module;
                self.names[at] = symbol;
                self.flags[at] = 0;
                slot
            }
            None => {
                self.values.push(Value::UNDEFINED);
                self.versions.push(0);
                self.owners.push(module);
                self.names.push(symbol);
                self.flags.push(0);
                self.values.len() as u32 - 1
            }
        };
        self.modules[module as usize].table.insert(key, slot);
        slot
    }

    /// `define`: the slot becomes a variable of its module, with `value`.
    pub(crate) fn define(&mut self, slot: u32, value: Value) {
        self.barrier(slot);
        let at = slot as usize;
        self.values[at] = value;
        self.flags[at] |= DEFINED;
        self.bump(at);
    }

    /// `set!` of a slot that is a variable or an imported one.
    pub(crate) fn set(&mut self, slot: u32, value: Value) {
        self.barrier(slot);
        let at = slot as usize;
        self.values[at] = value;
        self.bump(at);
    }

    /// Whether `slot` is a cell that refers to an import: nothing in its
    /// module defined it.
    pub(crate) fn is_reference(&self, slot: u32) -> bool {
        self.flags[slot as usize] & DEFINED == 0
    }

    /// The bound variable a reference's cell stands for, among its module's
    /// imports.
    pub(crate) fn resolve(&self, slot: u32) -> Option<u32> {
        if !self.is_reference(slot) {
            return None;
        }
        let at = slot as usize;
        self.imported(self.owners[at], self.names[at].payload())
    }

    /// Records a read while tracking is on (R10).
    #[inline]
    pub(crate) fn track(&mut self, slot: u32) {
        if self.tracking {
            let flags = &mut self.flags[slot as usize];
            if *flags & READ == 0 {
                *flags |= READ;
                self.reads.push((slot, self.versions[slot as usize]));
            }
        }
    }

    /// Takes the slots read since the last time with their versions,
    /// clearing their flags.
    pub(crate) fn take_reads(&mut self) -> Vec<(u32, u32)> {
        let reads = core::mem::take(&mut self.reads);
        for &(slot, _) in &reads {
            self.flags[slot as usize] &= !READ;
        }
        reads
    }

    /// How many changes of slots were made with logging on: a library file
    /// or part loading is not among them (R11; N8's snapshots, D258).
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Gives the slot at `at` a version no slot has had: versions are
    /// counted for all slots (MS6 step 3), so a version a redo gives back
    /// is the value's it had, and no other's (D235).
    fn bump(&mut self, at: usize) {
        self.last_version = self.last_version.wrapping_add(1);
        self.versions[at] = self.last_version;
    }

    /// R11's barrier for a slot about to change.
    #[inline]
    fn barrier(&mut self, slot: u32) {
        if self.logging {
            self.written += 1;
            if self.logged.insert(slot) {
                let at = slot as usize;
                self.log.push((slot, self.values[at], self.flags[at]));
            }
        }
    }

    /// Starts logging for an item: each slot is logged again at its first
    /// change in it.
    pub(crate) fn begin_item(&mut self) {
        self.logging = true;
        self.logged.clear();
    }

    /// Gives each slot logged since the log was `mark` long its value and
    /// whether it was a variable back, the latest change first, each with
    /// a new version.
    pub(crate) fn undo(&mut self, mark: usize) {
        if mark >= self.log.len() {
            return;
        }
        for (slot, value, flags) in self.log.split_off(mark).into_iter().rev() {
            let at = slot as usize;
            self.values[at] = value;
            self.flags[at] = (self.flags[at] & !DEFINED) | (flags & DEFINED);
            self.bump(at);
        }
        self.logged.clear();
    }

    /// [`Globals::undo`], keeping each slot's value and flags as they are
    /// now: the range of [`Globals::redo`]'s store.
    pub(crate) fn undo_keeping(&mut self, mark: usize) -> core::ops::Range<usize> {
        let start = self.redo.len();
        if mark < self.log.len() {
            let mut kept: Vec<(u32, Value, u8, u32)> = self.log[mark..]
                .iter()
                .rev()
                .map(|&(slot, _, _)| {
                    let at = slot as usize;
                    (slot, self.values[at], self.flags[at], self.versions[at])
                })
                .collect();
            kept.reverse();
            self.redo.extend(kept);
            self.undo(mark);
        }
        start..self.redo.len()
    }

    /// Gives each slot of `range` of the redo store its value, whether it
    /// was a variable and its version at the end of the item it was kept
    /// of, the oldest first, logged as the current item's change: a read
    /// of it then has the version it had, which no other value has had.
    pub(crate) fn redo(&mut self, range: core::ops::Range<usize>) {
        for i in range {
            let Some(&(slot, value, flags, version)) = self.redo.get(i) else {
                break;
            };
            self.barrier(slot);
            let at = slot as usize;
            self.values[at] = value;
            self.flags[at] = (self.flags[at] & !DEFINED) | (flags & DEFINED);
            self.versions[at] = version;
        }
    }

    // --- lookups

    /// The slot of `symbol` in `module` if the module itself defines it.
    pub(crate) fn local_slot(&self, module: u32, symbol: Value) -> Option<u32> {
        self.local(module, symbol.payload())
    }

    /// The slot a toplevel reference to `symbol` in `module` names: the
    /// module's own, or, for an alias that the module does not define
    /// itself, its name's in the macro's module.
    pub(crate) fn reference(&mut self, module: u32, symbol: Value) -> u32 {
        let (mut module, mut symbol) = (module, symbol);
        while self.local_slot(module, symbol).is_none()
            && let Some(alias) = self.aliases.get(&symbol.payload())
        {
            (module, symbol) = (alias.module, alias.name);
        }
        self.cell(module, symbol)
    }

    /// `module-use!`: `module` uses `used` too, searched before the modules
    /// it used already; nothing changes if it uses `used` or is `used`.
    pub(crate) fn use_module(&mut self, module: u32, used: u32) {
        let uses = &mut self.modules[module as usize].uses;
        if module != used
            && !uses
                .iter()
                .any(|u| matches!(u, Use::Module(m) if *m == used))
        {
            uses.push(Use::Module(used));
        }
    }

    /// The variables `module` binds itself, as symbols and their values.
    pub(crate) fn locals(&self, module: u32) -> Vec<(Value, Value)> {
        self.modules[module as usize]
            .table
            .iter()
            .filter(|&(_, &slot)| !self.is_reference(slot))
            .map(|(&key, &slot)| (Value::tagged(Tag::Symbol, key), self.values[slot as usize]))
            .filter(|&(_, value)| value != Value::UNDEFINED)
            .collect()
    }

    /// The variable `symbol` names in `module` itself.
    fn local(&self, module: u32, key: u32) -> Option<u32> {
        let slot = *self.modules[module as usize].table.get(&key)?;
        (!self.is_reference(slot)).then_some(slot)
    }

    /// What `module` exports as `key`.
    fn exported(&self, module: u32, key: u32) -> Option<u32> {
        let m = &self.modules[module as usize];
        if m.all_public {
            return self.local(module, key);
        }
        m.public.as_ref()?.get(&key).copied()
    }

    /// The bound variable `key` names among `module`'s imports, the last
    /// used first.
    fn imported(&self, module: u32, key: u32) -> Option<u32> {
        self.modules[module as usize]
            .uses
            .iter()
            .rev()
            .filter_map(|u| match u {
                Use::Module(m) => self.exported(*m, key),
                Use::Custom(names) => names.get(&key).copied(),
            })
            .find(|&slot| self.values[slot as usize] != Value::UNDEFINED)
    }

    /// The variable `symbol` names in `module`: its own, else an import.
    pub(crate) fn lookup(&self, module: u32, symbol: Value) -> Option<u32> {
        let key = symbol.payload();
        self.local(module, key)
            .or_else(|| self.imported(module, key))
    }

    /// `(@ module symbol)`'s variable, or `(@@ module symbol)`'s.
    pub(crate) fn at(&self, module: u32, symbol: Value, public: bool) -> Option<u32> {
        if public {
            self.exported(module, symbol.payload())
        } else {
            self.lookup(module, symbol)
        }
    }

    // --- modules

    fn make(&mut self, heap: &mut Heap, path: &[u32], kind: &'static str) -> u32 {
        let index = self.modules.len() as u32;
        let symbols: Vec<Value> = path
            .iter()
            .map(|&s| Value::tagged(Tag::Symbol, s))
            .collect();
        let name = heap.list(&symbols);
        let object = heap.wrap(ModuleObject { index, name, kind });
        self.modules.push(Module {
            path: path.into(),
            object,
            table: BTreeMap::new(),
            uses: Vec::new(),
            public: None,
            all_public: false,
            anonymous: false,
        });
        index
    }

    /// A module of the crate's own, made with every VM, named by `path`: it
    /// uses `(guile)` and exports every variable it defines.
    pub(crate) fn builtin(&mut self, heap: &mut Heap, path: &[&str]) -> u32 {
        let path: Vec<u32> = path.iter().map(|s| heap.intern(s).payload()).collect();
        let index = self.make(heap, &path, "directory");
        let m = &mut self.modules[index as usize];
        m.public = Some(BTreeMap::new());
        m.all_public = true;
        m.uses.push(Use::Module(GUILE));
        index
    }

    /// A module of no name, as Guile's `make-module` makes one, that
    /// exports every variable it defines and uses `uses`, the last searched
    /// first. Its path is an uninterned symbol, which no name finds.
    /// A module a collection freed is made again, with its name.
    pub(crate) fn anonymous(&mut self, heap: &mut Heap, uses: &[u32]) -> u32 {
        let index = match self.free_modules.pop() {
            Some(index) => {
                let m = &mut self.modules[index as usize];
                let name = heap.list(&[Value::tagged(Tag::Symbol, m.path[0])]);
                m.object = heap.wrap(ModuleObject {
                    index,
                    name,
                    kind: "module",
                });
                index
            }
            None => {
                let name = heap.uninterned("%anonymous");
                self.make(heap, &[name.payload()], "module")
            }
        };
        let m = &mut self.modules[index as usize];
        m.anonymous = true;
        m.public = Some(BTreeMap::new());
        m.all_public = true;
        m.uses = uses.iter().map(|&u| Use::Module(u)).collect();
        index
    }

    pub(crate) fn find(&self, path: &[u32]) -> Option<u32> {
        self.modules
            .iter()
            .position(|m| m.path == path)
            .map(|i| i as u32)
    }

    /// The module named by the words of `name`, as [`Globals::find`] finds
    /// it: `(lily)` without a search.
    pub(crate) fn named(&self, heap: &mut Heap, name: &str) -> Option<u32> {
        if name == "lily" {
            return Some(LILY);
        }
        let path: Vec<u32> = name
            .split_whitespace()
            .map(|s| heap.intern(s).payload())
            .collect();
        self.find(&path)
    }

    /// The module named `path`, made empty if there is none, as
    /// `resolve-module` makes one.
    pub(crate) fn ensure(&mut self, heap: &mut Heap, path: &[u32]) -> u32 {
        match self.find(path) {
            Some(index) => index,
            None => self.make(heap, path, "directory"),
        }
    }

    /// The module that is current: the compiler's, and `current-module`'s.
    pub(crate) fn current_module(&self, heap: &Heap) -> u32 {
        heap.foreign::<crate::control::Fluid>(self.current)
            .and_then(|f| heap.foreign::<ModuleObject>(f.value))
            .map_or(GUILE_USER, |m| m.index)
    }

    /// Makes `object`, a module, current; the module that was.
    pub(crate) fn set_current(&self, heap: &mut Heap, object: Value) -> Value {
        match heap.foreign_mut::<crate::control::Fluid>(self.current) {
            Some(fluid) => core::mem::replace(&mut fluid.value, object),
            None => Value::FALSE,
        }
    }

    pub(crate) fn object(&self, module: u32) -> Value {
        self.modules[module as usize].object
    }

    /// Exports `slot` from `module` as `as_name`.
    fn publish(&mut self, module: u32, as_name: u32, slot: u32) {
        self.modules[module as usize]
            .public
            .get_or_insert_with(BTreeMap::new)
            .insert(as_name, slot);
    }

    /// `export`: the variable is made if there is none, unbound.
    pub(crate) fn export(&mut self, module: u32, name: Value, as_name: Value) {
        let slot = self.cell(module, name);
        self.barrier(slot);
        self.flags[slot as usize] |= DEFINED;
        self.publish(module, as_name.payload(), slot);
    }

    /// Adds an interface to the end of `module`'s uses, unless it is a
    /// module it already uses, as `module-use-interfaces!` does.
    fn uses(&mut self, module: u32, interface: Use) {
        let uses = &mut self.modules[module as usize].uses;
        if let Use::Module(m) = interface
            && (m == module || uses.iter().any(|u| matches!(u, Use::Module(n) if *n == m)))
        {
            return;
        }
        uses.push(interface);
    }

    /// What the collector must keep of the modules before it marks: the
    /// fluid of the current module, and each module a name finds, with what
    /// it keeps ([`Globals::keep`]). `live` is then, by module, whether it
    /// is kept.
    pub(crate) fn roots(&self, live: &mut Vec<bool>) -> Vec<Value> {
        live.clear();
        live.resize(self.modules.len(), false);
        let named: Vec<u32> = (0..self.modules.len() as u32)
            .filter(|&m| !self.modules[m as usize].anonymous)
            .collect();
        let mut roots = self.keep(live, &named, &[]);
        roots.push(self.current);
        roots
    }

    /// The modules of `modules` and the owners of `slots` are live, and so
    /// is every module a live one uses: those not live yet are now, and
    /// their objects and slots' values are what they keep. An anonymous
    /// module is kept only so: by its object, a macro made in it, code
    /// that names its slots, or a module that uses it.
    pub(crate) fn keep(&self, live: &mut [bool], modules: &[u32], slots: &[u32]) -> Vec<Value> {
        let mut values = Vec::new();
        let mut work: Vec<u32> = modules.to_vec();
        work.extend(
            slots
                .iter()
                .filter_map(|&s| self.owners.get(s as usize).copied()),
        );
        while let Some(m) = work.pop() {
            match live.get_mut(m as usize) {
                Some(seen) if !*seen => *seen = true,
                _ => continue,
            }
            let module = &self.modules[m as usize];
            values.push(module.object);
            let mut slots: Vec<u32> = module.table.values().copied().collect();
            slots.extend(module.public.iter().flat_map(|p| p.values()));
            for u in &module.uses {
                match u {
                    Use::Module(used) => work.push(*used),
                    Use::Custom(names) => slots.extend(names.values()),
                }
            }
            for slot in slots {
                values.push(self.values[slot as usize]);
                work.push(self.owners[slot as usize]);
            }
        }
        values
    }

    /// After a collection: frees each anonymous module `live` does not
    /// keep, with its slots, and forgets the aliases into it. Returns the
    /// modules it freed, whose references the expander forgets too.
    pub(crate) fn free_dead(&mut self, live: &[bool]) -> Vec<u32> {
        let mut freed = Vec::new();
        for (index, m) in self.modules.iter_mut().enumerate() {
            if !m.anonymous || m.object == Value::FALSE || live.get(index) != Some(&false) {
                continue;
            }
            for (_, slot) in core::mem::take(&mut m.table) {
                let at = slot as usize;
                self.values[at] = Value::UNDEFINED;
                self.last_version = self.last_version.wrapping_add(1);
                self.versions[at] = self.last_version;
                self.owners[at] = FREE;
                self.names[at] = Value::FALSE;
                self.flags[at] = 0;
                self.free_slots.push(slot);
            }
            m.object = Value::FALSE;
            m.uses.clear();
            m.public = None;
            freed.push(index as u32);
        }
        if !freed.is_empty() {
            self.aliases
                .retain(|_, alias| !freed.contains(&alias.module));
            self.free_modules.extend(&freed);
        }
        freed
    }

    /// Records the code of a module to load when it is first used.
    pub(crate) fn provide(&mut self, path: Vec<u32>, source: String) {
        self.sources.retain(|(p, _)| *p != path);
        self.sources.push((path, source));
    }

    // --- the image

    /// Writes the slots, the modules with their tables, the modules not
    /// loaded yet and the aliases (`image.rs`).
    pub(crate) fn save(&self, w: &mut ImageWriter) {
        w.values(&self.values);
        save_u32s(w, &self.versions);
        save_u32s(w, &self.owners);
        w.values(&self.names);
        w.bytes(&self.flags);
        w.bool(self.tracking);
        let reads: Vec<u32> = self.reads.iter().map(|&(slot, _)| slot).collect();
        save_u32s(w, &reads);
        w.count(self.modules.len());
        for m in &self.modules {
            save_u32s(w, &m.path);
            w.value(m.object);
            save_names(w, &m.table);
            w.count(m.uses.len());
            for u in &m.uses {
                match u {
                    Use::Module(index) => {
                        w.u8(0);
                        w.u32(*index);
                    }
                    Use::Custom(names) => {
                        w.u8(1);
                        save_names(w, names);
                    }
                }
            }
            match &m.public {
                Some(public) => {
                    w.u8(1);
                    save_names(w, public);
                }
                None => w.u8(0),
            }
            w.bool(m.all_public);
            w.bool(m.anonymous);
        }
        w.value(self.current);
        w.count(self.sources.len());
        for (path, source) in &self.sources {
            save_u32s(w, path);
            w.str(source);
        }
        w.count(self.aliases.len());
        for (&key, alias) in &self.aliases {
            w.u32(key);
            w.value(alias.name);
            w.u32(alias.module);
            w.u32(alias.depth);
        }
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Globals> {
        let mut globals = Globals {
            values: r.values()?,
            versions: load_u32s(r)?,
            owners: load_u32s(r)?,
            names: r.values()?,
            flags: r.bytes()?.into(),
            tracking: r.bool()?,
            ..Globals::default()
        };
        let reads = load_u32s(r)?;
        globals.reads = reads
            .into_iter()
            .map(|slot| Some((slot, *globals.versions.get(slot as usize)?)))
            .collect::<Option<_>>()?;
        for _ in 0..r.count()? {
            let path = load_u32s(r)?;
            let object = r.value()?;
            let table = load_names(r)?;
            let uses = (0..r.count()?)
                .map(|_| match r.u8()? {
                    0 => Some(Use::Module(r.u32()?)),
                    1 => Some(Use::Custom(load_names(r)?)),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            let public = match r.u8()? {
                0 => None,
                1 => Some(load_names(r)?),
                _ => return None,
            };
            globals.modules.push(Module {
                path,
                object,
                table,
                uses,
                public,
                all_public: r.bool()?,
                anonymous: r.bool()?,
            });
        }
        globals.current = r.value()?;
        for _ in 0..r.count()? {
            let path = load_u32s(r)?;
            globals.sources.push((path, r.str()?.into()));
        }
        // In the order they were written, the keys': built at once, not
        // inserted one by one (MS2 step 31).
        let n = r.count()?;
        let mut aliases = Vec::with_capacity(n);
        for _ in 0..n {
            let key = r.u32()?;
            let alias = Alias {
                name: r.value()?,
                module: r.u32()?,
                depth: r.u32()?,
            };
            aliases.push((key, alias));
        }
        globals.aliases = aliases.into_iter().collect();
        globals.free_modules = (0..globals.modules.len() as u32)
            .filter(|&m| globals.modules[m as usize].object == Value::FALSE)
            .collect();
        globals.last_version = globals.versions.iter().copied().max().unwrap_or(0);
        globals.free_slots = (0..globals.owners.len() as u32)
            .filter(|&s| globals.owners[s as usize] == FREE)
            .collect();
        let n = globals.values.len();
        let same = [
            globals.versions.len(),
            globals.owners.len(),
            globals.names.len(),
            globals.flags.len(),
        ];
        same.iter().all(|&m| m == n).then_some(globals)
    }
}

/// A module name's symbols, as `path`: `(foo bar)` as their indices.
fn path_of(heap: &Heap, name: Value) -> Option<Vec<u32>> {
    let mut path = Vec::new();
    let mut at = name;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        if car.tag() != Tag::Symbol {
            return None;
        }
        path.push(car.payload());
        at = cdr;
    }
    (at == Value::NULL).then_some(path)
}

/// The items of a proper list.
fn items(heap: &Heap, list: Value) -> Option<Vec<Value>> {
    let mut out = Vec::new();
    let mut at = list;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        out.push(car);
        at = cdr;
    }
    (at == Value::NULL).then_some(out)
}

// --- primitives ---------------------------------------------------------------------

type R = Result<Value, Thrown>;

impl Cx {
    fn module_arg(&mut self, subr: &str, position: usize, value: Value) -> Result<u32, Thrown> {
        match self.heap.foreign::<ModuleObject>(value) {
            Some(m) => Ok(m.index),
            None => Err(self.wrong_type(subr, position, value, "module")),
        }
    }

    fn symbol_arg(&mut self, subr: &str, position: usize, value: Value) -> Result<Value, Thrown> {
        if value.tag() == Tag::Symbol {
            Ok(value)
        } else {
            Err(self.wrong_type(subr, position, value, "symbol"))
        }
    }

    /// A module's name as its symbols, or the error Guile's `resolve-module`
    /// gives for one that is not a list (its `car`'s).
    fn path_arg(&mut self, name: Value) -> Result<Vec<u32>, Thrown> {
        match path_of(&self.heap, name) {
            Some(path) => Ok(path),
            None => Err(self.wrong_type_compiled("car", 1, name, "pair")),
        }
    }

    /// A `misc-error` with no subr, as `boot-9.scm`'s `error` throws it.
    fn misc(&mut self, message: &str, args: &[Value]) -> Thrown {
        let key = self.heap.intern("misc-error");
        let message = self.heap.string(message);
        let args = self.heap.list(args);
        let args = self.heap.list(&[Value::FALSE, message, args, Value::FALSE]);
        self.error = Some(Exception { key, args });
        Thrown
    }

    /// `define-module`'s error for an option it does not know.
    fn bad_option(&mut self, keyword: Value, argument: Value) -> Thrown {
        let key = self.heap.intern("syntax-error");
        let who = self.heap.intern("define-module");
        let message = self.heap.string("unknown keyword or bad argument");
        let args = self
            .heap
            .list(&[who, message, Value::FALSE, keyword, argument]);
        self.error = Some(Exception { key, args });
        Thrown
    }

    fn current(&self) -> u32 {
        self.globals.current_module(&self.heap)
    }

    /// The interface a `use-modules` spec names: `(foo bar)`, or `((foo
    /// bar) #:select (a (b . c)) #:prefix p: #:hide (d))`.
    fn interface(&mut self, spec: Value) -> Result<Use, Thrown> {
        let head = self.heap.car(spec).unwrap_or(spec);
        let (name, options) = if head.tag() == Tag::Pair {
            (head, self.heap.cdr(spec).unwrap_or(Value::NULL))
        } else {
            (spec, Value::NULL)
        };
        let Some(path) = path_of(&self.heap, name) else {
            let keyword = self.heap.keyword("use-module");
            return Err(self.bad_option(keyword, spec));
        };
        let module = match self.globals.find(&path) {
            Some(m) if self.globals.modules[m as usize].public.is_some() => m,
            _ => return Err(self.misc("no code for module ~S", &[name])),
        };
        let options = items(&self.heap, options).unwrap_or_default();
        if options.is_empty() {
            return Ok(Use::Module(module));
        }
        let m = &self.globals.modules[module as usize];
        let mut names: BTreeMap<u32, u32> = if m.all_public {
            m.table
                .iter()
                .filter(|&(_, &slot)| !self.globals.is_reference(slot))
                .map(|(&k, &s)| (k, s))
                .collect()
        } else {
            m.public.clone().unwrap_or_default()
        };
        let mut prefix = None;
        for pair in options.chunks(2) {
            let [option, argument] = *pair else {
                return Err(self.bad_option(pair[0], Value::NULL));
            };
            match self
                .heap
                .name(option)
                .filter(|_| option.tag() == Tag::Keyword)
            {
                Some("select") => {
                    let mut selected = BTreeMap::new();
                    for item in items(&self.heap, argument).unwrap_or_default() {
                        let (from, to) = match (self.heap.car(item), self.heap.cdr(item)) {
                            (Some(from), Some(to)) => (from, to),
                            _ => (item, item),
                        };
                        match names.get(&from.payload()) {
                            Some(&slot) => {
                                selected.insert(to.payload(), slot);
                            }
                            None => {
                                return Err(
                                    self.misc("no binding `~A' in module ~A", &[from, name])
                                );
                            }
                        }
                    }
                    names = selected;
                }
                Some("hide") => {
                    for item in items(&self.heap, argument).unwrap_or_default() {
                        names.remove(&item.payload());
                    }
                }
                Some("prefix") => prefix = Some(argument),
                Some("version") => {}
                _ => return Err(self.bad_option(option, argument)),
            }
        }
        if let Some(prefix) = prefix {
            let prefix = String::from(self.heap.name(prefix).unwrap_or(""));
            names = names
                .into_iter()
                .map(|(key, slot)| {
                    let name = Value::tagged(Tag::Symbol, key);
                    let full = alloc::format!("{prefix}{}", self.heap.name(name).unwrap_or(""));
                    (self.heap.intern(&full).payload(), slot)
                })
                .collect();
        }
        Ok(Use::Custom(names))
    }
}

pub(crate) fn current_module(cx: &mut Cx, _: &[Value]) -> R {
    Ok(cx.globals.object(cx.current()))
}

/// `(set-current-module module)`: the module that was current.
pub(crate) fn set_current_module(cx: &mut Cx, args: &[Value]) -> R {
    cx.module_arg("set-current-module", 1, args[0])?;
    Ok(cx.globals.set_current(&mut cx.heap, args[0]))
}

pub(crate) fn is_module(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap.foreign::<ModuleObject>(args[0]).is_some(),
    ))
}

pub(crate) fn module_name(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<ModuleObject>(args[0]) {
        Some(m) => Ok(m.name),
        None => Err(cx.wrong_type("module-name", 1, args[0], "module")),
    }
}

/// `(%module-local-names module)`: the names `module` binds itself, for
/// `module-map` (OPEN C3's scores: `\enablePerStaffTiming`).
pub(crate) fn module_local_names(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-map", 2, args[0])?;
    let names: alloc::vec::Vec<Value> = cx
        .globals
        .locals(module)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    Ok(cx.heap.list(&names))
}

/// `(ly:module->alist mod)` (`ly-module.cc:54–72`): the variables `mod`
/// binds itself, with their values, as a hash fold of its table conses
/// them; the order is the engine's table's, as LilyPond's is Guile's.
pub(crate) fn module_to_alist(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("ly:module->alist", 1, args[0])?;
    let mut alist = Value::NULL;
    for (key, value) in cx.globals.locals(module) {
        let entry = cx.heap.cons(key, value);
        alist = cx.heap.cons(entry, alist);
    }
    Ok(alist)
}

/// `(ly:modules-lookup modules sym [def])` (`module-scheme.cc:53–71`):
/// the value `sym` has in the first module of the list `modules` that
/// binds it, its own or imported (`scm_module_variable`), else `def`, else
/// `#f`.
pub(crate) fn modules_lookup(cx: &mut Cx, args: &[Value]) -> R {
    let mut list = args[0];
    while let (Some(m), Some(rest)) = (cx.heap.car(list), cx.heap.cdr(list)) {
        let module = cx.module_arg("module-variable", 1, m)?;
        let symbol = cx.symbol_arg("module-variable", 2, args[1])?;
        let value = cx
            .globals
            .lookup(module, symbol)
            .map(|slot| cx.globals.values[slot as usize])
            .filter(|&v| v != Value::UNDEFINED);
        if let Some(value) = value {
            return Ok(value);
        }
        list = rest;
    }
    Ok(args.get(2).copied().unwrap_or(Value::FALSE))
}

/// `(%resolve-module name ensure)`: the module, made if `ensure`, else `#f`.
pub(crate) fn resolve_module(cx: &mut Cx, args: &[Value]) -> R {
    let path = cx.path_arg(args[0])?;
    let module = if args[1].is_false() {
        cx.globals.find(&path)
    } else {
        Some(cx.globals.ensure(&mut cx.heap, &path))
    };
    Ok(module.map_or(Value::FALSE, |m| cx.globals.object(m)))
}

/// `(%module-forms name)`: the data of the module's code if it has some not
/// loaded yet, which it then no longer has; else `#f`.
pub(crate) fn module_forms(cx: &mut Cx, args: &[Value]) -> R {
    let path = cx.path_arg(args[0])?;
    let Some(at) = cx.globals.sources.iter().position(|(p, _)| *p == path) else {
        return Ok(Value::FALSE);
    };
    let (_, text) = cx.globals.sources.remove(at);
    let mut forms = Vec::new();
    let mut at = 0;
    loop {
        match read(&mut cx.heap, &text, at, None) {
            Ok(r) if r.datum == Value::EOF => break,
            Ok(r) => {
                forms.push(r.datum);
                at = r.end;
            }
            Err(e) => {
                let message = cx.heap.string(e.message);
                return Err(cx.misc("~A", &[message]));
            }
        }
    }
    Ok(cx.heap.list(&forms))
}

/// `(%spec-module spec)`: the name of the module a `use-modules` spec uses.
pub(crate) fn spec_module(cx: &mut Cx, args: &[Value]) -> R {
    let spec = args[0];
    match cx.heap.car(spec) {
        Some(head) if head.tag() == Tag::Pair => Ok(head),
        _ => Ok(spec),
    }
}

/// `(%use-specs! specs)`: the current module uses each spec's interface.
pub(crate) fn use_specs(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.current();
    for spec in items(&cx.heap, args[0]).unwrap_or_default() {
        let interface = cx.interface(spec)?;
        cx.globals.uses(module, interface);
    }
    Ok(Value::UNSPECIFIED)
}

/// The keywords of `define-module` and whether each takes an argument.
fn option_arity(name: &str) -> Option<bool> {
    match name {
        "pure" | "no-backtrace" => Some(false),
        "use-module" | "autoload" | "export" | "re-export" | "replace" | "export-syntax"
        | "re-export-syntax" | "version" | "duplicates" | "declarative?" => Some(true),
        _ => None,
    }
}

/// `define-module`'s options as (keyword, argument) pairs.
fn options(cx: &mut Cx, list: Value) -> Result<Vec<(Value, Value)>, Thrown> {
    let mut out = Vec::new();
    let mut at = list;
    while let (Some(keyword), Some(rest)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        let takes = cx
            .heap
            .name(keyword)
            .filter(|_| keyword.tag() == Tag::Keyword)
            .and_then(option_arity);
        match takes {
            Some(false) => {
                out.push((keyword, Value::UNSPECIFIED));
                at = rest;
            }
            Some(true) if rest.tag() == Tag::Pair => {
                out.push((keyword, cx.heap.car(rest).unwrap_or(Value::NULL)));
                at = cx.heap.cdr(rest).unwrap_or(Value::NULL);
            }
            _ => {
                let argument = cx.heap.car(rest).unwrap_or(Value::NULL);
                return Err(cx.bad_option(keyword, argument));
            }
        }
    }
    Ok(out)
}

/// `(%option-modules options)`: the modules `define-module`'s
/// `#:use-module`s name, to load before it uses them.
pub(crate) fn option_modules(cx: &mut Cx, args: &[Value]) -> R {
    let mut names = Vec::new();
    for (keyword, argument) in options(cx, args[0])? {
        if cx.heap.name(keyword) == Some("use-module") {
            names.push(spec_module(cx, &[argument])?);
        }
    }
    Ok(cx.heap.list(&names))
}

/// `(%process-define-module name options)`: the module named, with a public
/// interface, using `(guile)` unless `#:pure`, and its options done.
pub(crate) fn process_define_module(cx: &mut Cx, args: &[Value]) -> R {
    let path = cx.path_arg(args[0])?;
    let options = options(cx, args[1])?;
    let module = cx.globals.ensure(&mut cx.heap, &path);
    let m = &mut cx.globals.modules[module as usize];
    m.public.get_or_insert_with(BTreeMap::new);
    let pure = options
        .iter()
        .any(|&(k, _)| cx.heap.name(k) == Some("pure"));
    if pure {
        m.uses.clear();
    } else if !m.uses.iter().any(|u| matches!(u, Use::Module(GUILE))) {
        m.uses.insert(0, Use::Module(GUILE));
    }
    for (keyword, argument) in options {
        match cx.heap.name(keyword) {
            Some("use-module") => {
                let interface = cx.interface(argument)?;
                cx.globals.uses(module, interface);
            }
            Some("autoload") => {
                // Its module and the names it gives, used when loaded.
                let name = cx.heap.car(argument).unwrap_or(Value::NULL);
                let names = cx.heap.cdr(argument).unwrap_or(Value::NULL);
                let names = cx.heap.car(names).unwrap_or(Value::NULL);
                let select = cx.heap.keyword("select");
                let options = cx.heap.list(&[select, names]);
                let spec = cx.heap.cons(name, options);
                let interface = cx.interface(spec)?;
                cx.globals.uses(module, interface);
            }
            Some("export" | "replace" | "export-syntax") => {
                export_names(cx, module, argument, false)?
            }
            Some("re-export" | "re-export-syntax") => export_names(cx, module, argument, true)?,
            _ => {}
        }
    }
    Ok(cx.globals.object(module))
}

/// Exports each of `names` (a symbol, or `(name . exported-name)`) from
/// `module`; `re` for imported variables, which must exist.
fn export_names(cx: &mut Cx, module: u32, names: Value, re: bool) -> Result<(), Thrown> {
    let Some(list) = items(&cx.heap, names) else {
        let keyword = cx.heap.keyword(if re { "re-export" } else { "export" });
        return Err(cx.bad_option(keyword, names));
    };
    for item in list {
        let (name, as_name) = match (cx.heap.car(item), cx.heap.cdr(item)) {
            (Some(name), Some(as_name)) => (name, as_name),
            _ => (item, item),
        };
        cx.symbol_arg("module-export!", 2, name)?;
        cx.symbol_arg("module-export!", 2, as_name)?;
        if re {
            match cx.globals.lookup(module, name) {
                Some(slot) => cx.globals.publish(module, as_name.payload(), slot),
                None => return Err(cx.misc("Undefined variable: ~S", &[name])),
            }
        } else {
            cx.globals.export(module, name, as_name);
        }
    }
    Ok(())
}

/// `(%export names [module])`, `export` from the current module, or from
/// `define-public`'s.
pub(crate) fn export(cx: &mut Cx, args: &[Value]) -> R {
    let module = match args.get(1) {
        Some(&m) => cx.module_arg("module-export!", 1, m)?,
        None => cx.current(),
    };
    export_names(cx, module, args[0], false)?;
    Ok(Value::UNSPECIFIED)
}

/// `(module-export! module names)`.
pub(crate) fn module_export(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-export!", 1, args[0])?;
    export_names(cx, module, args[1], false)?;
    Ok(Value::UNSPECIFIED)
}

/// `(%re-export names)`.
pub(crate) fn re_export(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.current();
    export_names(cx, module, args[0], true)?;
    Ok(Value::UNSPECIFIED)
}

/// `(%module-at name symbol public)`: `(@ name symbol)`, or `(@@ name
/// symbol)` when not `public`, looked up when it runs, as Guile's
/// `module-lookup` does.
pub(crate) fn module_at(cx: &mut Cx, args: &[Value]) -> R {
    let (name, symbol, public) = (args[0], args[1], !args[2].is_false());
    let path = cx.path_arg(name)?;
    let module = if public {
        let found = cx.globals.find(&path);
        match found.filter(|&m| cx.globals.modules[m as usize].public.is_some()) {
            Some(m) => m,
            None => return Err(cx.wrong_type("module-variable", 1, Value::FALSE, "module")),
        }
    } else {
        cx.globals.ensure(&mut cx.heap, &path)
    };
    let value = cx
        .globals
        .at(module, symbol, public)
        .map(|slot| cx.globals.values[slot as usize])
        .filter(|&v| v != Value::UNDEFINED);
    match value {
        Some(value) => Ok(value),
        None => Err(cx.raise(
            &crate::errors::UNBOUND_VARIABLE,
            Some("module-lookup"),
            &[symbol],
        )),
    }
}

/// `(module-define! module symbol value)`.
pub(crate) fn module_define(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-local-variable", 1, args[0])?;
    let symbol = cx.symbol_arg("module-local-variable", 2, args[1])?;
    let slot = cx.globals.cell(module, symbol);
    cx.globals.define(slot, args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `(module-remove! module symbol)`: the variable `module` binds itself
/// as `symbol`, if it has one, unbound, as if it had never been defined.
pub(crate) fn module_remove(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-remove!", 1, args[0])?;
    let symbol = cx.symbol_arg("module-remove!", 2, args[1])?;
    if let Some(slot) = cx.globals.lookup(module, symbol)
        && cx.globals.owners[slot as usize] == module
        && !cx.globals.is_reference(slot)
    {
        cx.globals.set(slot, Value::UNDEFINED);
    }
    Ok(Value::UNSPECIFIED)
}

/// `(module-ref module symbol [default])`.
pub(crate) fn module_ref(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-variable", 1, args[0])?;
    let symbol = cx.symbol_arg("module-variable", 2, args[1])?;
    let value = cx
        .globals
        .lookup(module, symbol)
        .map(|slot| cx.globals.values[slot as usize])
        .filter(|&v| v != Value::UNDEFINED);
    match (value, args.get(2)) {
        (Some(value), _) => Ok(value),
        (None, Some(&default)) => Ok(default),
        (None, None) => {
            let within = cx.heap.intern("in");
            Err(cx.misc("No variable named ~S ~S ~S", &[symbol, within, args[0]]))
        }
    }
}

/// `(defined? symbol [module])`: whether `symbol` is bound in `module`, its
/// own or imported, by default in the current module.
pub(crate) fn is_defined(cx: &mut Cx, args: &[Value]) -> R {
    let symbol = cx.symbol_arg("defined?", 1, args[0])?;
    let module = match args.get(1) {
        Some(&module) => cx.module_arg("defined?", 2, module)?,
        None => cx.globals.current_module(&cx.heap),
    };
    let bound = cx
        .globals
        .lookup(module, symbol)
        .is_some_and(|slot| cx.globals.values[slot as usize] != Value::UNDEFINED);
    Ok(Value::bool(bound))
}

/// `(module-set! module symbol value)`: the variable must exist.
pub(crate) fn module_set(cx: &mut Cx, args: &[Value]) -> R {
    let module = cx.module_arg("module-variable", 1, args[0])?;
    let symbol = cx.symbol_arg("module-variable", 2, args[1])?;
    match cx.globals.lookup(module, symbol) {
        Some(slot) => {
            cx.globals.set(slot, args[2]);
            Ok(Value::UNSPECIFIED)
        }
        None => {
            let within = cx.heap.intern("in");
            Err(cx.misc("No variable named ~S ~S ~S", &[symbol, within, args[0]]))
        }
    }
}

/// `define-module`, `use-modules`, `resolve-module`, `@` and `eval` in a
/// module, over the primitives above, as Guile's `boot-9.scm` has them. A
/// module's code is loaded in `(guile-user)`, and its own `define-module`
/// makes its module current until the load ends. `%module-hooks` gives a
/// module a thunk that runs before each `use-modules` of it and each
/// `#:use-module`.
pub(crate) const PRELUDE: &str = r#"
(define (save-module-excursion thunk)
  (let ((inner (current-module)) (outer #f))
    (dynamic-wind
      (lambda () (set! outer (current-module)) (set-current-module inner) (set! inner #f))
      thunk
      (lambda () (set! inner (current-module)) (set-current-module outer) (set! outer #f)))))
;; Thunks run each time a module is used, by its name: a part's own
;; module loads its part (D74), which the thunk does once.
(define %module-hooks '())
(define (%use-module! name)
  (let ((hook (assoc name %module-hooks)))
    (if hook ((cdr hook))))
  (%autoload name))
(define (%autoload name)
  (let ((forms (%module-forms name)))
    (if forms
        (with-fluid* %module-fluid (%resolve-module '(guile-user) #t)
          (lambda () (for-each primitive-eval forms))))))
(define (%use-modules specs)
  (for-each (lambda (spec) (%use-module! (%spec-module spec))) specs)
  (%use-specs! specs))
(define (%define-module name options)
  (for-each %use-module! (%option-modules options))
  (let ((module (%process-define-module name options)))
    (fluid-set! %module-fluid module)
    module))
(define (resolve-module name . args)
  (let ((autoload (if (and (pair? args) (not (keyword? (car args)))) (car args) #t))
        (ensure (let ((k (memq #:ensure args))) (if (and k (pair? (cdr k))) (cadr k) #t))))
    (if autoload (%autoload name))
    (%resolve-module name ensure)))
(define (%at name symbol public)
  (%autoload name)
  (%module-at name symbol public))
(define %undefined-marker (list 'undefined))
(define (module-defined? module name)
  (not (eq? (module-ref module name %undefined-marker) %undefined-marker)))
;; Guile's variables of a module's own bindings, as `module-map` and
;; `module-for-each` give them: a variable names its module and symbol.
(define %variable-tag (list 'variable))
(define (%module-variable module name) (vector %variable-tag module name))
(define (variable? v)
  (and (vector? v) (= (vector-length v) 3) (eq? (vector-ref v 0) %variable-tag)))
(define (variable-ref v) (module-ref (vector-ref v 1) (vector-ref v 2)))
(define (variable-set! v x) (module-set! (vector-ref v 1) (vector-ref v 2) x))
(define (variable-bound? v) (module-defined? (vector-ref v 1) (vector-ref v 2)))
(define (module-map proc module)
  (map (lambda (name) (proc name (%module-variable module name)))
       (%module-local-names module)))
(define (module-for-each proc module)
  (for-each (lambda (name) (proc name (%module-variable module name)))
            (%module-local-names module)))
(define (%in-module module thunk)
  (%dyn fluid %module-fluid module (thunk) #f))
"#;
