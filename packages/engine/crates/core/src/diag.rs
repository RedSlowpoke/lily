//! One list of diagnostics per compile, written as the two outputs of 08 §6:
//! `stderr` in LilyPond's own format, and `diagnostics.json`, the same list
//! as `LyDiagnostic[]` (`@lily/common/types`), which the studio's page
//! reads, so that neither it nor `packages/common` needs a stderr parser.
//!
//! The list is the parser's placed [`Message`]s, warnings and errors in
//! their place among what Scheme wrote to the error port (`At::Raw`). Its
//! JSON is what the extension's `parseStderr` (D6, D16) makes of the same
//! stderr, which chapter 00's C0 checks: a message starts a diagnostic, and
//! each line after it that does not start one is part of its message, the
//! two lines of its excerpt left out, trimmed, empty ones dropped, and
//! joined by newlines. Text Scheme wrote is read line by line as
//! `parseStderr` reads it, since a line of it may start a message. The
//! messages carry their place, so a source line in an excerpt is never read
//! as a message, as `parseStderr` could read it. Guile's error that ends a
//! run (case 20: `ERROR: In procedure ly:parse-file:` and the error) is
//! such text, so it is part of the message before it, as D64 has C0 read it.
//!
//! An error that ends a translation ends the run (MS3 step 35, D132):
//! [`ended`] gives what LilyPond prints for it. A font whose pack the host
//! could not fetch is the engine's own error, [`unfetched`] (W5, D208).

use lily_engine_parser::source::DATADIR;
use lily_engine_parser::{Located, Message, Severity};

/// What LilyPond prints when an error, `text`, ends the translations of a
/// book, as its run ends there (`parse-scm.cc:184–191`). For the book
/// `init.ly` makes, whose handler `init.ly` runs with
/// `protected-scheme-parsing` off: Guile's error for that expression
/// (2.26.0's `init.ly:66:2`), then the error. For a `\book` block, whose
/// handler the parser calls unprotected, Guile's report of the error it
/// does not catch, without the frame it names in Guile's own files
/// (`ice-9/boot-9.scm:2058:7: In procedure error:`), which the engine does
/// not have; `parseStderr` reads no diagnostic from LilyPond's either.
pub fn ended(text: &str, toplevel: bool) -> Vec<Message> {
    let mut out = Vec::new();
    if toplevel {
        out.push(Message {
            severity: Severity::Error,
            place: Located::Column(format!("{DATADIR}/ly/init.ly"), 66, 2),
            text: "Guile signaled an error for the expression beginning here".into(),
            excerpt: b"#\n (let ((book-handler (if (defined? 'default-toplevel-book-handler)"
                .into(),
        });
    }
    let newline = if text.ends_with('\n') { "" } else { "\n" };
    out.push(Message {
        severity: Severity::Error,
        place: Located::Raw,
        text: format!("{text}{newline}"),
        excerpt: Vec::new(),
    });
    out
}

/// `init.ly`'s error when the file sets `expect-error` and had none (P12):
/// `ly:parser-error` at the expression that checks it (2.26.0's
/// `init.ly:88:1`).
pub fn expected_none() -> Message {
    Message {
        severity: Severity::Error,
        place: Located::Column(format!("{DATADIR}/ly/init.ly"), 88, 1),
        text: "expected error, but none found".into(),
        excerpt: b"\n#(if (eq? expect-error (ly:parser-has-error?))".into(),
    }
}

/// The engine's error for the font `font`, whose pack `pack` the host
/// could not give (W5, E12): LilyPond's `cannot find font`
/// (all-font-metrics.cc:166) with the pack in place of the search path,
/// and not fatal, since the compile goes on without the font's pages.
pub fn unfetched(font: &str, pack: &str) -> Message {
    Message {
        severity: Severity::Error,
        place: Located::Nowhere,
        text: format!("cannot find font '{font}' (its font pack {pack} could not be fetched)"),
        excerpt: Vec::new(),
    }
}

/// The engine's warning for a text face whose outlines pack the host could
/// not give (D262): its glyphs are boxes in the skylines, so text may sit a
/// little apart from where LilyPond puts it.
pub fn unfetched_outlines(face: &str, pack: &str) -> Message {
    Message {
        severity: Severity::Warning,
        place: Located::Nowhere,
        text: format!(
            "the outlines of font '{face}' could not be fetched (font pack {pack}): its text is spaced by its glyphs' boxes"
        ),
        excerpt: Vec::new(),
    }
}

/// The engine's error for a score that needs more memory than B4's cap
/// (D256, D257): the compile is dropped, and what its pages would be is left
/// to the host, which may ask the server (K6).
pub fn over_memory() -> Message {
    Message {
        severity: Severity::Error,
        place: Located::Nowhere,
        text: format!(
            "this score needs more memory than the engine may use ({} MiB)",
            crate::MAX_MEMORY >> 20
        ),
        excerpt: Vec::new(),
    }
}

/// One compile's diagnostics, in the order LilyPond prints them.
#[derive(Default)]
pub struct List {
    messages: Vec<Message>,
}

impl List {
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.messages.extend(messages);
    }

    /// What LilyPond prints to stderr with `--loglevel=WARNING`.
    pub fn stderr(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for message in &self.messages {
            message.write(&mut out);
        }
        out
    }

    /// The list as `LyDiagnostic[]`: a message with no file is the root's,
    /// `root` being the name the host gave (`lily_put`), as is every file's.
    pub fn json(&self, root: &str) -> Vec<u8> {
        let mut out = String::from("[");
        for (i, d) in self.diagnostics(root).iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"file\":");
            push_json(&mut out, &d.file);
            out.push_str(&format!(",\"line\":{}", d.line));
            if let Some(column) = d.column {
                out.push_str(&format!(",\"column\":{column}"));
            }
            out.push_str(",\"severity\":");
            out.push_str(if d.error { "\"error\"" } else { "\"warning\"" });
            out.push_str(",\"message\":");
            push_json(&mut out, &d.message);
            out.push('}');
        }
        out.push(']');
        out.into_bytes()
    }

    /// `parseStderr`'s diagnostics of [`List::stderr`].
    fn diagnostics(&self, root: &str) -> Vec<Diagnostic> {
        let mut blocks: Vec<Block> = Vec::new();
        for m in &self.messages {
            // A newline ends the text, and a line Scheme left open is one
            // here, where LilyPond's next message would continue it.
            let text = m.text.strip_suffix('\n').unwrap_or(&m.text);
            let mut lines = text.split('\n');
            if m.place != Located::Raw {
                let (file, line, column) = match &m.place {
                    Located::Column(file, line, column) => {
                        (Some(file.clone()), *line, Some(*column))
                    }
                    Located::Line(file, line) => (Some(file.clone()), *line, None),
                    _ => (None, 1, None),
                };
                blocks.push(Block {
                    header: Header {
                        file,
                        line,
                        column,
                        keyword: m.keyword(),
                        message: lines.next().unwrap_or("").into(),
                    },
                    lines: Vec::new(),
                    // Its excerpt is not among the lines.
                    excerpt: false,
                });
            }
            for line in lines {
                let line = line.strip_suffix('\r').unwrap_or(line);
                match parse_header(line) {
                    Some(header) => blocks.push(Block {
                        header,
                        lines: Vec::new(),
                        excerpt: true,
                    }),
                    // Before the first message (Guile's notes), it is dropped.
                    None => {
                        if let Some(block) = blocks.last_mut() {
                            block.lines.push(line.into());
                        }
                    }
                }
            }
        }
        let mut out: Vec<Diagnostic> = Vec::new();
        for block in blocks {
            let d = block.diagnostic(root);
            if !out.contains(&d) {
                out.push(d);
            }
        }
        // `failed files:` closes a failed run; beside the errors that caused
        // it, it is noise.
        let summary =
            |d: &Diagnostic| d.column.is_none() && d.message.starts_with("failed files: ");
        if out.iter().any(|d| d.error && !summary(d)) {
            out.retain(|d| !summary(d));
        }
        out
    }
}

/// A `LyDiagnostic`.
#[derive(Debug, PartialEq, Eq)]
struct Diagnostic {
    file: String,
    line: usize,
    column: Option<usize>,
    error: bool,
    message: String,
}

/// The first line of a message: `file:line:column: keyword: message`.
struct Header {
    file: Option<String>,
    line: usize,
    column: Option<usize>,
    keyword: &'static str,
    message: String,
}

/// A message's first line and the lines after it that start none.
struct Block {
    header: Header,
    lines: Vec<String>,
    /// Whether its excerpt may be among the lines, as for a message read
    /// from text, which `parseStderr`'s `continuation` looks for.
    excerpt: bool,
}

const CROSS_FINGERS: &str = "continuing, cross fingers";

impl Block {
    fn diagnostic(self, root: &str) -> Diagnostic {
        let h = self.header;
        let lines = match h.column {
            Some(column) if self.excerpt => continuation(self.lines, column),
            _ => self.lines,
        };
        // Text Scheme parsed is in `<included string>` or `<string>`, which
        // is no file: the message goes to the root and names where it was.
        let in_string = h
            .file
            .as_deref()
            .is_some_and(|f| f.len() >= 2 && f.starts_with('<') && f.ends_with('>'));
        let origin = in_string.then(|| {
            let column = h
                .column
                .map_or_else(|| "undefined".into(), |c| c.to_string());
            format!(
                "(in {}, line {}, column {column})",
                h.file.as_deref().unwrap_or(""),
                h.line
            )
        });
        let message = core::iter::once(h.message.as_str())
            .chain(lines.iter().map(String::as_str))
            .chain(origin.as_deref())
            .map(trim)
            .filter(|line| !line.is_empty() && *line != CROSS_FINGERS)
            .collect::<Vec<_>>()
            .join("\n");
        let located = h.file.is_some() && !in_string;
        Diagnostic {
            file: match (&h.file, located) {
                (Some(file), true) => file.clone(),
                _ => root.into(),
            },
            line: if located { h.line.max(1) } else { 1 },
            column: h.column.filter(|_| located).map(|c| c.max(1)),
            error: matches!(h.keyword, "error" | "fatal error"),
            message: if h.keyword == "programming error" {
                format!("programming error: {message}")
            } else {
                message
            },
        }
    }
}

const KEYWORDS: [&str; 4] = ["fatal error", "programming error", "error", "warning"];

/// `keyword: message` at the start of `text`.
fn keyword(text: &str) -> Option<(&'static str, &str)> {
    KEYWORDS.iter().find_map(|&k| {
        let rest = text.strip_prefix(k)?.strip_prefix(": ")?;
        Some((k, rest))
    })
}

/// ASCII digits at the start of `text`, as many as there are.
fn digits(text: &str) -> Option<(usize, &str)> {
    let n = text.bytes().take_while(u8::is_ascii_digit).count();
    let value = text[..n].parse().ok()?;
    Some((value, &text[n..]))
}

/// `parseStderr`'s `parseHeader`: `keyword: message`, or else
/// `file:line[:column]: keyword: message` with the shortest file. Its
/// regular expressions match no line with a line terminator but `\n`.
fn parse_header(line: &str) -> Option<Header> {
    if line.contains(['\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    if let Some((k, message)) = keyword(line) {
        return Some(Header {
            file: None,
            line: 1,
            column: None,
            keyword: k,
            message: message.into(),
        });
    }
    for (i, _) in line.match_indices(':').filter(|&(i, _)| i > 0) {
        let Some((number, after)) = digits(&line[i + 1..]) else {
            continue;
        };
        let with_column = after
            .strip_prefix(':')
            .and_then(digits)
            .and_then(|(column, after)| Some((Some(column), keyword(after.strip_prefix(": ")?)?)));
        let found = with_column.or_else(|| Some((None, keyword(after.strip_prefix(": ")?)?)));
        if let Some((column, (k, message))) = found {
            return Some(Header {
                file: Some(line[..i].into()),
                line: number,
                column,
                keyword: k,
                message: message.into(),
            });
        }
    }
    None
}

/// `parseStderr`'s `continuation`: the lines of a message at `column`
/// without its excerpt, the first line as wide as the column's indent
/// followed by one that starts with that indent.
fn continuation(mut lines: Vec<String>, column: usize) -> Vec<String> {
    let indent = column.saturating_sub(1);
    let at = (0..lines.len().saturating_sub(1)).find(|&i| {
        let rest: Vec<char> = lines[i + 1].chars().collect();
        width(&lines[i]) == indent
            && rest.len() >= indent
            && trim(&rest[..indent].iter().collect::<String>()).is_empty()
    });
    if let Some(i) = at {
        lines.drain(i..i + 2);
    }
    lines
}

/// `displayWidth`: code points, a tab to the next multiple of 8.
fn width(text: &str) -> usize {
    text.chars()
        .fold(0, |w, c| if c == '\t' { w + 8 - w % 8 } else { w + 1 })
}

/// JavaScript's `trim`: its white space is Unicode's but U+0085, and U+FEFF.
fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// A JSON string literal.
fn push_json(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
