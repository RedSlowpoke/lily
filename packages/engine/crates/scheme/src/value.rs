//! Values (chapter 03 §5.1): one 8-byte word each, and the strings of the
//! heap. Also how Guile 3 writes characters, strings and symbols, which needs
//! nothing but their text.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::OnceCell;
use core::fmt::{self, Write};

/// A Scheme value: one 64-bit word, one wasm `i64`.
///
/// - Low two bits `00`: a fixnum, the integer in the upper 62 bits. That is
///   Guile's range on 64-bit hosts (`most-positive-fixnum` is 2^61 − 1), so
///   a number becomes a bignum where Guile's does.
/// - Low bit 1: a [`Tag`] in the low byte and a 32-bit payload in the upper
///   half: nothing for the immediates, a character's code point, a symbol's
///   or keyword's index in the symbol table, an object's index in its arena.
///
/// Low bits `10` are unused. An error is the reserved [`Value::ERROR`], not a
/// Rust `Result`; what went wrong is kept beside the heap.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Value(u64);

/// What a [`Value`] is. The variants from [`Tag::Pair`] on are objects in the
/// heap's arenas; the others are the whole value. Numbers other than fixnums
/// are flonums, bignums, exact ratios and complex numbers (chapter 03 §5.1).
/// A primitive is an index into the VM's table of them; a closure is
/// compiled code with the values it captured; `Values` is what `values`
/// returns for other than one value (03 §5.3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Tag {
    Fixnum,
    Null,
    False,
    True,
    Unspecified,
    Undefined,
    Eof,
    Error,
    Char,
    Symbol,
    Keyword,
    Primitive,
    Pair,
    String,
    Vector,
    Foreign,
    Flonum,
    Bignum,
    Ratio,
    Complex,
    Closure,
    Values,
}

const TAGS: [Tag; 22] = [
    Tag::Fixnum,
    Tag::Null,
    Tag::False,
    Tag::True,
    Tag::Unspecified,
    Tag::Undefined,
    Tag::Eof,
    Tag::Error,
    Tag::Char,
    Tag::Symbol,
    Tag::Keyword,
    Tag::Primitive,
    Tag::Pair,
    Tag::String,
    Tag::Vector,
    Tag::Foreign,
    Tag::Flonum,
    Tag::Bignum,
    Tag::Ratio,
    Tag::Complex,
    Tag::Closure,
    Tag::Values,
];

impl Value {
    /// `'()`.
    pub const NULL: Value = Value::tagged(Tag::Null, 0);
    pub const FALSE: Value = Value::tagged(Tag::False, 0);
    pub const TRUE: Value = Value::tagged(Tag::True, 0);
    /// What `(if #f #f)` returns; Guile writes it `#<unspecified>`.
    pub const UNSPECIFIED: Value = Value::tagged(Tag::Unspecified, 0);
    /// An unbound variable's or an unassigned `letrec` binding's value.
    pub const UNDEFINED: Value = Value::tagged(Tag::Undefined, 0);
    pub const EOF: Value = Value::tagged(Tag::Eof, 0);
    /// Returned instead of a value when an error is being raised.
    pub const ERROR: Value = Value::tagged(Tag::Error, 0);

    /// Guile's `most-negative-fixnum` and `most-positive-fixnum`.
    pub const FIXNUM_MIN: i64 = -(1 << 61);
    pub const FIXNUM_MAX: i64 = (1 << 61) - 1;

    pub(crate) const fn tagged(tag: Tag, payload: u32) -> Value {
        Value(((payload as u64) << 32) | ((tag as u64) << 1) | 1)
    }

    /// `n` as a fixnum, or `None` outside [`Value::FIXNUM_MIN`]..=[`Value::FIXNUM_MAX`].
    pub const fn fixnum(n: i64) -> Option<Value> {
        if n < Value::FIXNUM_MIN || n > Value::FIXNUM_MAX {
            None
        } else {
            Some(Value((n as u64) << 2))
        }
    }

    pub const fn as_fixnum(self) -> Option<i64> {
        if self.0 & 3 == 0 {
            Some(self.0 as i64 >> 2)
        } else {
            None
        }
    }

    pub const fn bool(b: bool) -> Value {
        if b { Value::TRUE } else { Value::FALSE }
    }

    /// Whether this is `#f`, the only false value.
    pub const fn is_false(self) -> bool {
        self.0 == Value::FALSE.0
    }

    pub const fn char(c: char) -> Value {
        Value::tagged(Tag::Char, c as u32)
    }

    pub fn as_char(self) -> Option<char> {
        if self.tag() == Tag::Char {
            char::from_u32(self.payload())
        } else {
            None
        }
    }

    pub fn tag(self) -> Tag {
        if self.0 & 3 == 0 {
            Tag::Fixnum
        } else {
            TAGS[(self.0 as u8 >> 1) as usize]
        }
    }

    /// Whether this is an object in one of the heap's arenas.
    pub const fn is_heap(self) -> bool {
        self.0 & 1 == 1 && (self.0 as u8 >> 1) >= Tag::Pair as u8
    }

    pub(crate) const fn payload(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The word itself, as the wasm ABI and the image (S4) pass it.
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    /// The value whose word is `bits`, if it is one of a fixnum or a tag
    /// (the image's); its index is not checked. Inlined, as the image's
    /// readers are (D90).
    #[inline(always)]
    pub(crate) fn from_bits(bits: u64) -> Option<Value> {
        let tagged = bits & 1 == 1 && ((bits as u8 >> 1) as usize) < TAGS.len();
        (bits & 3 == 0 || tagged).then_some(Value(bits))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.tag() {
            Tag::Fixnum => write!(f, "Fixnum({})", self.0 as i64 >> 2),
            Tag::Char => write!(f, "Char({:?})", self.as_char()),
            tag if tag >= Tag::Symbol => write!(f, "{tag:?}({})", self.payload()),
            tag => write!(f, "{tag:?}"),
        }
    }
}

/// What a dead slot of the heap holds.
impl Default for Value {
    fn default() -> Value {
        Value::UNSPECIFIED
    }
}

// --- strings -----------------------------------------------------------------

/// Characters between two entries of a string's index.
const STRIDE: usize = 32;

/// A Scheme string: UTF-8, mutable as Guile's are. It is all ASCII when it
/// has as many characters as bytes, and then `string-ref` is a byte load;
/// otherwise the byte offset of every 32nd character is indexed at the first
/// access by position, and kept until a change moves the bytes.
#[derive(Default)]
pub struct Str {
    text: String,
    chars: usize,
    index: OnceCell<Box<[u32]>>,
}

impl Str {
    pub fn new(text: String) -> Str {
        let chars = text.chars().count();
        Str {
            text,
            chars,
            index: OnceCell::new(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The length in characters, `string-length`.
    pub fn len(&self) -> usize {
        self.chars
    }

    pub fn is_empty(&self) -> bool {
        self.chars == 0
    }

    /// The flag of chapter 03 §5.1: one byte per character.
    pub fn is_ascii(&self) -> bool {
        self.chars == self.text.len()
    }

    /// The byte offset of character `k`, for `k` up to the length.
    pub fn byte_offset(&self, k: usize) -> Option<usize> {
        if k > self.chars {
            return None;
        }
        if self.is_ascii() || k == self.chars {
            return Some(if k == self.chars { self.text.len() } else { k });
        }
        let index = self.index.get_or_init(|| {
            let mut index = Vec::with_capacity(self.chars / STRIDE + 1);
            for (i, (offset, _)) in self.text.char_indices().enumerate() {
                if i % STRIDE == 0 {
                    index.push(offset as u32);
                }
            }
            index.into_boxed_slice()
        });
        let from = index[k / STRIDE] as usize;
        let rest = &self.text[from..];
        Some(
            from + rest
                .char_indices()
                .nth(k % STRIDE)
                .map_or(rest.len(), |(offset, _)| offset),
        )
    }

    /// Character `k`, `string-ref`.
    pub fn char_at(&self, k: usize) -> Option<char> {
        if self.is_ascii() {
            return self.text.as_bytes().get(k).map(|&b| b as char);
        }
        self.text[self.byte_offset(k)?..].chars().next()
    }

    /// Replaces character `k`, `string-set!`; false if `k` is out of range.
    pub fn set_char(&mut self, k: usize, c: char) -> bool {
        let Some(at) = self.byte_offset(k).filter(|_| k < self.chars) else {
            return false;
        };
        let old = self.text[at..].chars().next().map_or(0, char::len_utf8);
        self.text
            .replace_range(at..at + old, c.encode_utf8(&mut [0; 4]));
        if old != c.len_utf8() {
            self.index = OnceCell::new();
        }
        true
    }
}

// --- how Guile 3 writes text -------------------------------------------------

/// What Guile's printer asks of a character's Unicode general category.
#[derive(Clone, Copy, PartialEq)]
enum Class {
    /// L, M, N, P or S: written as itself.
    Graphic,
    /// Zs.
    Space,
    /// Co: not graphic, but allowed in symbols.
    Private,
    /// Cc, Cf, Zl, Zp and noncharacters.
    Other,
}

/// The non-ASCII characters that are not graphic, by category: Cc, Zs, Cf,
/// Zl, Zp and Co. Unassigned code points (Cn) count as graphic, except the
/// noncharacters.
const NOT_GRAPHIC: [(u32, u32, Class); 33] = [
    (0x80, 0x9f, Class::Other),
    (0xa0, 0xa0, Class::Space),
    (0xad, 0xad, Class::Other),
    (0x600, 0x605, Class::Other),
    (0x61c, 0x61c, Class::Other),
    (0x6dd, 0x6dd, Class::Other),
    (0x70f, 0x70f, Class::Other),
    (0x890, 0x891, Class::Other),
    (0x8e2, 0x8e2, Class::Other),
    (0x1680, 0x1680, Class::Space),
    (0x180e, 0x180e, Class::Other),
    (0x2000, 0x200a, Class::Space),
    (0x200b, 0x200f, Class::Other),
    (0x2028, 0x2029, Class::Other),
    (0x202a, 0x202e, Class::Other),
    (0x202f, 0x202f, Class::Space),
    (0x205f, 0x205f, Class::Space),
    (0x2060, 0x2064, Class::Other),
    (0x2066, 0x206f, Class::Other),
    (0x3000, 0x3000, Class::Space),
    (0xe000, 0xf8ff, Class::Private),
    (0xfdd0, 0xfdef, Class::Other),
    (0xfeff, 0xfeff, Class::Other),
    (0xfff9, 0xfffb, Class::Other),
    (0x110bd, 0x110bd, Class::Other),
    (0x110cd, 0x110cd, Class::Other),
    (0x13430, 0x1343f, Class::Other),
    (0x1bca0, 0x1bca3, Class::Other),
    (0x1d173, 0x1d17a, Class::Other),
    (0xe0001, 0xe0001, Class::Other),
    (0xe0020, 0xe007f, Class::Other),
    (0xf0000, 0xffffd, Class::Private),
    (0x100000, 0x10fffd, Class::Private),
];

fn class(c: char) -> Class {
    let n = c as u32;
    match n {
        0x20 => Class::Space,
        0..0x20 | 0x7f => Class::Other,
        0x21..0x7f => Class::Graphic,
        _ if n & 0xfffe == 0xfffe => Class::Other,
        _ => NOT_GRAPHIC
            .iter()
            .find(|&&(from, to, _)| (from..=to).contains(&n))
            .map_or(Class::Graphic, |&(_, _, class)| class),
    }
}

/// Guile's names for the characters up to the space; 0x7f is `delete`.
const CHAR_NAMES: [&str; 33] = [
    "nul",
    "soh",
    "stx",
    "etx",
    "eot",
    "enq",
    "ack",
    "alarm",
    "backspace",
    "tab",
    "newline",
    "vtab",
    "page",
    "return",
    "so",
    "si",
    "dle",
    "dc1",
    "dc2",
    "dc3",
    "dc4",
    "nak",
    "syn",
    "etb",
    "can",
    "em",
    "sub",
    "esc",
    "fs",
    "gs",
    "rs",
    "us",
    "space",
];

/// `(write c)`: `#\a`, `#\space`, `#\nul`, and a character that is not
/// graphic in octal, `#\240`. Guile also puts U+25CC before a combining
/// mark, which this does not (it has no table of the M categories).
pub fn write_char(c: char, out: &mut String) {
    out.push_str("#\\");
    match c as u32 {
        n @ 0..=0x20 => out.push_str(CHAR_NAMES[n as usize]),
        0x7f => out.push_str("delete"),
        _ if class(c) == Class::Graphic => out.push(c),
        n => {
            let _ = write!(out, "{n:o}");
        }
    }
}

/// `(write s)` of a string's text: `\"`, `\\`, the C escapes from `\a` to
/// `\r`, and other characters that are not graphic as `\xHH`, `\uHHHH` or
/// `\UHHHHHH`.
pub fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        let escape = match c {
            '"' => "\\\"",
            '\\' => "\\\\",
            '\x07' => "\\a",
            '\x08' => "\\b",
            '\t' => "\\t",
            '\n' => "\\n",
            '\x0b' => "\\v",
            '\x0c' => "\\f",
            '\r' => "\\r",
            ' ' => " ",
            _ if class(c) == Class::Graphic => {
                out.push(c);
                continue;
            }
            _ => {
                let _ = match c as u32 {
                    n @ 0..0x100 => write!(out, "\\x{n:02x}"),
                    n @ 0x100..0x10000 => write!(out, "\\u{n:04x}"),
                    n => write!(out, "\\U{n:06x}"),
                };
                continue;
            }
        };
        out.push_str(escape);
    }
    out.push('"');
}

/// Whether `c` may stand in a symbol written plainly: Guile's
/// `SUBSEQUENT_IDENTIFIER_MASK`, or `INITIAL_IDENTIFIER_MASK` for the first
/// character, which leaves out digits. Exact for ASCII; other characters are
/// taken as letters unless they are spaces, controls or format characters.
fn identifier(c: char, first: bool) -> bool {
    match c {
        '(' | ')' | '[' | ']' | '{' | '}' => false,
        '0'..='9' => !first,
        _ => matches!(class(c), Class::Graphic | Class::Private),
    }
}

/// `(write symbol)`, and `display`, which is the same in Guile: the name, or
/// `#{…}#` when the name would not read back as this symbol. Inside the
/// braces, a character that is neither an identifier character nor a space
/// is written `\xH;`.
pub fn write_symbol(name: &str, out: &mut String) {
    let mut chars = name.chars();
    let plain = match chars.next() {
        None | Some('\'' | '`' | ',' | '"' | ';' | '#') => false,
        Some('.') if name.len() == 1 => false,
        // Guile asks its reader; a name that reads as a number, or that
        // `string->number` refuses for its exponent, needs the braces.
        Some('+' | '-' | '.') if !matches!(crate::number::parse(name, 10), Ok(None)) => false,
        Some(c) => {
            identifier(c, true)
                && chars.all(|c| identifier(c, false) && !matches!(c, '"' | ';' | '#'))
        }
    };
    if plain {
        out.push_str(name);
        return;
    }
    out.push_str("#{");
    for c in name.chars() {
        if identifier(c, false) || class(c) == Class::Space {
            out.push(c);
        } else {
            let _ = write!(out, "\\x{:x};", c as u32);
        }
    }
    out.push_str("}#");
}
