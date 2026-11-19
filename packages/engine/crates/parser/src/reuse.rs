//! 02 §5.7's re-parse (MS6 step 3): a file parsed again after an edit is
//! parsed only from the first toplevel item the edit can change; the items
//! before it are done again from what undoing them kept, not parsed, and
//! so are the items after it that nothing the edit changed reaches.
//!
//! A main parse that keeps a [`Reuse`] runs its toplevel items in items of
//! the interpreter's undo log (`Vm::begin_item`), one a *segment*: the
//! items between two *boundaries*, places at the top of the toplevel loop
//! where the parse could start again with the lexer alone. A boundary is
//! one when the main file is being read with nothing pushed, included or
//! waiting, no mode or recovery open, nothing Scheme wrote unread, and the
//! lookahead, if one was lexed, said nothing, is no `$` form (whose value
//! its lexing made) and would be lexed alike now. There the parser's state
//! is recorded (02 §3.3's entry state: the default duration and tremolo,
//! the pitch names' language, what `\version` left, whether the file
//! failed; and what was said, the items made and the strings included
//! before it), with the last byte the parse had examined: the lexer's
//! position, lookahead included. Each segment keeps what it read, with the
//! versions, and the slots it wrote (R10).
//!
//! When the compile ends, [`Reuse::undo`] undoes the segments the latest
//! first and keeps what each changed as it was at its end
//! (`Vm::undo_keeping`), so the next compile starts where a fresh engine's
//! does, as since MS6 step 2. [`parse_again`] then compares the text with
//! the last one's: the segments that end at a boundary examined before the
//! first byte that differs are done again (`Vm::redo`), each in an item of
//! its own, and the parse starts at the boundary after them, in the same
//! module, with the state recorded there (02 §5.7's steps 1 and 2).
//!
//! **After the edit** (02 §5.7's step 4): at a boundary of the parse that
//! is one of the last parse's, past the edit's line, an edit that moved no
//! byte or line after it, the parser's state there the same, the old
//! segments from it on are done again, not parsed, as long as each read
//! every variable at the version it read it at; then the parse goes on at
//! the boundary after them. Only while the segments parsed again since the
//! start changed no older object, wrote the variables their old ones
//! wrote, and only the file's: what an old segment read of an object or of
//! a variable it did not name is then as it was. Its messages, items and
//! included strings are the old parse's. A taken-over segment's music keeps
//! its places, so an edit that moves the text after it parses it again.
//!
//! A file of another name, or a file one of the segments included that the
//! host now has otherwise, give a parse from the start in a new module.
//!
//! **A braced block** (02 §5.7's step 3): inside an item parsed again, a
//! `{ … }` or `<< … >>` of the last parse at the same place, before the
//! edit or past it as a segment taken over is, entered in the same state
//! (the lexer's modes, the pitch names, the default duration and
//! tremolo), is taken as it was, its music's syntax, not parsed: the lexer
//! goes on after its close, with the default duration it left. Only a
//! block whose parse kept no value, read no form, lexed no string, said
//! nothing and changed nothing is recorded: it looked up nothing and ran
//! no Scheme, so it depends on nothing else, and its syntax holds no index
//! of the parse it was made in.

use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Hosted, Mark, Redo, Slot, Value, Vm};

use crate::diag::{DiagMark, Diagnostics};
use crate::lexer::{Mode, Tok, Token, VersionSeen};
use crate::pitch::Pitch;
use crate::source::Source;
use crate::syntax::{Duration, Item, Music};
use crate::{Env, Include, NoteNames, Parse, Parser};

/// What a file's last parse leaves for the next (02 §5.7): its boundaries
/// and segments, and what the parse had said and made, kept between
/// compiles.
#[derive(Default)]
pub struct Reuse {
    /// The file's name and text.
    name: String,
    text: Vec<u8>,
    /// The file's module, kept alive by the interpreter between compiles.
    module: Value,
    /// The boundaries, the first the start of the file.
    boundaries: Vec<Boundary>,
    /// One a boundary: the segment that starts there, to the next boundary
    /// or the end.
    segments: Vec<Segment>,
    /// What the parse had said, made and included at its end.
    diagnostics: Diagnostics,
    items: Vec<Item>,
    included: Vec<&'static Source>,
    /// Whether the segments' items are in the undo log, not undone.
    live: bool,
    /// Its braced blocks that can be taken (§5.7's step 3), by place, and
    /// how many it took from the parse before it.
    blocks: Vec<Block>,
    blocks_taken_last: usize,
    /// How many segments the last parse did again, and how many it parsed.
    reused: (usize, usize),
}

/// A braced block a parse made (§5.7's step 3): its place, the state it
/// was entered in, where the lexer and the parser were at its close, and
/// its music.
#[derive(Clone, Debug)]
struct Block {
    start: u32,
    /// The lexer's position after its close: what it examined.
    end: u32,
    entry: BlockState,
    last: (usize, usize),
    last_end: u32,
    duration: Duration,
    tremolo: u32,
    music: Music,
}

/// The state a braced block is parsed in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockState {
    modes: Vec<Mode>,
    base: Mode,
    language: String,
    duration: Duration,
    tremolo: u32,
}

/// What a braced block is entered with: its place and state, and how much
/// was kept, read, lexed, said, logged and included before it.
pub(crate) struct BlockEntry {
    start: u32,
    state: BlockState,
    counts: [usize; 8],
}

/// A place a parse can start again, and the parser's state there.
#[derive(Clone, Debug)]
pub(crate) struct Boundary {
    /// Where the lexer starts: the lookahead's first byte, or its position.
    at: u32,
    /// The last byte the parse before it examined: the lexer's position.
    examined: u32,
    /// flex's `lexloc_` and the end of the last token taken.
    last: (usize, usize),
    last_end: u32,
    duration: Duration,
    tremolo: u32,
    /// The pitch names' language.
    language: String,
    version: VersionSeen,
    error_level: bool,
    /// What was said, made and included before it.
    said: DiagMark,
    items: usize,
    included: usize,
}

impl Boundary {
    /// Whether a parse is in the same state at both, what was said, made
    /// and included before them apart.
    fn same_state(&self, other: &Boundary) -> bool {
        (self.at, self.examined, self.last, self.last_end)
            == (other.at, other.examined, other.last, other.last_end)
            && (self.duration, self.tremolo) == (other.duration, other.tremolo)
            && self.language == other.language
            && self.version == other.version
            && self.error_level == other.error_level
            && self.included == other.included
    }
}

/// A segment: its item in the compile running, or what undoing it kept;
/// what it read, each with the version it read, and what it wrote.
#[derive(Clone, Debug)]
struct Segment {
    mark: Mark,
    redo: Option<Redo>,
    reads: Vec<(Slot, u32)>,
    writes: Vec<Slot>,
}

/// The segments of a parse that keeps a [`Reuse`], as it runs.
pub(crate) struct Segments {
    boundaries: Vec<Boundary>,
    segments: Vec<Segment>,
    /// The current segment's item, from its boundary on.
    mark: Option<Mark>,
    /// Since the last boundary, whether an item was parsed.
    moved: bool,
    /// The last parse, and where the parse started at its boundaries.
    last: Reuse,
    restart: Option<usize>,
    /// From what byte old segments may be taken over (§5.7's step 4), if
    /// the edit moved nothing after it; and the heap's undo log's length
    /// after the segments done again: what the log has after it, the
    /// segments parsed again logged.
    take_from: Option<u32>,
    heap: usize,
    /// How many old segments were taken over, and their places.
    taken: usize,
    taken_at: Vec<(u32, u32)>,
    /// The first byte the edit changed, if the parse is the same file's.
    differs: Option<u32>,
    /// The blocks this parse made or took, and how many it took.
    blocks: Vec<Block>,
    blocks_taken: usize,
    /// Whether the parse may stop at a boundary for the step's time, its
    /// compile's own returning (H3, D267), and whether it stopped at its last:
    /// [`parse_on`] goes on from there.
    returns: bool,
    paused: bool,
}

/// A parse in steps (H3, OPEN B23, D267) stopped at a boundary between its
/// segments when the step's time was spent: its segments live in the undo
/// log, the old ones it may still take over, and what it had said, made and
/// included there. [`parse_on`] goes on from that boundary in the next step,
/// as a parse started again there would, in the same module; [`Parsing::stop`]
/// leaves it to the next parse as a parse that ended there.
pub struct Parsing {
    name: String,
    text: Vec<u8>,
    module: Value,
    segments: Segments,
    diagnostics: Diagnostics,
    items: Vec<Item>,
    included: Vec<&'static Source>,
    /// Where it stopped inside a toplevel `\score`'s `scorify-music` (D272):
    /// the item under way, its segment's item begun.
    in_score: Option<crate::recover::InScore>,
}

/// What a step of a parse made: the parse, or a parse to go on with.
pub enum ParseStep {
    Done(alloc::boxed::Box<Parse>),
    Paused(alloc::boxed::Box<Parsing>),
}

/// What a parse with segments ends with: the parse and its segments, or the
/// parse stopped at a boundary.
pub(crate) enum Ending {
    Ended(alloc::boxed::Box<(Parse, Option<Ended>)>),
    Paused(alloc::boxed::Box<Paused>),
}

/// A parse stopped at its last boundary, or in a toplevel `\score`'s
/// `scorify-music` (D272): its segments, what it had said, made and included
/// there, and the score under way.
pub(crate) type Paused = (
    Segments,
    Diagnostics,
    Vec<Item>,
    Vec<&'static Source>,
    Option<crate::recover::InScore>,
);

/// What a stopped parse goes on with: [`Paused`] but its segments.
pub(crate) type Resumed = (
    Diagnostics,
    Vec<Item>,
    Vec<&'static Source>,
    Option<crate::recover::InScore>,
);

impl Reuse {
    /// Undoes what the last parse's segments did, the latest first,
    /// keeping what each changed as it was at its end, for the next parse:
    /// after the compile's own items are undone, before the session ends.
    /// The last segment, which ends at no boundary, is undone alone.
    pub fn undo(&mut self, vm: &mut Vm) {
        if !self.live {
            return;
        }
        self.live = false;
        vm.forget_redo();
        let complete = self.boundaries.len().saturating_sub(1);
        for (i, segment) in self.segments.iter_mut().enumerate().rev() {
            segment.redo = match i < complete {
                true => Some(vm.undo_keeping(segment.mark)),
                false => {
                    vm.undo(segment.mark);
                    None
                }
            };
        }
        vm.keep_for_redo(self.module);
    }

    /// The file `name` changed: unless it is the file parsed, whose edit
    /// the next parse compares, nothing of the last parse is done again,
    /// since a segment may have read it (an `\include`, a template's).
    pub fn changed(&mut self, vm: &mut Vm, name: &str) {
        if name == self.name || self.boundaries.is_empty() {
            return;
        }
        self.undo(vm);
        vm.forget_redo();
        *self = Reuse::default();
    }

    /// How many segments the last parse did again and how many it parsed.
    pub fn reused(&self) -> (usize, usize) {
        self.reused
    }

    /// How many braced blocks the last parse took from the one before it.
    pub fn blocks_taken(&self) -> usize {
        self.blocks_taken_last
    }

    /// The boundary a parse of `text` starts at: the last before the first
    /// byte that differs whose segments before it all ended there and
    /// whose included files the host still has as they were; None for a
    /// parse from the start in a new module.
    fn kept(
        &self,
        name: &str,
        text: &[u8],
        include: &mut dyn FnMut(&str) -> Hosted<Vec<u8>>,
    ) -> Option<usize> {
        if self.name != name || self.live || self.boundaries.is_empty() {
            return None;
        }
        let differs = common_prefix(&self.text, text) as u32;
        let mut kept = 0;
        for (i, boundary) in self.boundaries.iter().enumerate().skip(1) {
            if boundary.examined >= differs || self.segments[i - 1].redo.is_none() {
                break;
            }
            kept = i;
        }
        // A file the last parse included that the host has otherwise.
        let changed = self.included.iter().any(|src| {
            src.name() != "<included string>"
                && !matches!(include(src.name()), Hosted::Text(t) if t == src.text())
        });
        (!changed).then_some(kept)
    }

    /// The first byte of `text` from which the last parse's segments may
    /// be taken over (§5.7's step 4): the line break that ends the edit's
    /// last line, what a segment from there lexes being on the lines after,
    /// when the edit moved no byte and no line after it; else None.
    fn take_from(&self, text: &[u8]) -> Option<u32> {
        if self.text.len() != text.len() {
            return None;
        }
        let start = common_prefix(&self.text, text);
        if start == text.len() {
            return Some(0);
        }
        let end = text.len() - common_prefix(self.text.iter().rev(), text.iter().rev());
        let lines = |t: &[u8]| -> Vec<usize> {
            t[start..end.max(start)]
                .iter()
                .enumerate()
                .filter(|&(_, &b)| b == b'\n')
                .map(|(i, _)| i)
                .collect()
        };
        if lines(&self.text) != lines(text) {
            return None;
        }
        let next = text[end..].iter().position(|&b| b == b'\n')?;
        Some((end + next) as u32)
    }
}

/// How many items two sequences begin with alike.
fn common_prefix<'a>(
    a: impl IntoIterator<Item = &'a u8>,
    b: impl IntoIterator<Item = &'a u8>,
) -> usize {
    a.into_iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// [`crate::parse_including`] of `text` named `name`, parsed again from
/// the first segment of the last parse `reuse` kept that the edit can
/// change, the segments before it done again and those after it taken over
/// where they can be; and recorded in `reuse` for the next. [`Reuse::undo`]
/// undoes it when the compile ends. If `returns`, its compile's own (H3,
/// D267), the parse stops at the first boundary after the step's time is
/// spent ([`ParseStep::Paused`]), and [`parse_on`] goes on with it: the
/// deadline is the process's, so a compile that does not return never asks it.
pub fn parse_again(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    vm: &mut Vm,
    include: Include<'_>,
    reuse: &mut Reuse,
    returns: bool,
) -> ParseStep {
    reuse.undo(vm);
    let restart = reuse.kept(name, &text, include);
    let last = core::mem::take(reuse);
    let mut done = Vec::new();
    for segment in &last.segments[..restart.unwrap_or(0)] {
        let mark = vm.begin_item();
        if let Some(redo) = &segment.redo {
            vm.redo(redo);
        }
        vm.end_item(mark);
        done.push(Segment {
            mark,
            redo: None,
            ..segment.clone()
        });
    }
    let mut env = match restart {
        None => {
            vm.forget_redo();
            Env::new(vm)
        }
        Some(_) => Env::resume(vm, last.module),
    };
    env.include = Some(include);
    let module = env.module();
    let take_from = restart.and_then(|_| last.take_from(&text));
    let differs = restart.map(|_| common_prefix(&last.text, &text) as u32);
    let heap = env.vm().undo_len().0;
    let segments = Segments {
        boundaries: Vec::new(),
        segments: done,
        mark: None,
        moved: false,
        last,
        restart,
        take_from,
        heap,
        taken: 0,
        taken_at: Vec::new(),
        differs,
        blocks: Vec::new(),
        blocks_taken: 0,
        returns,
        paused: false,
    };
    let ending = crate::parse_with(
        name,
        text.clone(),
        names,
        env,
        false,
        true,
        Some(segments),
        None,
    );
    ended(name, text, module, ending, vm, reuse)
}

/// [`parse_again`]'s parse stopped at a boundary, gone on with from there in
/// a step of its own (H3, D267): the lexer at the boundary with what was
/// said and included before it, the parser's state as it was there, in the
/// same module; its segments' items still in the undo log.
pub fn parse_on(
    parsing: alloc::boxed::Box<Parsing>,
    names: &NoteNames,
    vm: &mut Vm,
    include: Include<'_>,
    reuse: &mut Reuse,
) -> ParseStep {
    let Parsing {
        name,
        text,
        module,
        mut segments,
        diagnostics,
        items,
        included,
        in_score,
    } = *parsing;
    segments.paused = false;
    let mut env = Env::resume(vm, module);
    env.include = Some(include);
    let ending = crate::parse_with(
        &name,
        text.clone(),
        names,
        env,
        false,
        true,
        Some(segments),
        Some((diagnostics, items, included, in_score)),
    );
    ended(&name, text, module, ending, vm, reuse)
}

/// What a step of [`parse_again`] or [`parse_on`] leaves: the parse, its
/// segments recorded in `reuse` for the next; or the parse stopped.
fn ended(
    name: &str,
    text: Vec<u8>,
    module: Value,
    ending: Ending,
    vm: &mut Vm,
    reuse: &mut Reuse,
) -> ParseStep {
    let (parse, ended) = match ending {
        Ending::Ended(ended) => *ended,
        Ending::Paused(paused) => {
            let (segments, diagnostics, items, included, in_score) = *paused;
            return ParseStep::Paused(alloc::boxed::Box::new(Parsing {
                name: name.into(),
                text,
                module,
                segments,
                diagnostics,
                items,
                included,
                in_score,
            }));
        }
    };
    // The old segments' ends are no longer needed: the parse took over
    // what it could.
    vm.forget_redo();
    if let Some((segments, diagnostics, items, included)) = ended {
        *reuse = Reuse::of(name, text, module, segments, diagnostics, items, included);
    }
    ParseStep::Done(alloc::boxed::Box::new(parse))
}

impl Reuse {
    /// What a parse that ended, or stopped at a boundary, leaves for the
    /// next: its segments live in the undo log until [`Reuse::undo`].
    fn of(
        name: &str,
        text: Vec<u8>,
        module: Value,
        segments: Segments,
        diagnostics: Diagnostics,
        items: Vec<Item>,
        included: Vec<&'static Source>,
    ) -> Reuse {
        let again = segments.restart.unwrap_or(0) + segments.taken;
        Reuse {
            name: name.into(),
            text,
            module,
            reused: (again, segments.segments.len() - again),
            boundaries: segments.boundaries,
            segments: segments.segments,
            diagnostics,
            items,
            included,
            live: true,
            blocks: segments.blocks,
            blocks_taken_last: segments.blocks_taken,
        }
    }
}

impl Parsing {
    /// The parse stopped for good (a newer edit, H3): left in `reuse` as a
    /// parse that ended at its last boundary, whose segments the compile's
    /// undo undoes and keeps for the next parse, which may do them again.
    pub fn stop(self, vm: &mut Vm, reuse: &mut Reuse) {
        let Parsing {
            name,
            text,
            module,
            mut segments,
            diagnostics,
            items,
            included,
            in_score,
        } = self;
        // Stopped in a `\score`'s `scorify-music` (D272): the call dropped,
        // and what the item under way did undone, as it never ended.
        if in_score.is_some() {
            vm.abandon_steps();
            if let Some(mark) = segments.mark.take() {
                vm.undo(mark);
            }
        }
        segments.blocks = segments.blocks_kept();
        segments.last = Reuse::default();
        *reuse = Reuse::of(&name, text, module, segments, diagnostics, items, included);
    }
}

/// What a parse that keeps a [`Reuse`] gives it at its end: its segments,
/// and what it said, made and included.
pub(crate) type Ended = (Segments, Diagnostics, Vec<Item>, Vec<&'static Source>);

impl<'a> Parser<'a, '_> {
    /// Keeps `segments` as it parses: started at the boundary of the last
    /// parse they name, with what was made before it, which it returns;
    /// else from the start, which is their first boundary.
    pub(crate) fn start_segments(&mut self, mut segments: Segments) -> Vec<Item> {
        let made = match segments.restart {
            Some(k) => {
                let last = &segments.last;
                let boundary = last.boundaries[k].clone();
                self.restart(&boundary, &last.diagnostics, &last.included);
                segments.boundaries = last.boundaries[..=k].to_vec();
                last.items[..boundary.items].to_vec()
            }
            None => {
                segments.boundaries = alloc::vec![self.start()];
                Vec::new()
            }
        };
        self.segments = Some(segments);
        made
    }

    /// Keeps `segments` as it parses, gone on with from their last boundary,
    /// where a parse in steps stopped (D267): what it had said and included
    /// there, the lexer at its place and the parser's state as it was.
    pub(crate) fn resume_segments(
        &mut self,
        segments: Segments,
        diagnostics: Diagnostics,
        included: &[&'static Source],
        in_score: Option<&Boundary>,
    ) {
        if let Some(boundary) = in_score.or(segments.boundaries.last()).cloned() {
            self.lexer.diags = diagnostics;
            self.go_to(&boundary, included);
        }
        self.segments = Some(segments);
    }

    /// Whether the parse stopped at its last boundary for the step's time,
    /// or in a toplevel `\score`'s `scorify-music` (D272).
    pub(crate) fn paused(&self) -> bool {
        self.in_score.is_some() || self.segments.as_ref().is_some_and(|s| s.paused)
    }

    /// Whether it may stop for its step's time: its compile returns (D267).
    pub(crate) fn returns(&self) -> bool {
        self.segments.as_ref().is_some_and(|s| s.returns)
    }

    /// Its segments where it stopped, with what it said, made and included.
    pub(crate) fn pause_segments(&mut self, items: Vec<Item>) -> Option<Paused> {
        let segments = self.segments.take()?;
        Some((
            segments,
            self.lexer.diags.clone(),
            items,
            self.lexer.included().to_vec(),
            self.in_score.take(),
        ))
    }

    /// Its segments at the end, with what it said, made (`items`, from the
    /// start) and included.
    pub(crate) fn end_segments(&mut self, items: &[Item]) -> Option<Ended> {
        let mut segments = self.segments.take()?;
        segments.blocks = segments.blocks_kept();
        segments.last = Reuse::default();
        Some((
            segments,
            self.lexer.diags.clone(),
            items.to_vec(),
            self.lexer.included().to_vec(),
        ))
    }

    /// Starts the parse at `boundary` of the last parse: the lexer at its
    /// place with what was said and included before it, the parser's
    /// state as it was there.
    fn restart(
        &mut self,
        boundary: &Boundary,
        diagnostics: &Diagnostics,
        included: &[&'static Source],
    ) {
        self.lexer.diags = diagnostics.cut(&boundary.said);
        self.go_to(boundary, &included[..boundary.included]);
    }

    /// The lexer at `boundary`'s place after the strings `included`, and
    /// the parser's state as it was there.
    fn go_to(&mut self, boundary: &Boundary, included: &[&'static Source]) {
        self.lexer
            .restart(boundary.at as usize, boundary.last, included);
        self.lexer.version = boundary.version.clone();
        self.lexer.error_level = boundary.error_level;
        if let Some(table) = self.names.language(&boundary.language) {
            self.lexer.set_pitch_names(table);
        }
        self.ahead = None;
        self.last_end = boundary.last_end;
        self.default_duration = boundary.duration;
        self.default_tremolo = boundary.tremolo;
        let language = self.language().map(String::from);
        if let Some(p) = self.env.lily_parser() {
            p.language = language;
        }
        self.publish();
    }

    /// At the top of the toplevel loop: the current segment's item begun,
    /// or, at a boundary after an item, ended, the old segments after it
    /// taken over where they can be, and the next begun.
    pub(crate) fn segment_boundary(&mut self, items: &mut Vec<Item>) {
        let Some(segments) = &self.segments else {
            return;
        };
        if segments.mark.is_none() {
            let mark = self.env.vm().begin_item();
            self.segments.as_mut().unwrap().mark = Some(mark);
            return;
        }
        if !segments.moved {
            return;
        }
        let Some(boundary) = self.boundary(items.len()) else {
            return;
        };
        let vm = self.env.vm();
        let segments = self.segments.as_mut().unwrap();
        if let Some(mark) = segments.mark.take() {
            let item = vm.end_item(mark);
            segments.segments.push(Segment {
                mark,
                redo: None,
                reads: item.reads,
                writes: item.writes,
            });
        }
        segments.boundaries.push(boundary);
        self.take_over(items);
        // A step's time spent (H3, D267): the parse stops here, where it
        // could start again, and goes on in the next step.
        let returns = self.segments.as_ref().is_some_and(|s| s.returns);
        if returns && lily_engine_music::returns::time_up() {
            if let Some(segments) = self.segments.as_mut() {
                segments.paused = true;
            }
            return;
        }
        let mark = self.env.vm().begin_item();
        let segments = self.segments.as_mut().unwrap();
        segments.mark = Some(mark);
        segments.moved = false;
    }

    /// 02 §5.7's step 4 at the boundary just recorded: the last parse's
    /// segments from the same boundary on done again, not parsed, while
    /// each read what it read at the versions it read them, and the parse
    /// gone on at the boundary after them, with their messages, items and
    /// included strings.
    fn take_over(&mut self, items: &mut Vec<Item>) {
        let Some(mut segments) = self.segments.take() else {
            return;
        };
        if let Some(j) = self.takes_over_at(&segments) {
            let mut at = j;
            while at + 1 < segments.last.boundaries.len()
                && let Some(redo) = segments.last.segments[at].redo.clone()
                && self.reads_unchanged(&segments.last.segments[at].reads)
            {
                let vm = self.env.vm();
                let mark = vm.begin_item();
                vm.redo(&redo);
                vm.end_item(mark);
                let old = &segments.last;
                let (from, to) = (&old.boundaries[at], &old.boundaries[at + 1]);
                if !self
                    .lexer
                    .diags
                    .append(&old.diagnostics, &from.said, &to.said)
                {
                    // What waits for Guile's flush differs: undone, and
                    // parsed from here.
                    self.env.vm().undo(mark);
                    break;
                }
                items.extend_from_slice(&old.items[from.items..to.items]);
                let mut included = self.lexer.included().to_vec();
                included.extend_from_slice(&old.included[from.included..to.included]);
                self.go_to(to, &included);
                segments.segments.push(Segment {
                    mark,
                    redo: None,
                    ..old.segments[at].clone()
                });
                segments.boundaries.push(Boundary {
                    said: self.lexer.diags.mark(),
                    items: items.len(),
                    included: self.lexer.included().len(),
                    ..to.clone()
                });
                segments.taken += 1;
                segments.taken_at.push((from.at, to.at));
                at += 1;
            }
            segments.heap = self.env.vm().undo_len().0;
        }
        self.segments = Some(segments);
    }

    /// The last parse's boundary that the one just recorded is, if its old
    /// segments may be taken over from it: past the edit, in the same
    /// state, the segments parsed again since the start having changed no
    /// older object and written the variables, of the file's module alone,
    /// that the old ones wrote.
    fn takes_over_at(&mut self, segments: &Segments) -> Option<usize> {
        let new = segments.boundaries.last()?;
        let (from, restart) = (segments.take_from?, segments.restart?);
        let old = &segments.last;
        if new.at < from || self.env.vm_ref().changed_since(segments.heap) {
            return None;
        }
        let j = old
            .boundaries
            .binary_search_by_key(&new.at, |b| b.at)
            .ok()?;
        if j <= restart
            || !new.same_state(&old.boundaries[j])
            || !self.lexer.diags.same_pending(&old.boundaries[j].said)
        {
            return None;
        }
        let written = |list: &[Segment]| {
            let mut slots: Vec<Slot> = Vec::new();
            for slot in list.iter().flat_map(|s| s.writes.iter()) {
                if !slots.contains(slot) {
                    slots.push(*slot);
                }
            }
            slots
        };
        let now = written(&segments.segments[restart..]);
        let then = written(&old.segments[restart..j]);
        let module = self.env.module();
        let vm = self.env.vm_ref();
        let same = now.len() == then.len() && now.iter().all(|s| then.contains(s));
        let own = now.iter().all(|&s| vm.slot_module(s) == module);
        (same && own).then_some(j)
    }

    /// Whether every slot of `reads` has the version it was read at.
    fn reads_unchanged(&self, reads: &[(Slot, u32)]) -> bool {
        let vm = self.env.vm_ref();
        reads
            .iter()
            .all(|&(slot, version)| vm.slot_version(slot) == version)
    }

    /// After an item: a boundary may follow.
    pub(crate) fn segment_moved(&mut self) {
        if let Some(segments) = &mut self.segments {
            segments.moved = true;
        }
    }

    /// After the toplevel loop: the current segment's item ended.
    pub(crate) fn segments_end(&mut self) {
        let vm = self.env.vm();
        let Some(segments) = &mut self.segments else {
            return;
        };
        if let Some(mark) = segments.mark.take() {
            let item = vm.end_item(mark);
            segments.segments.push(Segment {
                mark,
                redo: None,
                reads: item.reads,
                writes: item.writes,
            });
        }
    }

    /// The boundary here, if the parse can start again here, `items` made
    /// before it.
    pub(crate) fn boundary(&mut self, items: usize) -> Option<Boundary> {
        let quiet = self.modes.is_empty()
            && self.base == Mode::Notes
            && self.recovering == 0
            && self.pushed == 0
            && !self.aborted
            && !self.guile_abort
            && !self.markup_stuck;
        if !quiet || !self.env.vm_ref().error_port_empty() || self.env.vm_ref().waiting() {
            return None;
        }
        let (pos, last) = self.lexer.boundary()?;
        let at = match self.ahead {
            None => pos as u32,
            Some((token, mode)) => {
                let lexed_quietly = self.lexer.diags.said() == self.ahead_said;
                if token.span.file != self.lexer.file()
                    || mode != Mode::Notes
                    || !lexed_quietly
                    || !self.lexes_alike(&token)
                {
                    return None;
                }
                token.span.start
            }
        };
        let table = self.lexer.pitch_names()?;
        let language = String::from(self.names.language_of(table)?);
        Some(Boundary {
            at,
            examined: self.lexer.examined() as u32,
            last,
            last_end: self.last_end,
            duration: self.default_duration,
            tremolo: self.default_tremolo,
            language,
            version: self.lexer.version.clone(),
            error_level: self.lexer.error_level,
            said: self.lexer.diags.mark(),
            items,
            included: self.lexer.included().len(),
        })
    }

    /// Whether the lookahead `token` would be lexed alike now, as a parse
    /// started at it lexes it: what the item before it did after it was
    /// lexed (an assignment defines then, `assign.rs`; `\language` changes
    /// the pitch names) may give its word or its `\name` another meaning.
    /// A `$` form's value, which its lexing made, is never alike, nor a
    /// token of a kind no toplevel item starts with.
    fn lexes_alike(&mut self, token: &Token) -> bool {
        let text = self.lexer.text_of(token.span);
        match token.kind {
            Tok::Char(_) | Tok::DoubleAngleOpen | Tok::Keyword(_) | Tok::String => true,
            Tok::ScmToken => text.first() == Some(&b'#'),
            Tok::Symbol => self.lexer.pitch_name(text).is_none(),
            Tok::NotenamePitch => self.lexer.pitch_name(text) == Some(Pitch::unpack(token.value)),
            Tok::Identifier(_) => match text.strip_prefix(b"\\") {
                Some(name) => {
                    let name = String::from_utf8_lossy(name);
                    let lexed = self.env.value(token.value);
                    self.env.lookup(&name) == Some(lexed)
                }
                None => false,
            },
            _ => false,
        }
    }

    /// The boundary at the start of the file: nothing said, made or
    /// included.
    fn start(&self) -> Boundary {
        let language = self
            .lexer
            .pitch_names()
            .and_then(|table| self.names.language_of(table))
            .map(String::from)
            .unwrap_or_default();
        Boundary {
            at: 0,
            examined: 0,
            last: (0, 0),
            last_end: 0,
            duration: self.default_duration,
            tremolo: self.default_tremolo,
            language,
            version: VersionSeen::No,
            error_level: false,
            said: Diagnostics::default().mark(),
            items: 0,
            included: 0,
        }
    }
}

impl Segments {
    /// The blocks the next parse may take: this parse's, and the last
    /// parse's in the segments done again, which are where they were.
    fn blocks_kept(&self) -> Vec<Block> {
        let start = self.restart.map_or(0, |k| self.last.boundaries[k].at);
        let kept = |b: &Block| {
            b.end < start
                || self
                    .taken_at
                    .iter()
                    .any(|&(from, to)| from <= b.start && b.end <= to)
        };
        let mut blocks: Vec<Block> = self
            .last
            .blocks
            .iter()
            .filter(|b| kept(b))
            .cloned()
            .collect();
        blocks.extend(self.blocks.iter().cloned());
        blocks.sort_by_key(|b| b.start);
        blocks.dedup_by_key(|b| b.start);
        blocks
    }
}

impl<'a> Parser<'a, '_> {
    /// At a braced block's opening `open`, in a parse that keeps a
    /// [`Reuse`] and with nothing pushed, included or waiting: where and in
    /// what state it is entered.
    pub(crate) fn block_entry(&mut self, open: &Token) -> Option<BlockEntry> {
        self.segments.as_ref()?;
        let clean = self.ahead.is_none()
            && self.recovering == 0
            && !self.markup_stuck
            && !self.aborted
            && open.span.file == self.lexer.file()
            && self.lexer.boundary().is_some()
            && self.env.vm_ref().error_port_empty();
        if !clean {
            return None;
        }
        let table = self.lexer.pitch_names()?;
        let language = String::from(self.names.language_of(table)?);
        Some(BlockEntry {
            start: open.span.start,
            state: BlockState {
                modes: self.modes.clone(),
                base: self.base,
                language,
                duration: self.default_duration,
                tremolo: self.default_tremolo,
            },
            counts: self.block_counts(),
        })
    }

    /// What a block that changes nothing leaves as it was: the values
    /// kept, the forms read, the strings lexed, what was said (two), the
    /// undo logs' entries (two) and the strings included.
    fn block_counts(&self) -> [usize; 8] {
        let said = self.lexer.diags.said();
        let (heap, slots) = self.env.vm_ref().undo_len();
        [
            self.env.kept(),
            self.env.forms.len(),
            self.lexer.strings_len(),
            said.0,
            said.1,
            heap,
            slots,
            self.lexer.included().len(),
        ]
    }

    /// The last parse's block at `entry`'s place, taken if it can be: the
    /// lexer after its close, the default duration it left; its music.
    pub(crate) fn reuse_block(&mut self, entry: &BlockEntry) -> Option<Music> {
        let segments = self.segments.as_ref()?;
        let old = &segments.last.blocks;
        let block = &old[old.binary_search_by_key(&entry.start, |b| b.start).ok()?];
        let before = segments.differs.is_some_and(|differs| block.end < differs);
        let after = segments.take_from.is_some_and(|from| block.start >= from);
        if !(before || after) || block.entry != entry.state {
            return None;
        }
        let block = block.clone();
        self.lexer.seek(block.end as usize, block.last);
        self.last_end = block.last_end;
        self.default_duration = block.duration;
        self.default_tremolo = block.tremolo;
        let music = block.music.clone();
        let segments = self.segments.as_mut()?;
        segments.blocks.push(block);
        segments.blocks_taken += 1;
        Some(music)
    }

    /// After a block entered at `entry` was parsed: recorded if its parse
    /// changed nothing and the lexer stands clean after its close.
    pub(crate) fn record_block(&mut self, entry: BlockEntry, music: &Music) {
        if self.segments.is_none()
            || self.ahead.is_some()
            || self.aborted
            || self.recovering != 0
            || self.lexer.boundary().is_none()
            || !self.env.vm_ref().error_port_empty()
            || self.block_counts() != entry.counts
        {
            return;
        }
        let (pos, last) = self.lexer.place();
        if self.lexer.examined() > pos + 1 {
            // Its reader looked past its close.
            return;
        }
        let block = Block {
            start: entry.start,
            end: pos as u32,
            entry: entry.state,
            last,
            last_end: self.last_end,
            duration: self.default_duration,
            tremolo: self.default_tremolo,
            music: music.clone(),
        };
        if let Some(segments) = &mut self.segments {
            segments.blocks.push(block);
        }
    }
}
