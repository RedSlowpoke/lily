//! The image (chapter 03 §5.6, R5, S4): a VM's state written to bytes after
//! it has loaded its Scheme, and a VM restored from them.
//!
//! A value is one word whose payload is an index (a symbol's in the symbol
//! table, an object's in its arena, a primitive's in the tables), never a
//! pointer, so the image holds every word as it is and restoring copies it
//! back to the same index: no relocation. What it holds, in order:
//!
//! - a header: [`MAGIC`], [`FORMAT`], and a signature of the primitive
//!   tables, the crate's own and the [`Extension`]s', which the restoring VM
//!   must have in the same order;
//! - the compiled code: every template the closures reach, children before
//!   their parents, with its instructions as the VM last rewrote them;
//! - the heap (`heap.rs`): the symbols, each arena chunk by chunk with its
//!   epoch and live bits, and each foreign object by the name of its type,
//!   written by [`Foreign::save`] and read back by that type's [`Loader`];
//! - the global slots and the modules (`module.rs`), and the VM's own
//!   values: the ports' fluids, the expander's aliases, whether `#{` reads
//!   as LilyPond's, the slots of `%location` and `%in-module`.
//!
//! The VM is collected before it is written, so the arenas hold only what
//! lives. Hash tables are rebuilt as they are read: their entries keep their
//! order and their hashes, which for `eq?` and `eqv?` are of the words and
//! so still hold, and the index over them is made again; the symbol table's
//! is too. The image is the build's own (S4, step 12b), read by the same
//! build: a header that does not match is refused, but a corrupt body is not
//! looked for.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt;

use crate::compile::{Op, Syntax, Template};
use crate::control::DynKind;
use crate::expand::Expander;
use crate::heap::{Foreign, Heap};
use crate::module::Globals;
use crate::prims::{Inline, PRIMS, Prim};
use crate::value::Value;
use crate::vm::{Cx, Vm};

/// The first bytes of an image.
const MAGIC: &[u8; 8] = b"LILYIMG\0";
/// The layout's version: changed with any change to what is written.
const FORMAT: u32 = 3;

/// Why an image was not written or not restored.
#[derive(Debug, PartialEq, Eq)]
pub enum ImageError {
    /// The VM is running Scheme, or Rust holds roots: only a VM at rest
    /// between evaluations is written.
    NotAtRest,
    /// A foreign object of this type cannot be saved ([`Foreign::save`]).
    Unsaved(&'static str),
    /// Not an image of this build's format.
    Format,
    /// The restoring VM's primitive tables are not the writer's.
    Primitives,
    /// No [`Loader`] reads foreign objects of this type.
    NoLoader(String),
    /// The image ends early or holds what this build does not write.
    Corrupt,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::NotAtRest => f.write_str("the VM is not at rest"),
            ImageError::Unsaved(name) => write!(f, "a {name} cannot be saved in an image"),
            ImageError::Format => f.write_str("not an image of this build"),
            ImageError::Primitives => f.write_str("the primitive tables differ from the image's"),
            ImageError::NoLoader(name) => write!(f, "no loader for {name}"),
            ImageError::Corrupt => f.write_str("the image is corrupt"),
        }
    }
}

/// Reads a foreign object back: the type it is for, by its
/// [`Foreign::type_name`], and what reads the fields its
/// [`Foreign::save`] wrote.
pub struct Loader {
    pub name: &'static str,
    pub load: fn(&mut ImageReader<'_>) -> Option<Box<dyn Foreign>>,
}

/// What another crate adds to a VM that restoring its image needs: its
/// primitives, as it gave them to [`Vm::define_primitives`] slice by slice,
/// the loaders of its foreign types, its methods for the arithmetic
/// operators, as it gave them to [`Vm::set_operators`], and its handlers
/// for `%host-call`, as it gave them to [`Vm::add_host_handlers`].
pub struct Extension {
    pub prims: &'static [&'static [Prim]],
    pub loaders: &'static [Loader],
    pub operators: Option<crate::OperatorFn>,
    pub hosts: &'static [crate::HostHandler],
}

// --- bytes ---------------------------------------------------------------------

/// What an image is written with: little-endian numbers, values as their
/// words, texts and byte strings after their length.
#[derive(Default)]
pub struct ImageWriter {
    bytes: Vec<u8>,
}

impl ImageWriter {
    pub fn u8(&mut self, n: u8) {
        self.bytes.push(n);
    }

    pub fn bool(&mut self, b: bool) {
        self.u8(b as u8);
    }

    pub fn u32(&mut self, n: u32) {
        self.bytes.extend_from_slice(&n.to_le_bytes());
    }

    pub fn i32(&mut self, n: i32) {
        self.u32(n as u32);
    }

    pub fn u64(&mut self, n: u64) {
        self.bytes.extend_from_slice(&n.to_le_bytes());
    }

    pub fn f64(&mut self, x: f64) {
        self.u64(x.to_bits());
    }

    pub fn value(&mut self, v: Value) {
        self.u64(v.to_bits());
    }

    pub fn values(&mut self, vs: &[Value]) {
        self.count(vs.len());
        for &v in vs {
            self.value(v);
        }
    }

    pub fn count(&mut self, n: usize) {
        self.u32(n as u32);
    }

    pub fn bytes(&mut self, b: &[u8]) {
        self.count(b.len());
        self.bytes.extend_from_slice(b);
    }

    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    pub(crate) fn append(&mut self, other: ImageWriter) {
        self.bytes.extend_from_slice(&other.bytes);
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads what an [`ImageWriter`] wrote; each read is `None` past the end.
pub struct ImageReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> ImageReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> ImageReader<'a> {
        ImageReader { bytes, at: 0 }
    }

    // The readers are inlined into the loops that call them, so that a
    // fresh worker's first restore, which runs in V8's baseline tier,
    // Liftoff, before its code is optimised, makes no call a value and
    // compiles fewer functions lazily (MS2 step 31, D90).
    #[inline(always)]
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let b = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(b)
    }

    #[inline(always)]
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    #[inline(always)]
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    #[inline(always)]
    pub fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }

    #[inline(always)]
    pub fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }

    #[inline(always)]
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array()?))
    }

    #[inline(always)]
    pub fn f64(&mut self) -> Option<f64> {
        Some(f64::from_bits(self.u64()?))
    }

    /// A value whose word is one this build makes; its index is not
    /// checked.
    #[inline(always)]
    pub fn value(&mut self) -> Option<Value> {
        Value::from_bits(self.u64()?)
    }

    /// A count of values, then the values, taken as one slice. Made
    /// first and written in a plain loop: no call a value (MS3 step 56).
    pub fn values(&mut self) -> Option<Vec<Value>> {
        let n = self.count()?;
        let words = self.take(n.checked_mul(8)?)?;
        let mut values = alloc::vec![Value::UNSPECIFIED; n];
        let mut i = 0;
        while i < n {
            let word = words[8 * i..8 * i + 8].try_into().ok()?;
            values[i] = Value::from_bits(u64::from_le_bytes(word))?;
            i += 1;
        }
        Some(values)
    }

    /// A count: at most what the rest of the image could hold, so a corrupt
    /// one does not reserve memory for nothing.
    #[inline(always)]
    pub fn count(&mut self) -> Option<usize> {
        let n = self.u32()? as usize;
        (n <= self.bytes.len() - self.at).then_some(n)
    }

    #[inline(always)]
    pub fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.count()?;
        self.take(n)
    }

    #[inline(always)]
    pub fn str(&mut self) -> Option<&'a str> {
        core::str::from_utf8(self.bytes()?).ok()
    }

    fn at_end(&self) -> bool {
        self.at == self.bytes.len()
    }
}

// --- compiled code -----------------------------------------------------------------

/// The templates an image holds, numbered as they are first met, each
/// after its children.
#[derive(Default)]
pub(crate) struct Templates {
    numbers: BTreeMap<*const Template, u32>,
    list: Vec<Rc<Template>>,
}

impl Templates {
    /// The number of `code`, given its children's first.
    pub(crate) fn number(&mut self, code: &Rc<Template>) -> u32 {
        if let Some(&n) = self.numbers.get(&Rc::as_ptr(code)) {
            return n;
        }
        for child in &code.children {
            self.number(child);
        }
        let n = self.list.len() as u32;
        self.numbers.insert(Rc::as_ptr(code), n);
        self.list.push(code.clone());
        n
    }

    fn save(&self, w: &mut ImageWriter) {
        w.count(self.list.len());
        for code in &self.list {
            w.value(code.name);
            w.u32(code.required);
            w.bool(code.rest);
            w.u32(code.frame);
            w.u32(code.free);
            w.bool(code.star);
            w.value(code.header);
            w.value(code.names);
            w.count(code.ops.len());
            for op in &code.ops {
                save_op(op.get(), w);
            }
            w.values(&code.consts);
            w.count(code.children.len());
            for child in &code.children {
                w.u32(self.numbers[&Rc::as_ptr(child)]);
            }
        }
    }

    fn load(r: &mut ImageReader<'_>) -> Option<Vec<Rc<Template>>> {
        let n = r.count()?;
        let mut list: Vec<Rc<Template>> = Vec::with_capacity(n);
        for _ in 0..n {
            let name = r.value()?;
            let required = r.u32()?;
            let rest = r.bool()?;
            let frame = r.u32()?;
            let free = r.u32()?;
            let star = r.bool()?;
            let header = r.value()?;
            let names = r.value()?;
            let n = r.count()?;
            let bytes = r.take(n.checked_mul(9)?)?;
            // Made first and written in a plain loop: no call an
            // instruction (MS3 step 56).
            let mut ops = alloc::vec![Cell::new(Op::Pop); n];
            let mut i = 0;
            while i < n {
                ops[i] = Cell::new(load_op(bytes[9 * i..9 * i + 9].try_into().ok()?)?);
                i += 1;
            }
            let consts = r.values()?;
            let n = r.count()?;
            let mut children = Vec::with_capacity(n);
            let mut i = 0;
            while i < n {
                children.push(list.get(r.u32()? as usize)?.clone());
                i += 1;
            }
            list.push(Rc::new(Template {
                name,
                required,
                rest,
                frame,
                free,
                ops: ops.into(),
                consts,
                children,
                star,
                header,
                names,
                mark: Cell::new(0),
                digest: Cell::new(0),
            }));
        }
        Some(list)
    }
}

const DYN_KINDS: [DynKind; 4] = [
    DynKind::Catch,
    DynKind::Prompt,
    DynKind::Wind,
    DynKind::Fluid,
];

/// An instruction as a code and two operands, nine bytes.
fn save_op(op: Op, w: &mut ImageWriter) {
    let (code, a, b) = match op {
        Op::Const(n) => (0, n, 0),
        Op::Fixnum(n) => (1, n as u32, 0),
        Op::Null => (2, 0, 0),
        Op::False => (3, 0, 0),
        Op::True => (4, 0, 0),
        Op::Unspecified => (5, 0, 0),
        Op::Local(n) => (6, n, 0),
        Op::LocalBox(n) => (7, n, 0),
        Op::Free(n) => (8, n, 0),
        Op::FreeBox(n) => (9, n, 0),
        Op::Myself => (10, 0, 0),
        Op::Global(n) => (11, n, 0),
        Op::SetLocal(n) => (12, n, 0),
        Op::SetLocalBox(n) => (13, n, 0),
        Op::SetFreeBox(n) => (14, n, 0),
        Op::SetGlobal(n) => (15, n, 0),
        Op::Define(n) => (16, n, 0),
        Op::Box(n) => (17, n, 0),
        Op::NewBox(n) => (18, n, 0),
        Op::Pop => (19, 0, 0),
        Op::Jump(n) => (20, n, 0),
        Op::JumpFalse(n) => (21, n, 0),
        Op::AndJump(n) => (22, n, 0),
        Op::OrJump(n) => (23, n, 0),
        Op::Closure(n) => (24, n, 0),
        Op::Call(n) => (25, n, 0),
        Op::CallMulti(n) => (26, n, 0),
        Op::TailCall(n) => (27, n, 0),
        Op::CallValues => (28, 0, 0),
        Op::TailCallValues => (29, 0, 0),
        Op::Return => (30, 0, 0),
        Op::Inline(inline, slot, direct) => {
            let i = Inline::ALL.iter().position(|&x| x == inline).unwrap_or(0);
            (31, slot, i as u32 | (direct as u32) << 8)
        }
        Op::Memv(n) => (32, n, 0),
        Op::Direct => (33, 0, 0),
        Op::PushDyn(kind, landing) => {
            let k = DYN_KINDS.iter().position(|&x| x == kind).unwrap_or(0);
            (34, landing, k as u32)
        }
        Op::PopDyn => (35, 0, 0),
        Op::Pending => (36, 0, 0),
    };
    w.u8(code);
    w.u32(a);
    w.u32(b);
}

/// The instruction [`save_op`] wrote as `op`.
#[inline(always)]
fn load_op(op: &[u8; 9]) -> Option<Op> {
    let word = |at: usize| u32::from_le_bytes([op[at], op[at + 1], op[at + 2], op[at + 3]]);
    let (code, a, b) = (op[0], word(1), word(5));
    Some(match code {
        0 => Op::Const(a),
        1 => Op::Fixnum(a as i32),
        2 => Op::Null,
        3 => Op::False,
        4 => Op::True,
        5 => Op::Unspecified,
        6 => Op::Local(a),
        7 => Op::LocalBox(a),
        8 => Op::Free(a),
        9 => Op::FreeBox(a),
        10 => Op::Myself,
        11 => Op::Global(a),
        12 => Op::SetLocal(a),
        13 => Op::SetLocalBox(a),
        14 => Op::SetFreeBox(a),
        15 => Op::SetGlobal(a),
        16 => Op::Define(a),
        17 => Op::Box(a),
        18 => Op::NewBox(a),
        19 => Op::Pop,
        20 => Op::Jump(a),
        21 => Op::JumpFalse(a),
        22 => Op::AndJump(a),
        23 => Op::OrJump(a),
        24 => Op::Closure(a),
        25 => Op::Call(a),
        26 => Op::CallMulti(a),
        27 => Op::TailCall(a),
        28 => Op::CallValues,
        29 => Op::TailCallValues,
        30 => Op::Return,
        31 => Op::Inline(*Inline::ALL.get(b as usize & 0xff)?, a, b >> 8 != 0),
        32 => Op::Memv(a),
        33 => Op::Direct,
        34 => Op::PushDyn(*DYN_KINDS.get(b as usize)?, a),
        35 => Op::PopDyn,
        36 => Op::Pending,
        _ => return None,
    })
}

// --- the VM ----------------------------------------------------------------------------

/// FNV-1a over the primitives' names and arities, in the order a VM
/// numbers them.
fn signature<'a>(prims: impl Iterator<Item = &'a Prim>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |b: u8| h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    for prim in prims {
        prim.name.bytes().for_each(&mut mix);
        mix(0);
        mix(prim.min);
        mix(prim.max);
    }
    h
}

/// The loaders of the crate's own foreign types.
static LOADERS: &[Loader] = &[
    Loader {
        name: "fluid",
        load: crate::control::load_fluid,
    },
    Loader {
        name: "<parameter>",
        load: crate::control::load_parameter,
    },
    Loader {
        name: "module",
        load: crate::module::load_module_object,
    },
    Loader {
        name: "syntax-transformer",
        load: crate::expand::load_macro,
    },
    Loader {
        name: "port",
        load: crate::ports::load_port,
    },
    Loader {
        name: "hash-table",
        load: crate::hash::load_table,
    },
    Loader {
        name: "procedure-with-setter",
        load: crate::hash::load_with_setter,
    },
    Loader {
        name: "class",
        load: crate::goops::load_class,
    },
    Loader {
        name: "instance",
        load: crate::goops::load_instance,
    },
    Loader {
        name: "Regex",
        load: crate::regex::load_regex,
    },
    Loader {
        name: "bytevector",
        load: crate::reader::load_bytevector,
    },
    Loader {
        name: "array",
        load: crate::reader::load_array,
    },
    Loader {
        name: "display-stub",
        load: crate::lily::load_display_stub,
    },
];

/// The parts of [`Vm::restore_marked`], in order: the compiled code; the
/// symbols, read and their table built; the arenas but the foreign one,
/// each chunk's objects read and its free list made; the foreign objects
/// (hash tables rebuilt among them); the global slots and the modules; the
/// expander, the ports and the VM's own values.
pub const RESTORE_MARKS: [&str; 6] = ["code", "symbols", "arenas", "foreign", "globals", "vm"];

impl Vm {
    /// Collects, then writes the VM's state as an image, which
    /// [`Vm::restore`] makes a VM of. The VM must be at rest: not inside
    /// [`Vm::eval`] or [`Vm::apply`], with no values on the heap's root
    /// stack. What was written to the output and error ports and not taken
    /// is not in the image.
    pub fn write_image(&mut self) -> Result<Vec<u8>, ImageError> {
        if !self.stack.is_empty()
            || !self.frames.is_empty()
            || !self.dynamic.is_empty()
            || self.cx.heap.root_depth() != 0
        {
            return Err(ImageError::NotAtRest);
        }
        self.pending = Value::NULL;
        self.collect();
        let mut templates = Templates::default();
        let mut heap = ImageWriter::default();
        self.cx.heap.save(&mut heap, &mut templates)?;
        let mut w = ImageWriter::default();
        w.bytes.extend_from_slice(MAGIC);
        w.u32(FORMAT);
        w.count(self.cx.heap.natives.len());
        w.u64(signature(
            PRIMS.iter().chain(self.cx.heap.natives.iter().copied()),
        ));
        templates.save(&mut w);
        w.append(heap);
        self.cx.globals.save(&mut w);
        w.value(self.syntax.procedure());
        self.expander.save(&mut w);
        for v in [self.cx.output, self.cx.error_port, self.cx.input] {
            w.value(v);
        }
        w.u64(self.cx.gensyms);
        w.u32(u32::from(self.cx.lily_braces));
        w.u32(self.location);
        w.u32(self.in_module);
        w.u64(self.applications);
        Ok(w.into_bytes())
    }

    /// A VM restored from an image that [`Vm::write_image`] wrote, given
    /// the extensions the writing VM was given, in the same order.
    pub fn restore(image: &[u8], extensions: &[&Extension]) -> Result<Vm, ImageError> {
        Vm::restore_marked(image, extensions, &mut |_| {})
    }

    /// [`Vm::restore`], calling `mark` with each of [`RESTORE_MARKS`] as
    /// that part of the restore ends: for the harness's restore.html, which
    /// times the parts of a first restore (MS2 step 31).
    pub fn restore_marked(
        image: &[u8],
        extensions: &[&Extension],
        mark: &mut dyn FnMut(&'static str),
    ) -> Result<Vm, ImageError> {
        let mut r = ImageReader::new(image);
        if r.take(MAGIC.len()) != Some(&MAGIC[..]) || r.u32() != Some(FORMAT) {
            return Err(ImageError::Format);
        }
        let natives: Vec<&'static Prim> = extensions
            .iter()
            .flat_map(|e| e.prims.iter().copied().flatten())
            .collect();
        let count = r.count().ok_or(ImageError::Corrupt)?;
        let sign = r.u64().ok_or(ImageError::Corrupt)?;
        if count != natives.len() || sign != signature(PRIMS.iter().chain(natives.iter().copied()))
        {
            return Err(ImageError::Primitives);
        }
        let loaders: Vec<&Loader> = LOADERS
            .iter()
            .chain(extensions.iter().flat_map(|e| e.loaders))
            .collect();
        let templates = Templates::load(&mut r).ok_or(ImageError::Corrupt)?;
        mark("code");
        let mut heap = Heap::load(&mut r, &templates, &loaders, mark)?;
        heap.natives = natives;
        heap.operators = extensions.iter().find_map(|e| e.operators);
        let corrupt = || ImageError::Corrupt;
        let globals = Globals::load(&mut r).ok_or_else(corrupt)?;
        mark("globals");
        let procedure = r.value().ok_or_else(corrupt)?;
        let syntax = Syntax::new(&mut heap, procedure);
        let mut expander = Expander::new(&mut heap, procedure);
        expander.load(&mut r).ok_or_else(corrupt)?;
        let [output, error_port, input] = [(); 3].map(|_| r.value());
        let mut vm = Vm::assemble(
            Cx::new(
                heap,
                globals,
                [
                    output.ok_or_else(corrupt)?,
                    error_port.ok_or_else(corrupt)?,
                    input.ok_or_else(corrupt)?,
                ],
            ),
            syntax,
            expander,
        );
        for e in extensions {
            vm.add_host_handlers(e.hosts);
        }
        vm.cx.gensyms = r.u64().ok_or_else(corrupt)?;
        vm.cx.lily_braces = r.u32().ok_or_else(corrupt)? != 0;
        vm.location = r.u32().ok_or_else(corrupt)?;
        vm.in_module = r.u32().ok_or_else(corrupt)?;
        vm.applications = r.u64().ok_or_else(corrupt)?;
        if !r.at_end() {
            return Err(ImageError::Corrupt);
        }
        mark("vm");
        Ok(vm)
    }
}
