//! LilyPond's regular expressions (`glib-regex-scheme.cc`, MS2 step 20):
//! `ly:make-regex`, `ly:regex-exec`, `ly:regex-exec->list`, `ly:regex-quote`
//! and `ly:regex-replace`. LilyPond compiles them with GLib's `GRegex`,
//! which is PCRE, so the matcher here backtracks as PCRE does: the first
//! alternative and the greediest repetition that let the rest match win,
//! not POSIX's longest match. Positions are counted in characters, as
//! `fix_up_position` gives them.
//!
//! The syntax is the part of PCRE's that LilyPond's Scheme and scores use:
//! literals and `.`, which matches no newline; classes with ranges,
//! negation, `\d \w \s` and POSIX names; `\d \D \w \W \s \S \b \B`, `\A
//! \z \Z` and the escapes of control characters, `\xhh` and `\x{h…}`;
//! groups, `(?:…)` and back references; `|`; `* + ? {n} {n,} {n,m}`, lazy
//! with `?` after them; `^` at the start and `$` at the end or before a
//! final newline. Anything else is a compile error, worded as GLib's
//! message is introduced (`check_error`).
//!
//! A match is a `<regex-match>` of `lily-library.scm`, made by
//! [`PRELUDE`] from the positions `%regex-exec` gives: a vector of `(start
//! . end)` per group, the whole match first, `#f` for a group that took no
//! part. After an empty match, the next search starts one character on.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::OnceCell;

use crate::heap::Foreign;
use crate::image::{ImageReader, ImageWriter};
use crate::value::Value;
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

/// What `ly:make-regex` makes: its pattern, which an image keeps, and the
/// pattern compiled, which a restored one compiles when it is first
/// matched (MS3 step 56, D215): the pattern compiled when it was made.
pub(crate) struct Regex {
    pattern: String,
    program: OnceCell<Program>,
}

impl Foreign for Regex {
    fn type_name(&self) -> &'static str {
        "Regex"
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.str(&self.pattern);
        true
    }
}

pub(crate) fn load_regex(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let pattern: String = r.str()?.into();
    Some(Box::new(Regex {
        pattern,
        program: OnceCell::new(),
    }))
}

/// A set of characters: ranges, negated or not.
#[derive(Clone, Debug)]
struct Class {
    negated: bool,
    ranges: Vec<(char, char)>,
}

impl Class {
    fn of(negated: bool, ranges: &[(char, char)]) -> Class {
        Class {
            negated,
            ranges: ranges.to_vec(),
        }
    }

    fn contains(&self, c: char) -> bool {
        self.ranges.iter().any(|&(a, b)| a <= c && c <= b) != self.negated
    }
}

const DIGIT: &[(char, char)] = &[('0', '9')];
const WORD: &[(char, char)] = &[('0', '9'), ('A', 'Z'), ('_', '_'), ('a', 'z')];
const SPACE: &[(char, char)] = &[('\t', '\r'), (' ', ' ')];

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    /// `.`: anything but a newline.
    Any,
    Class(Class),
    /// `^` and `\A`.
    Start,
    /// `$` and `\Z`: the end, or before a newline that ends the subject.
    End,
    /// `\z`.
    EndStrict,
    /// `\b` (true) and `\B`.
    Boundary(bool),
    /// A group, capturing into its index or not, of alternatives.
    Group(Option<usize>, Vec<Vec<Node>>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
    Backref(usize),
}

#[derive(Clone, Debug)]
struct Program {
    alternatives: Vec<Vec<Node>>,
    /// The capturing groups, the whole match not counted.
    groups: usize,
}

struct Parser<'a> {
    chars: core::iter::Peekable<core::str::Chars<'a>>,
    groups: usize,
}

type Compiled<T> = Result<T, &'static str>;

fn compile(pattern: &str) -> Compiled<Program> {
    let mut p = Parser {
        chars: pattern.chars().peekable(),
        groups: 0,
    };
    let alternatives = p.alternatives()?;
    if p.chars.next().is_some() {
        return Err("unmatched closing parenthesis");
    }
    Ok(Program {
        alternatives,
        groups: p.groups,
    })
}

impl Parser<'_> {
    fn alternatives(&mut self) -> Compiled<Vec<Vec<Node>>> {
        let mut alternatives = alloc::vec![self.sequence()?];
        while self.chars.next_if_eq(&'|').is_some() {
            alternatives.push(self.sequence()?);
        }
        Ok(alternatives)
    }

    fn sequence(&mut self) -> Compiled<Vec<Node>> {
        let mut nodes = Vec::new();
        while let Some(&c) = self.chars.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            let node = self.quantified(atom)?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    fn quantified(&mut self, atom: Node) -> Compiled<Node> {
        let star = matches!(self.chars.peek(), Some('*' | '+' | '?'));
        let (min, max) = match self.chars.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => match self.bounds() {
                Some(bounds) => bounds,
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        if star {
            self.chars.next();
        }
        if matches!(
            atom,
            Node::Start | Node::End | Node::EndStrict | Node::Boundary(_)
        ) {
            return Err("nothing to repeat");
        }
        let greedy = self.chars.next_if_eq(&'?').is_none();
        // A possessive `+` is taken as greedy.
        self.chars.next_if_eq(&'+');
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    /// `{n}`, `{n,}` or `{n,m}`, taken with its braces; None, and nothing
    /// taken, for a `{` that begins none, which PCRE reads as a literal.
    fn bounds(&mut self) -> Option<(u32, Option<u32>)> {
        let mut ahead = self.chars.clone();
        ahead.next();
        let body: String = ahead.by_ref().take_while(|&c| c != '}').collect();
        let (min, max) = match body.split_once(',') {
            None => {
                let n = body.parse().ok()?;
                (n, Some(n))
            }
            Some((a, "")) => (a.parse().ok()?, None),
            Some((a, b)) => (a.parse().ok()?, Some(b.parse().ok()?)),
        };
        // `take_while` took the `}`; one was there if `body` ended before
        // the pattern did.
        if self.chars.clone().nth(body.chars().count() + 1) != Some('}') {
            return None;
        }
        self.chars = ahead;
        Some((min, max))
    }

    fn atom(&mut self) -> Compiled<Node> {
        let c = self.chars.next().ok_or("unexpected end of pattern")?;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                let capture = if self.chars.next_if_eq(&'?').is_some() {
                    if self.chars.next_if_eq(&':').is_none() {
                        return Err("unrecognized character after (? or (?-");
                    }
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let alternatives = self.alternatives()?;
                if self.chars.next_if_eq(&')').is_none() {
                    return Err("missing terminating ) for subpattern");
                }
                Node::Group(capture, alternatives)
            }
            '[' => Node::Class(self.class()?),
            '*' | '+' | '?' => return Err("quantifier does not follow a repeatable item"),
            '\\' => self.escape(false)?,
            c => Node::Char(c),
        })
    }

    /// The escape after `\`, in a class if `in_class`, where only
    /// characters and sets mean something.
    fn escape(&mut self, in_class: bool) -> Compiled<Node> {
        let c = self.chars.next().ok_or("\\ at end of pattern")?;
        Ok(match c {
            'd' => Node::Class(Class::of(false, DIGIT)),
            'D' => Node::Class(Class::of(true, DIGIT)),
            'w' => Node::Class(Class::of(false, WORD)),
            'W' => Node::Class(Class::of(true, WORD)),
            's' => Node::Class(Class::of(false, SPACE)),
            'S' => Node::Class(Class::of(true, SPACE)),
            'b' if in_class => Node::Char('\u{8}'),
            'b' => Node::Boundary(true),
            'B' if !in_class => Node::Boundary(false),
            'A' if !in_class => Node::Start,
            'Z' if !in_class => Node::End,
            'z' if !in_class => Node::EndStrict,
            't' => Node::Char('\t'),
            'n' => Node::Char('\n'),
            'r' => Node::Char('\r'),
            'f' => Node::Char('\u{c}'),
            'v' => Node::Char('\u{b}'),
            'e' => Node::Char('\u{1b}'),
            'a' => Node::Char('\u{7}'),
            'x' => {
                let digits: String = if self.chars.next_if_eq(&'{').is_some() {
                    let d: String =
                        core::iter::from_fn(|| self.chars.next_if(|&c| c != '}')).collect();
                    if self.chars.next_if_eq(&'}').is_none() {
                        return Err("missing terminating } in \\x{}");
                    }
                    d
                } else {
                    (0..2)
                        .filter_map(|_| self.chars.next_if(|c| c.is_ascii_hexdigit()))
                        .collect()
                };
                let code = u32::from_str_radix(if digits.is_empty() { "0" } else { &digits }, 16)
                    .map_err(|_| "non-hex character in \\x{}")?;
                Node::Char(
                    char::from_u32(code)
                        .ok_or("character code point value in \\x{} is too large")?,
                )
            }
            '1'..='9' if !in_class => {
                let mut n = c.to_digit(10).unwrap_or(0) as usize;
                while let Some(d) = self.chars.next_if(|c| c.is_ascii_digit()) {
                    n = n * 10 + d.to_digit(10).unwrap_or(0) as usize;
                }
                Node::Backref(n)
            }
            c if c.is_ascii_alphanumeric() => return Err("unrecognized character follows \\"),
            c => Node::Char(c),
        })
    }

    /// A class after `[`, to its `]`, which comes first as a character.
    fn class(&mut self) -> Compiled<Class> {
        let negated = self.chars.next_if_eq(&'^').is_some();
        let mut ranges = Vec::new();
        let mut first = true;
        loop {
            let c = self
                .chars
                .next()
                .ok_or("missing terminating ] for character class")?;
            if c == ']' && !first {
                break;
            }
            first = false;
            let low = match c {
                '[' if self.chars.peek() == Some(&':') => {
                    let name: String = self
                        .chars
                        .clone()
                        .skip(1)
                        .take_while(|&c| c != ':')
                        .collect();
                    let set: &[(char, char)] = match name.as_str() {
                        "alpha" => &[('A', 'Z'), ('a', 'z')],
                        "digit" => DIGIT,
                        "alnum" => &[('0', '9'), ('A', 'Z'), ('a', 'z')],
                        "space" => SPACE,
                        "upper" => &[('A', 'Z')],
                        "lower" => &[('a', 'z')],
                        "punct" => &[('!', '/'), (':', '@'), ('[', '`'), ('{', '~')],
                        "xdigit" => &[('0', '9'), ('A', 'F'), ('a', 'f')],
                        "word" => WORD,
                        _ => return Err("unknown POSIX class name"),
                    };
                    for _ in 0..name.chars().count() + 1 {
                        self.chars.next();
                    }
                    if self.chars.next() != Some(':') || self.chars.next() != Some(']') {
                        return Err("unknown POSIX class name");
                    }
                    ranges.extend_from_slice(set);
                    continue;
                }
                '\\' => match self.escape(true)? {
                    Node::Char(c) => c,
                    Node::Class(set) if !set.negated => {
                        ranges.extend(set.ranges);
                        continue;
                    }
                    Node::Class(set) => {
                        // A negated set in a class: its complement as ranges.
                        let mut next = '\0';
                        for &(a, b) in &set.ranges {
                            if a > next {
                                ranges.push((next, char::from_u32(a as u32 - 1).unwrap_or(next)));
                            }
                            next = char::from_u32(b as u32 + 1).unwrap_or(char::MAX);
                        }
                        ranges.push((next, char::MAX));
                        continue;
                    }
                    _ => return Err("invalid escape sequence in character class"),
                },
                c => c,
            };
            let is_range = self.chars.peek() == Some(&'-')
                && self.chars.clone().nth(1).is_some_and(|c| c != ']');
            if !is_range {
                ranges.push((low, low));
                continue;
            }
            self.chars.next();
            let high = match self.chars.next() {
                Some('\\') => match self.escape(true)? {
                    Node::Char(c) => c,
                    _ => return Err("invalid range in character class"),
                },
                Some(c) => c,
                None => return Err("missing terminating ] for character class"),
            };
            if high < low {
                return Err("range out of order in character class");
            }
            ranges.push((low, high));
        }
        Ok(Class { negated, ranges })
    }
}

/// The groups' positions while matching: the whole match's at 0.
type Captures = Vec<Option<(usize, usize)>>;
type Cont<'k> = &'k mut dyn FnMut(usize, &mut Captures) -> bool;

/// A subject being matched, as characters.
struct Matcher<'s> {
    s: &'s [char],
}

impl Matcher<'_> {
    fn sequence(&self, nodes: &[Node], at: usize, caps: &mut Captures, k: Cont<'_>) -> bool {
        match nodes.split_first() {
            None => k(at, caps),
            Some((node, rest)) => self.node(node, at, caps, &mut |p, caps| {
                self.sequence(rest, p, caps, k)
            }),
        }
    }

    fn alternatives(
        &self,
        alts: &[Vec<Node>],
        at: usize,
        caps: &mut Captures,
        k: Cont<'_>,
    ) -> bool {
        alts.iter().any(|alt| self.sequence(alt, at, caps, k))
    }

    fn node(&self, node: &Node, at: usize, caps: &mut Captures, k: Cont<'_>) -> bool {
        let s = self.s;
        let one = |ok: bool, k: Cont<'_>, caps: &mut Captures| ok && k(at + 1, caps);
        match node {
            Node::Char(c) => one(s.get(at) == Some(c), k, caps),
            Node::Any => one(s.get(at).is_some_and(|&c| c != '\n'), k, caps),
            Node::Class(class) => one(s.get(at).is_some_and(|&c| class.contains(c)), k, caps),
            Node::Start => at == 0 && k(at, caps),
            Node::End => (at == s.len() || at + 1 == s.len() && s[at] == '\n') && k(at, caps),
            Node::EndStrict => at == s.len() && k(at, caps),
            Node::Boundary(wanted) => {
                let before = at > 0 && is_word(s[at - 1]);
                let after = s.get(at).is_some_and(|&c| is_word(c));
                (before != after) == *wanted && k(at, caps)
            }
            Node::Group(capture, alts) => self.alternatives(alts, at, caps, &mut |p, caps| {
                let Some(i) = *capture else {
                    return k(p, caps);
                };
                let old = caps[i];
                caps[i] = Some((at, p));
                if k(p, caps) {
                    return true;
                }
                caps[i] = old;
                false
            }),
            Node::Repeat {
                node,
                min,
                max,
                greedy,
            } => self.repeat(node, *min, *max, *greedy, 0, at, caps, k),
            Node::Backref(i) => match caps.get(*i).copied().flatten() {
                Some((a, b)) => {
                    let n = b - a;
                    s.get(at..at + n) == Some(&s[a..b]) && k(at + n, caps)
                }
                // PCRE fails a reference to a group that took no part.
                None => false,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        node: &Node,
        min: u32,
        max: Option<u32>,
        greedy: bool,
        count: u32,
        at: usize,
        caps: &mut Captures,
        k: Cont<'_>,
    ) -> bool {
        let more = max.is_none_or(|max| count < max);
        if !greedy && count >= min && k(at, caps) {
            return true;
        }
        let mut again = |p: usize, caps: &mut Captures| {
            // An empty iteration past the minimum ends the repetition.
            if p == at && count >= min {
                return false;
            }
            self.repeat(node, min, max, greedy, count + 1, p, caps, k)
        };
        if more && self.node(node, at, caps, &mut again) {
            return true;
        }
        greedy && count >= min && k(at, caps)
    }
}

impl Program {
    /// The first match at or after `from`: each group's positions.
    fn search(&self, s: &[char], from: usize) -> Option<Captures> {
        let m = Matcher { s };
        for start in from..=s.len() {
            let mut caps: Captures = alloc::vec![None; self.groups + 1];
            let mut end = None;
            if m.alternatives(&self.alternatives, start, &mut caps, &mut |p, _| {
                end = Some(p);
                true
            }) {
                caps[0] = Some((start, end.unwrap_or(start)));
                return Some(caps);
            }
        }
        None
    }

    /// The matches of `s` that do not overlap, in order.
    fn matches(&self, s: &[char], all: bool) -> Vec<Captures> {
        let mut found = Vec::new();
        let mut from = 0;
        while from <= s.len() {
            let Some(caps) = self.search(s, from) else {
                break;
            };
            let (start, end) = caps[0].unwrap_or((from, from));
            found.push(caps);
            if !all {
                break;
            }
            from = if end == start { end + 1 } else { end };
        }
        found
    }
}

/// `(ly:make-regex pattern)`.
pub(crate) fn make_regex(cx: &mut Cx, args: &[Value]) -> R {
    let Some(pattern) = cx.heap.str(args[0]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("ly:make-regex", 1, args[0], "string"));
    };
    match compile(&pattern) {
        Ok(program) => Ok(cx.heap.wrap(Regex {
            pattern,
            program: OnceCell::from(program),
        })),
        Err(message) => Err(engine_error(cx, "ly:make-regex", message)),
    }
}

/// `check_error`'s message.
fn engine_error(cx: &mut Cx, subr: &str, message: &str) -> Thrown {
    let message = cx.heap.string(message);
    cx.misc_error(
        Some(subr),
        "the GLib regex engine signaled an error: ~a",
        &[message],
    )
}

/// `(%regex-exec regex string all subr)`: the positions of the first match
/// of `regex` in `string`, or `#f`; with `all`, a list of every match's.
/// `subr` names the procedure in errors.
pub(crate) fn regex_exec(cx: &mut Cx, args: &[Value]) -> R {
    let subr = match cx.heap.name(args[3]) {
        Some(name) => String::from(name),
        None => String::from("ly:regex-exec"),
    };
    let Some(regex) = cx.heap.foreign::<Regex>(args[0]) else {
        return Err(cx.wrong_type(&subr, 1, args[0], "Regex"));
    };
    let program = match regex.program.get() {
        Some(program) => program.clone(),
        None => match compile(&regex.pattern) {
            Ok(program) => regex.program.get_or_init(|| program).clone(),
            Err(message) => return Err(engine_error(cx, &subr, message)),
        },
    };
    let Some(s) = cx.heap.str(args[1]) else {
        return Err(cx.wrong_type(&subr, 2, args[1], "string"));
    };
    let chars: Vec<char> = s.as_str().chars().collect();
    let all = args[2] != Value::FALSE;
    let found = program.matches(&chars, all);
    let mut vectors = Vec::with_capacity(found.len());
    for caps in found {
        let items = caps
            .into_iter()
            .map(|pos| match pos {
                Some((a, b)) => {
                    let (a, b) = (cx.heap.integer(a as i64), cx.heap.integer(b as i64));
                    cx.heap.cons(a, b)
                }
                None => Value::FALSE,
            })
            .collect();
        vectors.push(cx.heap.vector(items));
    }
    if all {
        Ok(cx.heap.list(&vectors))
    } else {
        Ok(vectors.pop().unwrap_or(Value::FALSE))
    }
}

/// `(ly:regex-quote string)`: `g_regex_escape_string`.
pub(crate) fn regex_quote(cx: &mut Cx, args: &[Value]) -> R {
    let Some(s) = cx.heap.str(args[0]) else {
        return Err(cx.wrong_type("ly:regex-quote", 1, args[0], "string"));
    };
    let mut out = String::new();
    for c in s.as_str().chars() {
        match c {
            '\0' => out.push_str("\\0"),
            '\\' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '*' | '+' | '?' | '.' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    Ok(cx.heap.string(out))
}

/// The primitives of `(lily)` here.
pub(crate) const NAMES: &[&str] = &["ly:make-regex", "ly:regex-quote"];

/// `ly:regex-exec`, `ly:regex-exec->list` and `ly:regex-replace` in
/// Scheme, over `%regex-exec`; `<regex-match>` and
/// `ly:regex-match-positions` are `lily-library.scm`'s, found when they
/// are called.
pub(crate) const PRELUDE: &str = r#"
(define (%regex-match string positions)
  (make <regex-match> #:original-string string #:substring-positions positions))
(define-public (ly:regex-exec regex string)
  (let ((positions (%regex-exec regex string #f 'ly:regex-exec)))
    (and positions (%regex-match string positions))))
(define-public (ly:regex-exec->list regex string)
  (map (lambda (positions) (%regex-match string positions))
       (%regex-exec regex string #t 'ly:regex-exec->list)))
(define-public (ly:regex-replace regex string . replacements)
  (let check ((rest replacements) (i 3))
    (when (pair? rest)
      (let ((r (car rest)))
        (if (not (or (string? r) (and (exact-integer? r) (>= r 0)) (procedure? r)))
            (scm-error 'wrong-type-arg "ly:regex-replace"
                       "Wrong type argument in position ~A (expecting ~A): ~S"
                       (list i "string, non-negative integer or procedure" r) (list r))))
      (check (cdr rest) (1+ i))))
  (let loop ((matches (%regex-exec regex string #t 'ly:regex-replace)) (end 0) (out '()))
    (if (null? matches)
        (apply string-append (reverse! (cons (substring string end) out)))
        (let* ((positions (car matches))
               (whole (vector-ref positions 0))
               (parts
                (map (lambda (r)
                       (cond ((string? r) r)
                             ((procedure? r) (r (%regex-match string positions)))
                             ((and (< r (vector-length positions)) (vector-ref positions r))
                              => (lambda (p) (substring string (car p) (cdr p))))
                             (else "")))
                     replacements)))
          (loop (cdr matches) (cdr whole)
                (append (reverse parts) (cons (substring string end (car whole)) out)))))))
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(pattern: &str, s: &str) -> Option<Vec<Option<(usize, usize)>>> {
        let chars: Vec<char> = s.chars().collect();
        compile(pattern).unwrap().search(&chars, 0)
    }

    #[test]
    fn clef_names_split_as_parser_clef_scm_reads_them() {
        let clef = "^(.*)([_^])([^0-9a-zA-Z]*)([1-9][0-9]*)([^0-9a-zA-Z]*)$";
        assert_eq!(exec(clef, "bass"), None);
        assert_eq!(
            exec(clef, "G_8"),
            Some(alloc::vec![
                Some((0, 3)),
                Some((0, 1)),
                Some((1, 2)),
                Some((2, 2)),
                Some((2, 3)),
                Some((3, 3))
            ])
        );
        // Greedy `(.*)` takes the last modifier.
        let caps = exec(clef, "treble^(15)").unwrap();
        assert_eq!(caps[1], Some((0, 6)));
        assert_eq!(caps[3], Some((7, 8)));
        assert_eq!(caps[4], Some((8, 10)));
    }

    #[test]
    fn pcre_order_alternatives_and_laziness() {
        assert_eq!(exec("a|ab", "ab").unwrap()[0], Some((0, 1)));
        assert_eq!(exec("a+?", "aaa").unwrap()[0], Some((0, 1)));
        assert_eq!(
            exec("(a+)|(b+)", "bb").unwrap(),
            alloc::vec![Some((0, 2)), None, Some((0, 2))]
        );
        assert_eq!(
            exec("\\d{4}-\\d{2}", "on 2026-10").unwrap()[0],
            Some((3, 10))
        );
        assert_eq!(exec("x{2,}", "xxxx").unwrap()[0], Some((0, 4)));
        assert_eq!(exec("a{,2}", "a{,2}").unwrap()[0], Some((0, 5)));
        assert_eq!(
            exec("-markup(-list)?$", "bold-markup").unwrap()[0],
            Some((4, 11))
        );
        assert_eq!(
            exec("(SCM|\\)|\\() *", "f (SCM x)").unwrap()[0],
            Some((2, 3))
        );
        assert_eq!(exec("~[^.]~", "a~b~c").unwrap()[0], Some((1, 4)));
        assert_eq!(exec("é(.)", "aéb").unwrap()[0], Some((1, 3)));
        assert_eq!(exec("(a)\\1", "xaa").unwrap()[0], Some((1, 3)));
        assert_eq!(exec("\\bend\\b", "bend end").unwrap()[0], Some((5, 8)));
        assert_eq!(
            exec("end of file$", "x end of file\n").unwrap()[0],
            Some((2, 13))
        );
    }

    #[test]
    fn bad_patterns_are_errors() {
        for pattern in ["(a", "a)", "[a", "*a", "\\q", "[z-a]"] {
            assert!(compile(pattern).is_err(), "{pattern}");
        }
    }

    #[test]
    fn every_match_and_empty_ones() {
        let chars: Vec<char> = "a,b,,c".chars().collect();
        let found = compile(",").unwrap().matches(&chars, true);
        assert_eq!(
            found.iter().map(|c| c[0].unwrap()).collect::<Vec<_>>(),
            [(1, 2), (3, 4), (4, 5)]
        );
        let found = compile("x*").unwrap().matches(&['a', 'b'], true);
        assert_eq!(found.len(), 3);
    }
}
