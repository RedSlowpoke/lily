//! `paper-score.cc` (part 3's, MS3): what `Score_engraver` calls of it,
//! ported first (MS3 step 15): the constructor (`paper-score.cc:35–40`),
//! `typeset_system` (`:52–63`) and `root_system` (`:136–140`), 23 lines,
//! and the accessor `layout` (`:143–146`) that `System::typeset_grob`
//! calls. Its columns and break indices (MS3 step 32):
//! `find_break_indices` (`:65–83`), `get_break_indices` (`:85–91`),
//! `get_columns` (`:93–99`) and `get_break_ranks` (`:101–107`), with
//! `Output_def::c_variable` (`output-def.cc:122–126`) borrowed. What the
//! axis groups' `%break-ranks` and `System::get_pure_bound`
//! (`system.cc:992–1007`, 15 lines, step 33's file) read of them is
//! Scheme that reaches them through `%host-call` (D127, [`HOSTS`]), as is
//! paper-score-scheme.cc's `ly:paper-score-paper-systems`
//! (`paper-score-scheme.cc:22–31`, 10, [`PRELUDE`]). Its line breaking
//! (MS3 step 33): `calc_breaking` ([`calc_breaking`], `:108–122`), whose
//! `Constrained_breaking` chooses the breaks since MS3 step 61, and
//! `get_paper_systems` ([`get_paper_systems`], `:148–160`),
//! which breaks the root system into its lines and makes their paper
//! systems; `process` ([`process`], `:125–134`), which every compile
//! calls after a `\layout` translation. The breaking alone
//! ([`break_systems`]) was a compile's until MS3 step 62, when the page
//! breaker took it over (`page_breaking.rs`).
//!
//! score-scheme.cc's `ly:score-embedded-format` (`:111–141`, MS3 step
//! 43), with which `score-markup` prints a `\score` in a markup: the
//! score translated with its `\layout` and its Paper_score processed
//! (`ly:format-output`), over `%host-call` ([`HOSTS`]). A traced compile
//! lends it its trace while it lays a translation out ([`lend_trace`]),
//! as `trace.ly`'s file is open to every translation of a run.
//!
//! [`PaperScore`] is a Scheme object, as LilyPond's `Music_output` smob
//! is, written `#<Paper_score>`. `process` is a later step's.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Host, OutputDef, Score, Trace, Translation, embedded_layout, host_lookup_variable, run_embedded,
};
use lily_engine_scheme::{Cx, Exception, Foreign, Heap, HostHandler, Tracer, Value, Vm};

use crate::constrained_breaking::{ConstrainedBreaking, VPOS};
use crate::geometry::Direction;
use crate::grob::{grobs, with_grob, with_grob_mut};
use crate::item;
use crate::paper_column;
use crate::simple_spacer::ColumnXPositions;
use crate::system;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `Paper_score`: the handles of its layout definition, its system once
/// one is typeset in it, and its paper systems (`#f` until
/// [`get_paper_systems`] makes them); and what `find_break_indices` last found, LilyPond's mutable
/// `cols_`, `break_indices_` and `break_ranks_`, empty until then.
/// `broken` once [`break_systems`] or the page breaker has begun breaking
/// its system, which LilyPond does once, and `substituted` once its grobs
/// are broken and substituted after (MS3 step 62).
#[derive(Clone)]
pub struct PaperScore {
    pub layout: Value,
    pub system: Option<Value>,
    pub paper_systems: Value,
    pub broken: bool,
    pub substituted: bool,
    pub cols: Vec<Value>,
    pub break_indices: Vec<usize>,
    pub break_ranks: Vec<u64>,
    /// Its cells while it is laid out (Y7, `cell.rs`), none for a score in
    /// a markup, which keeps none.
    pub cells: Option<alloc::rc::Rc<core::cell::RefCell<crate::cell::State>>>,
}

impl Foreign for PaperScore {
    /// `ly:music-output?`'s smob, `Music_output`.
    fn type_name(&self) -> &'static str {
        "Music_output"
    }

    /// `Music_output::print_smob`: its class's name, `#<Paper_score>`.
    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Paper_score>");
    }

    /// A copy for S5's undo log: layout breaks it in place after the
    /// translation, and N8's snapshots give it back as a checkpoint had it
    /// (D241).
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(self.clone()))
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.layout);
        if let Some(system) = self.system {
            tracer.trace(system);
        }
        tracer.trace(self.paper_systems);
        for &c in &self.cols {
            tracer.trace(c);
        }
        if let Some(cells) = &self.cells
            && let Ok(state) = cells.try_borrow()
        {
            state.trace(tracer);
        }
    }
}

impl PaperScore {
    /// `Paper_score (Output_def *)`: no system and no paper systems yet.
    pub fn new(layout: Value) -> PaperScore {
        PaperScore {
            layout,
            system: None,
            paper_systems: Value::FALSE,
            broken: false,
            substituted: false,
            cols: Vec::new(),
            break_indices: Vec::new(),
            break_ranks: Vec::new(),
            cells: None,
        }
    }
}

/// `typeset_system`: `system` the root system of the Paper_score `pscore`
/// if it has none, and either way typeset in it, with its layout.
pub fn typeset_system(heap: &mut Heap, pscore: Value, system: Value) {
    let Some(ps) = heap.foreign_mut::<PaperScore>(pscore) else {
        return;
    };
    ps.system.get_or_insert(system);
    let layout = ps.layout;
    with_grob_mut(heap, system, |g| {
        g.pscore = Some(pscore);
        g.layout = Some(layout);
    });
}

/// `root_system`: the first system typeset in `pscore`.
pub fn root_system(heap: &Heap, pscore: Value) -> Option<Value> {
    heap.foreign::<PaperScore>(pscore)?.system
}

/// `layout`: the handle of `pscore`'s layout definition.
pub fn layout(heap: &Heap, pscore: Value) -> Option<Value> {
    heap.foreign::<PaperScore>(pscore).map(|ps| ps.layout)
}

/// `find_break_indices`: `pscore`'s columns, its root system's used
/// columns ([`system::used_columns_in_range`] over all of them), and
/// among them the breakable ones that are broken where a line would end
/// and begin at them, the first needing no piece on the left and the last
/// none on the right: their indices into the columns and their ranks. A
/// Paper_score with no root system, which LilyPond reads through a null,
/// has no columns.
pub fn find_break_indices(host: &mut dyn Host, pscore: Value) -> Result<(), Exception> {
    let cols = match root_system(host.heap(), pscore) {
        Some(system) => system::used_columns_in_range(host, system, 0, usize::MAX)?,
        None => Vec::new(),
    };
    let mut break_indices = Vec::new();
    let mut break_ranks = Vec::new();
    for (i, &it) in cols.iter().enumerate() {
        if paper_column::is_breakable(host, it)?
            && (i == 0 || item::find_prebroken_piece(host.heap(), it, LEFT).is_some())
            && (i == cols.len() - 1 || item::find_prebroken_piece(host.heap(), it, RIGHT).is_some())
        {
            break_indices.push(i);
            // `it->get_column ()`: a paper column is its own.
            let col = item::get_column(host.heap(), it).unwrap_or(it);
            break_ranks.push(paper_column::rank(host.heap(), col));
        }
    }
    if let Some(ps) = host.heap().foreign_mut::<PaperScore>(pscore) {
        ps.cols = cols;
        ps.break_indices = break_indices;
        ps.break_ranks = break_ranks;
    }
    Ok(())
}

/// `get_break_indices`: the ascending indices into [`get_columns`] of
/// `pscore`'s breakable columns, found ([`find_break_indices`]) while
/// there are none.
pub fn get_break_indices(host: &mut dyn Host, pscore: Value) -> Result<Vec<usize>, Exception> {
    if unfound(host.heap(), pscore, |ps| ps.break_indices.is_empty()) {
        find_break_indices(host, pscore)?;
    }
    Ok(host
        .heap()
        .foreign::<PaperScore>(pscore)
        .map_or_else(Vec::new, |ps| ps.break_indices.clone()))
}

/// `get_columns`: `pscore`'s used columns, found ([`find_break_indices`])
/// while there are none.
pub fn get_columns(host: &mut dyn Host, pscore: Value) -> Result<Vec<Value>, Exception> {
    if unfound(host.heap(), pscore, |ps| ps.cols.is_empty()) {
        find_break_indices(host, pscore)?;
    }
    Ok(host
        .heap()
        .foreign::<PaperScore>(pscore)
        .map_or_else(Vec::new, |ps| ps.cols.clone()))
}

/// `get_break_ranks`: the ascending ranks of `pscore`'s breakable columns,
/// found ([`find_break_indices`]) while there are none.
pub fn get_break_ranks(host: &mut dyn Host, pscore: Value) -> Result<Vec<u64>, Exception> {
    if unfound(host.heap(), pscore, |ps| ps.break_ranks.is_empty()) {
        find_break_indices(host, pscore)?;
    }
    Ok(host
        .heap()
        .foreign::<PaperScore>(pscore)
        .map_or_else(Vec::new, |ps| ps.break_ranks.clone()))
}

/// Whether `pscore` is a Paper_score whose cache `empty` finds empty;
/// not for anything else, which has nothing to find.
fn unfound(heap: &Heap, pscore: Value, empty: impl Fn(&PaperScore) -> bool) -> bool {
    heap.foreign::<PaperScore>(pscore).is_some_and(empty)
}

/// `calc_breaking`: the lines `pscore`'s columns are broken into, by its
/// [`ConstrainedBreaking`]: `solve` into the layout's `system-count`
/// lines, if it is a positive integer, or else its `best_solution`; or,
/// in their place, the lines between the breaks a test gave
/// ([`give_breaks`], MS3 step 60), as `solve` spaces the lines it
/// chooses. LilyPond's "Calculating line breaks..." is not written, as
/// the engine writes no progress.
pub fn calc_breaking(
    host: &mut dyn Host,
    pscore: Value,
) -> Result<Vec<ColumnXPositions>, Exception> {
    let mut algorithm = ConstrainedBreaking::new(host, Some(pscore))?;
    if let Some(at) = given_breaks(host, &algorithm)? {
        return algorithm.given_lines(host, &at);
    }
    let system_count = match layout(host.heap(), pscore) {
        Some(l) => c_variable(host, l, "system-count"),
        None => Value::UNDEFINED,
    };
    let system_count = host
        .heap()
        .to_i64(system_count)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
    if system_count > 0 {
        return algorithm.solve(host, 0, VPOS, system_count);
    }
    algorithm.best_solution(host, 0, VPOS)
}

/// Gives the compiles of `vm` the breaks to break their Paper_scores'
/// lines at until the engine chooses them (MS3 step 60, part 5's
/// `solve`): each a column's `when` as grobs.tsv writes it (`main grace`,
/// two rationals), at which a line ends and the next begins; none for
/// the one line. Not LilyPond's: a test's, which takes LilyPond's breaks
/// from its capture (`line.tsv`).
pub fn give_breaks(vm: &mut Vm, whens: &[String]) {
    let heap = vm.heap_mut();
    let items: Vec<Value> = whens.iter().map(|w| heap.string(w)).collect();
    let given = if items.is_empty() {
        Value::FALSE
    } else {
        heap.list(&items)
    };
    if let Some(cell) = vm
        .slot("lily", "%given-breaks")
        .and_then(|slot| vm.slot_value(slot))
    {
        vm.heap_mut().set_car(cell, given);
    }
}

/// The indices into `algorithm`'s breaks of those [`give_breaks`] gave,
/// in order: each break but the first and the last whose column's `when`
/// is one of them; none if none were given.
pub(crate) fn given_breaks(
    host: &mut dyn Host,
    algorithm: &ConstrainedBreaking,
) -> Result<Option<Vec<usize>>, Exception> {
    let given = host
        .lily("%given-breaks")
        .and_then(|cell| host.heap().car(cell))
        .unwrap_or(Value::FALSE);
    if given == Value::FALSE {
        return Ok(None);
    }
    let mut whens = Vec::new();
    let mut rest = given;
    while let (Some(w), Some(next)) = (host.heap().car(rest), host.heap().cdr(rest)) {
        let mut s = String::new();
        host.heap().display(w, &mut s);
        whens.push(s);
        rest = next;
    }
    let mut at = Vec::new();
    for k in 1..algorithm.breaks.len().saturating_sub(1) {
        let col = algorithm.all[algorithm.breaks[k]];
        let col = item::get_column(host.heap(), col).unwrap_or(col);
        let when = paper_column::when_mom(host, col)?;
        let (mut s, mut grace) = (String::new(), String::new());
        when.main.write(&mut s);
        when.grace.write(&mut grace);
        s.push(' ');
        s.push_str(&grace);
        if whens.contains(&s) {
            at.push(k);
        }
    }
    Ok(Some(at))
}

/// `get_paper_systems`: `pscore`'s paper systems, a vector, made the first
/// time: its root system broken into the lines of [`calc_breaking`]
/// ([`system::break_into_pieces`]), its grobs broken and substituted
/// ([`system::do_break_substitution_and_fixup_refpoints`]), and a paper
/// system made of each broken system ([`system::get_paper_systems`]).
/// LilyPond's "Drawing systems..." is not written, as the engine writes
/// no progress. A Paper_score with no root system, which LilyPond reads
/// through a null, has none: an empty vector. `#f` for what is no
/// Paper_score.
pub fn get_paper_systems(host: &mut dyn Host, pscore: Value) -> Result<Value, Exception> {
    let Some(made) = host
        .heap()
        .foreign::<PaperScore>(pscore)
        .map(|ps| ps.paper_systems)
    else {
        return Ok(Value::FALSE);
    };
    if made != Value::FALSE {
        return Ok(made);
    }
    let depth = host.heap().root_depth();
    host.heap().root(pscore);
    let systems = break_and_draw(host, pscore);
    host.heap().unroot_to(depth);
    let systems = systems?;
    if let Some(ps) = host.heap().foreign_mut::<PaperScore>(pscore) {
        ps.paper_systems = systems;
    }
    Ok(systems)
}

fn break_and_draw(host: &mut dyn Host, pscore: Value) -> Result<Value, Exception> {
    break_systems(host, pscore)?;
    let Some(system) = root_system(host.heap(), pscore) else {
        return Ok(host.heap().vector(Vec::new()));
    };
    system::get_paper_systems(host, system)
}

/// `process` (`paper-score.cc:125–134`): the root system's
/// `pre_processing` ([`system::pre_processing`]), as `ly:format-output`
/// calls it after the translation. LilyPond's debug output of the
/// element count and its "Preprocessing graphical objects..." are not
/// given, as the engine writes no progress. A Paper_score with no root
/// system, which LilyPond reads through a null, has nothing to process.
pub fn process(host: &mut dyn Host, pscore: Value) -> Result<(), Exception> {
    match root_system(host.heap(), pscore) {
        Some(system) => system::pre_processing(host, system),
        None => Ok(()),
    }
}

/// What [`get_paper_systems`] does before it draws (`paper-score.cc:153–
/// 156`): `pscore`'s root system broken into the lines of
/// [`calc_breaking`] ([`system::break_into_pieces`]) and its grobs broken
/// and substituted ([`system::do_break_substitution_and_fixup_refpoints`]),
/// the first time only, whether or not it completes: LilyPond breaks once,
/// and a system broken halfway cannot be broken again; nothing for what is
/// no Paper_score, one with no root system, or one the page breaker broke
/// (`page_breaking.rs`), which breaks a compile's scores since MS3 step 62.
pub fn break_systems(host: &mut dyn Host, pscore: Value) -> Result<(), Exception> {
    match host.heap().foreign_mut::<PaperScore>(pscore) {
        Some(ps) if !ps.broken => ps.broken = true,
        _ => return Ok(()),
    }
    let breaking = calc_breaking(host, pscore)?;
    let Some(system) = root_system(host.heap(), pscore) else {
        return Ok(());
    };
    system::break_into_pieces(host, system, &breaking)?;
    substitute(host, pscore, system)
}

/// [`system::do_break_substitution_and_fixup_refpoints`] of `pscore`'s
/// root system `system`, and `pscore`'s `substituted` once it completes.
pub fn substitute(host: &mut dyn Host, pscore: Value, system: Value) -> Result<(), Exception> {
    system::do_break_substitution_and_fixup_refpoints(host, system)?;
    substituted(host, pscore);
    Ok(())
}

/// `pscore`'s break substitution has completed.
pub fn substituted(host: &mut dyn Host, pscore: Value) {
    if let Some(ps) = host.heap().foreign_mut::<PaperScore>(pscore) {
        ps.substituted = true;
    }
}

/// The Paper_score of the translation `t`: the one its grobs are typeset
/// in, none for a translation that typeset none (a `\midi`'s).
pub fn of_translation(t: &Translation) -> Option<Value> {
    let mut a = t.arena.borrow_mut();
    let g = grobs(&mut a);
    (0..g.len() as u32).find_map(|id| g.get(id).and_then(|g| g.pscore))
}

/// `Output_def::c_variable` (`output-def.cc:122–126`), borrowed:
/// `lookup_variable` of the symbol `name` in `def` or its parents,
/// `SCM_UNDEFINED` if none binds it.
pub fn c_variable(host: &mut dyn Host, def: Value, name: &str) -> Value {
    let sym = host.heap().intern(name);
    host_lookup_variable(host, def, sym).unwrap_or(Value::UNDEFINED)
}

/// `pure_bound`, `System::get_pure_bound` of a system whose Paper_score
/// is `pscore`: the column of the break whose rank is `target_rank` (the
/// first break ranked at or after it, `lower_bound`, if that is it), none
/// for no such break.
pub fn pure_bound(
    host: &mut dyn Host,
    pscore: Value,
    target_rank: u64,
) -> Result<Option<Value>, Exception> {
    let ranks = get_break_ranks(host, pscore)?;
    let indices = get_break_indices(host, pscore)?;
    let cols = get_columns(host, pscore)?;
    let i = ranks.partition_point(|&r| r < target_rank);
    Ok((ranks.get(i) == Some(&target_rank)).then(|| cols[indices[i]]))
}

/// The Paper_score of the system `sys`, `System::paper_score`: none for
/// what is no grob or a system typeset in none.
fn paper_score_of(heap: &Heap, sys: Value) -> Option<Value> {
    with_grob(heap, sys, |g| g.pscore).flatten()
}

/// What `%host-call` runs for [`PRELUDE`] (D127): the break ranks of the
/// Paper_score of a grob's root system, the pure bound of a system, and a
/// Paper_score's paper systems, with the VM as their host.
pub(crate) const HOSTS: [HostHandler; 4] = [
    HostHandler {
        what: "paper-score-break-ranks",
        f: host_break_ranks,
    },
    HostHandler {
        what: "system-pure-bound",
        f: host_pure_bound,
    },
    HostHandler {
        what: "paper-score-paper-systems",
        f: host_paper_systems,
    },
    HostHandler {
        what: "score-embedded-format",
        f: host_score_embedded_format,
    },
];

/// `get_root_system (me)->paper_score ()->get_break_ranks ()`: none for a
/// grob with no root system or a root system typeset in no Paper_score,
/// where LilyPond reads through a null.
fn host_break_ranks(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let pscore = system::get_root_system(vm.heap_mut(), args[1])
        .and_then(|sys| paper_score_of(vm.heap(), sys));
    let ranks = match pscore {
        Some(pscore) => get_break_ranks(vm, pscore)?,
        None => Vec::new(),
    };
    let items = ranks
        .into_iter()
        .map(|r| Value::fixnum(r as i64).unwrap_or(Value::FALSE))
        .collect();
    Ok(vm.heap_mut().vector(items))
}

/// `System::get_pure_bound (d, start, end)`: [`pure_bound`] of the rank
/// `start` for `LEFT`, `end` otherwise, `#f` for none, for a rank no break
/// can have (no exact non-negative fixnum), and for a system typeset in no
/// Paper_score, where LilyPond reads through a null.
fn host_pure_bound(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let target = if args[2].as_fixnum() == Some(LEFT as i64) {
        args[3]
    } else {
        args[4]
    };
    let target = target.as_fixnum().and_then(|n| u64::try_from(n).ok());
    let bound = match (paper_score_of(vm.heap(), args[1]), target) {
        (Some(pscore), Some(target)) => pure_bound(vm, pscore, target)?,
        _ => None,
    };
    Ok(bound.unwrap_or(Value::FALSE))
}

/// `Paper_score::get_paper_systems` ([`get_paper_systems`]);
/// `LY_ASSERT_SMOB (Paper_score …)`'s type error for what is no
/// Paper_score.
fn host_paper_systems(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    match vm.heap().foreign::<PaperScore>(args[1]) {
        Some(_) => get_paper_systems(vm, args[1]),
        None => Err(crate::wrong_type(
            vm.heap_mut(),
            Some("ly:paper-score-paper-systems"),
            1,
            args[1],
            "Paper_score",
        )),
    }
}

/// `ly:score-embedded-format` (`score-scheme.cc:111–141`): `score`
/// translated with the first of its definitions that is a `\layout`, made
/// for the markup's `layout` ([`embedded_layout`]), and its Paper_score
/// processed (`ly:format-output`, [`process`]); `'()` for a score with
/// errors, `#f` for one with no `\layout`. The translation is recorded on
/// the trace lent to the VM, if one is ([`lend_trace`]), as `trace.ly`
/// records it ([`run_embedded`]). Layout's handler translates with
/// layout's classes; the core's, which replaces it, with a `\layout`
/// translation's, the notation crates' among them (A47, MS4 step 17).
fn host_score_embedded_format(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    score_embedded_format(vm, args, crate::CLASSES)
}

/// `ly:score-embedded-format`'s `%host-call` arguments, the score
/// translated with `classes`.
pub fn score_embedded_format(
    vm: &mut Vm,
    args: &[Value],
    classes: &[&'static lily_engine_music::TranslatorClass],
) -> Result<Value, Exception> {
    let (score, od) = (args[1], args[2]);
    let subr = Some("ly:score-embedded-format");
    let Some((error, music, defs)) = vm
        .heap()
        .foreign::<Score>(score)
        .map(|s| (s.error, s.music, s.defs.clone()))
    else {
        return Err(crate::wrong_type(vm.heap_mut(), subr, 1, score, "Score"));
    };
    if vm.heap().foreign::<OutputDef>(od).is_none() {
        return Err(crate::wrong_type(vm.heap_mut(), subr, 2, od, "Output_def"));
    }
    if error {
        return Ok(Value::NULL);
    }
    let layout = vm.heap_mut().intern("layout");
    let Some(def) = defs
        .into_iter()
        .find(|&d| c_variable(vm, d, "output-def-kind") == layout)
    else {
        return Ok(Value::FALSE);
    };
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(score);
    vm.heap_mut().root(od);
    let formatted = embedded_layout(vm, def, od).and_then(|def| {
        vm.heap_mut().root(def);
        let mut trace = lent(vm).and_then(|t| t.take());
        let ran = run_embedded(vm, music, def, trace.as_mut(), classes);
        if let (Some(trace), Some(lent)) = (trace, lent(vm)) {
            *lent = Some(trace);
        }
        let Some(done) = ran? else {
            return Ok(Value::NULL);
        };
        vm.heap_mut().root(done.translation.value);
        match of_translation(&done.translation) {
            Some(pscore) => process(vm, pscore).map(|()| pscore),
            None => Ok(Value::NULL),
        }
    });
    vm.heap_mut().unroot_to(depth);
    formatted
}

/// `(ly:music-output? x)`: a Paper_score, the engine's one
/// `Music_output` in Scheme.
pub(crate) fn is_music_output(cx: &mut Cx, args: &[Value]) -> crate::R {
    Ok(Value::bool(
        cx.heap().foreign::<PaperScore>(args[0]).is_some(),
    ))
}

/// A compile's trace, lent to the VM's `ly:score-embedded-format`
/// ([`lend_trace`]); none while one of its translations records on it,
/// so a score in a markup of a score in a markup is not recorded.
struct LentTrace(Option<Trace>);

impl Foreign for LentTrace {
    fn type_name(&self) -> &'static str {
        "Trace"
    }
}

/// The cell of `(lily)`'s `%lent-trace`, which holds a [`LentTrace`] or
/// `#f`.
fn lent_cell(vm: &mut Vm) -> Option<Value> {
    vm.slot("lily", "%lent-trace")
        .and_then(|slot| vm.slot_value(slot))
}

/// The trace lent to `vm`, if one is.
fn lent(vm: &mut Vm) -> Option<&mut Option<Trace>> {
    let lent = lent_cell(vm).and_then(|cell| vm.heap().car(cell))?;
    vm.heap_mut()
        .foreign_mut::<LentTrace>(lent)
        .map(|t| &mut t.0)
}

/// Lends `trace` to `vm` until [`return_trace`]: the translations that
/// `ly:score-embedded-format` runs meanwhile are recorded on it.
pub fn lend_trace(vm: &mut Vm, trace: Trace) {
    let lent = vm.heap_mut().wrap(LentTrace(Some(trace)));
    if let Some(cell) = lent_cell(vm) {
        vm.heap_mut().set_car(cell, lent);
    }
}

/// The trace [`lend_trace`] lent, with what was recorded on it since.
pub fn return_trace(vm: &mut Vm) -> Trace {
    let trace = lent(vm).and_then(|t| t.take()).unwrap_or_default();
    if let Some(cell) = lent_cell(vm) {
        vm.heap_mut().set_car(cell, Value::FALSE);
    }
    trace
}

/// paper-score-scheme.cc's face and what the axis groups and the spacing
/// specs read of the Paper_score, through [`HOSTS`]:
///
/// - `%break-ranks me`: the ascending break ranks of the Paper_score of
///   `me`'s root system, a vector, empty for none.
/// - `%system-pure-bound sys d start end`, `System::get_pure_bound`: the
///   column of the break ranked `start` (`LEFT`) or `end`, `#f` for none.
/// - `ly:paper-score-paper-systems paper-score`: its paper systems, which
///   [`get_paper_systems`] makes by breaking the System into its lines;
///   checked by the handler.
/// - `ly:score-embedded-format score layout`, checked by the handler; and
///   `%lent-trace`, the cell of the trace a compile lends it.
/// - `%given-breaks`, the cell of the breaks a test gives
///   ([`give_breaks`]), `#f` for none.
pub(crate) const PRELUDE: &str = r#"
(define (%break-ranks me)
  (%host-call 'paper-score-break-ranks me))
(define (%system-pure-bound sys d start end)
  (%host-call 'system-pure-bound sys d start end))
(define-public (ly:paper-score-paper-systems paper-score)
  (%host-call 'paper-score-paper-systems paper-score))
(define-public (ly:score-embedded-format score layout)
  (%host-call 'score-embedded-format score layout))
(define %lent-trace (list #f))
(define %given-breaks (list #f))
"#;
