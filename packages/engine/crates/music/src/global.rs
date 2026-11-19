//! `Global_context::iterate` (`global-context.cc:137–210`) and
//! `ly:run-translator` (`global-context-scheme.cc`), N4's first part (04
//! §8, MS2 step 23): music translated in lockstep, one timestep at a time.
//!
//! The loop asks the top iterator for its pending moment, takes the first
//! moment a translator asked for (`ly:context-schedule-moment`) if it is
//! earlier, and stops past the music's length. Each timestep sends
//! `Prepare` with its moment, makes the iterators' contexts the first time,
//! lets the iterators process it, sends `OneTimeStep`, undoes the `\once`s
//! and removes the contexts no iterator holds. After the last, the
//! iterators quit, the contexts are removed once more and `Finish` is sent.
//!
//! `ly:run-translator` gives Global its translator group first, which
//! makes each new context's (`translator.rs`, step 25); the Score's group
//! runs the translators' timestep protocol on `Prepare`, `OneTimeStep` and
//! `Finish`, and a traced translation records its trace there (`trace.rs`).
//! A performance's may write the playback map (`playback.rs`, step 26),
//! and its `Performance` is taken from it (`performance.rs`, MS5 step 1).
//!
//! `ly:make-global-context` and `ly:interpret-music-expression` are their
//! Scheme face (MS5 step 6a), over `%host-call` ([`HOSTS`], D127), as
//! `part-combiner.scm`'s `recording-group-emulate` calls them: a Global
//! with no translator group, whose contexts make no translators, iterated
//! as `ly:run-translator` iterates but forced.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::context::{GLOBAL, Host, Translation, check_removal, diagnose, send, translation_of};
use crate::iterator::{INFINITY, IterId, Iteration};
use crate::playback::Playback;
use crate::trace::{Recorder, Trace};
use crate::translator::TranslatorClass;
use crate::{Input, Moment};

type Run<T> = Result<T, Exception>;

/// A translation that ran: its contexts in their final state, and the
/// iterators it lacked.
pub struct Translated {
    pub translation: Translation,
    pub unported: Vec<&'static str>,
    /// What N8 keeps of it for the next compile's (`checkpoint.rs`): none
    /// for a `\score` in a markup.
    pub n8: Option<crate::checkpoint::Kept>,
    /// What the snapshots' check found ([`crate::set_check_resume`]).
    pub resumed: Vec<String>,
    /// The snapshot it was resumed from in the last compile's translation
    /// (D241), if it was.
    pub resumed_at: Option<usize>,
    /// Whether, resumed, it converged with that translation and ended as it
    /// did (D243).
    pub spliced: bool,
    /// What it replayed (MS6 step 7c).
    pub replayed: crate::replay::Replayed,
    /// Its top iterator, the trace it began with and the epochs its
    /// objects were made in, in each compile it ran in, for
    /// [`Translated::resumable`].
    top: Option<crate::iterator::IterId>,
    trace_start: Option<(String, String)>,
    epochs: Vec<(u32, u32)>,
    /// What it wrote to the error port.
    said: String,
}

impl Translated {
    /// It kept for the next compile's translation of the same score and
    /// output definition to resume (D241): `None` for a translation of
    /// no snapshots.
    pub fn resumable(&self) -> Option<crate::resume::Resumable> {
        let (kept, top) = (self.n8.clone()?, self.top?);
        if self.translation.arena.borrow().snapshots.is_empty() {
            return None;
        }
        Some(crate::resume::Resumable::new(
            self.translation.clone(),
            kept,
            top,
            (
                self.trace_start.clone(),
                self.epochs.clone(),
                self.said.clone(),
            ),
        ))
    }

    /// How many grobs it made.
    pub fn grobs(&self) -> usize {
        self.translation.arena.borrow().records.grobs.len()
    }

    /// The performance's playback map, as JSON, if its performers wrote one.
    pub fn playback(&self) -> Option<String> {
        let a = self.translation.arena.borrow();
        a.playback.as_ref().and_then(|p| p.json.clone())
    }

    /// [`Translated::playback`], and the map as `timing.ly` writes it when
    /// another performance follows in its file, its staves' voices none
    /// (OPEN C2.3, `Playback::followed_json`).
    pub fn playback_pair(&self) -> Option<(String, String)> {
        let a = self.translation.arena.borrow();
        let p = a.playback.as_ref()?;
        Some((p.json.clone()?, p.followed_json()))
    }

    /// The `Performance` of a `\midi` translation (`performance.rs`), once,
    /// processed as `ly:format-output` processes it: `None` for a `\layout`
    /// translation, or after the first time.
    pub fn performance(&self, heap: &mut lily_engine_scheme::Heap) -> Option<Value> {
        crate::performance::finished(heap, &mut self.translation.arena.borrow_mut())
    }
}

/// `ly:run-translator`: `music` translated with the output definition
/// `odef`, recorded on `trace` if one is given. `None` without a definition
/// of Global. A score of no length is skipped with LilyPond's warning.
pub fn run_translator(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
) -> Run<Option<Translated>> {
    run_translator_with(host, music, odef, trace, &[])
}

/// [`run_translator`] with translator classes of its own, which names in
/// the definitions find before LilyPond's.
pub fn run_translator_with(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
    classes: &[&'static TranslatorClass],
) -> Run<Option<Translated>> {
    run(host, music, odef, trace, classes, (false, false), false)
}

/// `ly:score-embedded-format`'s translation (MS3 step 43): as
/// [`run_translator_with`], recorded on `trace` as `trace.ly` records a
/// `\score` in a markup, by its Score's translator alone
/// ([`Recorder::score_only`]).
pub fn run_embedded(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
    classes: &[&'static TranslatorClass],
) -> Run<Option<Translated>> {
    run(host, music, odef, trace, classes, (false, false), true)
}

/// A `\midi` translation of `music` with `odef` whose performers write the
/// playback map (`playback.rs`): [`Translated::playback`] is its JSON, an
/// item of `<name>.timing.json`'s array.
pub fn run_performance(host: &mut dyn Host, music: Value, odef: Value) -> Run<Option<Translated>> {
    run(host, music, odef, None, &[], (true, false), false)
}

/// [`run_translator_with`], or [`run_performance`] if `playback`, resuming
/// `old`, the last compile's translation of the same score and output
/// definition, where it can (D241, `resume.rs`), with the edit that moved
/// the text since, if it did (D244).
/// It is [`Going::on`]'s to run: between timesteps, if `returning` (H3,
/// D254), it gives the host back when the step's time is spent. `None`
/// without a definition of Global.
#[allow(clippy::too_many_arguments)]
pub fn start_resuming(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
    classes: &[&'static TranslatorClass],
    playback: bool,
    old: Option<(&crate::resume::Resumable, Option<&crate::Moved>)>,
    returning: bool,
) -> Run<Option<Going>> {
    check_music(host, music)?;
    // N8's replay: a translation is logged where the last compile's shows
    // it can be replayed (MS6 step 7c).
    let logged = old.is_some_and(|(old, _)| old.replayable());
    let begun = host.heap().root_depth();
    if let Some((old, moved)) = old {
        let depth = host.heap().root_depth();
        let mut probe = |host: &mut dyn Host, recording| {
            probe_start(host, music, odef, classes, playback, recording)
        };
        let resumed = crate::resume::resume(
            host,
            old,
            (music, odef),
            &mut probe,
            trace.as_deref(),
            moved,
        )?;
        if let Some(r) = resumed {
            let mut going = begin_resumed(host, r, music, trace, begun);
            going.it.returns = returning;
            return Ok(Some(going));
        }
        host.heap().unroot_to(depth);
    }
    let going = begin(host, music, odef, trace, classes, (playback, logged), false)?;
    Ok(going.map(|mut going| {
        going.it.returns = returning;
        going
    }))
}

/// The state's digest after the first timestep's `Prepare` of a new
/// translation of `music` with `odef` (`Kept::start`), what the output
/// definition gives its contexts, from a translation made for it and
/// dropped; what it said taken back.
fn probe_start(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    classes: &[&'static TranslatorClass],
    playback: bool,
    recording: crate::checkpoint::Recording,
) -> Run<Option<u64>> {
    let errors = host.error_len();
    let placed = crate::resume::diagnostics_now(host);
    let depth = host.heap().root_depth();
    let ran = (|| {
        let Some(t) = Translation::new(host, odef)? else {
            return Ok(None);
        };
        t.arena.borrow_mut().extra_classes = classes.to_vec();
        if playback {
            t.arena.borrow_mut().playback = Some(Playback::default());
        }
        crate::translator::connect_global(host, &t);
        host.heap().root(t.value);
        host.heap().root(music);
        t.arena.borrow_mut().n8 = Some(alloc::boxed::Box::new(recording));
        let mut it = Iteration::new(host.heap(), &t);
        let top = it.create_top(host, music)?;
        t.arena.borrow_mut().add_moment(Moment::default());
        let w = it.pending(top);
        let w = t.arena.borrow_mut().take_extra(w);
        let recording = t.arena.borrow_mut().n8.take();
        if let Some(mut r) = recording {
            r.step(host.heap(), &t.arena.borrow(), w);
            t.arena.borrow_mut().n8 = Some(r);
        }
        t.prepare(host, w)?;
        it.init_context(host, top, GLOBAL)?;
        checkpoint(host.heap(), &t, &it, top, true);
        let start = t.arena.borrow().n8.as_ref().map(|r| r.kept().start);
        Ok(start)
    })();
    host.heap().unroot_to(depth);
    host.cut_errors(errors);
    crate::resume::unsay(host, placed);
    ran
}

/// The epochs a translation that began in epoch `since` made its objects
/// in, to the end of its run; the epoch after begins (`Heap::checkpoint_mark`),
/// so that what layout makes is none of its own (D241).
fn own_epochs(heap: &mut lily_engine_scheme::Heap, since: u32) -> (u32, u32) {
    let end = heap.epoch();
    heap.checkpoint_mark();
    (since, end)
}

/// A translation under way (H3, D254): [`Going::on`] runs it until it ends
/// or the step's time is spent, and [`Going::end`] gives what it made. What
/// it has rooted stays rooted between the two.
pub struct Going {
    t: Translation,
    it: Iteration,
    music: Value,
    trace_start: Option<(String, String)>,
    since: u32,
    /// The root stack as it was before it.
    depth: usize,
    how: How,
    /// How the run ended, once it has.
    ran: Option<Run<bool>>,
}

enum How {
    /// From its start, what had been written to the error port before it,
    /// and whether its first timestep has begun.
    Whole { errors: usize, begun: bool },
    /// From a snapshot of the last compile's ([`begin_resumed`]).
    Resumed {
        replay: Replaying,
        epochs: Vec<(u32, u32)>,
        snapshot: usize,
        /// The snapshot a replay that ran to its end began at.
        replayed: Option<usize>,
    },
}

impl Going {
    /// Runs on; whether the translation has ended ([`Going::end`]).
    pub fn on(&mut self, host: &mut dyn Host) -> bool {
        // Spliced as it began (D258).
        if self.ran.is_some() {
            return true;
        }
        let Going { t, it, music, .. } = self;
        let ran = match &mut self.how {
            How::Whole { begun, .. } if !*begun => {
                // The lists `create_top` reads asked first, in steps (B23, D272).
                match it.ask_ahead(host, *music) {
                    Ok(false) => {
                        crate::returns::returned_ahead();
                        return false;
                    }
                    Ok(true) => {
                        *begun = true;
                        iterate(host, t, it, *music, false)
                    }
                    Err(e) => {
                        *begun = true;
                        Err(e)
                    }
                }
            }
            How::Whole { .. } => steps(host, t, it, None).map(|_| true),
            How::Resumed {
                replay, replayed, ..
            } => match replaying(host, t, it, replay) {
                Some((ran, began)) => {
                    *replayed = began;
                    ran
                }
                None => {
                    it.paused = false;
                    return false;
                }
            },
        };
        if core::mem::take(&mut it.paused) {
            return false;
        }
        self.ran = Some(ran);
        true
    }

    /// What [`Going::on`] ran, the trace back on `trace`.
    pub fn end(self, host: &mut dyn Host, trace: Option<&mut Trace>) -> Run<Option<Translated>> {
        match self.how {
            How::Whole { .. } => end_whole(host, self, trace),
            How::Resumed { .. } => end_resumed(host, self, trace),
        }
    }
}

/// A resumed translation, to run on to its end, as [`begin`]'s does.
fn begin_resumed(
    host: &mut dyn Host,
    r: crate::resume::Resumed,
    music: Value,
    trace: Option<&mut Trace>,
    depth: usize,
) -> Going {
    let crate::resume::Resumed {
        t,
        it,
        at,
        snapshot,
        step,
        same,
        epochs,
    } = r;
    let since = host.heap().epoch();
    host.heap().root(t.value);
    host.heap().root(music);
    let trace_start = trace.as_deref().map(|t| (t.text.clone(), t.grobs.clone()));
    crate::replay::count_begin();
    // Where it begins, in place of the snapshot it resumed from (D258), and
    // to restore a replay that fails.
    crate::resume::take(host, &t, &it, (step, at.0, at.1));
    let replay = Replaying::new(&t, &it, at, step);
    let ran = (same && crate::resume::splice_same(host, &t)).then_some(Ok(true));
    Going {
        t,
        it,
        music,
        trace_start,
        since,
        depth,
        how: How::Resumed {
            replay,
            epochs,
            snapshot,
            replayed: None,
        },
        ran,
    }
}

/// A resumed translation run on to its end, as [`end_whole`] ends one.
fn end_resumed(
    host: &mut dyn Host,
    going: Going,
    trace: Option<&mut Trace>,
) -> Run<Option<Translated>> {
    let Going {
        t,
        mut it,
        trace_start,
        since,
        depth,
        how,
        ran,
        ..
    } = going;
    let How::Resumed {
        mut epochs,
        snapshot,
        replayed,
        ..
    } = how
    else {
        return Ok(None);
    };
    it.returns = false;
    let ran = ran.unwrap_or(Ok(false));
    let counts = crate::replay::counted();
    let spliced = matches!(ran, Ok(true));
    // The last compile's run is no more use once this one ends.
    t.arena.borrow_mut().splice = None;
    let failed = core::mem::take(&mut t.arena.borrow_mut().failed_parts);
    let resumed = match ran {
        Ok(_) if crate::resume::checking() => check_snapshots(host, &t, &mut it, replayed),
        _ => Vec::new(),
    };
    t.arena.borrow_mut().failed_parts = failed;
    if ran.is_ok() {
        crate::resume::ended(host, &t);
    }
    epochs.push(own_epochs(host.heap(), since));
    let base = t.arena.borrow().errors_base;
    let said = host.errors_since(base);
    let recording = t.arena.borrow_mut().n8.take();
    let n8 = recording.map(|r| r.finish(host.heap(), &t.arena.borrow()));
    if let (Some(trace), Some(r)) = (trace, t.arena.borrow_mut().recorder.take()) {
        *trace = r.into_trace();
    }
    host.heap().unroot_to(depth);
    ran?;
    Ok(Some(Translated {
        translation: t,
        unported: it.unported,
        n8,
        resumed,
        resumed_at: Some(snapshot),
        spliced,
        replayed: counts,
        top: it.top.map(|(top, _)| top),
        trace_start,
        epochs,
        said,
    }))
}

fn run(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
    classes: &[&'static TranslatorClass],
    how: (bool, bool),
    score_only: bool,
) -> Run<Option<Translated>> {
    check_music(host, music)?;
    let mut trace = trace;
    let begun = begin(
        host,
        music,
        odef,
        trace.as_deref_mut(),
        classes,
        how,
        score_only,
    )?;
    let Some(mut going) = begun else {
        return Ok(None);
    };
    going.on(host);
    going.end(host, trace)
}

/// A translation from its start, to run ([`Going::on`]); `None` without a
/// definition of Global.
fn begin(
    host: &mut dyn Host,
    music: Value,
    odef: Value,
    trace: Option<&mut Trace>,
    classes: &[&'static TranslatorClass],
    (playback, logged): (bool, bool),
    score_only: bool,
) -> Run<Option<Going>> {
    let Some(t) = Translation::new(host, odef)? else {
        return Ok(None);
    };
    let since = host.heap().epoch();
    let errors = host.error_len();
    {
        let heap = host.heap();
        let log = heap.undo_len();
        let globals = host.globals_written();
        t.arena.borrow_mut().resume_base = crate::context::ResumeBase {
            log,
            since,
            globals,
        };
        t.arena.borrow_mut().errors_base = errors;
    }
    let placed = crate::resume::diagnostics_now(host);
    t.arena.borrow_mut().diagnostics_base = placed;
    let trace_start = trace.as_deref().map(|t| (t.text.clone(), t.grobs.clone()));
    t.arena.borrow_mut().extra_classes = classes.to_vec();
    if playback {
        t.arena.borrow_mut().playback = Some(Playback::default());
    }
    crate::translator::connect_global(host, &t);
    let depth = host.heap().root_depth();
    host.heap().root(t.value);
    host.heap().root(music);
    if let Some(trace) = trace {
        let taken = core::mem::take(trace);
        let recorder = if score_only {
            Recorder::score_only(host, taken)
        } else {
            Recorder::new(host, taken)
        };
        t.arena.borrow_mut().recorder = Some(recorder);
    }
    if !score_only {
        let recording = crate::checkpoint::Recording::new(host.heap(), music);
        let mut a = t.arena.borrow_mut();
        a.n8 = Some(Box::new(recording));
        if logged {
            a.log = Some(alloc::rc::Rc::default());
        }
    }
    let it = Iteration::new(host.heap(), &t);
    Ok(Some(Going {
        t,
        it,
        music,
        trace_start,
        since,
        depth,
        how: How::Whole {
            errors,
            begun: false,
        },
        ran: None,
    }))
}

/// What [`begin`]'s translation ran.
fn end_whole(
    host: &mut dyn Host,
    going: Going,
    trace: Option<&mut Trace>,
) -> Run<Option<Translated>> {
    let Going {
        t,
        it: mut iteration,
        music,
        trace_start,
        since,
        depth,
        how,
        ran,
    } = going;
    let How::Whole { errors, .. } = how else {
        return Ok(None);
    };
    iteration.returns = false;
    let ran = ran.unwrap_or(Ok(false));
    let resumed = match ran {
        Ok(true) if crate::resume::checking() && t.arena.borrow().n8.is_some() => {
            check_snapshots(host, &t, &mut iteration, None)
        }
        _ => Vec::new(),
    };
    if matches!(ran, Ok(true)) {
        crate::resume::ended(host, &t);
    }
    let epochs = alloc::vec![own_epochs(host.heap(), since)];
    let said = host.errors_since(errors);
    let recording = t.arena.borrow_mut().n8.take();
    let n8 = recording.map(|r| r.finish(host.heap(), &t.arena.borrow()));
    if let (Some(trace), Some(r)) = (trace, t.arena.borrow_mut().recorder.take()) {
        *trace = r.into_trace();
    }
    let ran = ran.and_then(|found| {
        if !found {
            warning(host, music, "skipping zero-duration score")?;
            warning(
                host,
                music,
                "to suppress this, consider adding a spacer rest",
            )?;
        }
        Ok(())
    });
    host.heap().unroot_to(depth);
    ran?;
    Ok(Some(Translated {
        translation: t,
        unported: iteration.unported,
        n8,
        resumed,
        resumed_at: None,
        spliced: false,
        replayed: crate::replay::Replayed::default(),
        top: iteration.top.map(|(top, _)| top),
        trace_start,
        epochs,
        said,
    }))
}

/// `Music::warning`: at its origin.
fn warning(host: &mut dyn Host, music: Value, text: &str) -> Run<()> {
    let heap = host.heap();
    let key = heap.intern("origin");
    let origin = crate::music_property(heap, music, key).unwrap_or(Value::NULL);
    if heap.foreign::<Input>(origin).is_none() {
        return diagnose(host, "warning", text);
    }
    if let Some(proc) = host.lily("ly:input-warning") {
        let text = host.heap().string(text);
        host.call(proc, &[origin, text])?;
    }
    Ok(())
}

/// `Global_context::iterate`: whether there was music, which `force`
/// assumes.
pub fn iterate(
    host: &mut dyn Host,
    t: &Translation,
    it: &mut Iteration,
    music: Value,
    force: bool,
) -> Run<bool> {
    let depth = host.heap().root_depth();
    let ran = (|| {
        let top = it.create_top(host, music)?;
        let found = force || {
            let len = it
                .length(top)
                .checked_sub(it.start(top))
                .unwrap_or(INFINITY);
            len != Moment::default() && it.ok(top)
        };
        if !found {
            return Ok(false);
        }
        let mut last = it.length(top);
        if last.main.is_inf() {
            // Lyrics combined at the top: cut short.
            warning(host, music, "cannot determine music length")?;
            last = Moment::default();
        }
        it.top = Some((top, last));
        // At least one pass, which makes the contexts.
        t.arena.borrow_mut().add_moment(Moment::default());
        steps(host, t, it, None)?;
        Ok(true)
    })();
    // A return keeps what is rooted, for [`Going::on`] to run on with.
    if !it.paused {
        host.heap().unroot_to(depth);
    }
    ran
}

/// The global loop from its first timestep, or from the processing of the
/// timestep a snapshot was taken at (`resumed`: its moment, and whether
/// its iterators run, `resume.rs`), to its end; or, a resumed translation,
/// to where it converges with the last compile's run and that run is
/// spliced in (D243): whether it was.
fn steps(
    host: &mut dyn Host,
    t: &Translation,
    it: &mut Iteration,
    mut resumed: Option<(Moment, bool)>,
) -> Run<bool> {
    let Some((top, last)) = it.top else {
        return Ok(false);
    };
    let mut first = resumed.is_none() && !core::mem::take(&mut it.under_way);
    // A timestep returned in the middle of goes on where it was (D272).
    let mut mid = match resumed {
        Some(_) => None,
        None => it.mid.take(),
    };
    loop {
        // Whether its iterators have run, and whether this call went on in
        // its middle, where it does a part before it may return again.
        let (mut processed, mut went_on) = (false, false);
        let (w, ok) = match (mid.take(), resumed.take()) {
            (Some((w, ok, done)), _) => {
                (processed, went_on) = (done, true);
                (w, ok)
            }
            (None, Some(at)) => at,
            (None, None) => {
                let w = it.pending(top);
                let ok = w.compare(INFINITY).is_lt() || it.run_always(top);
                let w = t.arena.borrow_mut().take_extra(w);
                if w.compare(last).is_gt() {
                    break;
                }
                if w == t.arena.borrow().prev() {
                    diagnose(
                        host,
                        "programming-error",
                        "Moment is not increasing.  Aborting interpretation.",
                    )?;
                    break;
                }
                let recording = t.arena.borrow_mut().n8.take();
                if let Some(mut r) = recording {
                    r.step(host.heap(), &t.arena.borrow(), w);
                    let step = r.steps() as u32 - 1;
                    let ended = step.checked_sub(1).and_then(|s| r.moment_of(s as usize));
                    if let Some(log) = &t.arena.borrow().log {
                        log.borrow_mut().step(step);
                    }
                    let mut a = t.arena.borrow_mut();
                    a.n8 = Some(r);
                    // N8's replay (MS6 step 7b): the timestep that ended
                    // checked; a part that failed ends the run, to be run
                    // again from where it resumed.
                    if let Some(m) = ended {
                        crate::replay::check(&mut a, step as usize - 1, m);
                        if a.replay.as_ref().is_some_and(|r| !r.failed.is_empty()) {
                            return Ok(false);
                        }
                    }
                }
                t.prepare(host, w)?;
                if first {
                    it.init_context(host, top, GLOBAL)?;
                    first = false;
                }
                if let Some((at, wanted)) = checkpoint(host.heap(), t, it, top, ok) {
                    // No snapshot in a replay (no compile resumes from it)
                    // but a point to go back to where a part goes back to
                    // replay (MS6 step 7c); and no splice.
                    let replaying = t.arena.borrow().replay.is_some();
                    if !replaying {
                        if wanted || crate::resume::converges(t, at.1) {
                            crate::resume::take(host, t, it, at);
                        }
                        // Converged with the last compile's run: it ends as
                        // that one did (D243).
                        if crate::resume::splice(host, t) {
                            return Ok(true);
                        }
                    } else if crate::replay::converged(&mut t.arena.borrow_mut(), at.1) {
                        crate::resume::take(host, t, it, at);
                        let mut a = t.arena.borrow_mut();
                        let k = a.snapshots.len() - 1;
                        let dropped = a.replay.as_mut().and_then(|r| r.add_point(k, at.1));
                        if let Some(d) = dropped {
                            a.snapshots.remove(d);
                        }
                    }
                }
                (w, ok)
            }
        };
        // A timestep's parts are steps of their own (B23, D272): its start
        // and checkpoint, its iterators' events, then its translators'
        // timestep. T29's first, which makes every context, was one step of
        // 10 ms natively.
        if !processed {
            if !went_on && it.returns && crate::returns::due() {
                it.mid = Some((w, ok, false));
                crate::returns::returned_mid();
                it.paused = true;
                it.under_way = true;
                return Ok(false);
            }
            if ok {
                it.process(host, top, w)?;
            }
            if it.returns && crate::returns::due() {
                it.mid = Some((w, ok, true));
                crate::returns::returned_mid();
                it.paused = true;
                it.under_way = true;
                return Ok(false);
            }
        }
        send(host, t, GLOBAL, "OneTimeStep", &[])?;
        t.apply_finalizations(host)?;
        check_removal(host, t, GLOBAL)?;
        // Between timesteps (04 §8, H3): the next finds what it needs in
        // the iterators and the arena.
        if it.returns && crate::returns::due() {
            it.paused = true;
            it.under_way = true;
            return Ok(false);
        }
    }
    // What ends a translation is one step: its engravers' last write to every
    // grob, each row a copy of what its snapshots hold, and N8's end (D259).
    let grobs = t.arena.borrow().records.grobs.len();
    if it.returns && crate::returns::short_by(grobs * crate::returns::GROB_END) {
        host.heap().over_memory();
        it.paused = true;
        it.under_way = true;
        return Ok(false);
    }
    it.quit(host, top)?;
    check_removal(host, t, GLOBAL)?;
    send(host, t, GLOBAL, "Finish", &[])?;
    if let Some(log) = &t.arena.borrow().log {
        log.borrow_mut().count();
    }
    Ok(false)
}

/// Where a resumed translation is in [`replaying`].
enum ReplayStage {
    /// Nothing to replay, or past the attempts: it runs to its end.
    Running,
    /// An attempt at a replay.
    Attempting,
}

/// A resumed translation run on from its snapshot, its partitions that the
/// old run's log can replay replayed (MS6 step 7b): where a replayed part
/// fails, the translation restored where it resumed (a snapshot taken there)
/// and run again with that part running.
struct Replaying {
    stage: ReplayStage,
    /// Where the next run of [`steps`] begins; none after a return.
    at: Option<(Moment, bool)>,
    /// What the next attempt replays.
    next: Option<Box<crate::replay::Replay>>,
    replays: bool,
    attempts: usize,
    step: u32,
    /// The snapshot taken where it begins, to restore a replay that fails.
    s0: usize,
}

impl Replaying {
    /// The snapshot where it begins is the last taken.
    fn new(t: &Translation, it: &Iteration, at: (Moment, bool), step: usize) -> Replaying {
        let moment = at.0;
        let from = t
            .arena
            .borrow()
            .log
            .as_ref()
            .map_or(0, |l| l.borrow().mark().effects) as u32;
        let made = {
            let r = &t.arena.borrow().records;
            (r.grobs.len() as u32, r.audio.len() as u32)
        };
        // The parts whose music changed run from the start, until a
        // checkpoint where they hold what the old run's did; those that
        // failed in the last compile's replay past the moment they did, which
        // saves failing again.
        let running: Vec<(u32, Moment)> = {
            let a = t.arena.borrow();
            let parts = a.splice.as_ref().map(|s| s.parts_of(&a, &it.changed));
            let failed = a.splice.as_ref().map(|s| s.failed_before());
            let mut running: Vec<(u32, Moment)> = parts
                .unwrap_or_default()
                .into_iter()
                .map(|p| (p, Moment::default()))
                .collect();
            for (p, m) in failed.unwrap_or_default() {
                if m.compare(moment).is_gt() && !running.iter().any(|r| r.0 == p) {
                    running.push((p, m));
                }
            }
            running
        };
        let mut next = {
            let a = t.arena.borrow();
            a.splice
                .as_ref()
                .and_then(|s| s.replay(&running, step as u32, from, made))
        };
        let mut replaying = Replaying {
            stage: ReplayStage::Running,
            at: Some(at),
            next: None,
            replays: false,
            attempts: 0,
            step: step as u32,
            s0: 0,
        };
        // No log to replay; or nothing changed that a replay would run.
        if next.is_none() || running.is_empty() {
            return replaying;
        }
        let s0 = t.arena.borrow().snapshots.len() - 1;
        if let Some(r) = next.as_mut() {
            r.add_point(s0, moment);
        }
        replaying.stage = ReplayStage::Attempting;
        replaying.next = next;
        replaying.s0 = s0;
        replaying
    }
}

/// Runs a resumed translation on until it ends, whether it converged and was
/// spliced, and the snapshot it began at if a replay ran to its end; `None`
/// when the step's time is spent (H3).
fn replaying(
    host: &mut dyn Host,
    t: &Translation,
    it: &mut Iteration,
    r: &mut Replaying,
) -> Option<(Run<bool>, Option<usize>)> {
    loop {
        if matches!(r.stage, ReplayStage::Running) {
            let ran = steps(host, t, it, r.at.take());
            if it.paused {
                return None;
            }
            return Some((ran, None));
        }
        if r.attempts == ATTEMPTS && r.at.is_some() {
            // Past a few attempts, no replay: every part runs.
            let splice = t.arena.borrow_mut().splice.take();
            let Some((resumed, _)) = crate::resume::restore(host, t, it, r.s0) else {
                t.arena.borrow_mut().splice = splice;
                return Some((Ok(false), None));
            };
            t.arena.borrow_mut().splice = splice;
            r.at = Some(resumed);
            r.stage = ReplayStage::Running;
            continue;
        }
        // An attempt begins, unless it is going on after a return.
        if r.at.is_some() {
            r.attempts += 1;
            crate::replay::attempted();
            r.replays = r.next.is_some();
            t.arena.borrow_mut().replay = r.next.take();
        }
        let ran = steps(host, t, it, r.at.take());
        if it.paused {
            return None;
        }
        let replays = r.replays;
        // The last timestep checked as `finish` will end it, if it ran to
        // its end.
        let cut = t
            .arena
            .borrow()
            .replay
            .as_ref()
            .is_some_and(|r| !r.failed.is_empty());
        if ran.is_ok() && replays && !cut {
            let heap = host.heap();
            let mut a = t.arena.borrow_mut();
            if let Some(r) = a.n8.as_ref() {
                let last = r.steps() - 1;
                let ended = r.last_ended(heap, &a);
                let m = ended.moment_of(last);
                let kept = a.n8.replace(alloc::boxed::Box::new(ended));
                if let Some(m) = m {
                    crate::replay::check(&mut a, last, m);
                }
                a.n8 = kept;
            }
        }
        let done = t.arena.borrow_mut().replay.take();
        if let Some(d) = &done {
            t.arena.borrow_mut().failed_parts = d.last_failed().to_vec();
        }
        let Some(done) = done.filter(|r| !r.failed.is_empty() && ran.is_ok()) else {
            if ran.is_ok() && replays {
                // Its replayed parts' translators were not kept: nothing
                // from here on is resumed from or converged at.
                crate::resume::impure_after(t, r.s0);
                let mut a = t.arena.borrow_mut();
                if let Some(n8) = a.n8.as_mut() {
                    let from = n8.checkpoints_before(r.step as usize + 1);
                    n8.forget_from(from);
                }
            }
            return Some((ran, (replays && ran.is_ok()).then_some(r.s0)));
        };
        // Back to where those that failed held what they ran to.
        let (k, replay) = done.back();
        r.next = replay;
        let splice = t.arena.borrow_mut().splice.take();
        let Some((resumed, _)) = crate::resume::restore(host, t, it, k) else {
            t.arena.borrow_mut().splice = splice;
            return Some((Ok(false), None));
        };
        t.arena.borrow_mut().splice = splice;
        r.at = Some(resumed);
    }
}

/// The attempts a replay makes before it runs every part (MS6 step 7b).
const ATTEMPTS: usize = 6;

/// N8's check of the snapshots (`resume.rs`, the harness's `--n8`): the
/// translation restored at its middle snapshot and at its first and run on
/// to its end each time, what N8 keeps, its records and its trace the
/// whole run's; what differs. A replayed run (MS6 step 7b) is restored only
/// where it began, `replayed`, and run on without a replay: its records and
/// trace the replay's.
fn check_snapshots(
    host: &mut dyn Host,
    t: &Translation,
    it: &mut Iteration,
    replayed: Option<usize>,
) -> Vec<String> {
    let whole = crate::resume::summary(host.heap(), &t.arena.borrow(), replayed.is_none());
    // Those of the running compile only: another's marks are not in its
    // log.
    let ours = crate::resume::this_compiles(host.heap(), t);
    let mut wrong = Vec::new();
    let mut at = Vec::new();
    if let Some(s0) = replayed {
        at.push(s0);
    } else if let (Some(&first), Some(&middle)) = (ours.first(), ours.get(ours.len() / 2)) {
        at.push(middle);
        if middle != first {
            at.push(first);
        }
    }
    for k in at {
        let depth = host.heap().root_depth();
        let Some((resumed, step)) = crate::resume::restore(host, t, it, k) else {
            continue;
        };
        let ran = steps(host, t, it, Some(resumed));
        host.heap().unroot_to(depth);
        match ran {
            Err(_) => wrong.push(alloc::format!(
                "restored at snapshot {k} (timestep {step}) and run on: an error"
            )),
            Ok(_) => {
                let again =
                    crate::resume::summary(host.heap(), &t.arena.borrow(), replayed.is_none());
                wrong.extend(crate::resume::differs(k, step, &whole, &again));
            }
        }
    }
    wrong
}

/// N8's checkpoint of the timestep being run, if its translation is
/// recorded and the timestep gets one (`checkpoint.rs`): its number, its
/// moment and `ok`, whether its iterators run, for its snapshot
/// (`resume.rs`), and whether it wants that snapshot. Its state is digested
/// only where a splice can read it (D258): at the first timestep, at a
/// snapshot this run takes and at one the old run took, or under N8's checks.
fn checkpoint(
    heap: &lily_engine_scheme::Heap,
    t: &Translation,
    it: &Iteration,
    top: IterId,
    ok: bool,
) -> Option<((usize, Moment, bool), bool)> {
    // Out of the arena, which is only read while the state is digested:
    // writing a grob's or a context's handle reads it too.
    let mut r = t.arena.borrow_mut().n8.take()?;
    let (taken, wanted) = {
        let a = t.arena.borrow();
        let taken = r.wants_checkpoint(heap, &a);
        let mut wanted = false;
        if taken {
            let now = a.now();
            wanted = r.place.wants(now);
            let old_run = a.splice.as_ref().is_some_and(|s| s.has(now));
            if r.full() || r.steps() == 1 || wanted || old_run {
                r.see(heap, &a);
                let mut each = Vec::new();
                let iterators = it.digest(
                    top,
                    heap,
                    &|key, at| r.music(heap, key, at, false),
                    &|key, at| r.music(heap, key, at, true),
                    &|c| r.parts.key(c),
                    &mut each,
                );
                let skip = |key: u64| a.replay.as_ref().is_some_and(|r| r.is_replayed(key));
                r.checkpoint(heap, &a, iterators, &each, &skip);
            } else {
                r.pass_checkpoint(&a);
            }
        }
        (taken, wanted)
    };
    let step = r.steps() - 1;
    t.arena.borrow_mut().n8 = Some(r);
    let now = t.arena.borrow().now();
    taken.then_some(((step, now, ok), wanted))
}

/// What `%host-call` runs for [`PRELUDE`] (D127).
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "make-global-context",
        f: host_make_global_context,
    },
    HostHandler {
        what: "interpret-music-expression",
        f: host_interpret_music_expression,
    },
];

/// `global-context-scheme.cc`'s two functions, checked by their handlers.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:make-global-context output-def)
  (%host-call 'make-global-context output-def))
(define-public (ly:interpret-music-expression mus ctx)
  (%host-call 'interpret-music-expression mus ctx))
"#;

/// `LY_ASSERT_SMOB`'s error for the argument `v` at `position` of `subr`.
fn wrong_type(vm: &mut Vm, subr: &str, position: i64, v: Value, expecting: &str) -> Exception {
    wrong_type_in(vm.heap_mut(), subr, position, v, expecting)
}

/// `ly:run-translator`'s `LY_ASSERT_SMOB (Music, mus, 1)`, before a score's
/// music is read: a `\score` that lost its music says so as LilyPond's
/// does, not where its length is asked (OPEN F, MS8 step 10).
fn check_music(host: &mut dyn Host, music: Value) -> Run<()> {
    let heap = host.heap();
    if heap.foreign::<crate::Music>(music).is_some() {
        return Ok(());
    }
    Err(wrong_type_in(heap, "ly:run-translator", 1, music, "Music"))
}

fn wrong_type_in(
    heap: &mut lily_engine_scheme::Heap,
    subr: &str,
    position: i64,
    v: Value,
    expecting: &str,
) -> Exception {
    let key = heap.intern("wrong-type-arg");
    let subr = heap.string(subr);
    let message = heap.string("Wrong type argument in position ~A (expecting ~A): ~S");
    let position = heap.integer(position);
    let expecting = heap.string(expecting);
    let items = heap.list(&[position, expecting, v]);
    let rest = heap.list(&[v]);
    let args = heap.list(&[subr, message, items, rest]);
    Exception { key, args }
}

/// `ly:make-global-context`: a Global of `output-def`'s definition, `#f`
/// without one (`Translation::new`'s programming error).
fn host_make_global_context(vm: &mut Vm, args: &[Value]) -> Run<Value> {
    let odef = args[1];
    if vm.heap().foreign::<crate::OutputDef>(odef).is_none() {
        return Err(wrong_type(
            vm,
            "ly:make-global-context",
            1,
            odef,
            "Output_def",
        ));
    }
    Ok(match Translation::new(vm, odef)? {
        Some(t) => t.arena.borrow().handle(GLOBAL),
        None => Value::FALSE,
    })
}

/// `ly:interpret-music-expression`: `mus` iterated in the Global `ctx`,
/// forced (`Global_context::iterate (music, true)`); `ctx`.
fn host_interpret_music_expression(vm: &mut Vm, args: &[Value]) -> Run<Value> {
    const SUBR: &str = "ly:interpret-music-expression";
    let (music, ctx) = (args[1], args[2]);
    if vm.heap().foreign::<crate::Music>(music).is_none() {
        return Err(wrong_type(vm, SUBR, 1, music, "Music"));
    }
    let Some(t) = translation_of(vm.heap(), ctx, GLOBAL) else {
        return Err(wrong_type(vm, SUBR, 2, ctx, "Global_context"));
    };
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(ctx);
    let mut iteration = Iteration::new(vm.heap_mut(), &t);
    let ran = iterate(vm, &t, &mut iteration, music, true);
    vm.heap_mut().unroot_to(depth);
    ran.map(|_| ctx)
}

/// `(ly:context-schedule-moment context moment)`: a moment to stop at,
/// which must not be past.
pub(crate) fn ly_context_schedule_moment(
    cx: &mut lily_engine_scheme::Cx,
    args: &[Value],
) -> crate::R {
    const SUBR: &str = "ly:context-schedule-moment";
    let Some((arena, _)) = crate::context::handle_of(cx.heap(), args[0]) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Context"));
    };
    let Some(&m) = cx.heap().foreign::<Moment>(args[1]) else {
        return Err(cx.wrong_type(SUBR, 2, args[1], "Moment"));
    };
    if !arena.borrow_mut().add_moment(m) {
        crate::warn::diagnose(cx, "programming-error", "trying to freeze in time");
    }
    Ok(Value::UNSPECIFIED)
}
