//! A compile (K2, MS2 steps 10–11): the root file parsed in one step, and
//! its diagnostics written as `stderr` and `diagnostics.json` from one list
//! (`diag.rs`, 08 §6). A root the host does not have is LilyPond's missing
//! file. What the parse made is left to the collector: the values of each
//! toplevel item are unrooted after it, and the file's module and its
//! scopes' are collected once nothing reaches them, so that compiles in a
//! row in one instance leave the live heap where the first left it. After
//! each, `session-terminate` gives the init files' variables their values
//! again, as `lilypond-all` does after each file (MS2 step 16), and
//! `ly:reset-all-fonts` forgets the fonts it loaded (MS3 step 5).
//!
//! The files a score includes and those of a part a stub loads (D74) are
//! the host's too (MS2 step 19). A file it was not asked for stops the
//! parse ([`lily_engine_parser::Parse::waiting`], `Vm::take_waiting`): what
//! the parse made is dropped, the step ends with [`NEEDS`] and
//! `lily_needs` names the file, and the compile is parsed again from its
//! start once the host has put it, as the studio's driver starts it again.
//! A part's files load whole or not at all, so a part never stops halfway.
//! Since MS2 step 30 a stop asks for every file of a part not given yet:
//! the steps after it give [`NEEDS`] for each in turn, one name a step,
//! and parse again only once the host has put them all, so that a part
//! costs one parse more, not one a file.
//!
//! A compile then translates its scores, as `ly:book-process` does after
//! the file is parsed (`Score::book_rendering`): the scores of the `\book`
//! blocks first, then those of the book `init.ly` makes of the toplevel
//! parts and scores the toplevel handlers collected, each book's in the
//! order its `Paper_book` and its parts' print them (`Book::process`,
//! layout's `paper_book.rs`, MS3 step 35), a score with errors skipped;
//! each `\layout`'s Paper_score is added to its `Paper_book`, after the
//! score's header. Each is given a `\midi` performance
//! as `tools/compare/capture.ly` gives it (`$defaultmidi`, and
//! `$defaultlayout` too for a score with no output definition), and each
//! of its output definitions is translated in turn: a `\layout` with N4's
//! global loop, a `\midi` with its performers, which write the playback
//! map (MS2 step 26, D87): `<name>.timing.json`, after `stderr` and
//! `diagnostics.json`, an array of the performances in order, as
//! `timing.ly` writes it; a file the score's Scheme writes is an output
//! too, in place of the engine's of that name, so `timing.ly` included
//! runs as it does in LilyPond (G8). What each translation says goes to
//! `stderr` after the parser's messages, as LilyPond prints it (MS2 step
//! 40, D99): its warnings, errors and programming errors at their places,
//! and what Scheme wrote to the error port; then the expected warnings
//! none met, then `failed files:`, which only the parser's errors cause.
//!
//! Since MS5 step 1 each `\midi` translation's `Performance` is added to
//! its score's `Paper_book` with the headers it is named by
//! (`Book::process_score`), and each book's performances, its parts' after
//! its own, are written before its pages by `write-performances-midis`, as
//! `Paper_book::output_aux` writes them: `<name>.midi`, then
//! `<name>-1.midi` and on, a book after the first `<name>-<k>.midi` and
//! on, after the pages and before the playback map. A run a translation's
//! error ended writes those of the books before it only.
//!
//! The parse and each translation are timed ([`Engine::times`], MS2 step
//! 27), the translation from frozen music to the `\layout`'s end or the
//! playback map's events.
//!
//! A traced compile ([`Engine::set_trace`], MS2 step 23) records the
//! `\layout` translations' trace in `tools/compare/trace.ly`'s format
//! (`trace.tsv`) and what they said (`trace.log`): their messages without
//! excerpts and the iterators they lacked. Since MS3 step 12 it writes the
//! grobs they made too, in `tools/compare/grobs.ly`'s format
//! (`grobs.tsv`), from their grob records, the columns `-` until step 15.
//! Since MS3 step 25 it writes their laid-out grobs too, in
//! `tools/compare/layout.ly`'s format (`layout.tsv`), from layout's state
//! ([`lily_engine_layout::dump::lay_out`]): a line a live grob of the
//! System, every field after grobs.tsv's six unreached but, since MS3 step
//! 28, the Y extent. A file its reads would ask the host for is not asked
//! for: the stop is the read's, which is unreached, not the compile's.
//! Since MS3 step 31 it writes the springs and rods between the columns
//! too, in `layout.ly`'s other format (`columns.tsv`, D128), after reading
//! each SpacingSpanner's `springs-and-rods`, the springs and rods
//! unreached where the read did not complete. Since MS3 step 32 it writes
//! the X offset of each column of the System's line (`line.tsv`, D129).
//!
//! Since MS3 step 33 every `\layout` translation is laid out after it, as
//! LilyPond's is before its pages: its Paper_score processed
//! (`System::pre_processing`: the breakable items' prebroken pieces, then
//! each grob's `before-line-breaking` and `springs-and-rods`) and its
//! System broken into its lines (`calc_breaking`, `break_into_pieces`,
//! the break substitution). The first error ends it unreported, and a
//! stop for the host is not asked, as a traced read's is not (D126): a
//! placeholder's `misc-error`, or a file the layout reads, ends the layout
//! and not the compile. A traced
//! compile runs the same parts, each a read of its own
//! ([`lily_engine_layout::dump::lay_out`]), and writes `columns.tsv`
//! after `pre_processing` and `layout.tsv` and `line.tsv` after breaking.
//! Since MS3 step 62 (D219) the breaking is the paper's page breaker's
//! (`ly:optimal-breaking`), when the book's pages are made: a translation
//! is laid out as far as `pre_processing`, and a traced compile writes
//! `layout.tsv` and `line.tsv` after every book's pages
//! ([`lily_engine_layout::dump::laid_out`]), each system placed on its
//! page, what those reads say in `trace.log` only. Since MS4 step 6 it
//! writes chapter 05's shape dump for C5 after them, as
//! `tools/compare/c5dump.ly` writes it (`c5.tsv`,
//! [`lily_engine_layout::dump::shapes`]).
//!
//! Since MS3 step 35 each book's pages are made after the translations,
//! as `Paper_book::output` makes them (layout's [`paper_book::output`]),
//! and each page is written by `crates/svg` as LilyPond's SVG backend
//! writes it: `<name>.svg`, or `<name>-<n>.svg` for the page numbered n of
//! a book of more, the books after the first named `<name>-1`, `<name>-2`
//! and so on (protocol.ts' names), after `stderr` and `diagnostics.json`
//! and before the playback map. A book whose layout failed has no pages,
//! and an error of its pages ends them unreported, as the layout's (D130).
//! What making and writing them said goes to `stderr` after the
//! translations'.
//!
//! Since MS3 step 35 too, an error of a translation, `\layout` or `\midi`,
//! a Scheme engraver's `finalize` among them, ends the run as it ends
//! LilyPond's (D132): what the translation said, then the error as
//! LilyPond prints it ([`diag::ended`], at `init.ly` for the book it
//! makes), and nothing after it: no other translation, no pages of its
//! book or the books after it, no playback map, no unmet expected warnings
//! and no `failed files:`. `diagnostics.json` has the error, and the
//! compile example exits with 1, as LilyPond's run does. The engine's own
//! errors (a placeholder's call, a font the host gave no pack of) are no
//! LilyPond's: they end the translation unreported, and its book's pages,
//! as before.
//!
//! Since MS3 step 50 (W5, D208) a font of the build's packs that the
//! compile asked for and the host had not given (an Emmentaler size, the
//! braces, a text face: layout's [`font::take_wanted`]), named by the
//! grobs of a `\layout` translation before its layout
//! (`font_select::want_fonts`) or met by a layout, stops it as a file
//! does: what it made is dropped, the steps after it give [`NEEDS`] for
//! each of its packs not put yet (`MODULE`, the brace's four parts in
//! order), one a step, and it is compiled again once the host has put
//! them all. A pack the host put as none, which it could not fetch, is
//! not asked for again in that compile: the font's error ends its layout
//! as before, and the compile says so ([`diag::unfetched`]), after what
//! the translations said. Since MS7 step 5 (E14, D262) the outlines of a
//! text face are such a pack, `<face>.outlines`, asked for when a skyline
//! first needs one: a compile in steps stops there ([`Engine::step`]), one
//! call at its end; one the host could not give leaves the glyphs' boxes
//! in the skylines, and the compile says so.
//!
//! Since MS6 step 2 (S5) a compile is one item of the Scheme heap's undo
//! log (`Vm::begin_item`), from its parse to its last output, and is
//! undone after it ([`undo`]), whether it ended or stopped for the host:
//! every object and global older than the compile is as it was before it,
//! so a compile in a row starts where a fresh engine's starts (OPEN C7),
//! and an option a score sets ends with it (C2.2), `session-terminate`'s
//! reset kept for what the log does not reach. A part a stub loads is not
//! logged, and stays loaded (D74).
//!
//! Since MS6 step 3 (02 §5.7, D235) the parse's segments are items of their
//! own before the compile's: the parser's [`Reuse`] keeps them between
//! compiles, and the compile, when it ends or stops, undoes its own item
//! and then them, keeping what each changed for the next parse of its file,
//! which does again those before the first byte an edit changed instead of
//! parsing them.
//!
//! Since MS4 step 17 (A46, D228) a part the grobs of a `\layout`
//! translation name a stub of is loaded before its layout
//! ([`load_parts`]), whose stop would not be asked: a part whose files the
//! host has not given stops the compile there as a parse's file does. And
//! a `\score` in a markup is translated with the notation crates' classes
//! too ([`host_score_embedded_format`], A47).
//!
//! Since MS6 step 11 (H3, D254) a compile is no longer one call. With
//! [`Engine::set_returns`], as `lily_start` has it, [`Engine::step`] keeps to
//! its budget: what it had in its frames is a [`Running`] and a [`Work`], and
//! it returns [`MORE`] between timesteps, grobs, lines, systems and pages,
//! each loop going on from its own state (`lily_engine_music::returns`), with
//! everything rooted and the parse's module current until it ends. What it
//! writes is what the one call writes. A newer `lily_put` or `lily_compile`
//! stops it ([`Engine::stop_running`]): it is undone as a stop for the host is,
//! and what the compile before it left stands (D261), so the next resumes it.
//! A traced compile, and one nested in a markup, is one call. Since D261 a
//! return can divide a collection, as nothing runs on the heap between steps,
//! a spacing spanner's read, a line's pure heights and the page's spacing
//! problem. What no return divides: the parse, the iterators made at the
//! start, N8's snapshots, one grob's callback.
//!
//! Since MS7 step 8 (M7, 07 §3.8, D264) a compile for a page leaves its
//! `\midi` definitions out ([`Engine::set_performs`]): it translates the
//! `\layout`s and writes the pages, and no MIDI file, playback map or
//! performer's message. [`Engine::perform`] then gives them, after the page
//! and when the host wants them: the root parsed again (the parser's reuse
//! makes that cheap), its `\midi` translations alone, whole, and the books'
//! MIDI files and playback map written as a compile writes them, with
//! `stderr` and `diagnostics.json` of what those translations said, which
//! LilyPond prints after the `\layout`s' (the parse's messages and `failed
//! files:` are the page's); no `\layout` is translated and no page made. It is a compile in
//! steps like any other, which a newer `lily_put` or `lily_compile` stops; and
//! it keeps nothing, as a stopped compile keeps nothing (B24, D261): N8's
//! translations, layout's cells and the writer's markup stay as the compile
//! before it left them, so the next compile resumes those.

use crate::diag::{self, List};
use crate::{Asked, DONE, Engine, FILE, MODULE, MORE, NEEDS, Times, hosted, memory, need_of, now};
use lily_engine_layout::paper_book::Output;
use lily_engine_layout::system::Processing;
use lily_engine_layout::{font, paper_book};
use lily_engine_music::{
    Going, Input, Kept, Moved, OutputDef, Resumable, Score, Trace, Translated, TranslatorClass,
    returns, scaled_layout, start_resuming, toplevel_book,
};
use lily_engine_parser::{At, Diagnostics, Message, Parse, ParseStep, Reuse, Severity};
use lily_engine_scheme::{Exception, Mark, Value, Vm};
use lily_engine_svg::Markup;
use std::collections::BTreeSet;

/// Bytes a grob takes as its layout starts, with no place to stop (D259;
/// measured on staves of 8,000 and 9,000 notes).
const GROB_START: usize = 1 << 10;

/// The bytes that must be left for collections to wait for a step's end (B20,
/// D261): past it a compile collects as it did. What a collection waits for
/// grows the arenas, whose free slots count as used for good; and where it is
/// less, T29 × 8 fits less often, not more (at 120 MiB on none of six runs).
const DEFERRING_ROOM: usize = 64 << 20;

impl Engine {
    /// Starts compiling `root`, which [`Engine::step`] asks for if it was
    /// not given. The same compile started again, its revision and root
    /// those of a compile that stopped, as a host starts it after each
    /// answer, goes on asking for the files its stop asked for.
    pub fn compile(&mut self, revision: u32, root: &str) {
        self.stop_running();
        self.performing = false;
        self.vm.take_over_memory();
        if self.revision != revision || self.root.as_deref() != Some(root) {
            self.asked.borrow_mut().clear();
            self.packs.clear();
            self.unfetched.clear();
            self.parses = 0;
        }
        self.revision = revision;
        self.root = Some(root.into());
        self.needs = None;
        self.outputs.clear();
        self.changes.clear();
        self.n8_wrong.clear();
    }

    /// Starts the performance of the compile of `root` at `revision` (M7,
    /// D264): its `\midi` translations alone, whole, and its MIDI files and
    /// playback map, which a compile for a page leaves out
    /// ([`Engine::set_performs`]). [`Engine::step`] runs it as it runs a
    /// compile, and a host that answers its stops starts it again the same
    /// way. A compile that a translation's error ended performs nothing, as
    /// LilyPond's run then writes no MIDI file (D132): it is done at once,
    /// with no output.
    pub fn perform(&mut self, revision: u32, root: &str) {
        let ended = self.ended && self.revision == revision;
        self.compile(revision, root);
        self.performing = true;
        if ended {
            self.root = None;
        }
    }

    /// Which of its output definitions the compile running translates.
    fn defs(&self) -> Defs {
        match (self.performing, self.performs) {
            (true, _) => Defs::Midis,
            (false, true) => Defs::All,
            (false, false) => Defs::Layouts,
        }
    }

    /// N8 (MS6 steps 4 and 6): with the check on ([`Engine::set_check_n8`]),
    /// what each translation of a compile of `root` that ran to its end
    /// finds against the same translation of the compile of `root` before
    /// it, its partitions' plan with it, checked against both whole runs;
    /// then what they `kept` is the next compile's. A compile that stopped
    /// for the host keeps nothing. Nothing else reads what they find: a
    /// translation asks what it needs as it runs (D258).
    fn keep_n8(&mut self, root: &str, kept: Vec<Kept>) {
        if self.check_n8 && self.n8_root.as_deref() == Some(root) {
            for (k, (old, new)) in self.n8.iter().zip(&kept).enumerate() {
                let change = lily_engine_music::compare(old, new);
                for wrong in lily_engine_music::verify_all(old, new, &change) {
                    self.n8_wrong
                        .push(format!("translation {}: {wrong}", k + 1));
                }
                self.changes.push(change);
            }
        }
        self.n8 = kept;
        self.n8_root = Some(root.into());
    }

    /// N8's kept translations (D241): those of a compile of `root` that ran
    /// to its end kept for the next compile's to resume, the objects they
    /// made held and their changes since `item` taken out of the undo log
    /// as pictures before the compile is undone; none after a compile
    /// that stopped or ended in an error (`kept` none), which may have
    /// resumed and changed the last ones.
    fn keep_translations(
        &mut self,
        item: Mark,
        item_epoch: u32,
        kept: Option<Vec<Option<Resumable>>>,
        root: (String, Vec<u8>),
        cells: Vec<Option<Value>>,
    ) {
        self.stopped_at = None;
        let heap = self.vm.heap_mut();
        heap.release_held();
        heap.forget_given();
        // Layout's cells for the next compile's (Y7, D248), held as the
        // translations are.
        for c in cells.iter().flatten() {
            heap.hold_between(*c);
        }
        self.cells = cells;
        let kept = kept.unwrap_or_default();
        let generation = heap.generation();
        for r in kept.iter().flatten() {
            r.ended();
            r.forget_old(generation);
        }
        let need = kept.iter().flatten().map(Resumable::generation).min();
        match need {
            Some(need) => {
                heap.keep_pictures(item.heap_len(), item_epoch);
                heap.drop_pictures(need);
                for r in kept.iter().flatten() {
                    heap.hold_between(r.value());
                }
                self.resume_text = Some(root);
            }
            None => {
                heap.drop_pictures(u32::MAX);
                self.resume_text = None;
            }
        }
        self.resumables = kept;
    }

    /// Runs the compile for about `budget_ms`, if it returns ([`Engine::set_returns`];
    /// H3, D254), else to its end. A step of a compile under way begins with
    /// its collection, if one is wanted or under way (B20, D261), and may be
    /// only that. The parse is one step, whatever `budget_ms` is: it is not
    /// split yet. The compile's outputs are what
    /// LilyPond prints to stderr for the file with `--loglevel=WARNING`, as
    /// far as the parser goes, and the same diagnostics as
    /// `diagnostics.json`. [`MORE`] when the time is spent and the compile
    /// goes on at the next step; in a preview's compile that returns, also
    /// as soon as the visible page is written, which the outputs then hold
    /// until the next call, its `.svg` and its `.cells` as the outputs at the
    /// end will hold them (OPEN I7, D269), and are otherwise empty. [`NEEDS`] when the host must be asked for a
    /// file first, the root or one the parse reached; after a stop, for each
    /// file it asked for that the host has not put, with no parse until it
    /// has put them all.
    pub fn step(&mut self, budget_ms: f64) -> u32 {
        // A traced compile holds what it dumps in frames no return can leave.
        let deadline = match self.returns && !self.trace {
            true => now() + budget_ms,
            false => f64::INFINITY,
        };
        returns::begin(deadline);
        let returning = deadline.is_finite();
        // Near the cap a compile collects as a safe point comes, as it did:
        // what it fits by is less than a step leaves to collect (T29 × 8),
        // and its steps are too long for a pause to matter.
        let room = !returns::short_by(DEFERRING_ROOM);
        self.vm.heap_mut().set_deferring(returning && room);
        if let Some(running) = self.running.take() {
            // A visible page handed over was the host's until this call (D269).
            self.outputs.clear();
            // A compile between its steps collects there, in steps of its own
            // (B20, D261): nothing runs on the heap meanwhile.
            let collects = returning && (room || self.vm.heap().collecting());
            if collects && !self.collected_by(deadline, deadline - budget_ms) {
                self.running = Some(running);
                return MORE;
            }
            lily_engine_layout::queue::pause();
            return self.go_on(running);
        }
        // A parse under way goes on from the boundary it stopped at (D267).
        if let Some(parsing) = self.parsing.take() {
            return self.parse_on(parsing);
        }
        let Some(root) = self.root.take() else {
            return DONE;
        };
        let next = {
            let files = self.files.borrow();
            let mut asked = self.asked.borrow_mut();
            asked.retain(|name| !files.contains_key(name));
            asked.first().cloned()
        };
        if let Some(name) = next {
            self.needs = Some(need_of(&name));
            self.root = Some(root);
            return NEEDS;
        }
        let (modules, unfetched) = (&self.modules, &self.unfetched);
        self.packs
            .retain(|name| !modules.contains_key(name) && !unfetched.contains(name));
        if let Some(name) = self.packs.first().cloned() {
            self.needs = Some((MODULE, name));
            self.root = Some(root);
            return NEEDS;
        }
        let mut list = List::default();
        if !self.performing {
            self.ended = false;
        }
        let text = self.files.borrow().get(&root).cloned();
        let text = match text {
            None => {
                self.needs = Some((FILE, root.clone()));
                self.root = Some(root);
                return NEEDS;
            }
            Some(None) => {
                list.push(Message::missing_file(&root));
                list.push(Message::failed_files(&[&root]));
                return self.close(&root, list, Vec::new());
            }
            Some(Some(text)) => text,
        };
        // N8 (D244): the edit since the text the kept translations were made
        // from, if they were of this root, and the epoch the old parse's
        // objects are older than.
        // An epoch of its own for the parse where there are translations to
        // move to its text, as only then is it needed: a new epoch retires the
        // free slots, which a large score's parse then leaves for new chunks
        // (T29 × 8 over the cap).
        // A performance resumes none (D264), nor does a compile of other
        // definitions than those they were made with.
        let same_defs = self.kept_defs == self.defs();
        let kept = matches!(&self.resume_text, Some((r, _)) if *r == root)
            && self.resumables.iter().any(Option::is_some)
            && same_defs;
        let parse_epoch = match kept {
            true => self.vm.heap_mut().begin_epoch(),
            false => self.vm.heap().epoch(),
        };
        let moved = match &self.resume_text {
            Some((_, old)) if kept => {
                let before = self.stopped_at.unwrap_or(parse_epoch);
                Moved::new(&root, old, &text).map(|m| (m, before))
            }
            _ => None,
        };
        // An edit in more places than its lines can be followed in leaves the
        // places of the music between them unknown (OPEN B31, D268).
        let resumes = same_defs
            && self.resume_text.as_ref().is_some_and(|(r, _)| *r == root)
            && moved.as_ref().is_none_or(|(m, _)| m.exact());
        let name = self.vm.heap_mut().string(output_name(&root));
        set_list(&mut self.vm, "lily", "%output-name", name);
        font::take_wanted(&mut self.vm);
        self.parses += 1;
        self.times = Times::default();
        let parsing = Box::new(Parsing {
            root,
            text,
            moved,
            parse_epoch,
            resumes,
            list,
            parse_ms: 0.0,
            parsing: None,
        });
        self.parse_on(parsing)
    }

    /// The parse of a compile, begun or gone on with (H3, D267): in a compile
    /// that returns it stops at the first boundary between its segments after
    /// the step's time is spent, [`MORE`], and goes on at the next step; at
    /// its end the compile goes on.
    fn parse_on(&mut self, mut p: Box<Parsing>) -> u32 {
        let (files, asked) = (self.files.clone(), self.asked.clone());
        let start = now();
        // As the step's deadline: a compile that does not return never stops.
        let returns = self.returns && !self.trace;
        let parsed = match p.parsing.take() {
            None => lily_engine_parser::parse_again(
                &p.root,
                p.text.clone(),
                &self.names,
                &mut self.vm,
                &mut |name| hosted(&files, &asked, name),
                &mut self.reuse,
                returns,
            ),
            Some(parsing) => lily_engine_parser::parse_on(
                parsing,
                &self.names,
                &mut self.vm,
                &mut |name| hosted(&files, &asked, name),
                &mut self.reuse,
            ),
        };
        p.parse_ms += now() - start;
        let parse = match parsed {
            ParseStep::Done(parse) => *parse,
            ParseStep::Paused(parsing) => {
                p.parsing = Some(parsing);
                self.parsing = Some(p);
                return MORE;
            }
        };
        let Parsing {
            root,
            text,
            moved,
            parse_epoch,
            resumes,
            mut list,
            parse_ms,
            ..
        } = *p;
        self.times = Times {
            parse_ms,
            ..Times::default()
        };
        let item = self.vm.begin_item();
        let item_epoch = self.vm.heap().epoch();
        let waiting = self.vm.take_waiting() | parse.waiting;
        let needed = self.asked.borrow().first().cloned();
        if let (true, Some(needed)) = (waiting, needed) {
            take_written(&mut self.vm);
            let root_kept = (root.clone(), Vec::new());
            self.keep_translations(item, item_epoch, None, root_kept, Vec::new());
            undo(&mut self.vm, item, &mut self.reuse);
            lily_engine_parser::end_session(&mut self.vm);
            self.vm.take_output();
            self.vm.take_error_port();
            self.needs = Some(need_of(&needed));
            self.root = Some(root);
            return NEEDS;
        }
        self.asked.borrow_mut().clear();
        // The old parse's places moved to the new text: those the
        // kept translations hold, which a resume keeps.
        // What a stop puts back, each place's value and where it was.
        let mut places = Vec::new();
        if let Some((m, before)) = &moved {
            self.vm.heap_mut().each_older_foreign_mut::<Input>(
                *before,
                &|i| m.moves(i),
                &mut |v, i| {
                    places.push((v, Moved::where_is(i)));
                    m.input(i);
                },
            );
        }
        let (mut messages, expected) = parse.report();
        let failed = messages.pop_if(|m| m.severity == Severity::Fatal);
        // A performance says only what its translations say: the parse's
        // messages and `failed files:` are its page's compile's (D264).
        if !self.performing {
            list.extend(messages);
        }
        // What the writer kept is another score's: nothing it shows.
        if self.markup_root.as_ref() != Some(&root) {
            self.markup = Markup::default();
            self.markup_root = Some(root.clone());
        }
        // A performance writes no page: what the writer kept is the next page's.
        if !self.performing {
            self.markup.begin();
        }
        let work = Work::new(&mut self.vm, &parse, self.trace && !self.performing);
        self.go_on(Box::new(Running {
            root,
            text,
            moved,
            places,
            parse_epoch,
            resumes,
            parse,
            item,
            item_epoch,
            list,
            expected,
            failed,
            work,
        }))
    }

    /// Collects if a collection is wanted or under way, until `deadline`:
    /// whether the step, which began at `started`, has time left for the
    /// compile. The test's mode collects whole and gives the step its time
    /// again, so that a compile goes on between as it would.
    fn collected_by(&mut self, deadline: f64, started: f64) -> bool {
        if self.collect_every_step {
            self.vm.collect();
            returns::begin(now() + (deadline - started));
            return true;
        }
        let heap = self.vm.heap();
        if !heap.collecting() {
            if !heap.collection_wanted() {
                return true;
            }
            self.vm.begin_collecting();
        }
        self.vm.go_on_collecting(deadline) && now() < deadline
    }

    /// The compile under way runs on: [`MORE`] if the step's time is spent,
    /// else it ends, or stops for the host.
    fn go_on(&mut self, mut running: Box<Running>) -> u32 {
        let r = &mut *running;
        let (defs, performing) = (self.defs(), self.performing);
        let traced = self.trace && !performing;
        let on = {
            let (modules, unfetched) = (&self.modules, &self.unfetched);
            let to_ask = |pack: &str| !modules.contains_key(pack) && !unfetched.contains(pack);
            r.work.on(
                &mut self.vm,
                &mut Cx {
                    parse: &r.parse,
                    root_name: output_name(&r.root),
                    traced,
                    playback: self.playback,
                    defs,
                    returning: self.returns && !self.trace,
                    times: &mut self.times,
                    expected: &mut r.expected,
                    asked: &self.asked,
                    to_ask: &to_ask,
                    old: if r.resumes { &self.resumables } else { &[] },
                    moved: r.moved.as_ref().map(|(m, _)| m),
                    old_cells: (self.use_cells && !performing).then_some(&self.cells),
                    markup: (self.preview && !performing).then_some(&mut self.markup),
                    visible: self.visible_page as usize,
                    pdf: self.pdf && !performing,
                },
            )
        };
        if self.vm.take_over_memory() || !on && self.vm.short_of_memory() {
            return self.give_up(running);
        }
        if !on && font::wants_outlines(&mut self.vm) {
            let (modules, unfetched) = (&self.modules, &self.unfetched);
            let packs: Vec<String> = font::wanted(&mut self.vm)
                .iter()
                .flat_map(|name| font::pack_modules(name))
                .filter(|name| !modules.contains_key(name) && !unfetched.contains(name))
                .collect();
            if !packs.is_empty() {
                return self.stop_for_packs(running, packs);
            }
        }
        if !on {
            // The visible page written: the host's until the next call (D269).
            self.outputs = core::mem::take(&mut running.work.handed);
            self.running = Some(running);
            return MORE;
        }
        let Running {
            root,
            text,
            parse,
            item,
            item_epoch,
            mut list,
            mut expected,
            failed,
            work,
            ..
        } = *running;
        let mut done = work.finish(
            &mut self.vm,
            &parse,
            traced,
            &mut self.times,
            &mut expected,
            &self.asked,
        );
        // A part's files its grobs named (A46, D228), which the host
        // is asked for before it is compiled again, as a parse's.
        let needed = self.asked.borrow().first().cloned();
        if let (true, Some(needed)) = (self.vm.take_waiting(), needed) {
            self.stop(&root, item, item_epoch);
            self.needs = Some(need_of(&needed));
            self.root = Some(root);
            return NEEDS;
        }
        // The packs of the fonts it lacked, which the host is asked
        // for before it is compiled again; else what of them the
        // host could not give.
        let wanted = font::take_wanted(&mut self.vm);
        let (modules, unfetched) = (&self.modules, &self.unfetched);
        let packs: Vec<String> = wanted
            .iter()
            .flat_map(|name| font::pack_modules(name))
            .filter(|name| !modules.contains_key(name) && !unfetched.contains(name))
            .collect();
        if let Some(name) = packs.first().cloned() {
            self.stop(&root, item, item_epoch);
            self.packs = packs;
            self.needs = Some((MODULE, name));
            self.root = Some(root);
            return NEEDS;
        }
        list.extend(done.said);
        for name in &wanted {
            let modules = font::pack_modules(name);
            if let Some(pack) = modules.iter().find(|m| self.unfetched.contains(*m)) {
                match name.strip_suffix(".outlines") {
                    Some(face) => list.push(diag::unfetched_outlines(face, pack)),
                    None => list.push(diag::unfetched(name, pack)),
                }
            }
        }
        // `ly:check-expected-warnings` after the file, before the
        // run's `failed files:` (D96), neither of which a run ended
        // by a translation's error reaches (D132); a performance's
        // are its page's compile's, as the `\layout`s' messages are.
        // `init.ly`'s `expect-error` (P12), after its books: an error the
        // file expects clears its failure (`ly:parser-clear-error`); one it
        // expects and has not is an error of its own.
        let mut failed = failed;
        if !done.ended && !performing {
            let expects = self
                .vm
                .module_value(parse.module, "expect-error")
                .unwrap_or(Value::FALSE);
            let has = parse.failed();
            if expects == Value::bool(has) {
                failed = None;
            } else if expects != Value::FALSE {
                list.push(diag::expected_none());
                failed = Some(Message::failed_files(&[&root]));
            }
        }
        if !expected.is_empty() && !done.ended && !performing {
            list.push(Message::unmet(&expected));
        }
        if !done.ended && !performing {
            list.extend(failed);
        }
        if !performing {
            self.ended = done.ended;
            self.keep_n8(&root, core::mem::take(&mut done.kept));
            if self.check_n8 {
                self.n8_wrong.append(&mut done.resumed);
            }
        }
        if traced {
            self.outputs
                .push(("trace.tsv".into(), done.trace.into_bytes()));
            self.outputs
                .push(("trace.log".into(), done.log.into_bytes()));
            self.outputs
                .push(("grobs.tsv".into(), done.grobs.into_bytes()));
            self.outputs
                .push(("layout.tsv".into(), done.layout.into_bytes()));
            self.outputs
                .push(("columns.tsv".into(), done.columns.into_bytes()));
            self.outputs
                .push(("skylines.tsv".into(), done.skylines.into_bytes()));
            self.outputs
                .push(("line.tsv".into(), done.line.into_bytes()));
            self.outputs.push(("c5.tsv".into(), done.c5.into_bytes()));
        }
        // The files the translations made, after the two outputs of every
        // compile.
        let mut made: Vec<(String, Vec<u8>)> = Vec::new();
        // Each book's pages, named as LilyPond's SVG backend names
        // them: the first book's after the root, the others' after
        // it and their count (`get-outfile-name`), each page's
        // after its book and its number unless it is the book's
        // only page (`output-stencils`). A name met again is the
        // later page's, as LilyPond writes the file over. A page a
        // preview shows may also be sent by its cells (W4, D253):
        // `<page>.cells` beside it, with what `Markup::page` says.
        let mut written = 0;
        for (k, (first, pages)) in done.pages.into_iter().enumerate() {
            let base = match k {
                0 => output_name(&root).to_string(),
                k => format!("{}-{k}", output_name(&root)),
            };
            let count = pages.len();
            for (number, svg) in (first..).zip(pages) {
                let name = match count {
                    1 => format!("{base}.svg"),
                    _ => format!("{base}-{number}.svg"),
                };
                if self.preview {
                    let cells = self.markup.page(written, &name, &svg);
                    written += 1;
                    let page = name.trim_end_matches(".svg");
                    let file = format!("{page}.cells");
                    made.retain(|(n, _)| *n != file);
                    made.extend(cells.map(|cells| (file, cells)));
                }
                match made.iter_mut().find(|(n, _)| *n == name) {
                    Some(output) => output.1 = svg.into_bytes(),
                    None => made.push((name, svg.into_bytes())),
                }
            }
        }
        // Each book's PDF display list, named as LilyPond names the book's PDF.
        for (k, list) in done.lists {
            let base = match k {
                0 => output_name(&root).to_string(),
                k => format!("{}-{k}", output_name(&root)),
            };
            made.push((format!("{base}.pdf.list"), list));
        }
        if !performing {
            self.markup.end(self.visible_page as usize);
        }
        // Each book's MIDI files, as `write-performances-midis`
        // names them, before the playback map.
        for (name, bytes) in done.midis {
            match made.iter_mut().find(|(n, _)| *n == name) {
                Some(output) => output.1 = bytes,
                None => made.push((name, bytes)),
            }
        }
        // No playback map of a run a translation's error ended, as
        // LilyPond's writes no MIDI file then (D132); `translate`
        // left none.
        if !done.performances.is_empty() {
            let name = format!("{}.timing.json", output_name(&root));
            // `timing.ly` writes the file again after each performance,
            // its staves' thunks run then: those of every performance but
            // the last find no voices (OPEN C2.3).
            let last = done.performances.len() - 1;
            let maps: Vec<&str> = done
                .performances
                .iter()
                .enumerate()
                .map(|(i, (map, followed))| if i < last { followed } else { map }.as_str())
                .collect();
            let json = format!("[{}]\n", maps.join(","));
            made.push((name, json.into_bytes()));
        }
        // What the score's Scheme wrote, the last of a name, in place
        // of the engine's own file of that name.
        for (name, text) in take_written(&mut self.vm) {
            match made.iter_mut().find(|(n, _)| *n == name) {
                Some(output) => output.1 = text.into_bytes(),
                None => made.push((name, text.into_bytes())),
            }
        }
        if performing {
            self.leave_kept(item);
            return self.close(&root, list, made);
        }
        self.resumed_at = core::mem::take(&mut done.resumed_at);
        self.spliced = core::mem::take(&mut done.spliced);
        self.replayed = core::mem::take(&mut done.replayed);
        self.cell_reports = core::mem::take(&mut done.cell_reports);
        let resumables = core::mem::take(&mut done.resumables);
        let cells = core::mem::take(&mut done.cells);
        self.kept_defs = defs;
        self.keep_translations(
            item,
            item_epoch,
            (!done.ended).then_some(resumables),
            (root.clone(), text),
            cells,
        );
        undo(&mut self.vm, item, &mut self.reuse);
        self.close(&root, list, made)
    }

    /// The compile under way dropped for want of memory (D256, D257): undone
    /// as a stop for a newer one is, which leaves the engine as it was but
    /// for what it kept, and ended with a diagnostic and no pages.
    fn give_up(&mut self, running: Box<Running>) -> u32 {
        let Running {
            root,
            item,
            item_epoch,
            work,
            mut list,
            ..
        } = *running;
        work.drop_it(&mut self.vm);
        lily_engine_layout::unpure::stop_keeping();
        self.vm.take_waiting();
        self.stop(&root, item, item_epoch);
        self.vm.heap_mut().collect_soon();
        if !self.performing {
            self.ended = false;
        }
        list.push(diag::over_memory());
        self.outputs.push(("stderr".into(), list.stderr()));
        self.outputs
            .push(("diagnostics.json".into(), list.json(&root)));
        DONE
    }

    /// A compile in steps whose skylines need a text face's outlines no host
    /// gave (E14, D262): stopped at once, as one that met a font no host gave
    /// stops at its end, so that little is paid twice; it is compiled again
    /// once the host has put `packs`.
    fn stop_for_packs(&mut self, running: Box<Running>, packs: Vec<String>) -> u32 {
        let Running {
            root,
            item,
            item_epoch,
            work,
            ..
        } = *running;
        work.drop_it(&mut self.vm);
        lily_engine_layout::unpure::stop_keeping();
        self.vm.take_waiting();
        self.stop(&root, item, item_epoch);
        self.needs = Some((MODULE, packs[0].clone()));
        self.packs = packs;
        self.root = Some(root);
        NEEDS
    }

    /// What a compile that ran to its end leaves: an error of a
    /// `call-after-session` thunk is no diagnostic of the file's: LilyPond
    /// would end the run after it. Then the fonts the file loaded are
    /// forgotten, as `lilypond-all` forgets them.
    fn close(&mut self, root: &str, list: List, made: Vec<(String, Vec<u8>)>) -> u32 {
        lily_engine_parser::end_session(&mut self.vm);
        let _ = font::reset_all_fonts(&mut self.vm);
        // What Scheme wrote to the standard output port is LilyPond's
        // stdout, which is no output of a compile.
        self.vm.take_output();
        self.outputs.push(("stderr".into(), list.stderr()));
        self.outputs
            .push(("diagnostics.json".into(), list.json(root)));
        self.outputs.extend(made);
        DONE
    }

    /// What a compile that stopped leaves, for the host or for a newer one:
    /// nothing of it is kept, and it is undone.
    fn stop(&mut self, root: &str, item: Mark, item_epoch: u32) {
        take_written(&mut self.vm);
        if self.performing {
            self.leave_kept(item);
        } else {
            let root_kept = (root.to_string(), Vec::new());
            self.keep_translations(item, item_epoch, None, root_kept, Vec::new());
            undo(&mut self.vm, item, &mut self.reuse);
        }
        lily_engine_parser::end_session(&mut self.vm);
        let _ = font::reset_all_fonts(&mut self.vm);
        self.vm.take_output();
        self.vm.take_error_port();
    }

    /// What a performance leaves, ended or stopped (D264): it is undone, and
    /// N8's translations and layout's cells are held as the compile before
    /// it left them, as a stopped compile leaves them (B24, D261). It
    /// resumed none, and its parse was of their text.
    fn leave_kept(&mut self, item: Mark) {
        let heap = self.vm.heap_mut();
        heap.release_held();
        for c in self.cells.iter().flatten() {
            heap.hold_between(*c);
        }
        for r in self.resumables.iter().flatten() {
            heap.hold_between(r.value());
        }
        undo(&mut self.vm, item, &mut self.reuse);
        self.vm.heap_mut().put_given_back();
    }

    /// Drops the compile under way between its steps, if there is one (H3,
    /// D254): a newer request stops it (B8). Nothing of it is kept, and what
    /// the compile before it left stands as it was (B24, D261), of the text it
    /// was made from: typing is a stopped compile after a stopped compile, and
    /// each resumes what the last that ended kept.
    pub(crate) fn stop_running(&mut self) {
        self.stop_parsing();
        let Some(running) = self.running.take() else {
            return;
        };
        self.vm.drop_collecting();
        let Running {
            item,
            work,
            moved,
            places,
            parse_epoch,
            ..
        } = *running;
        work.drop_it(&mut self.vm);
        lily_engine_layout::unpure::stop_keeping();
        // What it parsed is of its text, and what the parser keeps of that is
        // before the next edit: only what is older moves to the next text. A
        // performance's parse is of the kept translations' text.
        if !self.performing {
            self.stopped_at.get_or_insert(parse_epoch);
        }
        take_written(&mut self.vm);
        let heap = self.vm.heap_mut();
        heap.release_held();
        for c in self.cells.iter().flatten() {
            heap.hold_between(*c);
        }
        for r in self.resumables.iter().flatten() {
            heap.hold_between(r.value());
        }
        undo(&mut self.vm, item, &mut self.reuse);
        let heap = self.vm.heap_mut();
        heap.put_given_back();
        // The old parse's places where they were: those still alive and of the
        // old parse, as a slot freed since may hold a newer object.
        let before = moved.map_or(0, |(_, before)| before);
        for (value, place) in places {
            if heap.is_live_foreign(value)
                && heap.epoch_of(value).is_some_and(|e| e < before)
                && let Some(input) = heap.foreign_mut::<Input>(value)
            {
                Moved::put_back(input, place);
            }
        }
        for r in self.resumables.iter().flatten() {
            r.stopped();
        }
        lily_engine_parser::end_session(&mut self.vm);
        let _ = font::reset_all_fonts(&mut self.vm);
        self.vm.take_output();
        self.vm.take_error_port();
    }
}

impl Engine {
    /// Drops the parse under way between its steps, if there is one (D267): a
    /// newer request stops it. What it parsed is left to the next parse as a
    /// parse that ended at the boundary it stopped at, and undone; what the
    /// compile before it left stands as it was, as [`Engine::stop_running`]
    /// leaves it.
    fn stop_parsing(&mut self) {
        let Some(p) = self.parsing.take() else {
            return;
        };
        if let Some(parsing) = p.parsing {
            parsing.stop(&mut self.vm, &mut self.reuse);
        }
        // What it parsed is of its text, as a stopped compile's.
        if !self.performing {
            self.stopped_at.get_or_insert(p.parse_epoch);
        }
        take_written(&mut self.vm);
        self.reuse.undo(&mut self.vm);
        lily_engine_parser::end_session(&mut self.vm);
        let _ = font::reset_all_fonts(&mut self.vm);
        self.vm.take_output();
        self.vm.take_error_port();
    }
}

impl Running {
    /// What [`Stage`] it goes on with.
    pub(crate) fn stage(&self) -> &'static str {
        match self.work.stage {
            Stage::Score | Stage::Def(_) => "scores",
            Stage::Translating(..) => "translating",
            Stage::Ended(..) => "ended",
            Stage::Processing(..) => "processing",
            Stage::Book => "books",
            Stage::Paging(_, ref output) => output.stage(),
            Stage::Writing(..) => "writing",
            Stage::Done => "done",
        }
    }
}

/// A compile's parse between its steps (H3, D267): what `Engine::step` had
/// before the parse, and the parse stopped at a boundary of its segments.
pub(crate) struct Parsing {
    root: String,
    text: Vec<u8>,
    moved: Option<(Moved, u32)>,
    parse_epoch: u32,
    resumes: bool,
    list: List,
    /// The parse's milliseconds so far, its steps'.
    parse_ms: f64,
    parsing: Option<Box<lily_engine_parser::Parsing>>,
}

/// A compile between its steps (H3, D254): what `Engine::step` had in its
/// frames, which a return leaves.
pub(crate) struct Running {
    root: String,
    text: Vec<u8>,
    moved: Option<(Moved, u32)>,
    /// The old parse's places it moved, and where each was (D261).
    places: Vec<(Value, [u32; 5])>,
    /// The epoch its parse began in.
    parse_epoch: u32,
    /// Whether the kept translations are of this root.
    resumes: bool,
    parse: Parse,
    item: Mark,
    item_epoch: u32,
    list: List,
    expected: Vec<String>,
    failed: Option<Message>,
    work: Work,
}

/// What a compile's translations made.
#[derive(Default)]
struct Translations {
    /// What they said, placed, for `stderr`.
    said: Vec<Message>,
    /// The `\layout` translations' trace, if traced, and what they said.
    trace: String,
    log: String,
    /// The grobs of the `\layout` translations, if traced, and their
    /// laid-out grobs.
    grobs: String,
    layout: String,
    /// Their columns' springs and rods (`columns.tsv`), in `layout.ly`'s
    /// other format.
    columns: String,
    /// Their columns' skylines (`skylines.tsv`, D260).
    skylines: String,
    /// Their one line's columns' X offsets (`line.tsv`), in `layout.ly`'s
    /// third format.
    line: String,
    /// Their systems' and grobs' shapes (`c5.tsv`), in `c5dump.ly`'s
    /// format.
    c5: String,
    /// Each performance's playback map, and the map as `timing.ly` writes
    /// it when another follows (`Translated::playback_pair`).
    performances: Vec<(String, String)>,
    /// The MIDI files of the books whose pages were made, by name, in
    /// the order LilyPond writes them.
    midis: Vec<(String, Vec<u8>)>,
    /// Each book's pages written as SVG, with the number of its first:
    /// none for a book that has none or whose layout failed.
    pages: Vec<(i64, Vec<String>)>,
    /// The PDF display lists of the books in `pages`, by their place there.
    lists: Vec<(usize, Vec<u8>)>,
    /// Whether a translation's error ended the run (D132), or one of
    /// `Book::process` or the pages (D220).
    ended: bool,
    /// What N8 kept of each `\layout` and `\midi` translation, in order,
    /// and what the check of their snapshots found.
    kept: Vec<Kept>,
    resumed: Vec<String>,
    /// Each translation kept for the next compile's (D241), and the
    /// snapshot each was resumed from.
    resumables: Vec<Option<Resumable>>,
    resumed_at: Vec<Option<usize>>,
    /// Whether each converged with it and ended as it did (D243).
    spliced: Vec<bool>,
    /// What each of them replayed (MS6 step 7c).
    replayed: Vec<lily_engine_music::Replayed>,
    /// Each translation's layout cells kept for the next compile's (Y7,
    /// D248), and what each `\layout` translation's cells did.
    cells: Vec<Option<Value>>,
    cell_reports: Vec<lily_engine_layout::cell::Report>,
}

/// Ends the compile's item and undoes it, then the parse's segments, the
/// latest first, keeping what they did for the next parse (S5, R11; D235):
/// every object and global older than the compile as it was before it,
/// whether the compile ended or stopped for the host, so that the next
/// starts where a fresh engine's would (OPEN C7) and a score's
/// `ly:set-option` ends with it (C2.2). What the log does not reach, a part
/// a stub loaded (D74) and the fonts, `session-terminate` and
/// `ly:reset-all-fonts` reset after it.
fn undo(vm: &mut Vm, item: Mark, reuse: &mut Reuse) {
    vm.end_item(item);
    vm.undo(item);
    reuse.undo(vm);
}

/// The name LilyPond's output files take from `root`: its base name
/// without `.ly` (`ly:parser-output-name`).
fn output_name(root: &str) -> &str {
    let base = root.rsplit(['/', '\\']).next().unwrap_or(root);
    base.strip_suffix(".ly").unwrap_or(base)
}

/// The kind of the output definition `def`: `layout` or `midi`.
fn kind(vm: &mut Vm, def: Value) -> Option<&str> {
    let scope = vm.heap().foreign::<OutputDef>(def)?.scope;
    let kind = vm.module_value(scope, "output-def-kind")?;
    vm.heap().name(kind)
}

/// The classes a `\layout` translation runs with (D108): layout's, then
/// the tablature's (D223, since MS4 step 7a) and the figured bass's
/// (since step 16). A `\score` in a markup is translated with them too
/// ([`host_score_embedded_format`], A47).
fn layout_classes() -> Vec<&'static TranslatorClass> {
    let mut classes = lily_engine_layout::CLASSES.to_vec();
    classes.extend_from_slice(lily_engine_tablature::CLASSES);
    classes.extend_from_slice(lily_engine_figured_bass::CLASSES);
    classes
}

/// What `%host-call` runs for `ly:score-embedded-format` in the core,
/// over layout's handler (`crate::HOSTS`): the score translated with
/// [`layout_classes`] (A47, MS4 step 17).
pub(crate) fn host_score_embedded_format(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    lily_engine_layout::paper_score::score_embedded_format(vm, args, &layout_classes())
}

/// [`Work`]: `parse`'s scores translated: each `\layout`, with its trace if
/// `traced`, and each `\midi`, with `capture.ly`'s `\midi` for a score
/// without one, its performers writing the playback map if `playback`.
/// Each translation is timed into `times`, and apart (MS3 step 36) its
/// layout and breaking, the pages' stencils and their writing, and the
/// live grobs counted after the pages; the linear memory is read after
/// the translations and after the pages (MS4 step 6). What they say meets the
/// warnings `expected`, which are not printed, and those left stay in it.
/// The files `layout.tsv`'s reads ask for are taken out of `asked` again.
/// A translation whose grobs need a pack still `to_ask` for, one the host
/// has neither given nor said it cannot give, ends the run before its
/// layout (W5). The parse's module is the current one throughout, as
/// LilyPond's books are processed by `init.ly`'s last Scheme while the
/// parser runs: `ly:parser-lookup` finds the file's `$defaultpaper` from a
/// markup (MS3 step 54, D212).
///
/// Between its steps nothing else runs on the VM, so what is rooted stays
/// rooted, the parse's module stays current, and its state is in the struct
/// (H3, D254).
struct Work {
    depth: usize,
    previous: Value,
    collected: lily_engine_scheme::Stats,
    /// `$defaultlayout`, `$defaultmidi`.
    layout: Value,
    midi: Value,
    trace: Trace,
    out: Translations,
    /// Each `\\layout` translation's number and Paper_score, whose cells are
    /// kept after the pages (Y7).
    with_cells: Vec<(usize, Value)>,
    /// The index of the book `init.ly` makes, after the `\\book` blocks'.
    made_at: usize,
    scores: std::vec::IntoIter<(Value, Value)>,
    /// Each book's `Paper_book`, rooted, and the books whose layout failed.
    printed: Vec<Option<Value>>,
    failed: Vec<Value>,
    /// Each `\\layout`'s Paper_score, whose live grobs are counted after the
    /// pages.
    pscores: Vec<Value>,
    pending: Vec<lily_engine_layout::dump::Pending>,
    /// The book whose `Book::process` LilyPond's own error ended, and the
    /// error (D220): no item of it or of the books after it is translated.
    process_error: Option<(usize, String)>,
    /// The books before the one whose translation's error ended the run,
    /// all if none did.
    ended_at: usize,
    /// How much had been said when each book's last translation ended, and
    /// how many playback maps there were.
    said_by_book: Vec<usize>,
    maps_by_book: Vec<usize>,
    /// The books whose pages were made, and the books left.
    made: Vec<Option<Value>>,
    books: std::iter::Enumerate<std::vec::IntoIter<Option<Value>>>,
    /// What the pages said, and the book whose pages LilyPond's own error
    /// ended, with the error (D220).
    pages_said: Vec<Message>,
    page_error: Option<(usize, String)>,
    /// The visible page, written, handed over before the compile's end (OPEN
    /// I7, D269): its `.svg` and its `.cells`, as the compile's outputs will
    /// hold them, until the step that wrote it returns.
    handed: Vec<(String, Vec<u8>)>,
    stage: Stage,
}

/// What a [`Work`] is doing.
enum Stage {
    /// The next score.
    Score,
    /// The next of a score's output definitions.
    Def(Box<Scoring>),
    /// A translation.
    Translating(Box<Scoring>, Def, Box<Going>),
    /// A translation over: what follows from it (B23, D261).
    Ended(Box<Scoring>, Def, Box<Translated>),
    /// A `\\layout` translation's Paper_score processed.
    Processing(Box<Scoring>, Def, Box<Translated>, Box<Processing>, Laying),
    /// The translations are done: the next book's pages.
    Book,
    /// A book's pages made, then written.
    Paging(Box<Paging>, Output),
    Writing(Box<Paging>, Writing),
    Done,
}

/// A score being translated.
struct Scoring {
    book: Value,
    paper: Value,
    header: Value,
    music: Value,
    defs: std::vec::IntoIter<Value>,
}

/// One of a score's output definitions being translated.
struct Def {
    layout: bool,
    /// How much of the trace's grobs had been heard before it.
    heard: usize,
    /// The milliseconds it has taken.
    ms: f64,
    log: String,
}

/// A layout under way: the host's stops as they stood before it, and the
/// root stack.
struct Laying {
    waiting: bool,
    asking: usize,
    depth: usize,
}

/// A book's pages under way.
struct Paging {
    k: usize,
    waiting: bool,
    asking: usize,
    warnings: Vec<String>,
}

/// A book's pages being written.
struct Writing {
    first: i64,
    stencils: std::vec::IntoIter<lily_engine_layout::Stencil>,
    output_scale: f64,
    pages: Vec<String>,
    mark: Option<usize>,
    /// The book's PDF display list, its pages walked as they are written.
    pdf: Option<lily_engine_pdf::stencil::Book>,
}

/// Which output definitions a compile translates (M7, D264): all, as
/// LilyPond does; a page's `\layout`s alone ([`Engine::set_performs`]); or a
/// performance's `\midi`s alone, which makes no page ([`Engine::perform`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Defs {
    All,
    Layouts,
    Midis,
}

/// What a [`Work`] is given at each step.
struct Cx<'a> {
    parse: &'a Parse,
    root_name: &'a str,
    traced: bool,
    playback: bool,
    defs: Defs,
    /// Whether it may return between its steps.
    returning: bool,
    times: &'a mut Times,
    expected: &'a mut Vec<String>,
    asked: &'a Asked,
    to_ask: &'a dyn Fn(&str) -> bool,
    old: &'a [Option<Resumable>],
    moved: Option<&'a Moved>,
    old_cells: Option<&'a Vec<Option<Value>>>,
    markup: Option<&'a mut Markup>,
    /// The page the host shows, counted over the books' pages from 0, which
    /// a preview's compile in steps hands over once it is written (I7, D269).
    visible: usize,
    /// Whether each book's PDF display list is made (W7, D270).
    pdf: bool,
}

impl Work {
    fn new(vm: &mut Vm, parse: &Parse, traced: bool) -> Work {
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(parse.module);
        let previous = vm.set_current_module(parse.module);
        // 05 §7's steps (D251), timed from here, layout's alone.
        lily_engine_layout::queue::set_clock(now);
        lily_engine_layout::queue::begin(vm.heap_mut());
        let collected = vm.heap().stats();
        let [paper, layout, midi, header] = [
            "$defaultpaper",
            "$defaultlayout",
            "$defaultmidi",
            "$defaultheader",
        ]
        .map(|name| vm.module_value(parse.module, name).unwrap_or(Value::FALSE));
        let mut out = Translations::default();
        // The `\\book` blocks' books, then the book `init.ly` makes of the
        // toplevel parts and scores, each processed into its `Paper_book`
        // (`Book::process`), with each item and the `Paper_book` it goes to,
        // whose paper `Score::book_rendering` scales its `\\layout`s by and
        // parents them to.
        let mut books = parse.toplevel_books(vm);
        let made_at = books.len();
        let (parts, toplevel) = (parse.toplevel_bookparts(vm), parse.toplevel_scores(vm));
        if let Some(book) = toplevel_book(vm, paper, header, &parts, &toplevel) {
            vm.heap_mut().root(book);
            books.push(book);
        }
        let mut scores = Vec::new();
        let mut printed = Vec::new();
        let mut process_error = None;
        for (k, book) in books.into_iter().enumerate() {
            let before = scores.len();
            match paper_book::process(vm, book, paper, &mut scores) {
                Ok(made) => {
                    if let Some(made) = made {
                        vm.heap_mut().root(made);
                    }
                    printed.push(made);
                }
                Err(e) => {
                    printed.push(None);
                    if traced {
                        out.log.push_str(&format!("error: {}\n", e.text(vm.heap())));
                    }
                    if !engines_own(vm, &e) {
                        scores.truncate(before);
                        process_error = Some((k, e.text(vm.heap())));
                        break;
                    }
                }
            }
        }
        // Each `Paper_book` rooted, a part's too where its book's failed.
        for &(_, book) in &scores {
            vm.heap_mut().root(book);
        }
        let ended_at = printed.len();
        let (said_by_book, maps_by_book) =
            (vec![out.said.len(); printed.len()], vec![0; printed.len()]);
        Work {
            depth,
            previous,
            collected,
            layout,
            midi,
            trace: Trace::default(),
            out,
            with_cells: Vec::new(),
            made_at,
            scores: scores.into_iter(),
            printed,
            failed: Vec::new(),
            pscores: Vec::new(),
            pending: Vec::new(),
            process_error,
            ended_at,
            said_by_book,
            maps_by_book,
            made: Vec::new(),
            books: Vec::new().into_iter().enumerate(),
            pages_said: Vec::new(),
            page_error: None,
            handed: Vec::new(),
            stage: Stage::Score,
        }
    }

    /// Goes on; whether it has done all but [`Work::finish`]. False when the
    /// step's time is spent, between timesteps, grobs, lines, systems or
    /// pages.
    fn on(&mut self, vm: &mut Vm, c: &mut Cx) -> bool {
        loop {
            match core::mem::replace(&mut self.stage, Stage::Done) {
                Stage::Score => self.next_score(vm, c),
                Stage::Def(score) => self.next_def(vm, c, score),
                Stage::Translating(score, def, going) => self.translating(vm, c, score, def, going),
                Stage::Ended(score, def, done) => self.ran(vm, c, score, def, Ok(Some(*done))),
                Stage::Processing(score, def, done, processing, laying) => {
                    self.processing(vm, c, score, def, done, processing, laying);
                }
                Stage::Book => self.next_book(vm, c),
                Stage::Paging(paging, output) => self.paging(vm, c, paging, output),
                Stage::Writing(paging, writing) => self.writing(vm, c, paging, writing),
                Stage::Done => return true,
            }
            if matches!(self.stage, Stage::Done) {
                return true;
            }
            // The visible page is handed over at once (D269).
            if c.returning && !self.handed.is_empty() {
                return false;
            }
            // A compile that is to stop for want of memory stops now, whatever
            // time its step has left: it would only meet its check again.
            if c.returning && (vm.heap().is_over_memory() || returns::due()) {
                return false;
            }
        }
    }

    /// The next score, its output definitions.
    fn next_score(&mut self, vm: &mut Vm, c: &mut Cx) {
        let Some((score, book)) = self.scores.next() else {
            self.translated(vm, c);
            return;
        };
        self.stage = Stage::Score;
        // `process_score`'s text or page marker, printed as it is.
        if vm.heap().foreign::<Score>(score).is_none() {
            paper_book::add_score(vm.heap_mut(), book, score);
            return;
        }
        let Some(paper) = vm
            .heap()
            .foreign::<paper_book::PaperBook>(book)
            .map(|b| b.paper)
        else {
            return;
        };
        let header = vm
            .heap()
            .foreign::<Score>(score)
            .map_or(Value::FALSE, |s| s.header);
        let Some((music, mut defs)) = vm
            .heap()
            .foreign::<Score>(score)
            .filter(|s| !s.error)
            .map(|s| (s.music, s.defs.clone()))
        else {
            return;
        };
        if !defs.iter().any(|&d| kind(vm, d) == Some("midi")) && self.midi != Value::FALSE {
            if defs.is_empty() {
                defs.push(self.layout);
            }
            defs.push(self.midi);
        }
        self.stage = Stage::Def(Box::new(Scoring {
            book,
            paper,
            header,
            music,
            defs: defs.into_iter(),
        }));
    }

    /// The next output definition of `score`, its translation begun.
    fn next_def(&mut self, vm: &mut Vm, c: &mut Cx, mut score: Box<Scoring>) {
        let Some(def) = score.defs.next() else {
            self.stage = Stage::Score;
            return;
        };
        let start = now();
        let layout = kind(vm, def) == Some("layout");
        // A page's compile leaves the `\midi`s to its performance, which
        // leaves the `\layout`s (D264).
        let left = match c.defs {
            Defs::All => false,
            Defs::Layouts => !layout,
            Defs::Midis => layout,
        };
        if left {
            self.stage = Stage::Def(score);
            return;
        }
        let heard = self.trace.grobs.len();
        // The last compile's translation at its place (N8, D241).
        let resumable = c
            .old
            .get(self.out.kept.len())
            .and_then(Option::as_ref)
            .map(|r| (r, c.moved));
        let (music, paper) = (score.music, score.paper);
        let begun = match kind(vm, def) {
            Some("layout") => scaled_layout(vm, def, paper).and_then(|def| {
                vm.heap_mut().root(def);
                let trace = c.traced.then_some(&mut self.trace);
                let classes = layout_classes();
                start_resuming(
                    vm,
                    music,
                    def,
                    trace,
                    &classes,
                    false,
                    resumable,
                    c.returning,
                )
            }),
            Some("midi") => start_resuming(
                vm,
                music,
                def,
                None,
                &[],
                c.playback,
                resumable,
                c.returning,
            ),
            _ => {
                self.stage = Stage::Def(score);
                return;
            }
        };
        let def = Def {
            layout,
            heard,
            ms: now() - start,
            log: String::new(),
        };
        match begun {
            Ok(Some(going)) => self.stage = Stage::Translating(score, def, Box::new(going)),
            Ok(None) => self.def_ended(vm, c, score, def, None),
            Err(e) => self.ran(vm, c, score, def, Err(e)),
        }
    }

    /// A translation runs on.
    fn translating(
        &mut self,
        vm: &mut Vm,
        c: &mut Cx,
        score: Box<Scoring>,
        mut def: Def,
        mut going: Box<Going>,
    ) {
        let start = now();
        let ended = going.on(vm);
        def.ms += now() - start;
        if !ended {
            self.stage = Stage::Translating(score, def, going);
            return;
        }
        let start = now();
        let trace = (c.traced && def.layout).then_some(&mut self.trace);
        let ran = going.end(vm, trace);
        def.ms += now() - start;
        match ran {
            // Between the steps it must be rooted, which `Going` held.
            Ok(Some(done)) => {
                vm.heap_mut().root(done.translation.value);
                self.stage = Stage::Ended(score, def, Box::new(done));
            }
            ran => self.ran(vm, c, score, def, ran),
        }
    }

    /// What a translation ran: its parts of the book, and its layout begun.
    fn ran(
        &mut self,
        vm: &mut Vm,
        c: &mut Cx,
        score: Box<Scoring>,
        mut def: Def,
        ran: Result<Option<Translated>, Exception>,
    ) {
        let start = now();
        let (book, header) = (score.book, score.header);
        let mut ended = None;
        match ran {
            Ok(Some(mut done)) => {
                // What follows, to layout's first step, is all one step, and
                // what it allocates goes by the grobs (`cell::begin`).
                if def.layout && returns::short_by(done.grobs() * GROB_START) {
                    vm.over_memory();
                    return;
                }
                // Held from here: nothing else keeps its object alive
                // till the compile ends (D241).
                let resumable = done.resumable();
                if let Some(r) = &resumable {
                    vm.heap_mut().hold_between(r.value());
                }
                let out = &mut self.out;
                out.resumables.push(resumable);
                out.resumed_at.push(done.resumed_at);
                out.spliced.push(done.spliced);
                out.replayed.push(done.replayed);
                out.kept.push(done.n8.take().unwrap_or_default());
                let k = out.kept.len();
                out.resumed.extend(
                    core::mem::take(&mut done.resumed)
                        .into_iter()
                        .map(|w| format!("translation {k}: {w}")),
                );
                for name in &done.unported {
                    def.log.push_str(&format!("unported: {name}\n"));
                }
                if def.layout {
                    // `process_score`'s Paper_score, after its score's
                    // header.
                    if let Some(pscore) =
                        lily_engine_layout::paper_score::of_translation(&done.translation)
                    {
                        if vm.heap().foreign_object(header).map(|f| f.type_name()) == Some("module")
                        {
                            paper_book::add_score(vm.heap_mut(), book, header);
                        }
                        paper_book::add_score(vm.heap_mut(), book, pscore);
                        vm.heap_mut().root(pscore);
                        self.pscores.push(pscore);
                        // The parts its grobs' definitions name, loaded
                        // before its layout; one the host must be asked
                        // for stops it as a parse's file does (A46,
                        // D228).
                        if load_parts(vm, &done.translation) {
                            said(vm, c.parse, c.expected, &[]);
                            self.ended_at = 0;
                            self.translated(vm, c);
                            return;
                        }
                        // The fonts its grobs would select, which the
                        // host is asked for before anything is laid out.
                        lily_engine_layout::font_select::want_fonts(vm, pscore);
                        let wanted = font::wanted(vm);
                        let packs = wanted.iter().flat_map(|name| font::pack_modules(name));
                        if packs.into_iter().any(|p| (c.to_ask)(&p)) {
                            // What it said is said again when it is
                            // compiled again.
                            said(vm, c.parse, c.expected, &[]);
                            self.ended_at = 0;
                            self.translated(vm, c);
                            return;
                        }
                        if let (Some(old), Some(kept)) = (c.old_cells, self.out.kept.last()) {
                            let k = self.out.kept.len() - 1;
                            let cells = old.get(k).copied().flatten();
                            lily_engine_layout::cell::begin(vm, pscore, kept, cells);
                            self.with_cells.push((k, pscore));
                        }
                    }
                    let laying = Laying {
                        waiting: vm.waiting(),
                        asking: c.asked.borrow().len(),
                        depth: vm.heap().root_depth(),
                    };
                    lily_engine_layout::queue::pause();
                    def.ms += now() - start;
                    if c.traced {
                        let laying_at = now();
                        use lily_engine_layout::paper_score::{lend_trace, return_trace};
                        let announced = String::from(&self.trace.grobs[def.heard..]);
                        // The scores its markups print are recorded
                        // on it (`ly:score-embedded-format`).
                        lend_trace(vm, core::mem::take(&mut self.trace));
                        let depth = vm.heap().root_depth();
                        let dumped =
                            lily_engine_layout::dump::lay_out(vm, &done.translation, &announced);
                        // What its processing threw ends the run as the
                        // untraced compile's does (B33).
                        let ended = dumped
                            .thrown
                            .filter(|e| !engines_own(vm, e))
                            .map(|e| e.text(vm.heap()));
                        vm.heap_mut().unroot_to(depth);
                        self.trace = return_trace(vm);
                        for e in &dumped.errors {
                            def.log.push_str(&format!("error: {e}\n"));
                        }
                        self.out.columns.push_str(&dumped.columns);
                        self.out.skylines.push_str(&dumped.skylines);
                        let laid = dumped.errors.is_empty() && dumped.processed;
                        // layout.tsv and line.tsv once the pages are
                        // made, which break the lines.
                        self.pending.extend(dumped.pending);
                        // Its reads' errors are swallowed: laid out
                        // only if processing completed, as `lay_out`.
                        let ms = now() - laying_at;
                        c.times.lay_out_ms += ms;
                        def.ms += ms;
                        self.laid(vm, c, score, def, Box::new(done), laid, laying, ended);
                    } else {
                        vm.heap_mut().root(done.translation.value);
                        self.stage =
                            Stage::Processing(score, def, Box::new(done), Box::default(), laying);
                    }
                    return;
                }
                self.out
                    .performances
                    .extend(Translated::playback_pair(&done));
                // `process_score`'s Performance, added to its book with
                // the headers it is named by.
                if let Some(performance) = done.performance(vm.heap_mut()) {
                    paper_book::add_performance(vm.heap_mut(), book, header, performance);
                }
            }
            Ok(None) => {}
            Err(e) => {
                let text = e.text(vm.heap());
                def.log.push_str(&format!("error: {text}\n"));
                if !engines_own(vm, &e) {
                    ended = Some(text);
                } else if def.layout {
                    let top = top(vm, book);
                    self.failed.push(top);
                }
            }
        }
        def.ms += now() - start;
        self.def_ended(vm, c, score, def, ended);
    }

    /// A `\\layout` translation's Paper_score is processed
    /// (`Paper_score::process`), as far as the step's time goes.
    #[allow(clippy::too_many_arguments)]
    fn processing(
        &mut self,
        vm: &mut Vm,
        c: &mut Cx,
        score: Box<Scoring>,
        mut def: Def,
        done: Box<Translated>,
        mut processing: Box<Processing>,
        laying: Laying,
    ) {
        let start = now();
        let pscore = lily_engine_layout::paper_score::of_translation(&done.translation);
        let system =
            pscore.and_then(|p| lily_engine_layout::paper_score::root_system(vm.heap(), p));
        let on = match system {
            Some(system) => processing.on(vm, system),
            None => Ok(true),
        };
        let ms = now() - start;
        c.times.lay_out_ms += ms;
        def.ms += ms;
        match on {
            Ok(false) => {
                self.stage = Stage::Processing(score, def, done, processing, laying);
            }
            Ok(true) => {
                vm.heap_mut().unroot_to(laying.depth);
                self.laid(vm, c, score, def, done, true, laying, None);
            }
            Err(e) => {
                // An error not the engine's own ends the run where
                // LilyPond's does, as a translation's (D132, B33); the
                // engine's own leaves the book without its layout.
                let ended = (!engines_own(vm, &e)).then(|| e.text(vm.heap()));
                vm.heap_mut().unroot_to(laying.depth);
                self.laid(vm, c, score, def, done, false, laying, ended);
            }
        }
    }

    /// A `\\layout` translation is laid out, or its layout failed, `ended`
    /// the text of an error that ends the run (B33).
    #[allow(clippy::too_many_arguments)]
    fn laid(
        &mut self,
        vm: &mut Vm,
        c: &mut Cx,
        score: Box<Scoring>,
        mut def: Def,
        done: Box<Translated>,
        laid: bool,
        laying: Laying,
        ended: Option<String>,
    ) {
        let start = now();
        let (book, header) = (score.book, score.header);
        if !laid {
            let top = top(vm, book);
            self.failed.push(top);
        }
        if !laying.waiting {
            vm.take_waiting();
        }
        c.asked.borrow_mut().truncate(laying.asking);
        self.out
            .performances
            .extend(Translated::playback_pair(&done));
        // `process_score`'s Performance, added to its book with
        // the headers it is named by.
        if let Some(performance) = done.performance(vm.heap_mut()) {
            paper_book::add_performance(vm.heap_mut(), book, header, performance);
        }
        def.ms += now() - start;
        self.def_ended(vm, c, score, def, ended);
    }

    /// An output definition's translation is over: what it took and said.
    fn def_ended(
        &mut self,
        vm: &mut Vm,
        c: &mut Cx,
        score: Box<Scoring>,
        def: Def,
        ended: Option<String>,
    ) {
        let book = score.book;
        let Def {
            layout, ms, log, ..
        } = def;
        let times = &mut *c.times;
        times.translate_ms += ms;
        times.translations += 1;
        times.longest_ms = times.longest_ms.max(ms);
        if !layout {
            times.perform_ms += ms;
        }
        let messages = said(vm, c.parse, c.expected, &[]);
        if c.traced && layout {
            let mut log = log;
            for message in &messages {
                log.push_str(&logged(message));
            }
            self.out.log.push_str(&log);
        }
        self.out.said.extend(messages);
        let top_book = Some(top(vm, book));
        if let Some(k) = self.printed.iter().position(|&b| b == top_book) {
            self.said_by_book[k] = self.out.said.len();
            self.maps_by_book[k] = self.out.performances.len();
        }
        // The error ends the run where LilyPond's ends (D132): no
        // translation after it and no pages of its book or later ones.
        if let Some(text) = ended {
            let top = top(vm, book);
            self.ended_at = self
                .printed
                .iter()
                .position(|&b| b == Some(top))
                .unwrap_or(0);
            self.out
                .said
                .extend(diag::ended(&text, self.ended_at == self.made_at));
            self.out.ended = true;
            self.out.performances.clear();
            self.translated(vm, c);
            return;
        }
        self.stage = Stage::Def(score);
    }

    /// The translations are over: on to the pages.
    fn translated(&mut self, vm: &mut Vm, c: &mut Cx) {
        // A `Book::process` error ends the run after the books before it, as
        // a translation's does (D220), unless a translation's ended it before.
        if let Some((k, text)) = self.process_error.take()
            && !self.out.ended
        {
            self.out.said.extend(diag::ended(&text, k == self.made_at));
            self.out.ended = true;
            self.out.performances.clear();
            self.ended_at = k;
        }
        // Each book's pages, then what making and writing them said.
        c.times.translated_bytes = memory() as f64;
        self.printed.truncate(self.ended_at);
        // The scores the pages' markups print (`ly:score-embedded-format`, an
        // instrument name's `\\score`) are recorded on the trace, as they
        // were while a translation was laid out before the pages broke the
        // lines (MS3 step 62), until the dump's reads after the pages are
        // done.
        if c.traced {
            lily_engine_layout::paper_score::lend_trace(vm, core::mem::take(&mut self.trace));
        }
        lily_engine_layout::queue::pause();
        self.books = core::mem::take(&mut self.printed).into_iter().enumerate();
        self.stage = Stage::Book;
    }

    /// The next book: its MIDI files, then its pages begun.
    fn next_book(&mut self, vm: &mut Vm, c: &mut Cx) {
        let Some((k, book)) = self.books.next() else {
            self.stage = Stage::Done;
            return;
        };
        let (waiting, asking) = (vm.waiting(), c.asked.borrow().len());
        // `Paper_book::output_aux` writes the book's MIDI files before its
        // pages, whose layout may have failed: named after the book as its
        // pages are.
        if let Some(book) = book {
            let base = match k {
                0 => String::from(c.root_name),
                k => format!("{}-{k}", c.root_name),
            };
            let writing = now();
            let written = paper_book::output_midis(vm, book, &base, &mut 0);
            c.times.midi_ms += now() - writing;
            match written {
                Ok(files) => self.out.midis.extend(files),
                Err(e) => {
                    if !engines_own(vm, &e) {
                        self.page_error = Some((k, e.text(vm.heap())));
                    }
                }
            }
        }
        self.made.push(book.filter(|b| !self.failed.contains(b)));
        // A performance makes no page (D264).
        let book = book.filter(|_| c.defs != Defs::Midis);
        let paging = Box::new(Paging {
            k,
            waiting,
            asking,
            warnings: Vec::new(),
        });
        match book.filter(|b| !self.failed.contains(b) && self.page_error.is_none()) {
            Some(book) => {
                let start = now();
                let output = Output::begin(vm, book);
                c.times.stencils_ms += now() - start;
                self.stage = Stage::Paging(paging, output);
            }
            None => self.book_ended(vm, c, &paging, (1, Vec::new())),
        }
    }

    /// A book's pages are made, as far as the step's time goes
    /// ([`paper_book::output`]).
    fn paging(&mut self, vm: &mut Vm, c: &mut Cx, paging: Box<Paging>, mut output: Output) {
        let start = now();
        let made = output.on(vm);
        c.times.stencils_ms += now() - start;
        let (first, stencils) = match made {
            Ok(None) => {
                self.stage = Stage::Paging(paging, output);
                return;
            }
            Ok(Some(made)) => made,
            Err(e) => return self.page_failed(vm, c, paging, e),
        };
        let paper = vm
            .heap()
            .foreign::<paper_book::PaperBook>(output.book())
            .map_or(Value::FALSE, |b| b.paper);
        let scale = lily_engine_layout::paper_score::c_variable(vm, paper, "output-scale");
        let Some(output_scale) = vm.heap().to_f64(scale) else {
            return self.book_ended(vm, c, &paging, (first, Vec::new()));
        };
        let mark = c.markup.as_deref().map(Markup::mark);
        let pdf = c.pdf.then(|| {
            let mut book = lily_engine_pdf::stencil::Book::new();
            let header = vm
                .heap()
                .foreign::<paper_book::PaperBook>(output.book())
                .map_or(Value::FALSE, |b| b.header);
            book.info(vm, header);
            book
        });
        self.stage = Stage::Writing(
            paging,
            Writing {
                first,
                stencils: stencils.into_iter(),
                output_scale,
                pages: Vec::new(),
                mark,
                pdf,
            },
        );
    }

    /// The pages made are written, each as `framework-svg.scm`'s
    /// `output-stencil` writes it, at its paper's `output-scale`, a link
    /// naming its file as the engine's tree does. The first error ends them,
    /// as it ends a layout (D130). With a `markup`, for a preview, they are
    /// written in the preview form (W4, D253), what was written last taken
    /// from it. One page a call.
    fn writing(&mut self, vm: &mut Vm, c: &mut Cx, mut paging: Box<Paging>, mut w: Writing) {
        let Some(stencil) = w.stencils.next() else {
            if let Some(book) = w.pdf.take() {
                paging.warnings.extend(book.warnings.iter().cloned());
                let list = book.finish().encode();
                self.out.lists.push((self.out.pages.len(), list));
            }
            return self.book_ended(vm, c, &paging, (w.first, w.pages));
        };
        let paper = lily_engine_svg::Paper {
            output_scale: w.output_scale,
            tree_roots: &[],
        };
        let start = now();
        let page = match c.markup.as_deref_mut() {
            Some(markup) => lily_engine_svg::write_preview(vm, &stencil, &paper, markup),
            None => lily_engine_svg::write_page(vm, &stencil, &paper),
        };
        lily_engine_layout::queue::check(lily_engine_layout::queue::Step::Written);
        // The same stencil walked into the PDF's display list; an error leaves the book's
        // PDF out, not its pages.
        if page.is_ok()
            && let Some(book) = w.pdf.as_mut()
            && let Err(e) = book.page(vm, &stencil, w.output_scale)
        {
            let text = e.text(vm.heap());
            paging
                .warnings
                .push(format!("pdf: a page is not walked: {text}"));
            w.pdf = None;
        }
        c.times.write_ms += now() - start;
        match page {
            Ok(page) => {
                paging.warnings.extend(page.warnings);
                w.pages.push(page.svg);
                self.hand_over(c, &paging, &w);
                self.stage = Stage::Writing(paging, w);
            }
            Err(e) => {
                // The caller keeps no page of a book an error stopped, so
                // `markup` keeps none.
                if let (Some(markup), Some(mark)) = (c.markup.as_deref_mut(), w.mark) {
                    markup.forget(mark);
                }
                self.page_failed(vm, c, paging, e);
            }
        }
    }

    /// The page just written, if it is the visible page of a preview's
    /// compile in steps, handed over before the compile's end (OPEN I7,
    /// D269): named as [`Engine`]'s outputs will name it, with the `.cells`
    /// they will hold, so that the host shows it while the pages after it are
    /// written and the compile ends. An error in a later page of its book
    /// takes it away again, as the outputs at the end then have none of it.
    fn hand_over(&mut self, c: &Cx, paging: &Paging, w: &Writing) {
        let Some(markup) = c.markup.as_deref() else {
            return;
        };
        let index = markup.mark().wrapping_sub(1);
        if !c.returning || index != c.visible {
            return;
        }
        let base = match paging.k {
            0 => String::from(c.root_name),
            k => format!("{}-{k}", c.root_name),
        };
        let svg = w.pages.last().map_or("", String::as_str);
        let name = match w.pages.len() + w.stencils.len() {
            1 => format!("{base}.svg"),
            _ => format!("{base}-{}.svg", w.first + w.pages.len() as i64 - 1),
        };
        let cells = markup.peek(index, &name, svg);
        let file = format!("{}.cells", name.trim_end_matches(".svg"));
        self.handed = vec![(name, svg.as_bytes().to_vec())];
        self.handed.extend(cells.map(|cells| (file, cells)));
    }

    /// An error ended a book's pages.
    fn page_failed(&mut self, vm: &mut Vm, c: &mut Cx, paging: Box<Paging>, e: Exception) {
        if !engines_own(vm, &e) {
            self.page_error = Some((paging.k, e.text(vm.heap())));
        }
        self.book_ended(vm, c, &paging, (1, Vec::new()));
    }

    /// A book's pages are written: what they said.
    fn book_ended(&mut self, vm: &mut Vm, c: &mut Cx, paging: &Paging, pages: (i64, Vec<String>)) {
        if !paging.waiting {
            vm.take_waiting();
        }
        c.asked.borrow_mut().truncate(paging.asking);
        self.out.pages.push(pages);
        self.pages_said
            .extend(said(vm, c.parse, c.expected, &paging.warnings));
        self.stage = match self.page_error.is_some() {
            true => Stage::Done,
            false => Stage::Book,
        };
    }

    /// Drops it between its steps: the parse's module no longer current,
    /// nothing of it rooted.
    fn drop_it(self, vm: &mut Vm) {
        vm.abandon_steps();
        vm.set_current_module(self.previous);
        vm.heap_mut().unroot_to(self.depth);
    }

    /// What its translations made, once [`Work::on`] has done.
    fn finish(
        mut self,
        vm: &mut Vm,
        parse: &Parse,
        traced: bool,
        times: &mut Times,
        expected: &mut Vec<String>,
        asked: &Asked,
    ) -> Translations {
        let mut out = core::mem::take(&mut self.out);
        times.paged_bytes = memory() as f64;
        let (longest, what, from, steps, visible) = lily_engine_layout::queue::longest();
        times.longest_step_ms = longest;
        times.longest_step = what.name();
        times.longest_from = from.name();
        let stats = vm.heap().stats();
        times.collections = stats.collections - self.collected.collections;
        times.collect_ms = stats.pause_ms - self.collected.pause_ms;
        times.longest_wall_ms = lily_engine_layout::queue::longest_wall();
        times.steps = steps;
        times.visible_ms = visible;
        // What the cells hold, before a traced compile's reads add to it: of a
        // run that ended, and whose pages said nothing, which a cell reused would
        // not say again.
        let quiet = self.pages_said.is_empty() && self.page_error.is_none() && !out.ended;
        out.cells = vec![None; out.kept.len()];
        for (k, pscore) in core::mem::take(&mut self.with_cells) {
            if let Some(cells) = lily_engine_layout::cell::keep(vm, pscore, quiet) {
                vm.heap_mut().hold_between(cells);
                out.cells[k] = Some(cells);
            }
            out.cell_reports
                .push(lily_engine_layout::cell::report(vm.heap(), pscore));
        }
        // The error ends the run there, as a translation's does (D220): what
        // the books after it said translating is not said, as LilyPond, which
        // makes each book's pages before it translates the next, translates
        // none of them; nor is the error that ended one of them. Their
        // playback maps are dropped, the book's own and those before it kept,
        // as `Paper_book::output_aux` writes a book's MIDI files before its
        // pages.
        if let Some((k, text)) = self.page_error.take() {
            let cut = self.said_by_book[..=k].iter().copied().max().unwrap_or(0);
            out.said.truncate(cut);
            let maps = self.maps_by_book[..=k].iter().copied().max().unwrap_or(0);
            out.performances.truncate(maps);
            out.said.extend(core::mem::take(&mut self.pages_said));
            out.said.extend(diag::ended(&text, k == self.made_at));
            out.ended = true;
        } else {
            out.said.extend(core::mem::take(&mut self.pages_said));
        }
        // The traced translations' layout.tsv and line.tsv, their systems
        // placed on the pages (MS3 step 62).
        if !self.pending.is_empty() {
            // A stop for the host is not asked, as a traced read's (D126).
            let (waiting, asking) = (vm.waiting(), asked.borrow().len());
            let books: Vec<Value> = self.made.iter().flatten().copied().collect();
            let places = lily_engine_layout::dump::places(vm.heap_mut(), &books);
            for p in &self.pending {
                let mut dumped = lily_engine_layout::dump::Dumped::default();
                lily_engine_layout::dump::laid_out(vm, p, &places, &mut dumped);
                for e in &dumped.errors {
                    out.log.push_str(&format!("error: {e}\n"));
                }
                out.layout.push_str(&dumped.layout);
                out.line.push_str(&dumped.line);
            }
            // The shapes of every book's systems, as `c5dump.ly` writes them
            // from `page-post-process` (MS4 step 6).
            match lily_engine_layout::dump::shapes(vm, &books) {
                Ok(c5) => out.c5 = c5,
                Err(e) => out.log.push_str(&format!("error: c5.tsv: {e}\n")),
            }
            // What the reads said is the dump's: in the trace's log only, as
            // LilyPond's layout.ly reads after the pages' stencils, which say
            // what drawing them says.
            for message in said(vm, parse, expected, &[]) {
                out.log.push_str(&logged(&message));
            }
            if !waiting {
                vm.take_waiting();
            }
            asked.borrow_mut().truncate(asking);
        }
        if traced {
            self.trace = lily_engine_layout::paper_score::return_trace(vm);
        }
        for &pscore in &self.pscores {
            let live = lily_engine_layout::dump::live_grobs(vm.heap_mut(), pscore);
            times.live_grobs += live as u32;
        }
        vm.set_current_module(self.previous);
        vm.heap_mut().unroot_to(self.depth);
        out.trace = self.trace.text;
        out.grobs = self.trace.grobs;
        out
    }
}

/// What a translation or a book's pages said since the last time, with
/// the writer's `warnings`, and what Scheme wrote to the error port,
/// which the next parse would take as its own; the warnings `expected`
/// that none of it met left in it, to be carried on.
fn said(
    vm: &mut Vm,
    parse: &Parse,
    expected: &mut Vec<String>,
    warnings: &[String],
) -> Vec<Message> {
    let mut said = Diagnostics::default();
    for text in expected.drain(..) {
        said.expect(text);
    }
    parse.take_said(vm, &mut said);
    for warning in warnings {
        said.warning(At::Nowhere, warning.as_str());
    }
    // The error port's text not flushed yet, at the translation's end, as
    // the parse puts its own at the file's end.
    said.flush();
    let (messages, left) = said.report(&parse.sources);
    *expected = left;
    messages
}

/// Whether the error `e` is the engine's own, which LilyPond's run would
/// not meet (D132): a placeholder's call, whose message names the step that
/// brings it (`types.rs`' `unimplemented`, `grob.rs`), or a font the host
/// gave no pack of (layout's `find_otf_font`).
fn engines_own(vm: &Vm, e: &Exception) -> bool {
    let heap = vm.heap();
    let message = heap
        .cdr(e.args)
        .and_then(|rest| heap.car(rest))
        .and_then(|m| heap.str(m))
        .map(|m| m.as_str());
    heap.name(e.key) == Some("misc-error")
        && message.is_some_and(|m| {
            m.contains("not in the engine before ") || m == "cannot find font '~a'"
        })
}

/// The `Paper_book` whose output prints `book`: `book`, or the book of
/// the part it is.
fn top(vm: &Vm, mut book: Value) -> Value {
    while let Some(parent) = vm
        .heap()
        .foreign::<paper_book::PaperBook>(book)
        .map(|b| b.parent)
        .filter(|&p| p != Value::FALSE)
    {
        book = parent;
    }
    book
}

/// `(%load-stubbed-parts! definitions)` of the distinct definitions of
/// `t`'s grobs: each part they hold a stub of loaded, as LilyPond's
/// layout finds them loaded (A46, D228). Whether the host must be asked
/// for a part's files first; another error of a part is its layout's, as
/// before.
fn load_parts(vm: &mut Vm, t: &lily_engine_music::Translation) -> bool {
    let Some(load) = vm
        .slot("lily", "%load-stubbed-parts!")
        .and_then(|s| vm.slot_value(s))
    else {
        return false;
    };
    let definitions: Vec<Value> = {
        let a = t.arena.borrow();
        let mut seen = BTreeSet::new();
        a.records
            .grobs
            .iter()
            .map(|g| g.definition)
            .filter(|d| seen.insert(d.to_bits()))
            .collect()
    };
    let depth = vm.heap().root_depth();
    let list = vm.heap_mut().list(&definitions);
    vm.heap_mut().root(list);
    let _ = vm.apply(load, &[list]);
    vm.heap_mut().unroot_to(depth);
    vm.waiting()
}

/// Sets the car of the list `name` of `module` holds to `value`.
fn set_list(vm: &mut Vm, module: &str, name: &str, value: Value) {
    if let Some(cell) = vm.slot(module, name).and_then(|s| vm.slot_value(s)) {
        vm.heap_mut().set_car(cell, value);
    }
}

/// The files Scheme wrote since the last time (`%written-files`), by name,
/// the last written of each name, in the order they were first written.
fn take_written(vm: &mut Vm) -> Vec<(String, String)> {
    let Some(cell) = vm
        .slot("guile", "%written-files")
        .and_then(|s| vm.slot_value(s))
    else {
        return Vec::new();
    };
    let heap = vm.heap_mut();
    let mut list = heap.car(cell).unwrap_or(Value::NULL);
    heap.set_car(cell, Value::NULL);
    let mut files: Vec<(String, String)> = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        let name = heap
            .car(item)
            .and_then(|n| heap.str(n))
            .map(|s| s.as_str().into());
        let text = heap
            .cdr(item)
            .and_then(|t| heap.str(t))
            .map(|s| s.as_str().into());
        if let (Some(name), Some(text)) = (name, text)
            && !files.iter().any(|(n, _)| *n == name)
        {
            files.push((name, text));
        }
        list = rest;
    }
    files.reverse();
    files
}

/// `message` as `trace.log` keeps it: as it prints, without its excerpt.
fn logged(message: &Message) -> String {
    let mut out = Vec::new();
    Message {
        excerpt: Vec::new(),
        ..message.clone()
    }
    .write(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}
