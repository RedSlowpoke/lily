//! Diagnostics in LilyPond's format (`flower/warn.cc`, `input.cc`): a
//! location, the severity and the message, and below a message located at a
//! byte the excerpt of [`Source::quote`]. They are kept in the order they
//! were given and printed only when asked, so the line table is read only
//! then (02 §5.2). [`Diagnostics::messages`] places them, which is the list
//! the core writes both its `stderr` and its `diagnostics.json` from (08 §6).
//!
//! `ly:expect-warning` is kept among them in its place (MS2 step 37,
//! `flower/warn.cc`): a warning, an error or a programming error after it
//! whose message starts with its text is not printed, and takes it out
//! ([`is_expected`]); the expected ones left are what
//! [`Diagnostics::report`] gives besides the messages.

use crate::source::{Source, Sources, Span};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    /// `fatal error:`, which ends the run (`failed files:`).
    Fatal,
}

/// Where a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// At a span's first byte: `file:line:column: ` and the excerpt (an `Input`).
    Span(Span),
    /// `file:line: `, without an excerpt (`ly:warning-located` with a string).
    Line(u16, usize),
    /// No location (`ly:non-fatal-error`, `ly:warning`).
    Nowhere,
    /// No message: text Scheme wrote to the error port, or LilyPond to
    /// stderr itself, printed as it is.
    Raw,
    /// An `Input` of [`Diagnostics`]' own, by its index: one this parse may
    /// not have the file of, which then prints its place without an excerpt.
    Input(u32),
    /// A message another parse placed already, by its index: a template's
    /// (`embedded.rs`), with its excerpt of the template's text.
    Placed(u32),
    /// No message: a warning `ly:expect-warning` expects, its text the
    /// diagnostic's.
    Expect,
    /// No message: `ly:check-expected-warnings`, where the expected
    /// warnings not met so far are warned of and forgotten (OPEN C3's
    /// scores).
    Check,
    /// No message: `ly:parser-clear-error` of the running parse
    /// (`clear_error`, P12), before which no error fails the file.
    Cleared,
    /// A programming error at an `Input` of [`Diagnostics`]' own, by its
    /// index: its place, the excerpt and its place again before
    /// `continuing, cross fingers` (`Input::programming_error`), printed as
    /// raw text.
    Programming(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub at: At,
    pub message: String,
}

/// An `Input` a diagnostic is at ([`At::Input`]).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    span: Span,
    name: Rc<str>,
    line: usize,
    column: usize,
    /// The text of a source no parse keeps (`Input::text`).
    text: Option<Rc<str>>,
}

#[derive(Clone, Default)]
pub struct Diagnostics {
    list: Vec<Diagnostic>,
    /// What Scheme wrote to Guile's error port and Guile has not flushed:
    /// LilyPond's own messages go straight to stderr, and Guile's port is
    /// flushed by `force-output`, when Guile prints an error, and at the end.
    pending: String,
    inputs: Vec<Place>,
    placed: Vec<Message>,
}

/// Where [`Diagnostics`] stood ([`Diagnostics::mark`]).
#[derive(Clone, Debug)]
pub(crate) struct DiagMark {
    list: usize,
    inputs: usize,
    placed: usize,
    pending: String,
}

impl Diagnostics {
    pub fn error(&mut self, at: At, message: impl Into<String>) {
        self.list.push(Diagnostic {
            severity: Severity::Error,
            at,
            message: message.into(),
        });
    }

    /// An error of Scheme: LilyPond's `message` at `at`, then Guile's
    /// `text` on its error port, which flushes it.
    pub fn guile_error(&mut self, at: At, message: impl Into<String>, text: &str) {
        self.error(at, message);
        self.output(text);
        self.output("\n");
        self.flush();
    }

    pub fn warning(&mut self, at: At, message: impl Into<String>) {
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at,
            message: message.into(),
        });
    }

    /// Where an `Input` is: at its span if the parse has its file, else at
    /// its place, whose excerpt the parse cannot print ([`At::Input`]).
    pub fn at_input(&mut self, input: &lily_engine_music::Input) -> At {
        self.inputs.push(Place {
            span: Span {
                file: input.file,
                start: input.start,
                end: input.end,
            },
            name: input.name.clone(),
            line: input.line as usize,
            column: input.column as usize,
            text: input.text.clone(),
        });
        At::Input(self.inputs.len() as u32 - 1)
    }

    /// A message another parse placed ([`At::Placed`]), in its place
    /// among these. Its error is that parse's, not this one's: it is not
    /// counted among [`Diagnostics::errors`].
    pub fn placed(&mut self, message: Message) {
        self.placed.push(message);
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at: At::Placed(self.placed.len() as u32 - 1),
            message: String::new(),
        });
    }

    /// `expect_warning`: a message starting with `text` is expected from
    /// here on.
    pub fn expect(&mut self, text: impl Into<String>) {
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at: At::Expect,
            message: text.into(),
        });
    }

    /// `check_expected_warnings`: here the expected warnings unmet so far
    /// are warned of, and none is expected after.
    pub fn check_expected(&mut self) {
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at: At::Check,
            message: String::new(),
        });
    }

    /// `clear_error`: the errors so far no longer fail the file.
    pub fn clear_errors(&mut self) {
        self.list.push(Diagnostic {
            severity: Severity::Error,
            at: At::Cleared,
            message: String::new(),
        });
    }

    /// Text Scheme wrote to Guile's error port, which waits for a flush.
    pub fn output(&mut self, text: &str) {
        self.pending.push_str(text);
    }

    /// Flushes Guile's error port among the diagnostics.
    pub fn flush(&mut self) {
        if !self.pending.is_empty() {
            let text = core::mem::take(&mut self.pending);
            self.raw(text);
        }
    }

    /// A programming error (`flower/warn.cc`): at an `Input`
    /// ([`At::Programming`]) when `at` is one, else LilyPond's two lines
    /// without a place.
    pub fn programming_error(&mut self, at: Option<At>, message: &str) {
        let at = match at {
            Some(At::Input(i)) => At::Programming(i),
            _ => {
                return self.raw(alloc::format!(
                    "programming error: {message}\ncontinuing, cross fingers\n"
                ));
            }
        };
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at,
            message: message.into(),
        });
    }

    /// Text LilyPond writes to stderr itself, printed in its place as it is.
    pub fn raw(&mut self, text: impl Into<String>) {
        self.list.push(Diagnostic {
            severity: Severity::Warning,
            at: At::Raw,
            message: text.into(),
        });
    }

    pub fn list(&self) -> &[Diagnostic] {
        &self.list
    }

    /// How much was said: what a lookahead's lexing said changes it.
    pub(crate) fn said(&self) -> (usize, usize) {
        (self.list.len(), self.pending.len())
    }

    /// Whether what waits for Guile's flush is what it was at `mark`.
    pub(crate) fn same_pending(&self, mark: &DiagMark) -> bool {
        self.pending == mark.pending
    }

    /// Where they stand, for [`Diagnostics::cut`]: a parse started again
    /// there (`reuse.rs`) has what was said before it.
    pub(crate) fn mark(&self) -> DiagMark {
        DiagMark {
            list: self.list.len(),
            inputs: self.inputs.len(),
            placed: self.placed.len(),
            pending: self.pending.clone(),
        }
    }

    /// Appends what `old` said from `from` to `to`, its indexes of inputs
    /// and placed messages moved after these (a segment of a parse taken
    /// over, `reuse.rs`); false, and nothing appended, unless what waits for
    /// Guile's flush is `from`'s.
    pub(crate) fn append(&mut self, old: &Diagnostics, from: &DiagMark, to: &DiagMark) -> bool {
        if self.pending != from.pending || to.list > old.list.len() || to.list < from.list {
            return false;
        }
        let inputs = self.inputs.len() as u32;
        let placed = self.placed.len() as u32;
        let moved = |i: u32, from: usize, to: u32| i - from as u32 + to;
        for d in &old.list[from.list..to.list] {
            let at = match d.at {
                At::Input(i) => At::Input(moved(i, from.inputs, inputs)),
                At::Programming(i) => At::Programming(moved(i, from.inputs, inputs)),
                At::Placed(i) => At::Placed(moved(i, from.placed, placed)),
                at => at,
            };
            self.list.push(Diagnostic { at, ..d.clone() });
        }
        self.inputs
            .extend_from_slice(&old.inputs[from.inputs..to.inputs]);
        self.placed
            .extend_from_slice(&old.placed[from.placed..to.placed]);
        self.pending = to.pending.clone();
        true
    }

    /// Them as they were at `mark`.
    pub(crate) fn cut(&self, mark: &DiagMark) -> Diagnostics {
        Diagnostics {
            list: self.list[..mark.list.min(self.list.len())].to_vec(),
            pending: mark.pending.clone(),
            inputs: self.inputs[..mark.inputs.min(self.inputs.len())].to_vec(),
            placed: self.placed[..mark.placed.min(self.placed.len())].to_vec(),
        }
    }

    /// The errors since `ly:parser-clear-error` last cleared them.
    pub fn errors(&self) -> usize {
        let from = self
            .list
            .iter()
            .rposition(|d| d.at == At::Cleared)
            .map_or(0, |i| i + 1);
        self.list[from..]
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// The diagnostics placed in `sources`: what LilyPond prints for each.
    pub fn messages(&self, sources: &Sources) -> Vec<Message> {
        self.report(sources).0
    }

    /// [`Diagnostics::messages`], and the expected warnings none of them
    /// met, the oldest first.
    pub fn report(&self, sources: &Sources) -> (Vec<Message>, Vec<String>) {
        let mut expected = Vec::new();
        let mut messages = Vec::new();
        for d in &self.list {
            if d.at == At::Expect {
                expected.push(d.message.clone());
                continue;
            }
            // `ly:check-expected-warnings`: those left unmet, warned of
            // here and forgotten (`flower/warn.cc`).
            if d.at == At::Check {
                if !expected.is_empty() {
                    messages.push(Message::unmet(&expected));
                    expected.clear();
                }
                continue;
            }
            if d.at == At::Cleared {
                continue;
            }
            let message = self.place(d, sources);
            if !message
                .expected_text()
                .is_some_and(|t| is_expected(&mut expected, &t))
            {
                messages.push(message);
            }
        }
        (messages, expected)
    }

    /// The diagnostic `d` placed in `sources`.
    fn place(&self, d: &Diagnostic, sources: &Sources) -> Message {
        if let At::Placed(i) = d.at {
            return self.placed[i as usize].clone();
        }
        let (place, excerpt) = match d.at {
            At::Span(span) => {
                let src = sources.get(span.file);
                (Message::at(src, span.start), src.quote(span.start))
            }
            At::Line(file, line) => (
                Located::Line(String::from(&*sources.get(file).shown_name()), line),
                Vec::new(),
            ),
            At::Nowhere => (Located::Nowhere, Vec::new()),
            At::Raw | At::Placed(_) | At::Expect | At::Check | At::Cleared => {
                (Located::Raw, Vec::new())
            }
            At::Input(i) => self.input_place(i, sources),
            At::Programming(i) => {
                let (place, excerpt) = self.input_place(i, sources);
                return Message::programming(&place, &d.message, &excerpt);
            }
        };
        Message {
            severity: d.severity,
            place,
            text: d.message.clone(),
            excerpt,
        }
    }

    /// Where the `Input` `i` of these is, and its excerpt: at its span if
    /// `sources` have its file, else at its place without one.
    fn input_place(&self, i: u32, sources: &Sources) -> (Located, Vec<u8>) {
        let p = &self.inputs[i as usize];
        match sources.source(p.span.file) {
            Some(src) if src.is_named(&p.name) => {
                (Message::at(src, p.span.start), src.quote(p.span.start))
            }
            _ => {
                // A clone's own text, which it kept (D92).
                let excerpt = p.text.as_ref().map_or_else(Vec::new, |text| {
                    Source::new(&p.name, text.as_bytes().to_vec()).quote(p.span.start)
                });
                (
                    Located::Column(String::from(&*p.name), p.line, p.column + 1),
                    excerpt,
                )
            }
        }
    }

    /// What a template's parse hands to the parse that runs it, in order
    /// (`embedded.rs`): each diagnostic placed in `sources`, but one at an
    /// `Input` of no file of them, which that parse places.
    pub(crate) fn hand_on(&self, sources: &Sources) -> Vec<Handed> {
        self.list
            .iter()
            // A template's own `ly:parser-clear-error` is its clone's.
            .filter(|d| d.at != At::Cleared)
            .map(|d| (d, self.place(d, sources)))
            .map(|(d, message)| match d.at {
                At::Expect => Handed::Expect(d.message.clone()),
                At::Check => Handed::Check,
                At::Input(i) | At::Programming(i) => {
                    let p = &self.inputs[i as usize];
                    match sources.source(p.span.file) {
                        Some(src) if src.is_named(&p.name) => Handed::Placed(message),
                        _ => Handed::At(
                            match d.at {
                                At::Programming(_) => "programming-error",
                                _ if d.severity == Severity::Warning => "warning",
                                _ => "error",
                            },
                            {
                                let mut input = lily_engine_music::Input::new(
                                    p.span.file,
                                    p.span.start,
                                    p.span.end,
                                    p.name.clone(),
                                    [p.line as u32, 0, p.column as u32],
                                );
                                input.text = p.text.clone();
                                input
                            },
                            d.message.clone(),
                        ),
                    }
                }
                _ => Handed::Placed(message),
            })
            .collect()
    }

    /// The diagnostics as LilyPond prints them to stderr.
    pub fn write(&self, sources: &Sources, out: &mut Vec<u8>) {
        for message in self.messages(sources) {
            message.write(out);
        }
    }
}

/// A diagnostic a template's parse hands on ([`Diagnostics::hand_on`]).
pub(crate) enum Handed {
    Placed(Message),
    /// At an `Input` of another parse, which that one places, by the kind
    /// `%diagnose` gives it.
    At(&'static str, lily_engine_music::Input, String),
    /// An expected warning, which that parse expects in its place.
    Expect(String),
    /// `ly:check-expected-warnings`, which that parse checks in its place.
    Check,
}

/// `is_expected`: whether `text` starts with one of `expected`, the first
/// such taken out.
pub fn is_expected(expected: &mut Vec<String>, text: &str) -> bool {
    match expected.iter().position(|e| text.starts_with(e.as_str())) {
        Some(i) => {
            expected.remove(i);
            true
        }
        None => false,
    }
}

/// Where a [`Message`] is, as it prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Located {
    /// `file:line:column: `, the column from 1.
    Column(String, usize, usize),
    /// `file:line: `.
    Line(String, usize),
    Nowhere,
    /// No message: text printed as it is.
    Raw,
}

impl Located {
    /// What prints before the keyword: `file:line:column: `, `file:line: `
    /// or nothing.
    fn prefix(&self) -> String {
        match self {
            Located::Column(file, line, column) => alloc::format!("{file}:{line}:{column}: "),
            Located::Line(file, line) => alloc::format!("{file}:{line}: "),
            Located::Nowhere | Located::Raw => String::new(),
        }
    }
}

/// A diagnostic placed: its file named as the host gave it, its line and
/// column counted, and its excerpt, so that it no longer needs the sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub severity: Severity,
    pub place: Located,
    /// The message, or the raw text.
    pub text: String,
    /// The two lines below a message at a column; the source's bytes, valid
    /// UTF-8 or not. Empty for the others.
    pub excerpt: Vec<u8>,
}

impl Message {
    fn at(src: &Source, pos: u32) -> Located {
        let c = src.counts(pos);
        Located::Column(String::from(&*src.shown_name()), c.line, c.column + 1)
    }

    /// `warning: cannot find file: `name'`, as `lily.scm` warns of a file
    /// it was given and cannot open.
    pub fn missing_file(name: &str) -> Message {
        Message {
            severity: Severity::Warning,
            place: Located::Nowhere,
            text: alloc::format!("cannot find file: `{name}'"),
            excerpt: Vec::new(),
        }
    }

    /// The line that ends a run whose files had errors (`lily.scm`): their
    /// names joined by spaces, written as one Scheme string.
    pub fn failed_files(names: &[&str]) -> Message {
        let mut text = String::from("failed files: \"");
        for c in names.join(" ").chars() {
            if c == '"' || c == '\\' {
                text.push('\\');
            }
            text.push(c);
        }
        text.push('"');
        Message {
            severity: Severity::Fatal,
            place: Located::Nowhere,
            text,
            excerpt: Vec::new(),
        }
    }

    /// `check_expected_warnings`: the warning of the expected warnings
    /// `expected` none met.
    pub fn unmet(expected: &[String]) -> Message {
        let mut text = alloc::format!("{} expected warning(s) not encountered: ", expected.len());
        for e in expected {
            text.push_str("\n        ");
            text.push_str(e);
        }
        Message {
            severity: Severity::Warning,
            place: Located::Nowhere,
            text,
            excerpt: Vec::new(),
        }
    }

    /// What `is_expected` is given of it: a warning's or an error's
    /// message, and its excerpt after a newline where it has one, as
    /// `Input::message_string` gives `flower/warn.cc` both (OPEN C2.10); a
    /// programming error's after its keyword, placed or not; `None` for a
    /// fatal error or raw text.
    pub fn expected_text(&self) -> Option<alloc::borrow::Cow<'_, str>> {
        use alloc::borrow::Cow;
        match (&self.place, self.severity) {
            (Located::Raw, _) => self
                .text
                .strip_prefix("programming error: ")
                .or_else(|| {
                    let (place, text) = self.text.split_once(": programming error: ")?;
                    (!place.contains('\n')).then_some(text)
                })
                .map(Cow::Borrowed),
            (_, Severity::Fatal) => None,
            _ if self.excerpt.is_empty() => Some(Cow::Borrowed(&self.text)),
            _ => Some(Cow::Owned(alloc::format!(
                "{}\n{}",
                self.text,
                String::from_utf8_lossy(&self.excerpt)
            ))),
        }
    }

    /// A programming error at `place` (`flower/warn.cc`'s two messages):
    /// raw text, as Scheme's output is, so that it prints in its place.
    /// The excerpt's bytes that are no UTF-8 print as U+FFFD.
    fn programming(place: &Located, message: &str, excerpt: &[u8]) -> Message {
        let place = place.prefix();
        let mut text = alloc::format!("{place}programming error: {message}\n");
        if !excerpt.is_empty() {
            text.push_str(&String::from_utf8_lossy(excerpt));
            text.push('\n');
        }
        text.push_str(&alloc::format!("{place}continuing, cross fingers\n"));
        Message {
            severity: Severity::Warning,
            place: Located::Raw,
            text,
            excerpt: Vec::new(),
        }
    }

    /// The keyword after the place: `error`, `warning` or `fatal error`.
    pub fn keyword(&self) -> &'static str {
        match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Fatal => "fatal error",
        }
    }

    /// The message as LilyPond prints it to stderr.
    pub fn write(&self, out: &mut Vec<u8>) {
        match &self.place {
            Located::Raw => {
                out.extend_from_slice(self.text.as_bytes());
                return;
            }
            place => out.extend_from_slice(place.prefix().as_bytes()),
        }
        out.extend_from_slice(self.keyword().as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(self.text.as_bytes());
        if !self.excerpt.is_empty() {
            out.push(b'\n');
            out.extend_from_slice(&self.excerpt);
        }
        out.push(b'\n');
    }
}

/// [`Message::failed_files`] as it prints.
pub fn failed_files(names: &[&str]) -> String {
    let mut out = Vec::new();
    Message::failed_files(names).write(&mut out);
    String::from_utf8(out).unwrap_or_default()
}
