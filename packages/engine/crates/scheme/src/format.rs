//! `simple-format`, which `(guile)` also binds as `format`, and the subset
//! of `(ice-9 format)`'s `format` that LilyPond's Scheme uses (chapter 03
//! §2.6): `~a`, `~s` and `~c`; `~d`, `~x`, `~o`, `~b` and `~r` with a radix
//! or as Roman numerals; `~f` with a width and digits; `~%`, `~_`, `~~` and
//! a tilde before a newline; `~*`; iteration with `~{ ~}`, `~:{`, `~@{` and
//! `~^`; `~[ ~; ~]`, `~:[`; and case conversion, `~( ~)`, `~:(`, `~@(` and
//! `~:@(` (MS3 step 64, for `number-format`'s Roman page numbers), which
//! converts each piece as it is written, as Guile's does: each character
//! of the text apart, each directive's output whole, so that `~:(` and
//! `~@(` up-case every letter of the text. Parameters may be numbers, `'c` characters,
//! `v` (from the arguments) and `#` (how many are left).
//!
//! `(ice-9 format)` is a module of its own, made with every VM, as Guile's
//! is a file; LilyPond's `lily.scm` imports its `format` as `ice9-format`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::control::Fluid;
use crate::ports::emit;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

/// Where a format writes: `#f` to a new string, `#t` to the current output
/// port, else the port.
fn destination(cx: &mut Cx, subr: &str, dest: Value) -> Result<Option<Value>, Thrown> {
    match dest {
        Value::FALSE => Ok(None),
        Value::TRUE => Ok(Some(
            cx.heap
                .foreign::<Fluid>(cx.output)
                .map_or(Value::FALSE, |f| f.value),
        )),
        port if cx.heap.foreign::<crate::ports::Port>(port).is_some() => Ok(Some(port)),
        other => Err(cx.wrong_type_at(subr, 1, other)),
    }
}

fn finish(cx: &mut Cx, port: Option<Value>, out: &str, written: Value) -> R {
    match port {
        None => Ok(cx.heap.string(out)),
        Some(port) => {
            emit(cx, port, out);
            Ok(written)
        }
    }
}

/// `(simple-format destination message arg …)`: `~a`, `~s`, `~%` and `~~`
/// only, with Guile's errors.
pub(crate) fn simple_format(cx: &mut Cx, args: &[Value]) -> R {
    let port = destination(cx, "simple-format", args[0])?;
    let Some(template) = cx.heap.str(args[1]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("simple-format", 2, args[1], "string"));
    };
    let mut rest = args[2..].iter();
    let mut out = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '~' {
            out.push(c);
            continue;
        }
        let Some(d) = chars.next() else {
            out.push('~');
            break;
        };
        match d {
            'a' | 'A' | 's' | 'S' => match rest.next() {
                Some(&v) if d == 'a' || d == 'A' => cx.heap.display(v, &mut out),
                Some(&v) => cx.heap.write(v, &mut out),
                None => {
                    let message = "FORMAT: Missing argument for ~~~A";
                    return Err(cx.misc_error(Some("simple-format"), message, &[Value::char(d)]));
                }
            },
            '%' => out.push('\n'),
            '~' => out.push('~'),
            _ => {
                let message = "FORMAT: Unsupported format option ~~~A - use (ice-9 format) instead";
                return Err(cx.misc_error(Some("simple-format"), message, &[Value::char(d)]));
            }
        }
    }
    let left = rest.len();
    if left > 0 {
        let n = cx.heap.integer(left as i64);
        let message = "FORMAT: ~A superfluous arguments";
        return Err(cx.misc_error(Some("simple-format"), message, &[n]));
    }
    finish(cx, port, &out, Value::UNSPECIFIED)
}

// --- (ice-9 format) ----------------------------------------------------------------------

/// What stops a run of directives.
enum Stop {
    /// The end of the text.
    End,
    /// `~^` with nothing left; `true` for `~:^`, which ends an enclosing
    /// `~:{` as well.
    Escape(bool),
}

/// `(ice-9 format)`'s single error, as its `format-error` throws it.
fn error(cx: &mut Cx) -> Thrown {
    cx.misc_error(None, "error in format", &[])
}

struct Args {
    items: Vec<Value>,
    at: usize,
}

impl Args {
    fn next(&mut self, cx: &mut Cx) -> Result<Value, Thrown> {
        let v = self.items.get(self.at).copied().ok_or_else(|| error(cx))?;
        self.at += 1;
        Ok(v)
    }

    fn left(&self) -> usize {
        self.items.len().saturating_sub(self.at)
    }
}

/// A directive's parameters, its `:` and `@`, and its character.
struct Directive {
    params: Vec<Option<Param>>,
    colon: bool,
    at: bool,
    kind: char,
}

#[derive(Clone, Copy)]
enum Param {
    Int(i64),
    Char(char),
}

impl Directive {
    fn int(&self, i: usize) -> Option<i64> {
        match self.params.get(i).copied().flatten() {
            Some(Param::Int(n)) => Some(n),
            _ => None,
        }
    }

    fn char(&self, i: usize, default: char) -> char {
        match self.params.get(i).copied().flatten() {
            Some(Param::Char(c)) => c,
            _ => default,
        }
    }
}

/// Reads the directive after a `~` at `text[*i..]`, `v` and `#` from `args`.
fn directive(
    cx: &mut Cx,
    text: &[char],
    i: &mut usize,
    args: &mut Args,
) -> Result<Directive, Thrown> {
    let mut params = Vec::new();
    loop {
        let c = *text.get(*i).ok_or_else(|| error(cx))?;
        let param = match c {
            '0'..='9' | '-' | '+' => {
                let start = *i;
                *i += 1;
                while text.get(*i).is_some_and(char::is_ascii_digit) {
                    *i += 1;
                }
                let digits: String = text[start..*i].iter().collect();
                Some(Param::Int(digits.parse().map_err(|_| error(cx))?))
            }
            '\'' => {
                let c = *text.get(*i + 1).ok_or_else(|| error(cx))?;
                *i += 2;
                Some(Param::Char(c))
            }
            'v' | 'V' => {
                *i += 1;
                let v = args.next(cx)?;
                match (v.as_fixnum(), v.as_char()) {
                    (Some(n), _) => Some(Param::Int(n)),
                    (_, Some(c)) => Some(Param::Char(c)),
                    _ => None,
                }
            }
            '#' => {
                *i += 1;
                Some(Param::Int(args.left() as i64))
            }
            ',' => None,
            _ => break,
        };
        let c = *text.get(*i).ok_or_else(|| error(cx))?;
        if c == ',' {
            *i += 1;
            params.push(param);
            continue;
        }
        if param.is_some() {
            params.push(param);
        }
        break;
    }
    let (mut colon, mut at) = (false, false);
    while let Some(&c) = text.get(*i).filter(|&&c| c == ':' || c == '@') {
        if c == ':' {
            colon = true;
        } else {
            at = true;
        }
        *i += 1;
    }
    let kind = *text.get(*i).ok_or_else(|| error(cx))?;
    *i += 1;
    Ok(Directive {
        params,
        colon,
        at,
        kind: kind.to_ascii_lowercase(),
    })
}

/// The index after the directive that closes the one opened before
/// `text[from..]`, and where its clauses start: for `~{` the `~}`, for `~[`
/// the `~;`s and the `~]`, for `~(` the first `~)` (case conversions do
/// not nest).
fn closing(
    cx: &mut Cx,
    text: &[char],
    from: usize,
    close: char,
) -> Result<(Vec<(usize, usize)>, usize), Thrown> {
    let mut depth = 0;
    let mut clauses = Vec::new();
    let mut start = from;
    let mut i = from;
    while i < text.len() {
        if text[i] != '~' {
            i += 1;
            continue;
        }
        let tilde = i;
        i += 1;
        while text.get(i).is_some_and(|&c| {
            c.is_ascii_digit() || matches!(c, ',' | ':' | '@' | '#' | 'v' | 'V' | '-' | '+')
        }) {
            i += 1;
        }
        if text.get(i) == Some(&'\'') {
            i += 2;
            while text.get(i).is_some_and(|&c| matches!(c, ',' | ':' | '@')) {
                i += 1;
            }
        }
        let Some(&c) = text.get(i) else { break };
        i += 1;
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' if depth > 0 => depth -= 1,
            ';' if depth == 0 && close == ']' => {
                clauses.push((start, tilde));
                start = i;
            }
            c if c == close && depth == 0 => {
                clauses.push((start, tilde));
                return Ok((clauses, i));
            }
            _ => {}
        }
    }
    Err(error(cx))
}

const ROMAN: [(u32, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

fn roman(mut n: u32, old: bool, out: &mut String) {
    for (value, letters) in ROMAN {
        if old && letters.len() == 2 {
            continue;
        }
        while n >= value {
            out.push_str(letters);
            n -= value;
        }
    }
}

/// Pads `text` to `width` with `fill`, on the left or the right.
fn padded(out: &mut String, text: &str, width: Option<i64>, fill: char, left: bool) {
    let n = width.unwrap_or(0).max(0) as usize;
    let missing = n.saturating_sub(text.chars().count());
    if left {
        out.extend(core::iter::repeat_n(fill, missing));
    }
    out.push_str(text);
    if !left {
        out.extend(core::iter::repeat_n(fill, missing));
    }
}

/// The decimal digits of a real: its sign, integer digits and fraction
/// digits, exact for an exact integer and the shortest that reads back for
/// the rest.
fn decimal(cx: &mut Cx, v: Value) -> Result<(bool, String, String), Thrown> {
    if cx.heap.is_exact_integer(v) {
        let text = cx.heap.number_to_string(v, 10).map_err(|_| error(cx))?;
        let (negative, digits) = match text.strip_prefix('-') {
            Some(d) => (true, String::from(d)),
            None => (false, text),
        };
        return Ok((negative, digits, String::new()));
    }
    if !cx.heap.is_real(v) {
        return Err(error(cx));
    }
    let x = cx.heap.to_f64(v).ok_or_else(|| error(cx))?;
    let text = alloc::format!("{}", x.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    Ok((x.is_sign_negative(), String::from(int), String::from(frac)))
}

/// Rounds `int.frac` half up to `digits` fraction digits, on the text.
fn round_digits(int: &str, frac: &str, digits: usize) -> (String, String) {
    let mut all: Vec<u8> = int.bytes().chain(frac.bytes()).collect();
    let keep = int.len() + digits;
    let up = frac.as_bytes().get(digits).is_some_and(|&d| d >= b'5');
    all.resize(keep.max(all.len()), b'0');
    all.truncate(keep);
    let mut carry = up;
    for d in all.iter_mut().rev() {
        if !carry {
            break;
        }
        if *d == b'9' {
            *d = b'0';
        } else {
            *d += 1;
            carry = false;
        }
    }
    if carry {
        all.insert(0, b'1');
    }
    let split = all.len() - digits;
    let text = String::from_utf8(all).unwrap_or_default();
    (String::from(&text[..split]), String::from(&text[split..]))
}

/// `~w,df`.
fn fixed(cx: &mut Cx, d: &Directive, v: Value, out: &mut String) -> Result<(), Thrown> {
    let (negative, int, frac) = decimal(cx, v)?;
    let width = d.int(0);
    let sign = if negative {
        "-"
    } else if d.at {
        "+"
    } else {
        ""
    };
    let digits = match (d.int(1), width) {
        (Some(n), _) => n.max(0) as usize,
        (None, Some(w)) => {
            let frac_len = frac.len().max(1);
            let room = (w as usize).saturating_sub(sign.len() + int.len() + 1);
            frac_len.min(room)
        }
        (None, None) => frac.len().max(1),
    };
    let (int, frac) = round_digits(&int, &frac, digits);
    let int = if int.is_empty() {
        String::from("0")
    } else {
        int
    };
    let text = alloc::format!("{sign}{int}.{frac}");
    padded(out, &text, width, d.char(4, ' '), true);
    Ok(())
}

/// A `~(`'s case conversion: its `:` and `@`.
type Case = Option<(bool, bool)>;

/// Runs `text`'s directives with `args`, writing to `out`, each piece
/// converted by `case`.
fn run(
    cx: &mut Cx,
    text: &[char],
    args: &mut Args,
    out: &mut String,
    case: Case,
) -> Result<Stop, Thrown> {
    let mut i = 0;
    while i < text.len() {
        let start = out.len();
        if text[i] != '~' {
            out.push(text[i]);
            i += 1;
            convert_case(out, start, case);
            continue;
        }
        i += 1;
        let d = directive(cx, text, &mut i, args)?;
        let count = d.int(0).unwrap_or(1).max(0) as usize;
        match d.kind {
            'a' | 's' => {
                let v = args.next(cx)?;
                let mut s = String::new();
                if d.kind == 'a' {
                    cx.heap.display(v, &mut s);
                } else {
                    cx.heap.write(v, &mut s);
                }
                padded(out, &s, d.int(0), d.char(3, ' '), d.at);
            }
            'c' => {
                let v = args.next(cx)?;
                out.push(v.as_char().ok_or_else(|| error(cx))?);
            }
            'd' | 'x' | 'o' | 'b' | 'r' => {
                let v = args.next(cx)?;
                if !cx.heap.is_exact_integer(v) {
                    return Err(error(cx));
                }
                if d.kind == 'r' && d.params.is_empty() {
                    let n = v.as_fixnum().filter(|n| (1..4000).contains(n));
                    let n = n.ok_or_else(|| error(cx))?;
                    if !d.at {
                        return Err(error(cx));
                    }
                    roman(n as u32, d.colon, out);
                } else {
                    let (radix, first) = match d.kind {
                        'x' => (16, 0),
                        'o' => (8, 0),
                        'b' => (2, 0),
                        'r' => (d.int(0).unwrap_or(10) as u32, 1),
                        _ => (10, 0),
                    };
                    let mut s = cx.heap.number_to_string(v, radix).map_err(|_| error(cx))?;
                    if d.at && !s.starts_with('-') {
                        s.insert(0, '+');
                    }
                    padded(out, &s, d.int(first), d.char(first + 1, ' '), true);
                }
            }
            'f' => {
                let v = args.next(cx)?;
                fixed(cx, &d, v, out)?;
            }
            '%' => out.extend(core::iter::repeat_n('\n', count)),
            '_' => out.extend(core::iter::repeat_n(' ', count)),
            '~' => out.extend(core::iter::repeat_n('~', count)),
            '\n' => {
                while text.get(i).is_some_and(|c| c.is_whitespace() && *c != '\n') {
                    i += 1;
                }
            }
            '*' if d.colon => args.at = args.at.saturating_sub(count),
            '*' => args.at += count,
            '^' => {
                if args.left() == 0 {
                    return Ok(Stop::Escape(d.colon));
                }
            }
            '{' => {
                let (clauses, after) = closing(cx, text, i, '}')?;
                let body = &text[clauses[0].0..clauses[0].1];
                i = after;
                iterate(cx, &d, body, args, out, case)?;
            }
            '[' => {
                let (clauses, after) = closing(cx, text, i, ']')?;
                i = after;
                let v = args.next(cx)?;
                let chosen = if d.colon {
                    Some(if v.is_false() { 0 } else { 1 })
                } else {
                    v.as_fixnum().map(|n| n as usize)
                };
                if let Some(&(start, end)) = chosen.and_then(|n| clauses.get(n))
                    && let Stop::Escape(all) = run(cx, &text[start..end], args, out, case)?
                {
                    return Ok(Stop::Escape(all));
                }
            }
            '(' => {
                let (clauses, after) = closing(cx, text, i, ')')?;
                i = after;
                let body = &text[clauses[0].0..clauses[0].1];
                if let Stop::Escape(all) = run(cx, body, args, out, Some((d.colon, d.at)))? {
                    return Ok(Stop::Escape(all));
                }
            }
            _ => return Err(error(cx)),
        }
        if !matches!(d.kind, '{' | '[' | '(') {
            convert_case(out, start, case);
        }
    }
    Ok(Stop::End)
}

/// The piece `out[start..]` as `case` converts it: down-cased
/// (`string-downcase`); with `:` each word's first letter up-cased and the
/// rest down (`string-capitalize`); with `@` its first letter up-cased and
/// the rest down (`string-capitalize-first`); with both up-cased
/// (`string-upcase`).
fn convert_case(out: &mut String, start: usize, case: Case) {
    let Some((colon, at)) = case else {
        return;
    };
    let piece = out.split_off(start);
    let mut first = true;
    let mut in_word = false;
    for c in piece.chars() {
        let up = match (colon, at) {
            (false, false) => false,
            (true, true) => true,
            (true, false) => c.is_alphabetic() && !in_word,
            (false, true) => c.is_alphabetic() && first,
        };
        if c.is_alphabetic() {
            first = false;
        }
        in_word = c.is_alphabetic();
        if up {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
}

/// `~{body~}` over a list argument, `~:{` over a list of argument lists,
/// `~@{` over the arguments left.
fn iterate(
    cx: &mut Cx,
    d: &Directive,
    body: &[char],
    args: &mut Args,
    out: &mut String,
    case: Case,
) -> Result<(), Thrown> {
    let items = if d.at {
        let rest = args.items[args.at..].to_vec();
        args.at = args.items.len();
        rest
    } else {
        let list = args.next(cx)?;
        crate::errors::list_items(&cx.heap, list).ok_or_else(|| error(cx))?
    };
    if d.colon {
        for item in items {
            let sub = crate::errors::list_items(&cx.heap, item).ok_or_else(|| error(cx))?;
            let mut sub = Args { items: sub, at: 0 };
            if let Stop::Escape(true) = run(cx, body, &mut sub, out, case)? {
                break;
            }
        }
        return Ok(());
    }
    let mut inner = Args { items, at: 0 };
    while inner.left() > 0 {
        if let Stop::Escape(_) = run(cx, body, &mut inner, out, case)? {
            break;
        }
    }
    Ok(())
}

/// `(ice-9 format)`'s `(format destination template arg …)`.
pub(crate) fn format(cx: &mut Cx, args: &[Value]) -> R {
    let port = destination(cx, "format", args[0])?;
    let Some(template) = cx
        .heap
        .str(args[1])
        .map(|s| s.as_str().chars().collect::<Vec<char>>())
    else {
        return Err(error(cx));
    };
    let mut rest = Args {
        items: args[2..].to_vec(),
        at: 0,
    };
    let mut out = String::new();
    run(cx, &template, &mut rest, &mut out, None)?;
    let written = Value::bool(args[0].tag() != Tag::False);
    finish(cx, port, &out, written)
}
