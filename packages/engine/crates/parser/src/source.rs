//! Sources (02 §2.5, §5.1): a file's bytes and its line table, and spans
//! into them. Line, character and column are computed from the line table
//! when a message is printed, as `source-file.cc` computes them, never
//! stored. And where `\include` looks for a file ([`include_candidates`]).

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use lily_engine_music::Input;

/// A byte range of one source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub file: u16,
    pub start: u32,
    pub end: u32,
}

/// Where a byte is: `source-file.cc`'s `get_counts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    /// From 1.
    pub line: usize,
    /// Characters before it on its line, from 0: bytes that are no UTF-8 continuation byte.
    pub char: usize,
    /// Its column, from 0: characters, with a tab advancing to the next multiple of 8.
    pub column: usize,
    /// Bytes before it on its line.
    pub offset: usize,
}

/// One file's text and the offsets of its newlines.
pub struct Source {
    /// The name it was given, which the [`Input`]s made of it are matched by.
    given: Rc<str>,
    /// What locations print, shared with the [`Input`]s made of it: the name
    /// given, or `\sourcefilename`'s (`name_`, P12).
    name: RefCell<Rc<str>>,
    text: Text,
    newlines: Vec<u32>,
    /// `line_offset_`: lines before its first, as a template's text is
    /// numbered from the line of its `#{` ([`Source::with_line`]), and as
    /// `\sourcefileline` sets it (P12).
    first_line: Cell<isize>,
    /// The `first_line` it was made with.
    given_line: isize,
    /// Its text, for the [`Input`]s made of it, where no parse keeps it: a
    /// clone's given no location ([`Source::detach`]).
    detached: Option<Rc<str>>,
}

/// A file's text, checked as UTF-8 once.
enum Text {
    /// All of it UTF-8, as most files are.
    Utf8(String),
    /// Its bytes, and a copy of the longest prefix that is UTF-8.
    Bytes(Vec<u8>, String),
}

impl Source {
    /// `name` is what locations print: the file as LilyPond was given it.
    pub fn new(name: &str, text: Vec<u8>) -> Source {
        let newlines = (0..text.len() as u32)
            .filter(|&i| text[i as usize] == b'\n')
            .collect();
        let text = match String::from_utf8(text) {
            Ok(text) => Text::Utf8(text),
            Err(e) => {
                let valid = e.utf8_error().valid_up_to();
                let bytes = e.into_bytes();
                let prefix = String::from_utf8_lossy(&bytes[..valid]).into_owned();
                Text::Bytes(bytes, prefix)
            }
        };
        let name: Rc<str> = Rc::from(name);
        Source {
            given: name.clone(),
            name: RefCell::new(name),
            text,
            newlines,
            first_line: Cell::new(0),
            given_line: 0,
            detached: None,
        }
    }

    /// [`Source::new`], its lines numbered from `line` + 1, as
    /// `ly:parse-string-expression` numbers a template's after
    /// `set_line (0, line)`.
    pub fn with_line(name: &str, text: Vec<u8>, line: usize) -> Source {
        let line = line as isize;
        Source {
            first_line: Cell::new(line),
            given_line: line,
            ..Source::new(name, text)
        }
    }

    /// Its [`Input`]s carry its text, as no parse keeps it (a clone's given
    /// no location, D92).
    pub fn detach(&mut self) {
        self.detached = Some(Rc::from(self.scheme_text()));
    }

    /// The name it was given.
    pub fn name(&self) -> &str {
        &self.given
    }

    /// What its locations print: its name, or `\sourcefilename`'s.
    pub fn shown_name(&self) -> Rc<str> {
        self.name.borrow().clone()
    }

    /// Whether `name`, an [`Input`]'s, is this source's.
    pub fn is_named(&self, name: &str) -> bool {
        *self.given == *name || **self.name.borrow() == *name
    }

    /// `\sourcefilename`: what its locations print from now on.
    pub fn rename(&self, name: &str) {
        *self.name.borrow_mut() = Rc::from(name);
    }

    /// `\sourcefileline` (`set_line`): byte `pos` numbered `line`, the
    /// others after it.
    pub fn set_line(&self, pos: u32, line: isize) {
        let now = self.line(pos) as isize;
        self.first_line.set(self.first_line.get() + line - now);
    }

    /// Its name and numbering as they were made, for a parse that starts
    /// again on it.
    pub fn reset(&self) {
        *self.name.borrow_mut() = self.given.clone();
        self.first_line.set(self.given_line);
    }

    /// Whether `\sourcefilename` or `\sourcefileline` changed it.
    pub fn changed(&self) -> bool {
        *self.given != **self.name.borrow() || self.first_line.get() != self.given_line
    }

    pub fn text(&self) -> &[u8] {
        match &self.text {
            Text::Utf8(text) => text.as_bytes(),
            Text::Bytes(bytes, _) => bytes,
        }
    }

    /// The text as Scheme's reader reads it: up to the first byte that is no
    /// UTF-8, where the reader meets the end of its input.
    pub fn scheme_text(&self) -> &str {
        match &self.text {
            Text::Utf8(text) | Text::Bytes(_, text) => text,
        }
    }

    /// The line of byte `pos`, from 1 (`get_line`).
    pub fn line(&self, pos: u32) -> usize {
        let line = self.newlines.partition_point(|&n| n < pos) as isize + 1 + self.first_line.get();
        line.max(0) as usize
    }

    /// The line around byte `pos` without its newline (`line_slice`); at the
    /// end of the text, the last line.
    fn line_bounds(&self, pos: u32) -> (usize, usize) {
        let text = self.text();
        let mut pos = pos as usize;
        if pos == text.len() && pos > 0 {
            pos -= 1;
        }
        let start = text[..pos]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |n| n + 1);
        let end = text[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(text.len(), |n| pos + n);
        (start, end)
    }

    /// Line, character and column of byte `pos` (`get_counts`). Continuation
    /// bytes are skipped whether or not the text is valid UTF-8, as there.
    pub fn counts(&self, pos: u32) -> Counts {
        let (start, _) = self.line_bounds(pos);
        let before = &self.text()[start..pos as usize];
        let mut counts = Counts {
            line: self.line(pos),
            char: 0,
            column: 0,
            offset: before.len(),
        };
        for &b in before.iter().filter(|&&b| b & 0xc0 != 0x80) {
            counts.column = if b == b'\t' {
                (counts.column / 8 + 1) * 8
            } else {
                counts.column + 1
            };
            counts.char += 1;
        }
        counts
    }

    /// LilyPond's `Input` of bytes `start` to `end`, this source being the
    /// parse's `file`.
    pub fn input(&self, file: u16, start: u32, end: u32) -> Input {
        let c = self.counts(start);
        let counts = [c.line, c.char, c.column].map(|n| n as u32);
        let mut input = Input::new(file, start, end, self.shown_name(), counts);
        input.text = self.detached.clone();
        input
    }

    /// `name:line:column`, the column from 1 (`file_line_column_string`).
    pub fn location(&self, pos: u32) -> String {
        let c = self.counts(pos);
        alloc::format!("{}:{}:{}", self.name.borrow(), c.line, c.column + 1)
    }

    /// The excerpt below a message (`quote_input`): the line up to `pos`,
    /// then the rest of it on a line of its own, indented to `pos`'s column.
    /// The bytes are the source's, valid UTF-8 or not.
    pub fn quote(&self, pos: u32) -> Vec<u8> {
        let (start, end) = self.line_bounds(pos);
        let c = self.counts(pos);
        let line = &self.text()[start..end];
        let cut = c.offset.min(line.len());
        let mut out = Vec::with_capacity(line.len() + c.column + 1);
        out.extend_from_slice(&line[..cut]);
        out.push(b'\n');
        out.resize(out.len() + c.column, b' ');
        out.extend_from_slice(&line[cut..]);
        out
    }
}

/// The sources of one parse, by [`Span::file`].
#[derive(Default)]
pub struct Sources {
    files: Vec<Source>,
    /// The number of the first: a template's are numbered apart from the
    /// files of the parse that runs it (`embedded.rs`).
    first: u16,
}

impl Sources {
    /// Sources numbered from `first`.
    pub(crate) fn from(first: u16) -> Sources {
        Sources {
            files: Vec::new(),
            first,
        }
    }

    pub fn add(&mut self, source: Source) -> u16 {
        self.files.push(source);
        self.first + (self.files.len() - 1) as u16
    }

    pub fn get(&self, file: u16) -> &Source {
        &self.files[(file - self.first) as usize]
    }

    /// Source `file`, if this parse has it: music another parse made (the
    /// init's) has origins in that parse's sources (D69).
    pub fn source(&self, file: u16) -> Option<&Source> {
        self.files.get(file.checked_sub(self.first)? as usize)
    }
}

/// How LilyPond's own data directory is named to the host, and in the
/// locations of its files: as the error corpus writes it.
pub const DATADIR: &str = "<datadir>";

/// LilyPond's search path after the score's directory (`main.cc`'s
/// `global_path`), as `cannot find file` prints it. Of its directories only
/// `ly/` holds files a score includes, and only it is searched.
const SEARCH_PATH: [&str; 5] = ["ly", "ps", "scm", "fonts/otf/", "fonts/svg/"];

/// The directory of the file `name`, `""` for none.
fn dir_name(name: &str) -> &str {
    match name.rfind('/') {
        Some(0) => "/",
        Some(i) => &name[..i],
        None => "",
    }
}

/// `name` in `dir`, which may be `""`.
fn join(dir: &str, name: &str) -> String {
    match dir {
        "" => String::from(name),
        dir if dir.ends_with('/') => alloc::format!("{dir}{name}"),
        dir => alloc::format!("{dir}/{name}"),
    }
}

/// Where `\include "name"` read in the file `current` looks for it, in
/// order, as `Sources::get_file` looks with `relative-includes`: beside
/// `current`, then along the search path, which is the main file's
/// directory and then LilyPond's `ly/` ([`DATADIR`]). An absolute name is
/// only itself. Each name is what the host is asked for and what locations
/// in the file print.
pub fn include_candidates(name: &str, current: &str, main: &str) -> Vec<String> {
    if name.starts_with('/') {
        return alloc::vec![String::from(name)];
    }
    let mut candidates: Vec<String> = Vec::with_capacity(3);
    let here = dir_name(current);
    if !here.is_empty() {
        candidates.push(join(here, name));
    }
    for candidate in [
        join(dir_name(main), name),
        alloc::format!("{DATADIR}/ly/{name}"),
    ] {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

/// LilyPond's message for an `\include "name"` in `current` that no
/// directory has: the directory of `current`, then the search path, a
/// directory of none being `.`.
pub fn cannot_find(name: &str, current: &str, main: &str) -> String {
    let dir = |file| match dir_name(file) {
        "" => ".",
        dir => dir,
    };
    let mut path = alloc::format!("{}:{}", dir(current), dir(main));
    for sub in SEARCH_PATH {
        path.push_str(&alloc::format!(":{DATADIR}/{sub}"));
    }
    alloc::format!("cannot find file: `{name}'\n(search path: `{path}:')")
}
