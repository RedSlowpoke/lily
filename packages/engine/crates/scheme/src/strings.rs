//! Guile's string and character procedures beyond R7RS's core (chapter 03
//! §2.6): SRFI-13's as Guile 3 has them in `(guile)` (`string-join`,
//! `string-contains`, `string-pad` …), the comparisons of both SRFI-13 and
//! R7RS, case mapping, `basename` and `dirname`, symbols made from strings,
//! and keywords.
//!
//! What takes a character predicate (`string-index`, `string-split`,
//! `string-trim` …), which may be a procedure, is Scheme ([`PRELUDE`]) over
//! the lists of characters; Guile's character sets are not provided.

use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::SIZE_MAX;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

fn text(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<String, Thrown> {
    match cx.heap.str(v) {
        Some(s) => Ok(String::from(s.as_str())),
        None => Err(cx.wrong_type(subr, position, v, "string")),
    }
}

fn chars(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Vec<char>, Thrown> {
    Ok(text(cx, subr, position, v)?.chars().collect())
}

fn character(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<char, Thrown> {
    v.as_char()
        .ok_or_else(|| cx.wrong_type(subr, position, v, "character"))
}

/// The optional `[start end]` at `args[at..]` of a string of `len`
/// characters.
fn range(cx: &mut Cx, args: &[Value], at: usize, len: usize) -> Result<(usize, usize), Thrown> {
    let bound = |cx: &mut Cx, v: Value, low: usize| match cx.heap.to_i64(v) {
        Some(n) if n >= low as i64 && n as usize <= len => Ok(n as usize),
        _ => Err(cx.out_of_range("substring", 2, v)),
    };
    let start = match args.get(at) {
        Some(&v) => bound(cx, v, 0)?,
        None => 0,
    };
    let end = match args.get(at + 1) {
        Some(&v) => bound(cx, v, start)?,
        None => len,
    };
    Ok((start, end))
}

fn string_of(cx: &mut Cx, chars: &[char]) -> Value {
    cx.heap.string(chars.iter().collect::<String>())
}

/// Unicode's simple mapping of one character, as Guile's `char-upcase`.
fn simple(mut mapped: impl Iterator<Item = char>, c: char) -> char {
    match (mapped.next(), mapped.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

fn upcase(c: char) -> char {
    simple(c.to_uppercase(), c)
}

fn downcase(c: char) -> char {
    simple(c.to_lowercase(), c)
}

// --- SRFI-13 -------------------------------------------------------------------------

pub(crate) fn is_string_null(cx: &mut Cx, args: &[Value]) -> R {
    let s = text(cx, "string-null?", 1, args[0])?;
    Ok(Value::bool(s.is_empty()))
}

/// `(string-join list [delimiter grammar])`.
pub(crate) fn string_join(cx: &mut Cx, args: &[Value]) -> R {
    let Some(items) = crate::errors::list_items(&cx.heap, args[0]) else {
        return Err(cx.wrong_type("string-join", 1, args[0], "list"));
    };
    let delimiter = match args.get(1) {
        Some(&d) => text(cx, "string-join", 2, d)?,
        None => String::from(" "),
    };
    let grammar = args
        .get(2)
        .map_or("infix", |&g| cx.heap.name(g).unwrap_or(""));
    let grammar = String::from(grammar);
    let mut parts = Vec::with_capacity(items.len());
    for &item in &items {
        parts.push(text(cx, "string-join", 1, item)?);
    }
    let joined = parts.join(&delimiter);
    let out = match grammar.as_str() {
        "infix" => joined,
        "strict-infix" if parts.is_empty() => {
            let message = "strict-infix grammar requires non-empty list";
            return Err(cx.misc_error(Some("string-join"), message, &[]));
        }
        "strict-infix" => joined,
        "suffix" if !parts.is_empty() => joined + &delimiter,
        "prefix" if !parts.is_empty() => delimiter + &joined,
        "suffix" | "prefix" => joined,
        _ => return Err(cx.wrong_type_at("string-join", 3, args[2])),
    };
    Ok(cx.heap.string(out))
}

pub(crate) fn string_concatenate(cx: &mut Cx, args: &[Value]) -> R {
    let Some(items) = crate::errors::list_items(&cx.heap, args[0]) else {
        return Err(cx.wrong_type("string-concatenate", 1, args[0], "list"));
    };
    let mut out = String::new();
    for item in items {
        out.push_str(&text(cx, "string-append", 1, item)?);
    }
    Ok(cx.heap.string(out))
}

/// `(string-contains s1 s2 [start1 end1])`: the index of `s2` in `s1`.
fn contains(cx: &mut Cx, subr: &str, args: &[Value], fold: bool) -> R {
    let haystack = chars(cx, subr, 1, args[0])?;
    let needle = chars(cx, subr, 2, args[1])?;
    let (start, end) = range(cx, args, 2, haystack.len())?;
    let same = |a: char, b: char| {
        if fold {
            downcase(a) == downcase(b)
        } else {
            a == b
        }
    };
    let found = (start..=end.saturating_sub(needle.len()))
        .filter(|&i| i + needle.len() <= end)
        .find(|&i| {
            needle
                .iter()
                .enumerate()
                .all(|(k, &c)| same(haystack[i + k], c))
        });
    Ok(match found {
        Some(i) => cx.heap.integer(i as i64),
        None => Value::FALSE,
    })
}

pub(crate) fn string_contains(cx: &mut Cx, args: &[Value]) -> R {
    contains(cx, "string-contains", args, false)
}
pub(crate) fn string_contains_ci(cx: &mut Cx, args: &[Value]) -> R {
    contains(cx, "string-contains-ci", args, true)
}

fn affix(cx: &mut Cx, subr: &str, args: &[Value], suffix: bool, fold: bool) -> R {
    let a = chars(cx, subr, 1, args[0])?;
    let b = chars(cx, subr, 2, args[1])?;
    if a.len() > b.len() {
        return Ok(Value::FALSE);
    }
    let at = if suffix { b.len() - a.len() } else { 0 };
    let same = a.iter().zip(&b[at..]).all(|(&x, &y)| {
        if fold {
            downcase(x) == downcase(y)
        } else {
            x == y
        }
    });
    Ok(Value::bool(same))
}

pub(crate) fn string_prefix(cx: &mut Cx, args: &[Value]) -> R {
    affix(cx, "string-prefix?", args, false, false)
}
pub(crate) fn string_suffix(cx: &mut Cx, args: &[Value]) -> R {
    affix(cx, "string-suffix?", args, true, false)
}
pub(crate) fn string_prefix_ci(cx: &mut Cx, args: &[Value]) -> R {
    affix(cx, "string-prefix-ci?", args, false, true)
}
pub(crate) fn string_suffix_ci(cx: &mut Cx, args: &[Value]) -> R {
    affix(cx, "string-suffix-ci?", args, true, true)
}

fn map_chars(cx: &mut Cx, subr: &str, args: &[Value], f: fn(char) -> char) -> R {
    let s = chars(cx, subr, 1, args[0])?;
    let (start, end) = range(cx, args, 1, s.len())?;
    let out: Vec<char> = s[start..end].iter().map(|&c| f(c)).collect();
    Ok(string_of(cx, &out))
}

pub(crate) fn string_upcase(cx: &mut Cx, args: &[Value]) -> R {
    map_chars(cx, "string-upcase", args, upcase)
}
pub(crate) fn string_downcase(cx: &mut Cx, args: &[Value]) -> R {
    map_chars(cx, "string-downcase", args, downcase)
}

/// `string-capitalize` and `string-titlecase`: each run of letters with its
/// first upper case and the rest lower.
pub(crate) fn string_titlecase(cx: &mut Cx, args: &[Value]) -> R {
    let s = chars(cx, "string-titlecase", 1, args[0])?;
    let mut in_word = false;
    let out: Vec<char> = s
        .iter()
        .map(|&c| {
            let letter = c.is_alphabetic();
            let mapped = match (letter, in_word) {
                (true, false) => upcase(c),
                (true, true) => downcase(c),
                _ => c,
            };
            in_word = letter;
            mapped
        })
        .collect();
    Ok(string_of(cx, &out))
}

pub(crate) fn string_reverse(cx: &mut Cx, args: &[Value]) -> R {
    let mut s = chars(cx, "string-reverse", 1, args[0])?;
    s.reverse();
    Ok(string_of(cx, &s))
}

/// `string-take`, `string-drop` and their `-right`s: `n` characters from one
/// end, or the rest.
fn cut(cx: &mut Cx, subr: &str, args: &[Value], keep_left: bool, from_right: bool) -> R {
    let s = chars(cx, subr, 1, args[0])?;
    let n = match cx.heap.to_i64(args[1]) {
        Some(n) if n >= 0 && n as usize <= s.len() => n as usize,
        _ => return Err(cx.out_of_bounds(0, s.len() as u64, args[1])),
    };
    let at = if from_right { s.len() - n } else { n };
    let piece = if keep_left { &s[..at] } else { &s[at..] };
    Ok(string_of(cx, piece))
}

pub(crate) fn string_take(cx: &mut Cx, args: &[Value]) -> R {
    cut(cx, "string-take", args, true, false)
}
pub(crate) fn string_drop(cx: &mut Cx, args: &[Value]) -> R {
    cut(cx, "string-drop", args, false, false)
}
pub(crate) fn string_take_right(cx: &mut Cx, args: &[Value]) -> R {
    cut(cx, "string-take-right", args, false, true)
}
pub(crate) fn string_drop_right(cx: &mut Cx, args: &[Value]) -> R {
    cut(cx, "string-drop-right", args, true, true)
}

/// `(string-pad s n [char])` keeps the right end, `string-pad-right` the
/// left.
fn pad(cx: &mut Cx, subr: &str, args: &[Value], right: bool) -> R {
    let s = chars(cx, subr, 1, args[0])?;
    let n = cx.size(args[1], SIZE_MAX)?;
    let fill = match args.get(2) {
        Some(&c) => character(cx, subr, 3, c)?,
        None => ' ',
    };
    let out: Vec<char> = if right {
        s.iter()
            .copied()
            .chain(core::iter::repeat(fill))
            .take(n)
            .collect()
    } else if s.len() >= n {
        s[s.len() - n..].to_vec()
    } else {
        core::iter::repeat_n(fill, n - s.len())
            .chain(s.iter().copied())
            .collect()
    };
    Ok(string_of(cx, &out))
}

pub(crate) fn string_pad(cx: &mut Cx, args: &[Value]) -> R {
    pad(cx, "string-pad", args, false)
}
pub(crate) fn string_pad_right(cx: &mut Cx, args: &[Value]) -> R {
    pad(cx, "string-pad-right", args, true)
}

/// `(string-copy s [start end])`, and `substring/shared`.
pub(crate) fn string_copy(cx: &mut Cx, args: &[Value]) -> R {
    let s = chars(cx, "string-copy", 1, args[0])?;
    let (start, end) = range(cx, args, 1, s.len())?;
    Ok(string_of(cx, &s[start..end]))
}

pub(crate) fn string_fill(cx: &mut Cx, args: &[Value]) -> R {
    let s = chars(cx, "string-fill!", 1, args[0])?;
    let c = character(cx, "string-fill!", 2, args[1])?;
    let (start, end) = range(cx, args, 2, s.len())?;
    let filled: Vec<char> = s
        .iter()
        .enumerate()
        .map(|(i, &x)| if (start..end).contains(&i) { c } else { x })
        .collect();
    set_text(cx, "string-fill!", args[0], &filled)
}

/// `(string-copy! to at from [start end])`.
pub(crate) fn string_copy_bang(cx: &mut Cx, args: &[Value]) -> R {
    let mut to = chars(cx, "string-copy!", 1, args[0])?;
    let at = cx.size(args[1], to.len() as u64)?;
    let from = chars(cx, "string-copy!", 3, args[2])?;
    let (start, end) = range(cx, args, 3, from.len())?;
    if at + (end - start) > to.len() {
        return Err(cx.out_of_range("string-copy!", 2, args[1]));
    }
    to[at..at + end - start].copy_from_slice(&from[start..end]);
    set_text(cx, "string-copy!", args[0], &to)
}

fn set_text(cx: &mut Cx, subr: &str, string: Value, new: &[char]) -> R {
    match cx.heap.str_mut(string) {
        Some(s) => {
            for (i, &c) in new.iter().enumerate() {
                s.set_char(i, c);
            }
            Ok(Value::UNSPECIFIED)
        }
        None => Err(cx.wrong_type(subr, 1, string, "string")),
    }
}

pub(crate) fn reverse_list_to_string(cx: &mut Cx, args: &[Value]) -> R {
    let Some(items) = crate::errors::list_items(&cx.heap, args[0]) else {
        return Err(cx.wrong_type("reverse-list->string", 1, args[0], "list"));
    };
    let mut out = Vec::with_capacity(items.len());
    for &c in items.iter().rev() {
        out.push(character(cx, "reverse-list->string", 1, c)?);
    }
    Ok(string_of(cx, &out))
}

// --- comparisons ----------------------------------------------------------------------

fn fold_case(s: &str) -> String {
    s.chars().map(downcase).collect()
}

fn ordering(op: &str, o: core::cmp::Ordering) -> bool {
    use core::cmp::Ordering::*;
    match op {
        "=" => o == Equal,
        "<" => o == Less,
        ">" => o == Greater,
        "<=" => o != Greater,
        ">=" => o != Less,
        _ => o != Equal,
    }
}

/// R7RS's `string<?` and the rest, of any number of strings; their errors
/// name SRFI-13's `string<`, which Guile's call.
fn compare_all(cx: &mut Cx, subr: &str, op: &str, fold: bool, args: &[Value]) -> R {
    for (i, &v) in args.iter().enumerate() {
        if v.tag() != Tag::String {
            return Err(cx.wrong_type(subr, i + 1, v, "string"));
        }
    }
    let heap = &cx.heap;
    let text = |v: Value| heap.str(v).map_or("", |s| s.as_str());
    Ok(Value::bool(args.windows(2).all(|w| {
        let (a, b) = (text(w[0]), text(w[1]));
        let order = if fold {
            fold_case(a).cmp(&fold_case(b))
        } else {
            a.cmp(b)
        };
        ordering(op, order)
    })))
}

/// SRFI-13's `string<` and the rest: two strings, each with an optional
/// range.
fn compare_two(cx: &mut Cx, subr: &str, op: &str, fold: bool, args: &[Value]) -> R {
    let a = chars(cx, subr, 1, args[0])?;
    let b = chars(cx, subr, 2, args[1])?;
    let (s1, e1) = range(cx, args, 2, a.len())?;
    let (s2, e2) = range(cx, args, 4, b.len())?;
    let key = |s: &[char]| -> String {
        let t: String = s.iter().collect();
        if fold { fold_case(&t) } else { t }
    };
    Ok(Value::bool(ordering(
        op,
        key(&a[s1..e1]).cmp(&key(&b[s2..e2])),
    )))
}

macro_rules! comparisons {
    ($(($f:ident, $name:literal, $op:literal, $fold:literal, $how:ident)),* $(,)?) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            $how(cx, $name, $op, $fold, args)
        })*
    };
}

comparisons!(
    (string_eq_p, "string=", "=", false, compare_all),
    (string_lt_p, "string<", "<", false, compare_all),
    (string_gt_p, "string>", ">", false, compare_all),
    (string_le_p, "string<=", "<=", false, compare_all),
    (string_ge_p, "string>=", ">=", false, compare_all),
    (string_ci_eq_p, "string-ci=", "=", true, compare_all),
    (string_ci_lt_p, "string-ci<", "<", true, compare_all),
    (string_ci_gt_p, "string-ci>", ">", true, compare_all),
    (string_ci_le_p, "string-ci<=", "<=", true, compare_all),
    (string_ci_ge_p, "string-ci>=", ">=", true, compare_all),
    (string_eq, "string=", "=", false, compare_two),
    (string_ne, "string<>", "<>", false, compare_two),
    (string_lt, "string<", "<", false, compare_two),
    (string_gt, "string>", ">", false, compare_two),
    (string_le, "string<=", "<=", false, compare_two),
    (string_ge, "string>=", ">=", false, compare_two),
    (string_ci_eq, "string-ci=", "=", true, compare_two),
    (string_ci_ne, "string-ci<>", "<>", true, compare_two),
    (string_ci_lt, "string-ci<", "<", true, compare_two),
    (string_ci_gt, "string-ci>", ">", true, compare_two),
);

/// `char<?` and the rest, of any number of characters.
fn char_compare(cx: &mut Cx, subr: &str, op: &str, fold: bool, args: &[Value]) -> R {
    let mut cs = Vec::with_capacity(args.len());
    for (i, &v) in args.iter().enumerate() {
        let c = character(cx, subr, i + 1, v)?;
        cs.push(if fold { downcase(c) } else { c });
    }
    Ok(Value::bool(
        cs.windows(2).all(|w| ordering(op, w[0].cmp(&w[1]))),
    ))
}

comparisons!(
    (char_eq, "char=?", "=", false, char_compare),
    (char_lt, "char<?", "<", false, char_compare),
    (char_gt, "char>?", ">", false, char_compare),
    (char_le, "char<=?", "<=", false, char_compare),
    (char_ge, "char>=?", ">=", false, char_compare),
    (char_ci_eq, "char-ci=?", "=", true, char_compare),
    (char_ci_lt, "char-ci<?", "<", true, char_compare),
    (char_ci_gt, "char-ci>?", ">", true, char_compare),
    (char_ci_le, "char-ci<=?", "<=", true, char_compare),
    (char_ci_ge, "char-ci>=?", ">=", true, char_compare),
);

/// A test of a character's class, which Guile makes with a character set.
macro_rules! char_test {
    ($f:ident, |$c:ident| $body:expr) => {
        pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            let $c = character(cx, "char-set-contains?", 2, args[0])?;
            Ok(Value::bool($body))
        }
    };
}

char_test!(is_char_alphabetic, |c| c.is_alphabetic());
char_test!(is_char_numeric, |c| c.is_numeric());
char_test!(is_char_whitespace, |c| c.is_whitespace());
char_test!(is_char_upper_case, |c| c.is_uppercase());
char_test!(is_char_lower_case, |c| c.is_lowercase());

// --- file names, symbols and keywords ---------------------------------------------------

/// Guile's `basename`: the last component, with its trailing slashes, and
/// without `suffix` if it ends with it and is longer.
pub(crate) fn basename(cx: &mut Cx, args: &[Value]) -> R {
    let s = chars(cx, "basename", 1, args[0])?;
    let suffix = match args.get(1) {
        Some(&v) => Some(chars(cx, "basename", 2, v)?),
        None => None,
    };
    let Some(last) = s.iter().rposition(|&c| c != '/') else {
        let out = if s.is_empty() { "" } else { "/" };
        return Ok(cx.heap.string(out));
    };
    let start = s[..last]
        .iter()
        .rposition(|&c| c == '/')
        .map_or(0, |i| i + 1);
    let mut component = &s[start..];
    if let Some(suffix) = suffix
        && component.len() > suffix.len()
        && component.ends_with(&suffix)
    {
        component = &component[..component.len() - suffix.len()];
    }
    Ok(string_of(cx, component))
}

/// Guile's `dirname`: what is before the last component, without its
/// trailing slashes; `.` if nothing, `/` if only the root.
pub(crate) fn dirname(cx: &mut Cx, args: &[Value]) -> R {
    let s = chars(cx, "dirname", 1, args[0])?;
    let out: &[char] = match s.iter().rposition(|&c| c != '/') {
        None if s.is_empty() => &['.'],
        None => &['/'],
        Some(last) => match s[..last].iter().rposition(|&c| c == '/') {
            None => &['.'],
            Some(slash) => match s[..slash].iter().rposition(|&c| c != '/') {
                None => &['/'],
                Some(end) => &s[..=end],
            },
        },
    };
    Ok(string_of(cx, out))
}

/// `(symbol char …)`: Guile makes the string with `string`.
pub(crate) fn symbol(cx: &mut Cx, args: &[Value]) -> R {
    let mut name = String::new();
    for &c in args {
        match c.as_char() {
            Some(c) => name.push(c),
            None => return Err(cx.wrong_type_vm(Some("string"), c, "character")),
        }
    }
    Ok(cx.heap.intern(&name))
}

pub(crate) fn symbol_append(cx: &mut Cx, args: &[Value]) -> R {
    let mut name = String::new();
    for (i, &s) in args.iter().enumerate() {
        match cx.heap.name(s).filter(|_| s.tag() == Tag::Symbol) {
            Some(n) => name.push_str(n),
            None => return Err(cx.wrong_type("symbol-append", i + 1, s, "symbol")),
        }
    }
    Ok(cx.heap.intern(&name))
}

pub(crate) fn keyword_to_symbol(cx: &mut Cx, args: &[Value]) -> R {
    match cx
        .heap
        .name(args[0])
        .filter(|_| args[0].tag() == Tag::Keyword)
    {
        Some(name) => {
            let name = String::from(name);
            Ok(cx.heap.intern(&name))
        }
        None => Err(cx.wrong_type("keyword->symbol", 1, args[0], "keyword")),
    }
}

pub(crate) fn symbol_to_keyword(cx: &mut Cx, args: &[Value]) -> R {
    match cx
        .heap
        .name(args[0])
        .filter(|_| args[0].tag() == Tag::Symbol)
    {
        Some(name) => {
            let name = String::from(name);
            Ok(cx.heap.keyword(&name))
        }
        None => Err(cx.wrong_type("symbol->keyword", 1, args[0], "symbol")),
    }
}

/// `(gensym [prefix])`: a new uninterned symbol, ` g` and a count by
/// default.
pub(crate) fn gensym(cx: &mut Cx, args: &[Value]) -> R {
    let prefix = match args.first() {
        Some(&p) => text(cx, "gensym", 1, p)?,
        None => String::from(" g"),
    };
    cx.gensyms += 1;
    let name = alloc::format!("{prefix}{}", cx.gensyms);
    Ok(cx.heap.uninterned(&name))
}

/// `(make-symbol name)`: a new uninterned symbol named `name`.
pub(crate) fn make_symbol(cx: &mut Cx, args: &[Value]) -> R {
    let name = text(cx, "make-symbol", 1, args[0])?;
    Ok(cx.heap.uninterned(&name))
}

/// What takes a character, or a predicate, as SRFI-13's `char_pred`, over
/// the characters as a list; and the procedures of strings and vectors that
/// call one. Also Guile's `file-name-separator-string`, POSIX's, which
/// `withRelativeDir` joins to `dirname`'s.
pub(crate) const PRELUDE: &str = r#"
(define file-name-separator-string "/")
(define (%char-pred subr position pred)
  (cond ((char? pred) (lambda (c) (char=? c pred)))
        ((procedure? pred) pred)
        (else (scm-error 'wrong-type-arg subr "Wrong type argument in position ~A: ~S"
                         (list position pred) (list pred)))))
(define (%substring-chars s args)
  (string->list (if (null? args) s (apply substring s args))))
(define (%start args) (if (null? args) 0 (car args)))
(define (string-index s pred . range)
  (let ((pred (%char-pred "string-index" 2 pred)))
    (let loop ((l (%substring-chars s range)) (i (%start range)))
      (cond ((null? l) #f) ((pred (car l)) i) (else (loop (cdr l) (+ i 1)))))))
(define (string-rindex s pred . range)
  (let ((pred (%char-pred "string-rindex" 2 pred)))
    (let loop ((l (reverse (%substring-chars s range)))
               (i (- (if (and (pair? range) (pair? (cdr range))) (cadr range) (string-length s)) 1)))
      (cond ((null? l) #f) ((pred (car l)) i) (else (loop (cdr l) (- i 1)))))))
(define (string-count s pred . range)
  (let ((pred (%char-pred "string-count" 2 pred)))
    (let loop ((l (%substring-chars s range)) (n 0))
      (cond ((null? l) n) ((pred (car l)) (loop (cdr l) (+ n 1))) (else (loop (cdr l) n))))))
(define (string-filter pred s . range)
  (let ((pred (if (string? pred) (%char-pred "string-filter" 1 s) (%char-pred "string-filter" 1 pred)))
        (s (if (string? pred) pred s)))
    (list->string (filter pred (%substring-chars s range)))))
(define (string-delete pred s . range)
  (let ((pred (if (string? pred) (%char-pred "string-delete" 1 s) (%char-pred "string-delete" 1 pred)))
        (s (if (string? pred) pred s)))
    (list->string (filter (lambda (c) (not (pred c))) (%substring-chars s range)))))
(define (string-any pred s . range)
  (let ((pred (%char-pred "string-any" 1 pred)))
    (let loop ((l (%substring-chars s range)))
      (and (pair? l)
           (if (null? (cdr l)) (pred (car l)) (or (pred (car l)) (loop (cdr l))))))))
(define (string-every pred s . range)
  (let ((pred (%char-pred "string-every" 1 pred)))
    (let loop ((l (%substring-chars s range)))
      (or (null? l)
          (if (null? (cdr l)) (pred (car l)) (and (pred (car l)) (loop (cdr l))))))))
(define (%trim-left pred l)
  (if (and (pair? l) (pred (car l))) (%trim-left pred (cdr l)) l))
(define (string-trim s . args)
  (let ((pred (%char-pred "string-trim" 2 (if (pair? args) (car args) char-whitespace?))))
    (list->string (%trim-left pred (%substring-chars s (if (pair? args) (cdr args) '()))))))
(define (string-trim-right s . args)
  (let ((pred (%char-pred "string-trim-right" 2 (if (pair? args) (car args) char-whitespace?))))
    (list->string (reverse! (%trim-left pred (reverse (%substring-chars s (if (pair? args) (cdr args) '()))))))))
(define (string-trim-both s . args)
  (let ((pred (%char-pred "string-trim-both" 2 (if (pair? args) (car args) char-whitespace?))))
    (list->string
     (reverse! (%trim-left pred (reverse! (%trim-left pred (%substring-chars s (if (pair? args) (cdr args) '())))))))))
(define (string-split s pred)
  (let ((pred (%char-pred "string-split" 2 pred)))
    (let loop ((l (string->list s)) (field '()) (out '()))
      (cond ((null? l) (reverse! (cons (list->string (reverse! field)) out)))
            ((pred (car l)) (loop (cdr l) '() (cons (list->string (reverse! field)) out)))
            (else (loop (cdr l) (cons (car l) field) out))))))
(define (string-tokenize s . args)
  (let ((pred (if (pair? args)
                  (%char-pred "string-tokenize" 2 (car args))
                  (lambda (c) (not (char-whitespace? c))))))
    (let loop ((l (%substring-chars s (if (pair? args) (cdr args) '()))) (token '()) (out '()))
      (cond ((null? l)
             (reverse! (if (null? token) out (cons (list->string (reverse! token)) out))))
            ((pred (car l)) (loop (cdr l) (cons (car l) token) out))
            ((null? token) (loop (cdr l) '() out))
            (else (loop (cdr l) '() (cons (list->string (reverse! token)) out)))))))
(define (string-for-each proc s . range)
  (if (not (procedure? proc))
      (scm-error 'wrong-type-arg "string-for-each" "Wrong type argument in position ~A: ~S"
                 (list 1 proc) (list proc)))
  (for-each proc (%substring-chars s range)))
(define (string-map proc s . range)
  (if (not (procedure? proc))
      (scm-error 'wrong-type-arg "string-map" "Wrong type argument in position ~A: ~S"
                 (list 1 proc) (list proc)))
  (list->string (map proc (%substring-chars s range))))
(define (string-fold kons knil s . range)
  (let loop ((l (%substring-chars s range)) (acc knil))
    (if (null? l) acc (loop (cdr l) (kons (car l) acc)))))
(define (string-fold-right kons knil s . range)
  (let loop ((l (reverse (%substring-chars s range))) (acc knil))
    (if (null? l) acc (loop (cdr l) (kons (car l) acc)))))
(define (vector-copy v . range)
  (list->vector (if (null? range)
                    (vector->list v)
                    (list-head (list-tail (vector->list v) (car range))
                               (- (if (pair? (cdr range)) (cadr range) (vector-length v))
                                  (car range))))))
(define (vector-copy! to at from . range)
  (let loop ((l (vector->list (apply vector-copy from range))) (i at))
    (if (pair? l)
        (begin (vector-set! to i (car l)) (loop (cdr l) (+ i 1))))))
"#;
