//! The reader (chapter 03 §5.2, R7, R8): Guile 3.0.11's `read`
//! (`ice-9/read.scm`) over UTF-8 text, called at a byte offset. It returns
//! the datum, where it begins and ends, and the spans of the pairs it built,
//! which is what Guile records as source properties.
//!
//! The read options are Guile's defaults, as LilyPond leaves them: square
//! brackets on, keywords as `#:name` only, no curly infix, no R6RS hex
//! escapes, no hungry escapes of newlines. `#!fold-case`, `#!no-fold-case`
//! and `#!r6rs` change them until the end of one call; Guile keeps them on
//! the port. LilyPond's `#{ … #}` goes to an [`Embedded`] reader, the
//! parser's in chapter 02 §5.3 and [`LilyScan`] until then.
//!
//! Errors are Guile's: the key, the procedure, the message with its `~A` and
//! `~S`, and the arguments ([`ReadError`]). A `read-error`'s message begins
//! with the file, line and column of the port, which [`ReadError::text`]
//! puts in from [`Position`].

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt::Write;

use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::number::NumError;
use crate::value::{Tag, Value};

/// How deeply data may nest. The reader recurses once per level, on wasm's
/// stack, which is 1 MiB: an optimised build uses 260–440 bytes a level.
/// Guile has no limit.
pub const MAX_DEPTH: usize = 1000;

/// A datum read by [`read`].
#[derive(Debug)]
pub struct Read {
    /// The datum, or `Value::EOF` when only whitespace and comments were left.
    pub datum: Value,
    /// The byte where the datum begins, after whitespace and comments; `end`
    /// at the end of the text.
    pub start: usize,
    /// The byte after the datum, where Guile's port is left.
    pub end: usize,
    /// Whether reading looked at the character at `end`: it ended a token,
    /// or a `#t` or `#f`. A port that cannot put a character back loses it,
    /// as LilyPond's `#{ … #}` does.
    pub peeked: bool,
    /// Every pair read from the text, with its bytes, inner ones first. The
    /// pairs of `#;` comments are not kept.
    pub spans: Vec<Span>,
}

/// The bytes a pair was read from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub value: Value,
    pub start: usize,
    pub end: usize,
}

/// What Guile throws when it cannot read: `(key subr message args rest)`.
#[derive(Clone, PartialEq, Debug)]
pub struct ReadError {
    /// `read-error`, or the key of the procedure that failed while reading:
    /// `out-of-range` (`string->number`, `integer->char`) or
    /// `wrong-type-arg` (`integer->char`, `map`, `char=?`, `write-char`).
    pub key: &'static str,
    /// The procedure's name; none for a `read-error`.
    pub subr: Option<&'static str>,
    /// Guile's message, with `~A` and `~S` for `args`. A `read-error`'s
    /// begins with the place, which [`ReadError::text`] gives.
    pub message: &'static str,
    pub args: Vec<Value>,
    /// For a `read-error`, the byte Guile's port has reached.
    pub at: Option<usize>,
}

impl ReadError {
    #[cold]
    fn read(at: usize, message: &'static str, args: Vec<Value>) -> Box<ReadError> {
        Box::new(ReadError {
            key: "read-error",
            subr: None,
            message,
            args,
            at: Some(at),
        })
    }

    #[cold]
    fn of(
        key: &'static str,
        subr: &'static str,
        message: &'static str,
        args: Vec<Value>,
    ) -> Box<Self> {
        Box::new(ReadError {
            key,
            subr: Some(subr),
            message,
            args,
            at: None,
        })
    }

    /// The message with its arguments, as Guile's `simple-format` puts them
    /// in: `~A` displayed, `~S` written.
    pub fn message(&self, heap: &Heap) -> String {
        let mut out = String::new();
        let mut args = self.args.iter();
        let mut chars = self.message.chars();
        while let Some(c) = chars.next() {
            if c != '~' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('A' | 'a') => {
                    heap.display(*args.next().unwrap_or(&Value::UNSPECIFIED), &mut out)
                }
                Some('S' | 's') => {
                    heap.write(*args.next().unwrap_or(&Value::UNSPECIFIED), &mut out)
                }
                Some('%') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('~'),
            }
        }
        out
    }

    /// What Guile prints for the error: a `read-error` is
    /// `file:line:column: message`, with the line and column of `at` counted
    /// from 1, and `#<unknown port>` for a port without a file name.
    pub fn text(&self, heap: &Heap, file: Option<&str>, at: Position) -> String {
        let message = self.message(heap);
        if self.at.is_none() {
            return message;
        }
        let mut out = String::new();
        let _ = write!(
            out,
            "{}:{}:{}: {message}",
            file.unwrap_or("#<unknown port>"),
            at.line + 1,
            at.column + 1
        );
        out
    }
}

/// A port's line and column, from 0, as Guile counts them.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    /// Where the port is after reading `text` from here: a newline starts a
    /// line, a carriage return the column, a tab goes to the next multiple of
    /// 8, a backspace goes back one, a bell stays (Guile's
    /// `update_port_position`).
    pub fn advance(mut self, text: &str) -> Position {
        for c in text.chars() {
            match c {
                '\x07' => {}
                '\x08' => self.column = self.column.saturating_sub(1),
                '\n' => {
                    self.line += 1;
                    self.column = 0;
                }
                '\r' => self.column = 0,
                '\t' => self.column += 8 - self.column % 8,
                _ => self.column += 1,
            }
        }
        self
    }
}

/// A reader of LilyPond's `#{ … #}`, which `read-hash-extend` gives to the
/// parser (chapter 02 §5.3).
pub trait Embedded {
    /// Reads from `start`, the byte after `#{`, to the end of `#}`: the datum
    /// and the byte after `#}`.
    fn read(
        &mut self,
        heap: &mut Heap,
        text: &str,
        start: usize,
    ) -> Result<(Value, usize), ReadError>;
}

/// Reads one datum of `text` from the byte `offset`, as Guile's `read` from
/// a port at that byte. With `embedded`, `#{` starts LilyPond's syntax;
/// without, Guile's `#{symbol}#`.
///
/// The heap does not collect while reading; the caller roots the datum
/// before its next safe point.
pub fn read(
    heap: &mut Heap,
    text: &str,
    offset: usize,
    embedded: Option<&mut dyn Embedded>,
) -> Result<Read, ReadError> {
    let dot = heap.intern(".");
    let mut reader = Reader {
        heap,
        text,
        pos: offset,
        peeked: false,
        depth: 0,
        fold_case: false,
        r6rs_escapes: false,
        hungry_eol: false,
        dot,
        spans: Vec::new(),
        embedded,
    };
    let (datum, start) = match reader.next_non_whitespace().map_err(|e| *e)? {
        None => (Value::EOF, reader.pos),
        Some(c) => {
            let start = reader.pos - c.len_utf8();
            (reader.read_expr(c).map_err(|e| *e)?, start)
        }
    };
    Ok(Read {
        datum,
        start,
        end: reader.pos,
        peeked: reader.peeked,
        spans: reader.spans,
    })
}

/// Errors are boxed inside the reader, which keeps its frames small: it
/// recurses once per level of nesting.
type Result<T, E = Box<ReadError>> = core::result::Result<T, E>;

struct Reader<'t, 'e> {
    heap: &'t mut Heap,
    text: &'t str,
    pos: usize,
    /// The last thing done was to look at the character at `pos`.
    peeked: bool,
    depth: usize,
    fold_case: bool,
    r6rs_escapes: bool,
    hungry_eol: bool,
    dot: Value,
    spans: Vec<Span>,
    embedded: Option<&'e mut dyn Embedded>,
}

/// Guile's `delimiter?` with square brackets on.
fn delimiter(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | ';' | '"' | ' ' | '\r' | '\x0c' | '\n' | '\t' | '[' | ']'
    )
}

/// Unicode's space separators, Guile's `Zs`.
fn is_zs(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// The character names Guile reads after `#\`, in the order it tries them:
/// R5RS's, R6RS's, R7RS's, the C0 controls' and its own.
const CHAR_NAMES: &[(&str, char)] = &[
    ("space", ' '),
    ("newline", '\n'),
    ("nul", '\0'),
    ("alarm", '\x07'),
    ("backspace", '\x08'),
    ("tab", '\t'),
    ("linefeed", '\n'),
    ("vtab", '\x0b'),
    ("page", '\x0c'),
    ("return", '\r'),
    ("esc", '\x1b'),
    ("delete", '\x7f'),
    ("escape", '\x1b'),
    ("soh", '\x01'),
    ("stx", '\x02'),
    ("etx", '\x03'),
    ("eot", '\x04'),
    ("enq", '\x05'),
    ("ack", '\x06'),
    ("bel", '\x07'),
    ("bs", '\x08'),
    ("ht", '\x09'),
    ("lf", '\x0a'),
    ("vt", '\x0b'),
    ("ff", '\x0c'),
    ("cr", '\x0d'),
    ("so", '\x0e'),
    ("si", '\x0f'),
    ("dle", '\x10'),
    ("dc1", '\x11'),
    ("dc2", '\x12'),
    ("dc3", '\x13'),
    ("dc4", '\x14'),
    ("nak", '\x15'),
    ("syn", '\x16'),
    ("etb", '\x17'),
    ("can", '\x18'),
    ("em", '\x19'),
    ("sub", '\x1a'),
    ("fs", '\x1c'),
    ("gs", '\x1d'),
    ("rs", '\x1e'),
    ("us", '\x1f'),
    ("sp", ' '),
    ("del", '\x7f'),
    ("null", '\0'),
    ("nl", '\n'),
    ("np", '\x0c'),
];

impl Reader<'_, '_> {
    fn next(&mut self) -> Option<char> {
        let c = self.text[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        self.peeked = false;
        Some(c)
    }

    fn peek(&mut self) -> Option<char> {
        let c = self.text[self.pos..].chars().next();
        self.peeked = c.is_some();
        c
    }

    fn error(&self, message: &'static str) -> Box<ReadError> {
        ReadError::read(self.pos, message, Vec::new())
    }

    fn error_with(&self, message: &'static str, arg: Value) -> Box<ReadError> {
        ReadError::read(self.pos, message, vec![arg])
    }

    /// `take-until delimiter?`: the token that began at `start`, whose first
    /// character has been read.
    fn token(&mut self, start: usize) -> &str {
        while let Some(c) = self.peek() {
            if delimiter(c) {
                break;
            }
            self.next();
        }
        &self.text[start..self.pos]
    }

    fn next_non_whitespace(&mut self) -> Result<Option<char>> {
        loop {
            match self.next() {
                Some(';') => loop {
                    match self.next() {
                        None => return Ok(None),
                        Some('\n') => break,
                        Some(_) => {}
                    }
                },
                Some('#') => match self.peek() {
                    Some('!') => {
                        self.next();
                        self.shebang()?;
                    }
                    Some(';') => {
                        self.next();
                        let spans = self.spans.len();
                        self.read_subexpression(
                            "unexpected end of input while reading #; comment",
                        )?;
                        self.spans.truncate(spans);
                    }
                    Some('|') => {
                        self.next();
                        self.block_comment()?;
                    }
                    _ => return Ok(Some('#')),
                },
                Some(' ' | '\r' | '\x0c' | '\n' | '\t') => {}
                other => return Ok(other),
            }
        }
    }

    /// After `#|`, to the end of the comment; they nest.
    #[inline(never)]
    fn block_comment(&mut self) -> Result<()> {
        let mut depth = 1;
        loop {
            match self.next() {
                None => return Err(self.error("unterminated `#| ... |#' comment")),
                Some('|') if self.peek() == Some('#') => {
                    self.next();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some('#') if self.peek() == Some('|') => {
                    self.next();
                    depth += 1;
                }
                Some(_) => {}
            }
        }
    }

    /// After `#!`: a directive, or a comment to `!#`.
    #[inline(never)]
    fn shebang(&mut self) -> Result<()> {
        let directive_char = |c: char| c == '-' || c.is_alphanumeric();
        let start = self.pos;
        if self.peek().is_some_and(directive_char) {
            self.next();
            while self.peek().is_some_and(directive_char) {
                self.next();
            }
        }
        match &self.text[start..self.pos] {
            "r6rs" => {
                self.fold_case = false;
                self.r6rs_escapes = true;
                self.hungry_eol = true;
            }
            "fold-case" => self.fold_case = true,
            "no-fold-case" => self.fold_case = false,
            "curly-infix" | "curly-infix-and-bracket-lists" => {
                return Err(self.error("curly infix is not supported"));
            }
            _ => {
                let mut c = self.next();
                loop {
                    match c {
                        None => return Err(self.error("unterminated `#! ... !#' comment")),
                        Some('!') => {
                            c = self.next();
                            if c == Some('#') {
                                break;
                            }
                        }
                        Some(_) => c = self.next(),
                    }
                }
            }
        }
        Ok(())
    }

    fn read_subexpression(&mut self, eof: &'static str) -> Result<Value> {
        match self.next_non_whitespace()? {
            None => Err(self.error(eof)),
            Some(c) => self.read_expr(c),
        }
    }

    /// The datum that begins with `c`, which has been read; a pair's span
    /// is kept.
    fn read_expr(&mut self, c: char) -> Result<Value> {
        let start = self.pos - c.len_utf8();
        if self.depth == MAX_DEPTH {
            return Err(self.error("data nested too deeply"));
        }
        self.depth += 1;
        let value = self.read_expr_inner(c, start);
        self.depth -= 1;
        let value = value?;
        if value.tag() == Tag::Pair {
            self.spans.push(Span {
                value,
                start,
                end: self.pos,
            });
        }
        Ok(value)
    }

    fn read_expr_inner(&mut self, c: char, start: usize) -> Result<Value> {
        match c {
            '(' => self.read_parenthesized(')'),
            '[' => self.read_parenthesized(']'),
            '"' => self.read_string(),
            '\'' => self.wrap(
                "quote",
                "unexpected end of input while reading quoted expression",
            ),
            '`' => self.wrap(
                "quasiquote",
                "unexpected end of input while reading quasiquoted expression",
            ),
            ',' => {
                if self.peek() == Some('@') {
                    self.next();
                    self.wrap(
                        "unquote-splicing",
                        "unexpected end of input while reading subexpression of ,@",
                    )
                } else {
                    self.wrap(
                        "unquote",
                        "unexpected end of input while reading unquoted expression",
                    )
                }
            }
            '#' => self.read_sharp(start),
            ')' => Err(self.error("unexpected \")\"")),
            ']' => Err(self.error("unexpected \"]\"")),
            '0'..='9' | '+' | '-' | '.' => self.read_number(start),
            _ => Ok(self.read_symbol(start)),
        }
    }

    /// `(name datum)`, for `'datum` and its kin.
    fn wrap(&mut self, name: &str, eof: &'static str) -> Result<Value> {
        let datum = self.read_subexpression(eof)?;
        let name = self.heap.intern(name);
        Ok(self.heap.list(&[name, datum]))
    }

    fn read_parenthesized(&mut self, close: char) -> Result<Value> {
        let mut items = Vec::new();
        let mut tail = Value::NULL;
        loop {
            let Some(c) = self.next_non_whitespace()? else {
                return Err(self.error_with(
                    "unexpected end of input while searching for: ~A",
                    Value::char(close),
                ));
            };
            if c == close {
                break;
            }
            if c == ')' || c == ']' {
                return Err(self.error_with("mismatched close paren: ~A", Value::char(c)));
            }
            let item = self.read_expr(c)?;
            if c == '.' && item == self.dot {
                tail = self.read_subexpression(
                    "unexpected end of input while reading tail of improper list",
                )?;
                let after = self.next_non_whitespace()?;
                if after != Some(close) {
                    let after = after.map_or(Value::EOF, Value::char);
                    return Err(self.error_with("missing close paren: ~A", after));
                }
                break;
            }
            items.push(item);
        }
        Ok(items
            .into_iter()
            .rev()
            .fold(tail, |tail, item| self.heap.cons(item, tail)))
    }

    #[inline(never)]
    fn read_string(&mut self) -> Result<Value> {
        const EOF: &str = "unexpected end of input while reading string";
        let mut out = String::new();
        loop {
            let c = self.next().ok_or_else(|| self.error(EOF))?;
            match c {
                '"' => break,
                '\\' => {
                    let c = self.next().ok_or_else(|| self.error(EOF))?;
                    let escaped = match c {
                        '\n' => {
                            while self.hungry_eol
                                && self.peek().is_some_and(|c| c == '\t' || is_zs(c))
                            {
                                self.next();
                            }
                            continue;
                        }
                        '"' | '|' | '\\' | '(' => c,
                        '0' => '\0',
                        'f' => '\x0c',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'a' => '\x07',
                        'v' => '\x0b',
                        'b' => '\x08',
                        'x' if self.r6rs_escapes => self.r6rs_hex_escape()?,
                        'x' => self.fixed_hex_escape(2)?,
                        'u' => self.fixed_hex_escape(4)?,
                        'U' => self.fixed_hex_escape(6)?,
                        _ => {
                            return Err(self.error_with(
                                "invalid character in escape sequence: ~S",
                                Value::char(c),
                            ));
                        }
                    };
                    out.push(escaped);
                }
                _ => out.push(c),
            }
        }
        Ok(self.heap.string(out))
    }

    /// `\xHHHH;`, after the `x`: one hex digit or more, then `;`.
    fn r6rs_hex_escape(&mut self) -> Result<char> {
        let mut n: i64 = 0;
        let mut digits = 0;
        loop {
            let c = self.next().ok_or_else(|| {
                self.error("unexpected end of input in character escape sequence")
            })?;
            match c.to_digit(16) {
                Some(d) => {
                    n = n.saturating_mul(16).saturating_add(d.into());
                    digits += 1;
                }
                None if c == ';' && digits > 0 => return integer_to_char(self.heap, n),
                None => {
                    return Err(
                        self.error_with("invalid character in escape sequence: ~S", Value::char(c))
                    );
                }
            }
        }
    }

    /// `len` hex digits, after `\x`, `\u` or `\U`.
    fn fixed_hex_escape(&mut self, len: usize) -> Result<char> {
        let mut n = 0;
        for _ in 0..len {
            let c = self.next().ok_or_else(|| {
                self.error("unexpected end of input in character escape sequence")
            })?;
            let d = c.to_digit(16).ok_or_else(|| {
                self.error_with("invalid character in escape sequence: ~S", Value::char(c))
            })?;
            n = n * 16 + d;
        }
        integer_to_char(self.heap, n as i64)
    }

    #[inline(never)]
    fn read_symbol(&mut self, start: usize) -> Value {
        self.token(start);
        self.symbol(start)
    }

    /// A token that begins like a number: the number, or else the symbol.
    #[inline(never)]
    fn read_number(&mut self, start: usize) -> Result<Value> {
        let text = self.text;
        self.token(start);
        let token = &text[start..self.pos];
        match self.heap.string_to_number(token, 10) {
            Ok(v) if v != Value::FALSE => Ok(v),
            Err(NumError::Exponent { start, end }) => {
                Err(exponent_error(self.heap, &token[start..end]))
            }
            _ => Ok(self.symbol(start)),
        }
    }

    /// The symbol of the token from `start`.
    fn symbol(&mut self, start: usize) -> Value {
        let token = &self.text[start..self.pos];
        if self.fold_case {
            let lower = token.to_lowercase();
            self.heap.intern(&lower)
        } else {
            self.heap.intern(token)
        }
    }

    /// After `#`, at `start`.
    fn read_sharp(&mut self, start: usize) -> Result<Value> {
        let c = self
            .next()
            .ok_or_else(|| self.error("unexpected end of input after #"))?;
        if c == '{'
            && let Some(embedded) = self.embedded.as_deref_mut()
        {
            let (datum, end) = embedded
                .read(self.heap, self.text, self.pos)
                .map_err(Box::new)?;
            self.pos = end;
            self.peeked = false;
            return Ok(datum);
        }
        match c {
            '\\' => self.read_character(),
            '(' => self.read_vector(),
            't' | 'T' => {
                self.boolean_tail("rue");
                Ok(Value::TRUE)
            }
            'f' if matches!(self.peek(), Some('3' | '6')) => Err(self.unsupported(start)),
            'f' | 'F' => {
                self.boolean_tail("alse");
                Ok(Value::FALSE)
            }
            ':' => {
                let datum =
                    self.read_subexpression("unexpected end of input while reading keyword")?;
                match self.heap.name(datum) {
                    Some(name) if datum.tag() == Tag::Symbol => {
                        let name = String::from(name);
                        Ok(self.heap.keyword(&name))
                    }
                    _ => {
                        Err(self
                            .error_with("keyword prefix #: not followed by a symbol: ~a", datum))
                    }
                }
            }
            'i' | 'e' | 'b' | 'B' | 'o' | 'O' | 'd' | 'D' | 'x' | 'X' | 'I' | 'E' => {
                self.read_prefixed_number(start)
            }
            '{' => self.read_extended_symbol(),
            '\'' => self.wrap(
                "syntax",
                "unexpected end of input while reading syntax expression",
            ),
            '`' => self.wrap(
                "quasisyntax",
                "unexpected end of input while reading quasisyntax expression",
            ),
            ',' => {
                if self.peek() == Some('@') {
                    self.next();
                    self.wrap(
                        "unsyntax-splicing",
                        "unexpected end of input while reading unsyntax-splicing expression",
                    )
                } else {
                    self.wrap(
                        "unsyntax",
                        "unexpected end of input while reading unsyntax expression",
                    )
                }
            }
            'v' => self.read_bytevector(),
            '0'..='9' | '@' => self.read_array(c, start),
            // SRFI-4 vectors, bit vectors and Elisp's #nil: Guile reads
            // them, LilyPond's Scheme has none.
            'u' | 's' | 'c' | '*' | 'n' => Err(self.unsupported(start)),
            _ => {
                let text = self.heap.string(&self.text[start..self.pos]);
                Err(self.error_with("Unknown # object: ~S", text))
            }
        }
    }

    /// `#x1F` and its kin, from the `#` at `start`.
    #[inline(never)]
    fn read_prefixed_number(&mut self, start: usize) -> Result<Value> {
        let text = self.text;
        self.token(start);
        let token = &text[start..self.pos];
        match self.heap.string_to_number(token, 10) {
            Ok(v) if v != Value::FALSE => Ok(v),
            Err(NumError::Exponent { start, end }) => {
                Err(exponent_error(self.heap, &token[start..end]))
            }
            _ => {
                let token = self.heap.string(token);
                Err(self.error_with("unknown # object: ~S", token))
            }
        }
    }

    #[cold]
    fn unsupported(&mut self, start: usize) -> Box<ReadError> {
        let text = self.heap.string(&self.text[start..self.pos]);
        self.error_with("~S objects are not supported", text)
    }

    /// The rest of `#true` or `#false`, in any case; none of it if it is not
    /// all there. Guile needs no delimiter after `#t` or `#f`.
    fn boolean_tail(&mut self, tail: &str) {
        let start = self.pos;
        for expected in tail.chars() {
            match self.peek() {
                Some(c) if c.to_ascii_lowercase() == expected => {
                    self.next();
                }
                _ => {
                    self.pos = start;
                    self.peeked = start < self.text.len();
                    return;
                }
            }
        }
    }

    /// After `#\`.
    #[inline(never)]
    fn read_character(&mut self) -> Result<Value> {
        let start = self.pos;
        let c = self
            .next()
            .ok_or_else(|| self.error("unexpected end of input after #\\"))?;
        if delimiter(c) {
            return Ok(Value::char(c));
        }
        let text = self.text;
        self.token(start);
        let token = &text[start..self.pos];
        let mut chars = token.chars();
        chars.next();
        match (chars.next(), chars.next()) {
            (None, _) | (Some('\u{25cc}'), None) => return Ok(Value::char(c)),
            _ => {}
        }
        let code = if ('0'..='7').contains(&c) {
            self.heap.string_to_number(token, 8).ok()
        } else {
            None
        };
        let code = code.filter(|&v| v != Value::FALSE).or_else(|| {
            (c == 'x')
                .then(|| self.heap.string_to_number(&token[1..], 16).ok())
                .flatten()
                .filter(|&v| v != Value::FALSE)
        });
        if let Some(code) = code {
            if !self.heap.is_exact_integer(code) {
                return Err(ReadError::of(
                    "wrong-type-arg",
                    "integer->char",
                    "Wrong type argument in position 1 (expecting small integer): ~S",
                    vec![code],
                ));
            }
            return match self.heap.to_i64(code) {
                Some(n) => integer_to_char(self.heap, n).map(Value::char),
                None => Err(out_of_range_char(code)),
            };
        }
        if let Some(&(_, c)) = CHAR_NAMES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(token))
        {
            return Ok(Value::char(c));
        }
        let token = self.heap.string(token);
        Err(self.error_with("unknown character name ~a", token))
    }

    fn read_vector(&mut self) -> Result<Value> {
        let list = self.read_parenthesized(')')?;
        let items = self.items(list)?;
        Ok(self.heap.vector(items))
    }

    /// The items of a proper list, or `map`'s error: Guile maps over what
    /// `#(`, `#vu8(` and arrays read.
    fn items(&self, list: Value) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut rest = list;
        while let (Some(car), Some(cdr)) = (self.heap.car(rest), self.heap.cdr(rest)) {
            items.push(car);
            rest = cdr;
        }
        if rest == Value::NULL {
            Ok(items)
        } else {
            Err(ReadError::of(
                "wrong-type-arg",
                "map",
                "Not a list: ~S",
                vec![list],
            ))
        }
    }

    /// `#vu8(…)`, after `#v`.
    fn read_bytevector(&mut self) -> Result<Value> {
        for expected in ['u', '8', '('] {
            if self.next() != Some(expected) {
                return Err(self.error_with("invalid bytevector prefix", Value::char(expected)));
            }
        }
        let list = self.read_parenthesized(')')?;
        let mut bytes = Vec::new();
        for byte in self.items(list)? {
            let subr = "bytevector-u8-set!";
            match byte.as_fixnum() {
                Some(n) if (0..=255).contains(&n) => bytes.push(n as u8),
                _ if self.heap.is_exact_integer(byte) => {
                    return Err(ReadError::of(
                        "out-of-range",
                        subr,
                        "Value out of range: ~S",
                        vec![byte],
                    ));
                }
                _ => {
                    let position = self.heap.integer(3);
                    return Err(ReadError::of(
                        "wrong-type-arg",
                        subr,
                        "Wrong type argument in position ~A: ~S",
                        vec![position, byte],
                    ));
                }
            }
        }
        Ok(self.heap.wrap(Bytevector(bytes)))
    }

    /// Guile's `read-decimal-integer`: an optional `-` and digits from `c`,
    /// or `alt` without digits; and the character after them, which has been
    /// read.
    fn decimal(&mut self, c: Option<char>, alt: i64) -> (Option<char>, i64) {
        let (sign, mut c) = if c == Some('-') {
            (-1, self.next())
        } else {
            (1, c)
        };
        let mut n: Option<i64> = None;
        while let Some(digit) = c.and_then(|c| c.to_digit(10)) {
            n = Some(
                n.unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit.into()),
            );
            c = self.next();
        }
        (c, n.map_or(alt, |n| n * sign))
    }

    /// An array, after `#`; `c` is the rank's first digit or `@`: Guile's
    /// `read-array` and `list->typed-array`, for arrays of any type only, as
    /// `qr-code.scm`'s `#1@1(17 32 …)` and `#2((#f #f …) …)`. A rank-1
    /// array whose indices start at 0 is a vector; others are an [`Array`].
    fn read_array(&mut self, c: char, start: usize) -> Result<Value> {
        const EOF: &str = "unexpected end of input while reading array";
        let (c, rank) = self.decimal(Some(c), 1);
        let mut c = c.ok_or_else(|| self.error(EOF))?;
        let tag = self.pos - c.len_utf8();
        while !matches!(c, '(' | '@' | ':') {
            c = self.next().ok_or_else(|| self.error(EOF))?;
        }
        // Rank 0 and typed arrays (`#1u8(…)`) are not read.
        if !(1..=MAX_DEPTH as i64).contains(&rank) || self.pos - c.len_utf8() != tag {
            return Err(self.unsupported(start));
        }
        let rank = rank as usize;
        // Each dimension's lower bound and, if written, its length.
        let mut shape = Vec::new();
        while c == '@' || c == ':' {
            let (mut after, lower) = if c == '@' {
                let next = self.next();
                self.decimal(next, 0)
            } else {
                (Some(c), 0)
            };
            let mut length = None;
            if after == Some(':') {
                let next = self.next();
                let (rest, n) = self.decimal(next, 0);
                if n < 0 {
                    return Err(self.error("array length must be non-negative"));
                }
                (after, length) = (rest, Some(n as usize));
            }
            c = after.ok_or_else(|| self.error(EOF))?;
            shape.push((lower, length));
        }
        if c != '(' {
            return Err(self.error("missing '(' in vector or array literal"));
        }
        let elements = self.read_parenthesized(')')?;
        self.items(elements)?;
        if !shape.is_empty() && shape.len() != rank {
            return Err(self.error("the number of shape specifications must match the array rank"));
        }
        // A length not written is that of the first list at its depth.
        let mut dimensions = Vec::with_capacity(rank);
        let mut row = elements;
        for k in 0..rank {
            let (lower, length) = shape.get(k).copied().unwrap_or((0, None));
            let length = match length {
                Some(length) => length,
                None => self.items(row).map(|items| items.len()).map_err(|_| {
                    let position = self.heap.integer(1);
                    ReadError::of(
                        "wrong-type-arg",
                        "length",
                        "Wrong type argument in position ~A: ~S",
                        vec![position, row],
                    )
                })?,
            };
            dimensions.push((lower, length));
            if k + 1 < rank && row != Value::NULL {
                row = self.heap.car(row).ok_or_else(|| {
                    let (position, expecting) = (self.heap.integer(1), self.heap.string("pair"));
                    ReadError::of(
                        "wrong-type-arg",
                        "car",
                        "Wrong type argument in position ~A (expecting ~A): ~S",
                        vec![position, expecting, row],
                    )
                })?;
            }
        }
        let mut items = Vec::new();
        self.fill(elements, &dimensions, 0, &mut items)?;
        if rank == 1 && dimensions[0].0 == 0 {
            return Ok(self.heap.vector(items));
        }
        Ok(self.heap.wrap(Array {
            shape: dimensions,
            items,
        }))
    }

    /// Guile's `l2ra`: the elements of dimension `k` and inner ones, row by
    /// row, into `items`.
    fn fill(
        &mut self,
        list: Value,
        dimensions: &[(i64, usize)],
        k: usize,
        items: &mut Vec<Value>,
    ) -> Result<()> {
        if k == dimensions.len() {
            items.push(list);
            return Ok(());
        }
        let length = dimensions[k].1;
        let (mut n, mut rest) = (length, list);
        while n > 0
            && let (Some(car), Some(cdr)) = (self.heap.car(rest), self.heap.cdr(rest))
        {
            self.fill(car, dimensions, k + 1, items)?;
            (n, rest) = (n - 1, cdr);
        }
        let message = if rest != Value::NULL {
            "too many elements for array dimension ~a, want ~a"
        } else if n != 0 {
            "too few elements for array dimension ~a, need ~a"
        } else {
            return Ok(());
        };
        let args = vec![
            self.heap.integer(k as i64),
            self.heap.integer(length as i64),
        ];
        Err(Box::new(ReadError {
            key: "misc-error",
            subr: None,
            message,
            args,
            at: None,
        }))
    }

    /// `#{symbol}#`, after `#{`.
    fn read_extended_symbol(&mut self) -> Result<Value> {
        let mut name = String::new();
        let mut brace = false;
        let mut pending = None;
        loop {
            let c = match pending.take() {
                Some(c) => c,
                None => self
                    .next()
                    .ok_or_else(|| self.error("end of input while reading symbol"))?,
            };
            if brace {
                brace = false;
                if c == '#' {
                    break;
                }
                name.push('}');
                pending = Some(c);
            } else if c == '}' {
                brace = true;
            } else if c == '\\' {
                let c = self
                    .next()
                    .ok_or_else(|| self.error("end of input while reading symbol"))?;
                name.push(if c == 'x' { self.r6rs_hex_escape()? } else { c });
            } else {
                name.push(c);
            }
        }
        Ok(self.heap.intern(&name))
    }
}

/// A bytevector, `#vu8(…)`: a foreign object until the primitives of S3
/// need one of the heap's own. LilyPond's Scheme writes one, in
/// `framework-ps.scm`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bytevector(pub Vec<u8>);

impl Foreign for Bytevector {
    fn type_name(&self) -> &'static str {
        "bytevector"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#vu8(");
        for (i, byte) in self.0.iter().enumerate() {
            let _ = write!(out, "{}{byte}", if i == 0 { "" } else { " " });
        }
        out.push(')');
    }

    fn equal(&self, other: &dyn Foreign, _: &Heap) -> bool {
        (other as &dyn Any).downcast_ref::<Bytevector>() == Some(self)
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.bytes(&self.0);
        true
    }
}

pub(crate) fn load_bytevector(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(Bytevector(r.bytes()?.into())))
}

/// An array that is not a vector: of more than one dimension, or with
/// indices that do not start at 0; `array.rs`' primitives make and read it
/// (MS4 step 17, D229).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Array {
    /// Each dimension's lower bound and length.
    pub shape: Vec<(i64, usize)>,
    /// The elements, the last dimension's index the fastest.
    pub items: Vec<Value>,
}

impl Array {
    /// The elements of dimension `k` from `items[at..]`, nested; returns
    /// where they end.
    fn print_dimension(&self, heap: &Heap, k: usize, mut at: usize, out: &mut String) -> usize {
        if k == self.shape.len() {
            heap.write(self.items[at], out);
            return at + 1;
        }
        out.push('(');
        for i in 0..self.shape[k].1 {
            if i > 0 {
                out.push(' ');
            }
            at = self.print_dimension(heap, k + 1, at, out);
        }
        out.push(')');
        at
    }
}

impl Foreign for Array {
    fn type_name(&self) -> &'static str {
        "array"
    }

    /// As Guile: `#2@1@0:3…`, every lower bound when one is not 0, the
    /// lengths only when an empty dimension hides a later one's; an array
    /// of no dimension `#0(x)`.
    fn print(&self, heap: &Heap, out: &mut String) {
        let _ = write!(out, "#{}", self.shape.len());
        let lengths = self
            .shape
            .iter()
            .skip_while(|&&(_, length)| length != 0)
            .any(|&(_, length)| length != 0);
        let lowers = self.shape.iter().any(|&(lower, _)| lower != 0);
        for &(lower, length) in &self.shape {
            if lowers {
                let _ = write!(out, "@{lower}");
            }
            if lengths {
                let _ = write!(out, ":{length}");
            }
        }
        if self.shape.is_empty() {
            out.push('(');
            self.print_dimension(heap, 0, 0, out);
            out.push(')');
        } else {
            self.print_dimension(heap, 0, 0, out);
        }
    }

    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        (other as &dyn Any)
            .downcast_ref::<Array>()
            .is_some_and(|other| {
                other.shape == self.shape
                    && self
                        .items
                        .iter()
                        .zip(&other.items)
                        .all(|(&a, &b)| heap.equal(a, b))
            })
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for &item in &self.items {
            tracer.trace(item);
        }
    }

    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        Some(Box::new(self.clone()))
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.count(self.shape.len());
        for &(lower, length) in &self.shape {
            out.u64(lower as u64);
            out.u64(length as u64);
        }
        out.values(&self.items);
        true
    }
}

pub(crate) fn load_array(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let shape = (0..r.count()?)
        .map(|_| Some((r.u64()? as i64, r.u64()? as usize)))
        .collect::<Option<_>>()?;
    Some(Box::new(Array {
        shape,
        items: r.values()?,
    }))
}

/// `string->number`'s error for an exponent beyond a double's: the exponent
/// as written.
fn exponent_error(heap: &mut Heap, exponent: &str) -> Box<ReadError> {
    let value = heap
        .string_to_number(exponent.strip_prefix('+').unwrap_or(exponent), 10)
        .unwrap_or(Value::FALSE);
    ReadError::of(
        "out-of-range",
        "string->number",
        "Value out of range: ~S",
        vec![value],
    )
}

fn out_of_range_char(code: Value) -> Box<ReadError> {
    ReadError::of(
        "out-of-range",
        "integer->char",
        "Argument 1 out of range: ~S",
        vec![code],
    )
}

fn integer_to_char(heap: &mut Heap, n: i64) -> Result<char> {
    u32::try_from(n)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| out_of_range_char(heap.integer(n)))
}

// --- LilyPond's #{ … #} ---------------------------------------------------------

/// LilyPond's `read-lily-expression` (`scm/lily/parser-ly-from-scheme.scm`),
/// natively: the text to `#}`, skipping LilyPond's strings and comments,
/// with each `#` and `$` form in it read as Scheme. Its datum is the call
/// LilyPond's reads as, which the parser runs (`embedded.rs`):
///
/// ```scheme
/// ((@@ (lily) read-lily-expression-internal) "text" file line
///  (list (cons byte (lambda () form)) …))
/// ```
///
/// with a closure for each form that is a symbol or a list other than a
/// quotation, `(apply values form)` for `#@` and `$@`. The line is counted
/// from the start of the text read, so it is right when that is a file.
pub struct LilyScan {
    /// The port's file name; `#f` without one.
    pub file: Option<String>,
}

impl LilyScan {
    fn eof(heap: &mut Heap, subr: &'static str) -> ReadError {
        let expecting = heap.string("character");
        *ReadError::of(
            "wrong-type-arg",
            subr,
            "Wrong type argument in position ~A (expecting ~A): ~S",
            vec![heap.integer(1), expecting, Value::EOF],
        )
    }
}

impl Embedded for LilyScan {
    fn read(
        &mut self,
        heap: &mut Heap,
        text: &str,
        start: usize,
    ) -> Result<(Value, usize), ReadError> {
        let char_at = |i: usize| text[i..].chars().next();
        let mut closures = Vec::new();
        let mut i = start;
        // `copy-char`: LilyPond's port gives end of input to `write-char`.
        let copy = |heap: &mut Heap, i: &mut usize| match char_at(*i) {
            Some(c) => {
                *i += c.len_utf8();
                Ok(c)
            }
            None => Err(LilyScan::eof(heap, "write-char")),
        };
        let close = loop {
            let c = char_at(i).ok_or_else(|| LilyScan::eof(heap, "char=?"))?;
            i += c.len_utf8();
            if c == '#' {
                match char_at(i) {
                    Some('}') => break i - 1,
                    None => return Err(LilyScan::eof(heap, "char=?")),
                    Some(_) => {}
                }
            }
            match c {
                '#' | '$' => {
                    let byte = i - start;
                    let multiple = match char_at(i) {
                        Some(c) => c == '@',
                        None => return Err(LilyScan::eof(heap, "char=?")),
                    };
                    if multiple {
                        i += 1;
                    }
                    let form = read(heap, text, i, Some(&mut *self))?;
                    i = form.end;
                    if form.peeked {
                        i += char_at(i).map_or(0, char::len_utf8);
                    }
                    let quote = heap.intern("quote");
                    let datum = form.datum;
                    let closure = match datum.tag() {
                        Tag::Symbol => true,
                        Tag::Pair => heap.car(datum) != Some(quote),
                        _ => false,
                    };
                    if closure {
                        let body = if multiple {
                            let apply = heap.intern("apply");
                            let values = heap.intern("values");
                            heap.list(&[apply, values, datum])
                        } else {
                            datum
                        };
                        let lambda = heap.intern("lambda");
                        let thunk = heap.list(&[lambda, Value::NULL, body]);
                        let cons = heap.intern("cons");
                        let byte = heap.integer(byte as i64);
                        closures.push(heap.list(&[cons, byte, thunk]));
                    }
                }
                '"' => loop {
                    match copy(heap, &mut i)? {
                        '"' => break,
                        '\\' => {
                            copy(heap, &mut i)?;
                        }
                        _ => {}
                    }
                },
                // LilyPond's test for `%}` is only `%`, and it copies the
                // character after it.
                '%' => match copy(heap, &mut i)? {
                    '{' => loop {
                        if copy(heap, &mut i)? == '%' {
                            copy(heap, &mut i)?;
                            break;
                        }
                    },
                    '\n' => {}
                    _ => while copy(heap, &mut i)? != '\n' {},
                },
                _ => {}
            }
        };
        let internal = {
            let at = heap.intern("@@");
            let lily = heap.intern("lily");
            let module = heap.list(&[lily]);
            let name = heap.intern("read-lily-expression-internal");
            heap.list(&[at, module, name])
        };
        let lily = heap.string(&text[start..close]);
        let file = match &self.file {
            Some(file) => heap.string(file.as_str()),
            None => Value::FALSE,
        };
        let line = Position::default().advance(&text[..start]).line;
        let line = heap.integer(line as i64);
        let list = heap.intern("list");
        let closures = closures
            .into_iter()
            .rev()
            .fold(Value::NULL, |tail, closure| heap.cons(closure, tail));
        let closures = heap.cons(list, closures);
        Ok((
            heap.list(&[internal, lily, file, line, closures]),
            close + 2,
        ))
    }
}
