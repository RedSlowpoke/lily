//! LilyPond's `Input` (`input.cc`): where in the sources something was
//! written. The parser makes one for `(*location*)` while a form runs, and
//! music holds one as its `origin` (N2, 04 §6.1). It holds the span of
//! chapter 02 §5.1, a file's number in its parse and its bytes, with the
//! name of the file, as the host gave it to the core (`lily_put`), and the
//! counts of its first byte, so that it prints and answers
//! `ly:input-file-line-char-column` after its parse is gone (K2). A parse
//! takes its span only when that parse has the file of that name.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Value};

use crate::R;

/// An input location: bytes `start` to `end` of the parser's source `file`.
pub struct Input {
    pub file: u16,
    pub start: u32,
    pub end: u32,
    /// The file's name, shared by the inputs of a parse.
    pub name: Rc<str>,
    /// `start`'s line from 1, and its character and column from 0
    /// (`get_counts`).
    pub line: u32,
    pub char: u32,
    pub column: u32,
    /// The text of a source no parse keeps, a clone's given no location
    /// (D92, OPEN F), so that a message at it later prints its excerpt as
    /// LilyPond's does; none in an image.
    pub text: Option<Rc<str>>,
}

impl Input {
    /// `counts` are `start`'s line, character and column.
    pub fn new(file: u16, start: u32, end: u32, name: Rc<str>, counts: [u32; 3]) -> Input {
        let [line, char, column] = counts;
        Input {
            file,
            start,
            end,
            name,
            line,
            char,
            column,
            text: None,
        }
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Input> {
        let file = u16::try_from(r.u32()?).ok()?;
        Some(Input {
            file,
            start: r.u32()?,
            end: r.u32()?,
            name: Rc::from(r.str()?),
            line: r.u32()?,
            char: r.u32()?,
            column: r.u32()?,
            text: None,
        })
    }
}

impl Foreign for Input {
    fn type_name(&self) -> &'static str {
        "Input"
    }

    /// `#<location file:line:column>`, the column from 1.
    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str(&alloc::format!(
            "#<location {}:{}:{}>",
            self.name,
            self.line,
            self.column + 1
        ));
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.u32(self.file.into());
        out.u32(self.start);
        out.u32(self.end);
        out.str(&self.name);
        out.u32(self.line);
        out.u32(self.char);
        out.u32(self.column);
        true
    }
}

/// `(ly:input-location? x)`.
pub(crate) fn is_input(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Input>(cx, args[0])
}

/// `(ly:input-file-line-char-column sip)`: the file's name as the host gave
/// it, the line, the character and the column, these two from 0.
pub(crate) fn file_line_char_column(cx: &mut Cx, args: &[Value]) -> R {
    let Some(input) = cx.heap().foreign::<Input>(args[0]) else {
        return Err(cx.wrong_type("ly:input-file-line-char-column", 1, args[0], "Input"));
    };
    let (name, counts) = (
        String::from(&*input.name),
        [input.line, input.char, input.column],
    );
    let heap = cx.heap_mut();
    let mut items = alloc::vec![heap.string(name)];
    items.extend(counts.map(|n| heap.integer(n.into())));
    Ok(heap.list(&items))
}

/// A file's text before and after an edit (N8's resume, MS6 step 7, D244):
/// one span replaced, `[from, old_to)` of the old text by `[from, new_to)`
/// of the new. A place in the old text has its place in the new: the same
/// before the edit, moved with the bytes after it, its counts the new
/// text's, as the parser makes them (`Source::counts`). Inside the span, the
/// lines an edit in several places left as they were (a comment added above
/// and a note changed below) move with their bytes too (OPEN B31, D268).
pub struct Moved {
    name: Rc<str>,
    /// The file's name alone, as the trace's links write it, and as the
    /// playback map's encode it.
    base: String,
    encoded: String,
    old: Lines,
    new: Lines,
    from: u32,
    old_to: u32,
    new_to: u32,
    /// The span's blocks of lines left as they were, in order: each one's
    /// start in the old text, in the new, and its length; and whether they
    /// were all found ([`SAME_LINES`]).
    same: Vec<(u32, u32, u32)>,
    exact: bool,
}

/// How many lines of a span, changed, inserted or removed, the edit's blocks
/// left as they were are looked for among ([`same_lines`]): an edit of more
/// is not [`Moved::exact`].
const SAME_LINES: usize = 256;

/// The text `t` from `from` to `to` in lines, each with its start, the last
/// up to `to` with or without its newline.
fn lines_of(t: &[u8], from: usize, to: usize) -> Vec<(usize, &[u8])> {
    let mut out = Vec::new();
    let mut at = from;
    while at < to {
        let end = t[at..to]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(to, |i| at + i + 1);
        out.push((at, &t[at..end]));
        at = end;
    }
    out
}

/// The pairs of lines of `a` and `b` that a shortest edit of one into the
/// other leaves as they were (Myers' greedy diff), in order; none if that
/// edit changes more than `most` lines.
fn same_lines(a: &[&[u8]], b: &[&[u8]], most: usize) -> Option<Vec<(usize, usize)>> {
    // A side of one line or none: what the other side has of it, at most
    // one line (a selection replaced by a paste).
    match (a, b) {
        ([], _) | (_, []) => return Some(Vec::new()),
        ([one], _) => {
            return Some(
                b.iter()
                    .position(|l| l == one)
                    .map(|j| (0, j))
                    .into_iter()
                    .collect(),
            );
        }
        (_, [one]) => {
            return Some(
                a.iter()
                    .position(|l| l == one)
                    .map(|i| (i, 0))
                    .into_iter()
                    .collect(),
            );
        }
        _ => {}
    }
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m).min(most as isize);
    let off = max + 1;
    let mut v = alloc::vec![0isize; 2 * off as usize + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut found = None;
    'search: for d in 0..=max {
        // What the round before reached on the diagonals this one starts
        // from, `-d..=d`.
        trace.push(v[(off - d) as usize..=(off + d) as usize].to_vec());
        let mut k = -d;
        while k <= d {
            let i = (k + off) as usize;
            let mut x = if k == -d || (k != d && v[i - 1] < v[i + 1]) {
                v[i + 1]
            } else {
                v[i - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                (x, y) = (x + 1, y + 1);
            }
            v[i] = x;
            if x >= n && y >= m {
                found = Some(d);
                break 'search;
            }
            k += 2;
        }
    }
    let d = found?;
    let mut pairs = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (1..=d).rev() {
        let v = &trace[d as usize];
        let at = |k: isize| v[(k + d) as usize];
        let k = x - y;
        let prev = if k == -d || (k != d && at(k - 1) < at(k + 1)) {
            k + 1
        } else {
            k - 1
        };
        let px = at(prev);
        let py = px - prev;
        // The step from the last diagonal, a line inserted or removed, then
        // this one's lines left as they were.
        let (sx, sy) = if prev == k + 1 {
            (px, py + 1)
        } else {
            (px + 1, py)
        };
        while x > sx && y > sy {
            (x, y) = (x - 1, y - 1);
            pairs.push((x as usize, y as usize));
        }
        (x, y) = (px, py);
    }
    while x > 0 && y > 0 {
        (x, y) = (x - 1, y - 1);
        pairs.push((x as usize, y as usize));
    }
    pairs.reverse();
    Some(pairs)
}

/// How many bytes a change of lines may have on either side for its bytes
/// left as they were to be looked for ([`same_bytes`]).
const SAME_BYTES: usize = 4096;

/// The blocks of bytes left as they were between `old[a.0..a.1]` and
/// `new[b.0..b.1]`, a change of lines, pushed to `blocks` in order. A change
/// of more than [`SAME_BYTES`] on a side, or of more than [`SAME_LINES`] bytes
/// changed, is one replaced whole, as [`Moved`] had it: what is kept lies
/// between edits, not inside a replacement (a selection pasted over).
fn same_bytes(
    old: &[u8],
    new: &[u8],
    a: (u32, u32),
    b: (u32, u32),
    blocks: &mut Vec<(u32, u32, u32)>,
) {
    let (x, y) = (
        &old[a.0 as usize..a.1 as usize],
        &new[b.0 as usize..b.1 as usize],
    );
    if x.is_empty() || y.is_empty() || x.len() > SAME_BYTES || y.len() > SAME_BYTES {
        return;
    }
    let (xs, ys): (Vec<&[u8]>, Vec<&[u8]>) = (x.chunks(1).collect(), y.chunks(1).collect());
    let Some(pairs) = same_lines(&xs, &ys, SAME_LINES) else {
        return;
    };
    let start = blocks.len();
    for (i, j) in pairs {
        let (i, j) = (a.0 + i as u32, b.0 + j as u32);
        match blocks[start..].last_mut() {
            Some(last) if last.0 + last.2 == i && last.1 + last.2 == j => last.2 += 1,
            _ => blocks.push((i, j, 1)),
        }
    }
}

/// A text with the offsets of its newlines.
struct Lines {
    text: Vec<u8>,
    newlines: Vec<u32>,
}

impl Lines {
    fn new(text: &[u8]) -> Lines {
        let newlines = (0..text.len() as u32)
            .filter(|&i| text[i as usize] == b'\n')
            .collect();
        Lines {
            text: text.to_vec(),
            newlines,
        }
    }

    /// The offset of `line`'s (from 1) character `char` (from 0).
    fn offset(&self, line: u32, char: u32) -> Option<u32> {
        let start = match line.checked_sub(2) {
            None => 0,
            Some(n) => *self.newlines.get(n as usize)? + 1,
        };
        let mut chars = 0;
        let mut at = start as usize;
        while chars < char {
            let b = *self.text.get(at)?;
            if b == b'\n' {
                return None;
            }
            at += 1;
            if self.text.get(at).is_none_or(|&b| b & 0xc0 != 0x80) {
                chars += 1;
            }
        }
        Some(at as u32)
    }

    /// The line, character and column of byte `pos` (`Source::counts`).
    fn counts(&self, pos: u32) -> [u32; 3] {
        let line = self.newlines.partition_point(|&n| n < pos);
        let start = line.checked_sub(1).map_or(0, |l| self.newlines[l] + 1) as usize;
        let (mut char, mut column) = (0, 0);
        let end = (pos as usize).min(self.text.len());
        for &b in self.text[start.min(end)..end]
            .iter()
            .filter(|&&b| b & 0xc0 != 0x80)
        {
            column = if b == b'\t' {
                (column / 8 + 1) * 8
            } else {
                column + 1
            };
            char += 1;
        }
        [line as u32 + 1, char, column]
    }
}

impl Moved {
    /// The edit between `old` and `new`, the file `name`'s texts; `None`
    /// for the same text.
    pub fn new(name: &str, old: &[u8], new: &[u8]) -> Option<Moved> {
        if old == new {
            return None;
        }
        let from = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        let tail = old[from..]
            .iter()
            .rev()
            .zip(new[from..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let base = String::from(name.rsplit('/').next().unwrap_or_default());
        let encoded = lily_engine_scheme::percent_encode(&base);
        let (old_to, new_to) = (old.len() - tail, new.len() - tail);
        let (a, b) = (lines_of(old, from, old_to), lines_of(new, from, new_to));
        let (ta, tb): (Vec<&[u8]>, Vec<&[u8]>) = (
            a.iter().map(|l| l.1).collect(),
            b.iter().map(|l| l.1).collect(),
        );
        let pairs = same_lines(&ta, &tb, SAME_LINES);
        let mut same: Vec<(u32, u32, u32)> = Vec::new();
        for &(i, j) in pairs.as_deref().unwrap_or_default() {
            let ((x, line), (y, _)) = (a[i], b[j]);
            match same.last_mut() {
                Some(last) if last.0 + last.2 == x as u32 && last.1 + last.2 == y as u32 => {
                    last.2 += line.len() as u32;
                }
                _ => same.push((x as u32, y as u32, line.len() as u32)),
            }
        }
        // Inside each change of lines, its bytes: two edits on one line leave
        // what is between them as it was too.
        let exact = pairs.is_some();
        if exact {
            let mut blocks = Vec::with_capacity(same.len());
            let (mut x, mut y) = (from as u32, from as u32);
            for block in same
                .iter()
                .copied()
                .chain([(old_to as u32, new_to as u32, 0)])
            {
                same_bytes(old, new, (x, block.0), (y, block.1), &mut blocks);
                if block.2 > 0 {
                    blocks.push(block);
                }
                (x, y) = (block.0 + block.2, block.1 + block.2);
            }
            same = blocks;
        }
        Some(Moved {
            name: Rc::from(name),
            base,
            encoded,
            old: Lines::new(old),
            new: Lines::new(new),
            from: from as u32,
            old_to: old_to as u32,
            new_to: new_to as u32,
            same,
            exact,
        })
    }

    /// Whether each place the edit left as it was is found in the new text:
    /// an edit of more than [`SAME_LINES`] lines in several places is not,
    /// and a resume after it would hold places of the music between them
    /// where they were.
    pub fn exact(&self) -> bool {
        self.exact
    }

    /// Where the old text's byte `p` of the span is in it: the block of
    /// lines left as they were it is in, or the change it is in, each by its
    /// start in the old and the new text, and the end of the change in the
    /// new.
    fn within(&self, p: u32) -> (u32, u32, Option<u32>) {
        let at = self.same.partition_point(|&(x, _, _)| x <= p);
        match at.checked_sub(1).map(|i| self.same[i]) {
            Some((x, y, len)) if p < x + len => (x, y, None),
            before => {
                let (x, y) =
                    before.map_or((self.from, self.from), |(x, y, len)| (x + len, y + len));
                let end = self.same.get(at).map_or(self.new_to, |&(_, y, _)| y);
                (x, y, Some(end))
            }
        }
    }

    /// The new place of the old text's byte `p`; one in a change, as far into
    /// what replaced it.
    fn offset(&self, p: u32) -> u32 {
        if p < self.from {
            p
        } else if p >= self.old_to {
            p - self.old_to + self.new_to
        } else {
            match self.within(p) {
                (x, y, None) => y + (p - x),
                (x, y, Some(end)) => (y + (p - x)).min(end),
            }
        }
    }

    /// The length of the block left as it was that starts at `x`.
    fn same_len(&self, x: u32) -> u32 {
        self.same.iter().find(|s| s.0 == x).map_or(0, |s| s.2)
    }

    /// `input`'s place in the text, which [`Moved::input`] changes.
    pub fn where_is(input: &Input) -> [u32; 5] {
        [input.start, input.end, input.line, input.char, input.column]
    }

    /// `input` put back at `place`, a [`Moved::where_is`].
    pub fn put_back(input: &mut Input, place: [u32; 5]) {
        [input.start, input.end, input.line, input.char, input.column] = place;
    }

    /// Whether the edit moves `input`, a place in the old text.
    pub fn moves(&self, input: &Input) -> bool {
        *input.name == *self.name && !(input.end <= self.from && input.start < self.from)
    }

    /// Moves `input`, a place in the old text, to the new.
    pub fn input(&self, input: &mut Input) {
        if !self.moves(input) {
            return;
        }
        input.start = self.offset(input.start);
        input.end = self.offset(input.end);
        [input.line, input.char, input.column] = self.new.counts(input.start);
    }

    /// The old place at `line` and character `char`, its new line,
    /// character and column, and the old column.
    fn place(&self, line: u32, char: u32) -> Option<([u32; 3], u32)> {
        let p = self.old.offset(line, char)?;
        Some((self.new.counts(self.offset(p)), self.old.counts(p)[2]))
    }

    /// `text` with each of its links into the file (`name:line:char:column`,
    /// the trace's and the playback map's, the column from 0 or 1) moved to
    /// the new text.
    pub fn links(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some((i, n)) = [&self.base, &self.encoded]
            .iter()
            .filter_map(|n| rest.find(&format!("{n}:")).map(|i| (i, n.len() + 1)))
            .min()
        {
            out.push_str(&rest[..i + n]);
            rest = &rest[i + n..];
            // Three numbers, each but the last ended by a colon.
            let mut numbers = [0u32; 3];
            let mut at = 0;
            let mut found = true;
            for (k, number) in numbers.iter_mut().enumerate() {
                let d = rest[at..].bytes().take_while(u8::is_ascii_digit).count();
                let ok = d > 0 && (k == 2 || rest[at + d..].starts_with(':'));
                match rest[at..at + d].parse() {
                    Ok(v) if ok => *number = v,
                    _ => {
                        found = false;
                        break;
                    }
                }
                at += d + usize::from(k < 2);
            }
            let [line, char, column] = numbers;
            let Some(([l, c, new_column], old_column)) =
                found.then(|| self.place(line, char)).flatten()
            else {
                continue;
            };
            out.push_str(&format!(
                "{l}:{c}:{}",
                new_column + column.saturating_sub(old_column)
            ));
            rest = &rest[at..];
        }
        out.push_str(rest);
        out
    }

    /// `said`, messages written to the error port (`name:line:column: …`,
    /// each with its line quoted), with the lines of those after the edit
    /// moved; `None` where one is on a line the edit changed, whose quote a
    /// resume would write again as it was.
    pub fn said(&self, said: &str) -> Option<String> {
        let first = self.old.counts(self.from)[0];
        let last = self.old.counts(self.old_to)[0];
        let moved = i64::from(self.new.counts(self.new_to)[0]) - i64::from(last);
        // A line of the span is changed unless a block left as it was holds it
        // whole, and it moves with the block.
        let kept = |line: u32| {
            let start = self.old.offset(line, 0)?;
            let end = self
                .old
                .newlines
                .get(line as usize - 1)
                .map_or(self.old.text.len() as u32, |&n| n + 1);
            match self.within(start) {
                (x, y, None) if end <= x + self.same_len(x) => {
                    Some(self.new.counts(y + (start - x))[0])
                }
                _ => None,
            }
        };
        let mut out = String::with_capacity(said.len());
        let mut rest = said;
        let key = format!("{}:", self.base);
        while let Some(i) = rest.find(&key) {
            out.push_str(&rest[..i + key.len()]);
            rest = &rest[i + key.len()..];
            let d = rest.bytes().take_while(u8::is_ascii_digit).count();
            let Ok(line) = rest[..d].parse::<u32>() else {
                continue;
            };
            let line = if (first..=last).contains(&line) {
                kept(line)?
            } else if line > last {
                (i64::from(line) + moved) as u32
            } else {
                line
            };
            out.push_str(&format!("{line}"));
            rest = &rest[d..];
        }
        out.push_str(rest);
        Some(out)
    }
}
