//! The data form of a stencil (MS3 step 6), in which
//! `tools/compare/stencils.ly` writes LilyPond's pages and markups as
//! references (06 §3.3), read back into layout's stencils and fonts and
//! written out again.
//!
//! A stencil's data is its X extent, its Y extent and its expression, in
//! which a font is `(%font <file name> <name> <magnification>)`
//! (`ly:font-file-name`, `ly:font-name`, `ly:font-magnification`) and a
//! `grob-cause`'s grob the textedit link LilyPond writes for it, a string.
//! [`read`] gives the stencil, each font the pack's `Open_type_font` of
//! that file name scaled by that magnification ([`font::make_scaled`]), one
//! object for each font and magnification; [`dump`] writes a stencil back.
//! Pango's fonts (`#f` for a file name) stay data: a reference's markup is
//! interpreted again rather than read (MS3 step 8), and [`dump`] writes the
//! engine's as LilyPond's are, `(%font #f "unknown" 1.0)`.
//!
//! The laid-out grobs (MS3 step 25) in `tools/compare/layout.ly`'s form,
//! which a traced compile writes as `layout.tsv` ([`laid_out_lines`]): a
//! line a live grob of the System, grobs.tsv's six fields and then its
//! system, page, offsets, extents and stencil flag, each unreached (`?`,
//! `layout.mjs`' UNREACHED) until part 3's steps place it. Since MS3 step
//! 28 the Y extent is read ([`y_extent_field`]), `?` when the read does
//! not complete: a placeholder, another error, or a stop for the host
//! (D126); since step 33 the X and Y offsets relative to the broken
//! system, once the System is broken into its lines ([`offset_field`]).
//! A read leaves no property `calculation-in-progress` behind, nor an
//! offset cached ([`PRELUDE`]'s `%dump-read`), so that the grobs read
//! after it see no cyclic dependency and no offset of 0; the offsets it
//! computed are given back after it (MS3 step 44, [`give_back_offsets`]),
//! so that page layout sets the staves' as it does untraced, and a traced
//! page is the untraced one. A grob is
//! live if it is after all the reads: a VerticalAxisGroup's Y extent is
//! its suicide, and its elements'.
//!
//! The springs and rods between the columns (MS3 step 31, D128), in
//! `layout.ly`'s other form, which a traced compile writes as
//! `columns.tsv` ([`read_columns`]): after each SpacingSpanner's
//! `springs-and-rods` is read, a line a column with its spacing wishes,
//! springs and rods, the springs and rods unreached where the read does
//! not complete.
//!
//! The columns of each system's line (MS3 step 32, D129), their X offsets
//! in `layout.ly`'s third form, which a traced compile writes as
//! `line.tsv` ([`system_line`]), unreached where the System was not
//! broken into its lines.
//!
//! Since MS3 step 33 a traced compile runs the layout every compile runs
//! (`paper_score::process`) through [`lay_out`], each part a read of its
//! own: the breakable items' prebroken pieces and the rest of
//! `System::pre_processing`. The grobs heard are paired with the System's
//! elements before them, and `columns.tsv` is written after
//! `pre_processing`. Since MS3 step 62 the breaking into lines is the page
//! breaker's, as every compile's, when the book's pages are made, and
//! `layout.tsv` and `line.tsv` are written after the pages ([`laid_out`]),
//! from the broken systems, each placed on its page ([`places`]), as
//! `layout.ly` writes them after page breaking.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_music::{Arena, Host, Translation, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Thrown, Value, Vm};

use crate::font;
use crate::geometry::{Box, Interval, X, Y};
use crate::grob::{self, grobs, is_live_grob};
use crate::grob_array::{self, GrobArray};
use crate::item;
use crate::lily_procedure;
use crate::pango::pango_font;
use crate::paper_score;
use crate::spanner;
use crate::spring::SpringObject;
use crate::stencil::{Stencil, wrap};
use crate::system;
use crate::{R, number_pair_arg};

/// The fonts a [`read`] has made, by file name and magnification.
type Fonts = Vec<(Value, f64, Value)>;

/// The stencil whose data is `x`, `y` and `expr`.
pub fn read(cx: &mut Cx, x: Value, y: Value, expr: Value) -> Result<Stencil, Thrown> {
    const SUBR: &str = "%dump->stencil";
    let (x0, x1) = number_pair_arg(cx, SUBR, 1, x)?;
    let (y0, y1) = number_pair_arg(cx, SUBR, 2, y)?;
    let expr = read_value(cx, expr, &mut Vec::new())?;
    Ok(Stencil::new(
        Box::new(Interval::new(x0, x1), Interval::new(y0, y1)),
        expr,
    ))
}

/// `v` with each font of a pack in it read; what is unchanged is not
/// copied.
fn read_value(cx: &mut Cx, v: Value, fonts: &mut Fonts) -> R {
    if let Some(font) = read_font(cx, v, fonts)? {
        return Ok(font);
    }
    // The spine iteratively, so that a long list does not nest.
    let mut items = Vec::new();
    let mut rest = v;
    let mut changed = false;
    while let (Some(car), Some(cdr)) = (cx.heap().car(rest), cx.heap().cdr(rest)) {
        let item = read_value(cx, car, fonts)?;
        changed |= item != car;
        items.push(item);
        rest = cdr;
        if let Some(font) = read_font(cx, rest, fonts)? {
            rest = font;
            changed = true;
            break;
        }
    }
    if !changed {
        return Ok(v);
    }
    let heap = cx.heap_mut();
    Ok(items
        .into_iter()
        .rev()
        .fold(rest, |tail, item| heap.cons(item, tail)))
}

/// The font `v` is the data of, if it is `(%font <file name> <name>
/// <magnification>)` with a file name; LilyPond's error if no pack gave
/// the file, or if its name is not the font's.
fn read_font(cx: &mut Cx, v: Value, fonts: &mut Fonts) -> Result<Option<Value>, Thrown> {
    let heap = cx.heap();
    let items = list_items(heap, v);
    let [head, file, name, magnification] = items[..] else {
        return Ok(None);
    };
    if heap.name(head) != Some("%font") || heap.str(file).is_none() {
        return Ok(None);
    }
    let Some(m) = heap.to_f64(magnification) else {
        return Ok(None);
    };
    if let Some(&(_, _, font)) = fonts
        .iter()
        .find(|&&(f, fm, _)| fm.to_bits() == m.to_bits() && heap.equal(f, file))
    {
        return Ok(Some(font));
    }
    let file_name = alloc::string::String::from(heap.str(file).unwrap().as_str());
    let otf = font::find_otf_font(cx, &file_name)?;
    let scaled = font::make_scaled(cx.heap_mut(), otf, m);
    let actual = font::metric(cx.heap(), scaled).map(|m| m.font.postscript_name());
    if actual != cx.heap().str(name).map(|s| s.as_str()) {
        return Err(cx.error(
            "misc-error",
            Some("%dump->stencil"),
            "font ~s is not ~s",
            &[file, name],
        ));
    }
    fonts.push((file, m, scaled));
    Ok(Some(scaled))
}

/// The items of a proper list, or none.
fn list_items(heap: &Heap, mut v: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(car), Some(cdr)) = (heap.car(v), heap.cdr(v)) {
        out.push(car);
        v = cdr;
    }
    if v != Value::NULL {
        out.clear();
    }
    out
}

/// The data of `s`: its X extent, its Y extent and its expression.
pub fn dump(heap: &mut Heap, s: &Stencil) -> Value {
    let expr = dump_value(heap, s.expr);
    let [x, y] = [X, Y].map(|a| {
        let i = s.extent(a);
        let (l, r) = (heap.flonum(i.left), heap.flonum(i.right));
        heap.cons(l, r)
    });
    heap.list(&[x, y, expr])
}

/// `v` with each font metric in it written as its data.
fn dump_value(heap: &mut Heap, v: Value) -> Value {
    if pango_font(heap, v).is_some() {
        let head = heap.intern("%font");
        let name = heap.string("unknown");
        let one = heap.flonum(1.0);
        return heap.list(&[head, Value::FALSE, name, one]);
    }
    if let Some(m) = font::metric(heap, v) {
        let name = alloc::string::String::from(m.font.postscript_name());
        let description = m.description;
        let (file, magnification) = (heap.car(description), heap.cdr(description));
        let magnification = magnification.and_then(|m| heap.to_f64(m)).unwrap_or(1.0);
        let head = heap.intern("%font");
        let name = heap.string(name);
        let magnification = heap.flonum(magnification);
        return heap.list(&[head, file.unwrap_or(Value::FALSE), name, magnification]);
    }
    let mut items = Vec::new();
    let mut rest = v;
    let mut changed = false;
    while let (Some(car), Some(cdr)) = (heap.car(rest), heap.cdr(rest)) {
        let item = dump_value(heap, car);
        changed |= item != car;
        items.push(item);
        rest = cdr;
        if font::metric(heap, rest).is_some() || pango_font(heap, rest).is_some() {
            rest = dump_value(heap, rest);
            changed = true;
            break;
        }
    }
    if !changed {
        return v;
    }
    items
        .into_iter()
        .rev()
        .fold(rest, |tail, item| heap.cons(item, tail))
}

// --- the laid-out grobs ---------------------------------------------------------

/// The value of a field not reached: `layout.mjs`' UNREACHED.
const UNREACHED: &str = "?";

/// `layout.ly`'s lines of the root System `system`, its elements'
/// grobs.tsv lines by record number `lines` ([`lay_out`]): a line a live
/// grob, its grobs.tsv line (its original's for a broken copy,
/// [`line_of`]), then its system's index and its page's by `places` (MS3
/// step 62; unreached for a system on no page), its X and Y offsets
/// relative to its broken system ([`offset_field`], MS3 step 33), X
/// extent unreached, its Y extent ([`y_extent_field`]) and its stencil
/// unreached. The grobs are those of each of `system`'s broken systems'
/// `all-elements` in turn, in their order, after a line of the system
/// itself (MS3 step 60, OPEN H9: the System's grobs.tsv line, its index,
/// its page's and its place on the page, its line's offsets, by
/// `places`; its extents and stencil unreached), as `layout.ly` writes
/// each system's, if `broken` (the System was broken into its lines);
/// else its own `all-elements`, the prebroken pieces after the rest, and
/// their offsets unreached: before breaking a Y offset is the unbroken
/// VerticalAlignment's, which has no system (D126), and a system start
/// delimiter's Y extent unreached, its print LilyPond's after breaking
/// (MS3 step 40). Where breaking stopped after the System was broken into
/// its pieces, which then hold its grobs, they are the pieces', their
/// offsets unreached too (MS3 step 49). The grobs are read
/// first, each field in `layout.ly`'s order, and written after, those
/// still live: a read may kill (a VerticalAxisGroup's Y extent its
/// suicide), and LilyPond's layout.tsv has no dead grob. A grob neither
/// heard nor a copy of one is an error.
fn laid_out_lines(
    host: &mut dyn Host,
    system: Value,
    lines: &[Option<&str>],
    broken: bool,
    places: &[Place],
) -> Result<String, String> {
    let mut systems = spanner::broken_intos(host.heap(), system);
    if !broken && systems.is_empty() {
        systems.push(system);
    }
    let sym = host.heap().intern("all-elements");
    let mut live = Vec::new();
    for sys in systems {
        let heap = host.heap();
        let line = line_of(heap, lines, sys).ok_or("no System heard")?;
        live.push((sys, sys, line));
        let all = grob::object_data(heap, sys, sym);
        for elem in grob_array::grobs_of(heap, all) {
            if is_live_grob(heap, elem) {
                let line = line_of(heap, lines, elem).ok_or("a grob not heard")?;
                live.push((elem, sys, line));
            }
        }
    }
    let unreached = || String::from(UNREACHED);
    let mut read = Vec::new();
    for (elem, sys, line) in live {
        let place = places.iter().find(|p| p.system == sys);
        let [sysno, pageno] = match place {
            Some(p) => [p.sysno, p.pageno].map(|n| alloc::format!("{n}")),
            None => [unreached(), unreached()],
        };
        if elem == sys {
            // The system's own line (MS3 step 60, OPEN H9): its place on
            // its page, its line's offsets (MS3 step 62).
            let [x, y] = match place {
                Some(p) => [number_field(p.x), number_field(p.y)],
                None => [unreached(), unreached()],
            };
            read.push((elem, line, sysno, pageno, x, y, unreached()));
            continue;
        }
        let [x, y] = if broken {
            [
                offset_field(host, elem, sys, X)?,
                offset_field(host, elem, sys, Y)?,
            ]
        } else {
            [unreached(), unreached()]
        };
        // A system start delimiter's extent is its print's, which LilyPond
        // makes only once the lines are drawn: before breaking its staves
        // are the unbroken alignment's ("vertical alignment called before
        // line breaking"), and a print then could kill it (MS3 step 40).
        let delimiter = grob::has_interface(host.heap(), elem, "system-start-delimiter-interface");
        let y_extent = if broken || !delimiter {
            y_extent_field(host, elem)?
        } else {
            unreached()
        };
        read.push((elem, line, sysno, pageno, x, y, y_extent));
    }
    // A read may kill grobs read before it: a VerticalAxisGroup's suicide
    // kills its elements too.
    let mut out = String::new();
    for (elem, line, sysno, pageno, x, y, y_extent) in read {
        if is_live_grob(host.heap(), elem) {
            // A grob no engraver announced, its fields read now, as
            // `layout-fields-of` reads them as the pages are written (an
            // original broken since has suicided: its cause `<#f>`).
            let fresh;
            let line = if line.ends_with("\t-\t-") {
                fresh = unheard_lines(host, &[elem]).map_err(|e| alloc::format!("{e:?}"))?;
                fresh.trim_end_matches('\n')
            } else {
                line
            };
            let u = UNREACHED;
            out.push_str(&alloc::format!(
                "{line}\t{sysno}\t{pageno}\t{x}\t{y}\t{u}\t{y_extent}\t{u}\n"
            ));
        }
    }
    Ok(out)
}

/// How many grobs of the Paper_score `pscore` are live, as
/// [`laid_out_lines`] finds them untraced: those of each of its root
/// System's broken systems' `all-elements` once it is broken into its
/// lines, else of its own; 0 without a System. Counted after the pages'
/// stencils, as layout.tsv's are after its reads, for the layout per live
/// grob (R1; progress.md, *How layout per grob is measured*; MS3 step 36).
pub fn live_grobs(heap: &mut Heap, pscore: Value) -> usize {
    let Some(system) = paper_score::root_system(heap, pscore) else {
        return 0;
    };
    let mut systems = spanner::broken_intos(heap, system);
    if systems.is_empty() {
        systems.push(system);
    }
    let sym = heap.intern("all-elements");
    let mut live = 0;
    for sys in systems {
        let all = grob::object_data(heap, sys, sym);
        for elem in grob_array::grobs_of(heap, all) {
            live += usize::from(is_live_grob(heap, elem));
        }
    }
    live
}

/// Why [`system_elements`] could not pair the System's elements with the
/// lines heard: an error, or the live grobs past the last line that no
/// engraver announced, whose lines [`unheard_lines`] makes.
enum Unpaired {
    Error(String),
    Unheard(Vec<Value>),
}

impl From<&str> for Unpaired {
    fn from(e: &str) -> Unpaired {
        Unpaired::Error(e.into())
    }
}

impl From<String> for Unpaired {
    fn from(e: String) -> Unpaired {
        Unpaired::Error(e)
    }
}

/// `layout.ly`'s `layout-fields-of` for a grob no engraver announced: its
/// type, `grobs-cause` and `grobs-link` read now (a dead cause grob
/// `<#f>`), `grobs-columns`, and `-` for its context and engraver.
fn unheard_lines(host: &mut dyn Host, grobs: &[Value]) -> Result<String, Exception> {
    let mut out = String::new();
    for &g in grobs {
        let mut name = String::new();
        if let Some((arena, id)) = grob_of(host.heap(), g) {
            host.heap()
                .display(arena.borrow().records.grobs[id as usize].name, &mut name);
        }
        let cause = grob::property(host, g, "cause")?;
        let heap = host.heap();
        let cause_text = if heap
            .foreign::<lily_engine_music::StreamEvent>(cause)
            .is_some()
        {
            let class = lily_engine_music::event_property(heap, cause, "class");
            let mut t = String::from("<ev ");
            heap.display(heap.car(class).unwrap_or(Value::NULL), &mut t);
            t.push('>');
            t
        } else if grob_of(heap, cause).is_some() {
            let n = crate::call_lily(host, "grob::name", &[cause])?;
            let mut t = String::from("<");
            host.heap().display(n, &mut t);
            t.push('>');
            t
        } else {
            String::from("-")
        };
        // `grobs-link`: the origin of the event reached through grob causes.
        let mut c = cause;
        while grob_of(host.heap(), c).is_some() {
            c = grob::property(host, c, "cause")?;
        }
        let heap = host.heap();
        let link = if heap.foreign::<lily_engine_music::StreamEvent>(c).is_some() {
            let origin = lily_engine_music::event_property(heap, c, "origin");
            match heap.foreign::<lily_engine_music::Input>(origin) {
                Some(input) => {
                    let file = input.name.rsplit('/').next().unwrap_or_default();
                    alloc::format!("{file}:{}:{}:{}", input.line, input.char, input.column)
                }
                None => String::from("-"),
            }
        } else {
            String::from("-")
        };
        let columns = lily_engine_music::grob_columns(host, g);
        out.push_str(&[name.as_str(), &cause_text, &link, &columns, "-", "-"].join("\t"));
        out.push('\n');
    }
    Ok(out)
}

/// The lines [`system_elements`] pairs for a translation whose Score
/// recorded no grob: [`unheard_lines`] of each live element of its root
/// System no piece of another, in their order, then of the System.
fn unrecorded_lines(host: &mut dyn Host, t: &Translation) -> Result<String, Exception> {
    let heap = host.heap();
    let Some(system) =
        paper_score::of_translation(t).and_then(|ps| paper_score::root_system(heap, ps))
    else {
        return Ok(String::new());
    };
    let sym = heap.intern("all-elements");
    let all = grob::object_data(heap, system, sym);
    let elements: Vec<Value> = heap
        .foreign::<GrobArray>(all)
        .map(|a| a.grobs.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&e| {
            grob::with_grob(heap, e, |g| g.original).flatten().is_none()
                && grob::is_live_grob(heap, e)
        })
        .collect();
    let mut lines = unheard_lines(host, &elements)?;
    lines.push_str(&unheard_lines(host, &[system])?);
    Ok(lines)
}

/// The root System of the translation `t`, whose grobs.tsv lines are
/// `announced`, its `all-elements`, and each element with its line. Each
/// grob the Score hears is typeset into the System in the order its line
/// is written (`score_engraver.rs`), the System's own line last, so the
/// nth element is the nth line; an element whose type is not its line's
/// is an error, as is a translation without a System. Then the prebroken
/// pieces, typeset after them, each with its original's line, as
/// `layout.ly` names a piece.
#[allow(clippy::type_complexity)]
fn system_elements<'a>(
    host: &mut dyn Host,
    t: &Translation,
    announced: &'a str,
) -> Result<(Value, Value, Vec<(Value, &'a str)>), Unpaired> {
    let heap = host.heap();
    let system = paper_score::of_translation(t)
        .and_then(|ps| paper_score::root_system(heap, ps))
        .ok_or("no System")?;
    let sym = heap.intern("all-elements");
    let all = grob::object_data(heap, system, sym);
    let elements = heap
        .foreign::<GrobArray>(all)
        .map(|a| a.grobs.clone())
        .unwrap_or_default();
    // The prebroken pieces ([`prebroken_pieces`]), which no engraver
    // announces, are typeset after the rest.
    let original = |heap: &Heap, e: Value| grob::with_grob(heap, e, |g| g.original).flatten();
    let (originals, pieces): (Vec<Value>, Vec<Value>) = elements
        .into_iter()
        .partition(|&e| original(heap, e).is_none());
    let lines: Vec<&str> = announced.lines().collect();
    let Some((last, lines)) = lines.split_last() else {
        return Err("no grobs".into());
    };
    if !last.starts_with("System\t") || lines.len() > originals.len() {
        return Err(alloc::format!(
            "{} elements of the System, {} grobs heard",
            originals.len(),
            lines.len()
        )
        .into());
    }
    // A grob killed as it is made (a SpanBarStub, MS3 step 42) is typeset
    // but not heard: a dead element whose type is not its line's is
    // passed over, as many as the System has elements more than lines.
    let mut unheard = originals.len() - lines.len();
    let mut lines = lines.iter();
    let mut line = lines.next();
    let mut paired = Vec::new();
    // Past the last line heard, the live grobs no engraver announced to
    // the Score: made in a translator's `finalize` (an ambitus's
    // AccidentalPlacement, MS8 step 8's part 2), which `layout.ly` names by
    // their own fields ([`unheard_lines`]).
    let mut tail = Vec::new();
    for &elem in &originals {
        if line.is_none() && grob::is_live_grob(heap, elem) {
            tail.push(elem);
            continue;
        }
        let mut name = String::new();
        if let Some((arena, id)) = grob_of(heap, elem) {
            heap.display(arena.borrow().records.grobs[id as usize].name, &mut name);
        }
        let heard = line.filter(|l| l.split('\t').next() == Some(name.as_str()));
        match heard {
            Some(&l) => {
                paired.push((elem, l));
                line = lines.next();
            }
            None if unheard > 0 && !grob::is_live_grob(heap, elem) => unheard -= 1,
            None => {
                let line = line.copied().unwrap_or_default();
                return Err(alloc::format!("{name} typeset where {line} was heard").into());
            }
        }
    }
    if let Some(line) = line {
        return Err(alloc::format!("{line} heard and not typeset").into());
    }
    if !tail.is_empty() {
        return Err(Unpaired::Unheard(tail));
    }
    for piece in pieces {
        let of = original(heap, piece);
        let line = paired
            .iter()
            .find(|&&(e, _)| Some(e) == of)
            .map(|&(_, l)| l);
        paired.push((piece, line.ok_or("a piece of a grob not heard")?));
    }
    // The System's own line, which its broken systems' lines are named by
    // (MS3 step 60).
    paired.push((system, *last));
    Ok((system, all, paired))
}

// --- the columns' springs and rods -----------------------------------------------

/// What a traced compile writes of the translation `t`'s layout
/// ([`lay_out`]): `columns.tsv`'s lines, the errors that left one empty,
/// each with its file's name, and whether the layout completed
/// (`processed`: its Paper_score processed), as a per-edit compile's
/// would; and what [`laid_out`] needs after the book's pages, where its
/// System is broken into its lines: `layout.tsv`'s and `line.tsv`'s
/// lines, which [`laid_out`] fills.
#[derive(Default)]
pub struct Dumped {
    pub layout: String,
    pub columns: String,
    /// `skylines.tsv`'s lines ([`column_skylines`]).
    pub skylines: String,
    pub line: String,
    pub errors: Vec<String>,
    pub processed: bool,
    /// What the Paper_score's processing threw (its prebroken pieces or its
    /// `pre_processing`), as the untraced compile's processing would throw
    /// it (B33): its key and args rooted above the root depth [`lay_out`]
    /// was called at, for the caller to read and unroot.
    pub thrown: Option<Exception>,
    pub pending: Option<Pending>,
}

/// What [`lay_out`] keeps of a translation for [`laid_out`]: its root
/// System and the grobs.tsv lines heard, `announced`, each element's at
/// its record number, as places in `announced`.
pub struct Pending {
    system: Value,
    announced: String,
    lines: Vec<Option<(usize, usize)>>,
}

impl Pending {
    /// The lines by record number.
    fn lines(&self) -> Vec<Option<&str>> {
        self.lines
            .iter()
            .map(|l| l.map(|(a, b)| &self.announced[a..b]))
            .collect()
    }
}

/// The place of a broken System on its book's pages, as `layout.ly`
/// counts them across the file's books (MS3 step 62, OPEN H9): its index
/// and its page's, each from 0, and its line's `X-offset` and `Y-offset`
/// on its page (0 if unset).
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub system: Value,
    pub sysno: usize,
    pub pageno: usize,
    pub x: f64,
    pub y: f64,
}

/// The places of the systems on the pages of `books` ([`Place`]), in
/// order, the pages' lines' `system-grob`s; a page counted whether or not
/// it has one.
pub fn places(heap: &mut Heap, books: &[Value]) -> Vec<Place> {
    let [lines_sym, grob_sym, x_sym, y_sym] =
        ["lines", "system-grob", "X-offset", "Y-offset"].map(|n| heap.intern(n));
    let mut out = Vec::new();
    let mut pageno = 0;
    for &book in books {
        let pages = heap
            .foreign::<crate::paper_book::PaperBook>(book)
            .map_or(Value::NULL, |b| b.pages);
        for page in list_items(heap, pages) {
            let lines = lily_engine_music::prob_get(heap, page, lines_sym);
            for line in list_items(heap, lines) {
                let sys = lily_engine_music::prob_get(heap, line, grob_sym);
                if grob_of(heap, sys).is_some() {
                    let offset = |s| {
                        let v = lily_engine_music::prob_get(heap, line, s);
                        heap.to_f64(v).unwrap_or(0.0)
                    };
                    let (x, y) = (offset(x_sym), offset(y_sym));
                    let sysno = out.len();
                    out.push(Place {
                        system: sys,
                        sysno,
                        pageno,
                        x,
                        y,
                    });
                }
            }
            pageno += 1;
        }
    }
    out
}

// --- the shapes, for C5 ---------------------------------------------------------

/// Chapter 05's dump for C5 (§3, Appendix A) as `tools/compare/c5dump.ly`
/// writes it from `page-post-process`, which a traced compile writes as
/// `c5.tsv` (MS4 step 6): for each page that `books` give to
/// `page-post-process` ([`post_processed`]), each line's system, numbered
/// across the books, as a line `S`: its number, its page's among the pages
/// post-processed with it, its line's `Y-offset`, the moment of its first
/// column and its number of columns; after it a line `G` for each live
/// grob of its `all-elements` whose ultimate cause is an event with an
/// input location ([`grob::origin`]): its type, the location (its file's
/// base name, line and character), the system's number, its X and Y
/// offsets relative to the system, its shape ([`shape`]) and the index of
/// its line (the VerticalAxisGroup it is in) among the system's, `-1` for
/// none. Each grob's value is read through `%dump-read`, as layout.tsv's,
/// [`UNREACHED`] where the read does not complete, and written as
/// `c5dump.ly`'s `format` writes it: a number rounded to thousandths
/// ([`c5_number`]), a moment's main part as `display` writes a rational.
pub fn shapes(host: &mut dyn Host, books: &[Value]) -> Result<String, String> {
    let mut groups = Vec::new();
    for &book in books {
        post_processed(host.heap(), book, &mut groups);
    }
    let [lines_sym, grob_sym, y_sym, columns_sym, all_sym, when_sym] = [
        "lines",
        "system-grob",
        "Y-offset",
        "columns",
        "all-elements",
        "when",
    ]
    .map(|n| host.heap().intern(n));
    let mut out = String::new();
    let mut sysno = 0;
    for pages in groups {
        for (pageno, page) in pages.into_iter().enumerate() {
            let heap = host.heap();
            let lines = list_items(heap, lily_engine_music::prob_get(heap, page, lines_sym));
            for line in lines {
                let heap = host.heap();
                let sys = lily_engine_music::prob_get(heap, line, grob_sym);
                if grob_of(heap, sys).is_none() {
                    continue;
                }
                let y = lily_engine_music::prob_get(heap, line, y_sym);
                let y = heap.to_f64(y).unwrap_or(0.0);
                let y = c5_number(heap, y);
                let cols = grob::object_data(heap, sys, columns_sym);
                let cols = grob_array::grobs_of(heap, cols);
                let when = match cols.first().and_then(|&c| grob_of(heap, c).map(|g| (c, g))) {
                    Some((c, (arena, id))) => {
                        let when = grob::property_data(heap, &arena, id, c, when_sym);
                        heap.foreign::<lily_engine_music::Moment>(when)
                            .map_or_else(|| String::from("-"), |m| rational_text(m.main))
                    }
                    None => String::from("-"),
                };
                let n = cols.len();
                out.push_str(&alloc::format!("S\t{sysno}\t{pageno}\t{y}\t{when}\t{n}\n"));
                let all = grob::object_data(heap, sys, all_sym);
                let elements = grob_array::grobs_of(heap, all);
                let mut lines = Vec::new();
                for &e in &elements {
                    if grob::grob_name(heap, e) == "VerticalAxisGroup" {
                        lines.push(grob::original(heap, e).unwrap_or(e));
                    }
                }
                for g in elements {
                    if let Some(text) = shape_line(host, g, sys, sysno, &lines)? {
                        out.push_str(&text);
                    }
                }
                sysno += 1;
            }
        }
    }
    Ok(out)
}

/// The `G` line of [`shapes`] of the grob `g` of the system `sys`, whose
/// lines (VerticalAxisGroups, their originals) are `lines`; `None` for a
/// dead grob or one caused by no event with an input location.
fn shape_line(
    host: &mut dyn Host,
    g: Value,
    sys: Value,
    sysno: usize,
    lines: &[Value],
) -> Result<Option<String>, String> {
    let heap = host.heap();
    if !is_live_grob(heap, g) {
        return Ok(None);
    }
    let Some(origin) = grob::origin(heap, g) else {
        return Ok(None);
    };
    let Some(input) = heap.foreign::<lily_engine_music::Input>(origin) else {
        return Ok(None);
    };
    let name = input.name.trim_end_matches('/');
    let file = name.rsplit('/').next().unwrap_or(name);
    let link = alloc::format!("{file}:{}:{}", input.line, input.char);
    let name = grob::grob_name(heap, g);
    let line = line_index(heap, g, lines);
    let [x, y] = [X, Y].map(|a| Value::fixnum(a as i64).unwrap_or(Value::FALSE));
    let coordinate = |heap: &Heap, v: Value| heap.to_f64(v);
    let x = read_grob_value(
        host,
        "ly:grob-relative-coordinate",
        &[g, sys, x],
        coordinate,
    )?;
    let x = number_or_unreached(host.heap(), x.flatten());
    let y_offset = read_grob_value(
        host,
        "ly:grob-relative-coordinate",
        &[g, sys, y],
        coordinate,
    )?;
    let y_offset = number_or_unreached(host.heap(), y_offset.flatten());
    let shape = shape(host, g, &name, y)?;
    Ok(Some(alloc::format!(
        "G\t{name}\t{link}\t{sysno}\t{x}\t{y_offset}\t{shape}\t{line}\n"
    )))
}

/// `c5dump.ly`'s `c5-shape` of the grob `g` of type `name`: a Beam's
/// `positions` and a Stem's Y extent, two numbers, a Slur's, Tie's or
/// PhrasingSlur's `control-points`, each a pair, as lists; `""` for
/// another grob, or a Beam's positions that are no pair. `y` is the Y
/// axis as Scheme has it.
fn shape(host: &mut dyn Host, g: Value, name: &str, y: Value) -> Result<String, String> {
    let pair = |heap: &Heap, v: Value| {
        let (a, b) = (heap.car(v)?, heap.cdr(v)?);
        Some((heap.to_f64(a)?, heap.to_f64(b)?))
    };
    let read = match name {
        "Beam" => {
            let sym = host.heap().intern("positions");
            match read_grob_value(host, "ly:grob-property", &[g, sym], pair)? {
                Some(Some(p)) => Some(alloc::vec![p]),
                Some(None) => return Ok(String::new()),
                None => None,
            }
        }
        "Stem" => read_grob_value(host, "ly:grob-extent", &[g, g, y], pair)?
            .map(|p| p.into_iter().collect()),
        "Slur" | "Tie" | "PhrasingSlur" => {
            let sym = host.heap().intern("control-points");
            read_grob_value(host, "ly:grob-property", &[g, sym], |heap, v| {
                list_items(heap, v)
                    .into_iter()
                    .map(|p| pair(heap, p))
                    .collect::<Option<Vec<_>>>()
            })?
            .flatten()
        }
        _ => return Ok(String::new()),
    };
    let Some(points) = read else {
        return Ok(String::from(UNREACHED));
    };
    let heap = host.heap();
    let points: Vec<String> = points
        .into_iter()
        .map(|(a, b)| {
            let (a, b) = (c5_number(heap, a), c5_number(heap, b));
            match name {
                "Beam" | "Stem" => alloc::format!("{a} {b}"),
                _ => alloc::format!("({a} . {b})"),
            }
        })
        .collect();
    Ok(alloc::format!("({})", points.join(" ")))
}

/// `c5dump.ly`'s `c5-line-of`: the index among `lines` of the original of
/// the first of `g` and its Y parents that is a VerticalAxisGroup, `-1` if
/// none is or it is not among them.
fn line_index(heap: &mut Heap, g: Value, lines: &[Value]) -> i64 {
    let mut p = Some(g);
    while let Some(grob) = p.filter(|&p| grob_of(heap, p).is_some()) {
        if grob::grob_name(heap, grob) == "VerticalAxisGroup" {
            let original = grob::original(heap, grob).unwrap_or(grob);
            return lines
                .iter()
                .position(|&l| l == original)
                .map_or(-1, |i| i as i64);
        }
        p = grob::get_parent(heap, grob, Y);
    }
    -1
}

/// `c5dump.ly`'s `c5-r` of `x` as `format`'s `~a` writes it: rounded to
/// thousandths (`(/ (round (* x 1000)) 1000.0)`, `round` to even), an
/// inexact number as the engine's Scheme writes it, as Guile's.
fn c5_number(heap: &mut Heap, x: f64) -> String {
    let v = heap.flonum(libm::rint(x * 1000.0) / 1000.0);
    let mut out = String::new();
    heap.display(v, &mut out);
    out
}

/// [`c5_number`] of a read's number, [`UNREACHED`] for none.
fn number_or_unreached(heap: &mut Heap, x: Option<f64>) -> String {
    x.map_or_else(|| String::from(UNREACHED), |x| c5_number(heap, x))
}

/// A rational as Scheme's `display` writes it: `n`, or `n/d`.
fn rational_text(r: lily_engine_music::Rational) -> String {
    match r.denominator() {
        1 => alloc::format!("{}", r.numerator()),
        d => alloc::format!("{}/{d}", r.numerator()),
    }
}

/// The pages each `page-post-process` of the book `book` is given, as
/// `Paper_book::pages` calls it (`paper-book.cc:751–806`): its parts' in
/// turn if it prints its parts, else its own, if it made any; each list
/// pushed onto `out`.
fn post_processed(heap: &Heap, book: Value, out: &mut Vec<Vec<Value>>) {
    let Some(b) = heap.foreign::<crate::paper_book::PaperBook>(book) else {
        return;
    };
    if b.print_bookparts {
        for &p in &b.print_elements {
            post_processed(heap, p, out);
        }
    } else if b.pages != Value::FALSE && !b.print_elements.is_empty() {
        out.push(list_items(heap, b.pages));
    }
}

/// The layout of the translation `t`, whose grobs.tsv lines are
/// `announced`, as a per-edit compile runs it before its pages
/// (`paper_score::process`), each part a read of its own through
/// `%dump-read` (D126, [`HOSTS`]), the next run only if it completed: the
/// breakable items' pieces (MS3 step 32, D129) and the rest of
/// `System::pre_processing`. Before them, the root System's
/// `all-elements` paired with the lines heard ([`system_elements`]):
/// breaking, which the page breaker does (MS3 step 62), takes the grobs
/// out of the root System's arrays into its broken systems'. After
/// `pre_processing`, `columns.tsv`'s lines ([`read_columns`]: the root
/// System's columns, which breaking takes too). A file whose lines could
/// not be made is empty, and its error given.
pub fn lay_out(host: &mut dyn Host, t: &Translation, announced: &str) -> Dumped {
    let mut out = Dumped::default();
    // The lines of the grobs no engraver announced, before the System's.
    let extended;
    let mut announced = announced;
    if announced.is_empty() {
        // No grob heard: a Score of another name (a `VaticanaScore`, MS8
        // step 9), which `grobs.ly` does not record. `layout.ly` names each
        // grob by its own fields, the System's too.
        match unrecorded_lines(host, t) {
            Ok(lines) => {
                extended = lines;
                announced = &extended;
            }
            Err(e) => {
                out.errors.push(alloc::format!("layout.tsv: {e:?}"));
                return out;
            }
        }
    } else if let Err(Unpaired::Unheard(tail)) = system_elements(host, t, announced) {
        match unheard_lines(host, &tail) {
            Ok(lines) => {
                let at = announced
                    .trim_end_matches('\n')
                    .rfind('\n')
                    .map_or(0, |i| i + 1);
                extended = alloc::format!("{}{lines}{}", &announced[..at], &announced[at..]);
                announced = &extended;
            }
            Err(e) => {
                out.errors.push(alloc::format!("layout.tsv: {e:?}"));
                return out;
            }
        }
    }
    let (system, all, elements) = match system_elements(host, t, announced) {
        Ok(found) => found,
        Err(Unpaired::Error(e)) => {
            out.errors.push(alloc::format!("layout.tsv: {e}"));
            return out;
        }
        Err(Unpaired::Unheard(_)) => {
            out.errors
                .push("layout.tsv: a grob no engraver announced, past the lines heard".into());
            return out;
        }
    };
    let base = announced.as_ptr() as usize;
    let lines = lines_by_id(host.heap(), &elements)
        .into_iter()
        .map(|l| {
            l.map(|l| {
                (
                    l.as_ptr() as usize - base,
                    l.as_ptr() as usize - base + l.len(),
                )
            })
        })
        .collect();
    let depth = host.heap().root_depth();
    host.heap().root(t.value);
    host.heap().root(all);
    let read = |host: &mut dyn Host, name: &str, out: &mut Dumped, file: &str| match dump_read(
        host,
        name,
        &[system],
    ) {
        Ok(v) => {
            // What its processing threw, which `%dump-read` caught: kept
            // on the System by the host's handler (B33).
            let sym = host.heap().intern(THROWN);
            let thrown = grob::object_data(host.heap(), system, sym);
            if let (None, Some(key), Some(args)) =
                (out.thrown, host.heap().car(thrown), host.heap().cdr(thrown))
            {
                host.heap().root(key);
                host.heap().root(args);
                out.thrown = Some(Exception { key, args });
            }
            v != Value::FALSE
        }
        Err(e) => {
            out.errors.push(alloc::format!("{file}: {e}"));
            false
        }
    };
    let pieces = read(host, "%dump-prebroken-pieces", &mut out, "prebroken pieces");
    let processed = pieces && read(host, "%dump-pre-processing", &mut out, "pre-processing");
    match read_columns(host, system, all, &elements, pieces) {
        Ok((lines, _)) => out.columns = lines,
        Err(e) => out.errors.push(alloc::format!("columns.tsv: {e}")),
    }
    out.skylines = column_skylines(host.heap(), system);
    out.processed = processed;
    out.pending = Some(Pending {
        system,
        announced: String::from(announced),
        lines,
    });
    host.heap().unroot_to(depth);
    if let Some(e) = out.thrown {
        host.heap().root(e.key);
        host.heap().root(e.args);
    }
    out
}

/// What [`lay_out`] left of a translation written once its book's pages
/// are made: `layout.tsv`'s ([`laid_out_lines`]) and `line.tsv`'s
/// ([`system_line`]) lines of its System, broken if its Paper_score's
/// breaking completed (the page breaker's, `paper_score::PaperScore`'s
/// `substituted`), each system placed on its page by `places`
/// ([`places`]), into `out`, with the errors that left one empty. The
/// root System is held by its Paper_score, which the caller keeps.
pub fn laid_out(host: &mut dyn Host, pending: &Pending, places: &[Place], out: &mut Dumped) {
    let system = pending.system;
    let lines = pending.lines();
    let broken = grob::with_grob(host.heap(), system, |g| g.pscore)
        .flatten()
        .and_then(|ps| {
            host.heap()
                .foreign::<paper_score::PaperScore>(ps)
                .map(|ps| ps.substituted)
        })
        .unwrap_or(false);
    match laid_out_lines(host, system, &lines, broken, places) {
        Ok(lines) => out.layout = lines,
        Err(e) => out.errors.push(alloc::format!("layout.tsv: {e}")),
    }
    match system_line(host, system, &lines, broken, places) {
        Ok(lines) => out.line = lines,
        Err(e) => out.errors.push(alloc::format!("line.tsv: {e}")),
    }
}

/// `name`, a procedure of `(lily)`, called on `args` through `%dump-read`:
/// its value, `#f` if it did not complete; a stop for the host undone
/// (`%dump-undo`). An error only if `(lily)` lacks the procedures, or the
/// undo fails.
fn dump_read(host: &mut dyn Host, name: &str, args: &[Value]) -> Result<Value, String> {
    let [read, undo, proc] = ["%dump-read", "%dump-undo", name].map(|name| {
        lily_procedure(host, name).map_err(|_| alloc::format!("{name} is not defined"))
    });
    let (read, undo, proc) = (read?, undo?, proc?);
    let mut call = alloc::vec![proc];
    call.extend_from_slice(args);
    let arena = args.iter().find_map(|&g| grob_of(host.heap(), g));
    let arena = arena.map(|(arena, _)| arena);
    dumping(arena.as_deref(), true);
    let read = match host.call(read, &call) {
        Ok(v) => Ok(v),
        Err(_) => host
            .call(undo, &[])
            .map(|_| Value::FALSE)
            .map_err(|e| e.text(host.heap())),
    };
    dumping(arena.as_deref(), false);
    read
}

/// A read of the dump begun on the grobs of `arena`, or ended: while one
/// runs, their Rust reads go through `(lily)`'s procedures, which
/// `%dump-read` replaces (`grob.rs`' `Grobs::dumping`).
fn dumping(arena: Option<&RefCell<Arena>>, begun: bool) {
    if let Some(arena) = arena {
        let mut a = arena.borrow_mut();
        let grobs = grob::grobs(&mut a);
        grobs.dumping = if begun {
            grobs.dumping + 1
        } else {
            grobs.dumping - 1
        };
        // A read the dump's `catch` ended left no callback running.
        if grobs.dumping == 0 {
            grobs.tracker.current = crate::cell::NONE;
        }
    }
}

/// Each element's grobs.tsv line by its index in the translation's arena.
fn lines_by_id<'a>(heap: &Heap, elements: &[(Value, &'a str)]) -> Vec<Option<&'a str>> {
    let mut lines: Vec<Option<&str>> = Vec::new();
    for &(elem, line) in elements {
        if let Some((_, id)) = grob_of(heap, elem) {
            let id = id as usize;
            if lines.len() <= id {
                lines.resize(id + 1, None);
            }
            lines[id] = Some(line);
        }
    }
    lines
}

/// `layout.ly`'s columns.tsv lines of the root System `system`, whose
/// `all-elements` `all` are `elements`, each with its grobs.tsv line (MS3
/// step 31, D128): after each SpacingSpanner's `springs-and-rods` is read,
/// in `all-elements`' order as LilyPond's `System::pre_processing` reads
/// it, a line a column of the root System's `columns`, each of its
/// prebroken pieces after it, left then right ([`column_line`]). A broken
/// copy, which no engraver announces, is named by its original's line.
/// The springs and rods are unreached when their read does not complete
/// (D126), as `y_extent_field`'s; a read that stopped for the host is
/// undone as there.
pub fn system_columns(
    host: &mut dyn Host,
    system: Value,
    all: Value,
    elements: &[(Value, &str)],
) -> Result<String, String> {
    read_columns(host, system, all, elements, true).map(|(lines, _)| lines)
}

/// [`system_columns`], the springs and rods unreached unless `pieces`, the
/// pieces' read, completed: the lines, and whether every read completed
/// and there was one.
fn read_columns(
    host: &mut dyn Host,
    system: Value,
    all: Value,
    elements: &[(Value, &str)],
    pieces: bool,
) -> Result<(String, bool), String> {
    let lines = lines_by_id(host.heap(), elements);
    let spanners: Vec<Value> = elements
        .iter()
        .filter(|(_, line)| line.starts_with("SpacingSpanner\t"))
        .map(|&(elem, _)| elem)
        .collect();
    let depth = host.heap().root_depth();
    host.heap().root(all);
    let mut out = String::new();
    let mut all_reached = pieces && !spanners.is_empty();
    for spanner in spanners {
        let reached = match dump_read(host, "%dump-springs-and-rods", &[spanner]) {
            Ok(v) => pieces && v != Value::FALSE,
            Err(e) => {
                host.heap().unroot_to(depth);
                return Err(e);
            }
        };
        all_reached &= reached;
        let heap = host.heap();
        let sym = heap.intern("columns");
        let cols = grob::object_data(heap, system, sym);
        let cols = heap
            .foreign::<GrobArray>(cols)
            .map(|a| a.grobs.clone())
            .unwrap_or_default();
        for col in cols {
            let pieces = [-1, 1].map(|d| item::find_prebroken_piece(heap, col, d));
            for c in core::iter::once(col).chain(pieces.into_iter().flatten()) {
                column_line(heap, &lines, c, reached, &mut out);
            }
        }
    }
    host.heap().unroot_to(depth);
    Ok((out, all_reached))
}

/// Each column's `horizontal-skylines` as line breaking has them, of the root
/// System `system`, and its pieces': `rank  side  skylines`, the skylines'
/// numbers shortest round-trip, so that an output holds them bit for bit (the
/// differential's C4, D260). Nothing here is LilyPond's to compare with:
/// `layout.ly` writes no such file.
fn column_skylines(heap: &mut Heap, system: Value) -> String {
    let (columns, sym) = (heap.intern("columns"), heap.intern("horizontal-skylines"));
    let cols = grob::object_data(heap, system, columns);
    let cols = heap
        .foreign::<GrobArray>(cols)
        .map(|a| a.grobs.clone())
        .unwrap_or_default();
    let mut out = String::new();
    for col in cols {
        let rank = crate::paper_column::rank(heap, col);
        let pieces = [-1, 1].map(|d| (d, item::find_prebroken_piece(heap, col, d)));
        for (side, c) in core::iter::once((0, Some(col))).chain(pieces) {
            let Some(c) = c else {
                continue;
            };
            let Some((arena, id)) = grob_of(heap, c) else {
                continue;
            };
            let v = grob::property_data(heap, &arena, id, c, sym);
            if let Ok(Some(pair)) = crate::skylines::skyline_pair_in(heap, v) {
                out.push_str(&alloc::format!("{rank}\t{side}\t{pair:?}\n"));
            }
        }
    }
    out
}

// --- the line's columns ------------------------------------------------------

/// `layout.ly`'s line.tsv lines of the root System `system`, its
/// elements' grobs.tsv lines by record number `lines` (MS3 step 32, D129):
/// `system  column  x`, a line a column of a system's line, named as
/// columns.tsv names it. If `broken` (the System was broken into its
/// lines), each broken system's `columns` in turn, `system` its index
/// across the book's pages by `places` (its index among the System's
/// pieces if it is on none) and `x` each one's offset from it, as
/// `layout.ly` writes them; else the
/// Paper_score's used columns from its first break to its last, the first
/// and the last their pieces on the line's side, `system` 0 and `x`
/// [`UNREACHED`]: no line for fewer than two breaks, nor for a System
/// typeset in no Paper_score.
fn system_line(
    host: &mut dyn Host,
    system: Value,
    lines: &[Option<&str>],
    broken: bool,
    places: &[Place],
) -> Result<String, String> {
    let mut out = String::new();
    if broken {
        let sym = host.heap().intern("columns");
        for (k, sys) in spanner::broken_intos(host.heap(), system)
            .into_iter()
            .enumerate()
        {
            let k = places
                .iter()
                .find(|p| p.system == sys)
                .map_or(k, |p| p.sysno);
            let cols = grob::object_data(host.heap(), sys, sym);
            for col in grob_array::grobs_of(host.heap(), cols) {
                let x = match grob::relative_coordinate(host, col, sys, X) {
                    Ok(x) => number_field(x),
                    Err(_) => String::from(UNREACHED),
                };
                let name = column_name(host.heap(), lines, col);
                out.push_str(&alloc::format!("{k}\t{name}\t{x}\n"));
            }
        }
        return Ok(out);
    }
    let Some(pscore) = grob::with_grob(host.heap(), system, |g| g.pscore).flatten() else {
        return Ok(out);
    };
    let text = |e: Exception, host: &mut dyn Host| e.text(host.heap());
    let breaks = paper_score::get_break_indices(host, pscore).map_err(|e| text(e, host))?;
    let cols = paper_score::get_columns(host, pscore).map_err(|e| text(e, host))?;
    let [first, .., last] = breaks[..] else {
        return Ok(out);
    };
    for (i, &col) in cols.iter().enumerate().take(last + 1).skip(first) {
        let d = if i == first {
            1
        } else if i == last {
            -1
        } else {
            0
        };
        let Some(col) = item::find_prebroken_piece(host.heap(), col, d) else {
            continue;
        };
        let name = column_name(host.heap(), lines, col);
        out.push_str(&alloc::format!("0\t{name}\t{UNREACHED}\n"));
    }
    Ok(out)
}

/// The grobs.tsv line of `g`, its original's for a broken copy, if it was
/// heard.
fn line_of<'a>(heap: &Heap, lines: &[Option<&'a str>], g: Value) -> Option<&'a str> {
    let (arena, id) = grob_of(heap, g)?;
    let original = grobs(&mut arena.borrow_mut())
        .get(id)
        .and_then(|g| g.original);
    match original {
        Some(o) => line_of(heap, lines, o),
        None => lines.get(id as usize).copied().flatten(),
    }
}

/// `layout.ly`'s `type@columns@more` of `g`: its grobs.tsv type and
/// columns, as `layout-fields-of` gives them; `-` for a grob not heard,
/// whose name and columns `layout.ly` writes (no engraver of the engine's
/// makes a spacing wish it does not announce).
fn named(heap: &Heap, lines: &[Option<&str>], g: Value, more: &str) -> String {
    let fields: Vec<&str> =
        line_of(heap, lines, g).map_or_else(Vec::new, |l| l.split('\t').collect());
    let field = |i: usize| fields.get(i).copied().unwrap_or("-");
    alloc::format!("{}@{}@{more}", field(0), field(3))
}

/// `layout.ly`'s `columns-name`: the column's type, columns and break
/// direction.
fn column_name(heap: &Heap, lines: &[Option<&str>], col: Value) -> String {
    let dir = item::break_status_dir(heap, col);
    named(heap, lines, col, &alloc::format!("{dir}"))
}

/// `layout.ly`'s `columns-line` of the column `col`: its name, each grob of
/// its `spacing-wishes` (type, columns and context), each of its
/// `ideal-distances` (the other column, the spring's ideal and minimum
/// distances and inverse stretch and compress strengths) and each of its
/// `minimum-distances` (the other column and the distance), each list in
/// its order, `|` between entries and `-` for none; the springs and rods
/// [`UNREACHED`] unless `reached`.
fn column_line(
    heap: &mut Heap,
    lines: &[Option<&str>],
    col: Value,
    reached: bool,
    out: &mut String,
) {
    let list = |entries: Vec<String>| {
        if entries.is_empty() {
            String::from("-")
        } else {
            entries.join("|")
        }
    };
    let sym = heap.intern("spacing-wishes");
    let wishes = grob::object_data(heap, col, sym);
    let wishes = heap
        .foreign::<GrobArray>(wishes)
        .map(|a| a.grobs.clone())
        .unwrap_or_default();
    let wishes = list(
        wishes
            .into_iter()
            .map(|w| {
                let context = line_of(heap, lines, w)
                    .and_then(|l| l.split('\t').nth(4))
                    .unwrap_or("-");
                named(heap, lines, w, context)
            })
            .collect(),
    );
    let (springs, rods) = if reached {
        let entries = |heap: &mut Heap, name: &str| {
            let sym = heap.intern(name);
            let mut s = grob::object_data(heap, col, sym);
            let mut entries = Vec::new();
            while let (Some(entry), Some(rest)) = (heap.car(s), heap.cdr(s)) {
                entries.push(entry);
                s = rest;
            }
            entries
        };
        let springs = entries(heap, "ideal-distances")
            .into_iter()
            .filter_map(|e| {
                let (spring, other) = (heap.car(e)?, heap.cdr(e)?);
                let s = heap.foreign::<SpringObject>(spring)?.0;
                let numbers = [
                    s.ideal_distance,
                    s.min_distance,
                    s.inverse_stretch_strength,
                    s.inverse_compress_strength,
                ];
                let numbers: Vec<String> = numbers.into_iter().map(number_field).collect();
                Some(alloc::format!(
                    "{} {}",
                    column_name(heap, lines, other),
                    numbers.join(" ")
                ))
            })
            .collect();
        let rods = entries(heap, "minimum-distances")
            .into_iter()
            .filter_map(|e| {
                let (other, d) = (heap.car(e)?, heap.to_f64(heap.cdr(e)?)?);
                Some(alloc::format!(
                    "{} {}",
                    column_name(heap, lines, other),
                    number_field(d)
                ))
            })
            .collect();
        (list(springs), list(rods))
    } else {
        (String::from(UNREACHED), String::from(UNREACHED))
    };
    let name = column_name(heap, lines, col);
    out.push_str(&alloc::format!("{name}\t{wishes}\t{springs}\t{rods}\n"));
}

/// The Y extent of the grob `grob` as `layout.ly` writes it, `(ly:grob-extent
/// grob grob Y)` ([`extent_field`]), or [`UNREACHED`] when the read does not
/// complete (D126): a placeholder's `misc-error`, another error (a font no
/// pack has), or a stop for the host (a part's file, D74), which cancels
/// it past every `catch`. The grob's callbacks are called by `%dump-read`,
/// which leaves none of them `calculation-in-progress`. An error only if
/// `(lily)` lacks the procedures.
pub fn y_extent_field(host: &mut dyn Host, grob: Value) -> Result<String, String> {
    let y = Value::fixnum(Y as i64).unwrap_or(Value::FALSE);
    read_field(host, "ly:grob-extent", &[grob, grob, y], extent_field)
}

/// The offset of the grob `grob` on `axis` relative to `system` as
/// `layout.ly` writes it, `(ly:grob-relative-coordinate grob system axis)`
/// ([`number_field`]), or [`UNREACHED`] when the read does not complete,
/// as [`y_extent_field`]'s (MS3 step 33).
pub fn offset_field(
    host: &mut dyn Host,
    grob: Value,
    system: Value,
    axis: usize,
) -> Result<String, String> {
    let axis = Value::fixnum(axis as i64).unwrap_or(Value::FALSE);
    read_field(
        host,
        "ly:grob-relative-coordinate",
        &[grob, system, axis],
        |heap, x| {
            heap.to_f64(x)
                .map_or_else(|| String::from("-"), number_field)
        },
    )
}

/// `(name args …)` through `%dump-read`, its value written by `field`, or
/// [`UNREACHED`] when the read does not complete; a stop for the host
/// undone (`%dump-undo`). The offsets of `args[0]`'s translation that the
/// read computed are given back after it ([`give_back_offsets`]). An error
/// only if `(lily)` lacks the procedures.
fn read_field(
    host: &mut dyn Host,
    name: &str,
    args: &[Value],
    field: fn(&Heap, Value) -> String,
) -> Result<String, String> {
    let text = read_grob_value(host, name, args, field)?;
    Ok(text.unwrap_or_else(|| String::from(UNREACHED)))
}

/// [`read_field`]'s read, its value taken by `value` before anything else
/// is read: `None` when the read does not complete.
fn read_grob_value<T>(
    host: &mut dyn Host,
    name: &str,
    args: &[Value],
    value: impl FnOnce(&Heap, Value) -> T,
) -> Result<Option<T>, String> {
    let [read, undo, proc] = ["%dump-read", "%dump-undo", name].map(|name| {
        lily_procedure(host, name).map_err(|_| alloc::format!("{name} is not defined"))
    });
    let (read, undo, proc) = (read?, undo?, proc?);
    let mut call = alloc::vec![proc];
    call.extend_from_slice(args);
    let arena = grob_of(host.heap(), args[0]).map(|(arena, _)| arena);
    let uncomputed = arena.as_deref().map(uncomputed_offsets).unwrap_or_default();
    dumping(arena.as_deref(), true);
    let read = match host.call(read, &call) {
        Ok(Value::FALSE) => Ok(None),
        Ok(v) => Ok(Some(value(host.heap(), v))),
        // Stopped for the host: what `%dump-read` could not undo.
        Err(_) => host
            .call(undo, &[])
            .map(|_| None)
            .map_err(|e| e.text(host.heap())),
    };
    dumping(arena.as_deref(), false);
    let read = read?;
    if let Some(arena) = arena {
        give_back_offsets(host.heap(), &arena, &uncomputed);
    }
    Ok(read)
}

/// Each grob of `arena` and axis on which its offset is not computed.
fn uncomputed_offsets(arena: &RefCell<Arena>) -> Vec<(u32, usize)> {
    let made = arena.borrow().records.grobs.len() as u32;
    (0..made)
        .flat_map(|id| [X, Y].map(|a| (id, a)))
        .filter(|&(id, a)| grob::with_dim(arena, id, a, |d| d.offset.is_none()))
        .collect()
}

/// The offsets `uncomputed` before a read that the read computed, given
/// back: each one's value set as its `X-offset` or `Y-offset`, which
/// `get_offset` reads again to the same value, and the offset no longer
/// kept. A read computes them before the pages, where an untraced compile
/// computes the staves' only after page layout has set them
/// (`translate_axis`, which adds to a computed one): read so, a traced
/// page would be another than the untraced one (MS3 step 44). A callback's
/// translation of a grob (as `positioning-done` moves a VerticalAlignment's
/// staves) is given back as its `get_offset` is.
fn give_back_offsets(heap: &mut Heap, arena: &RefCell<Arena>, uncomputed: &[(u32, usize)]) {
    let syms = ["X-offset", "Y-offset"].map(|name| heap.intern(name));
    for &(id, a) in uncomputed {
        if let Some(offset) = grob::with_dim(arena, id, a, |d| d.offset.take()) {
            let v = heap.flonum(offset);
            grob::set_property_data(heap, arena, id, syms[a], v);
        }
    }
}

/// `layout.ly`'s `layout-extent`: the two ends of the extent `e` with a
/// space between ([`number_field`]), or `-` for an empty extent or what is
/// no pair.
fn extent_field(heap: &Heap, e: Value) -> String {
    match (
        heap.car(e).and_then(|l| heap.to_f64(l)),
        heap.cdr(e).and_then(|r| heap.to_f64(r)),
    ) {
        (Some(l), Some(r)) if l <= r => alloc::format!("{} {}", number_field(l), number_field(r)),
        _ => String::from("-"),
    }
}

/// `layout.ly`'s `layout-number`: `+inf`, `-inf`, or `(ly:format "~4f" (+
/// 0.0 x))`, `%.4f` of the binary value as printf rounds it (a negative
/// zero made positive), `format_single_argument`'s 0 for a NaN.
fn number_field(x: f64) -> String {
    match x {
        f64::INFINITY => String::from("+inf"),
        f64::NEG_INFINITY => String::from("-inf"),
        x if x.is_nan() => String::from("0.0000"),
        x => alloc::format!("{:.4}", x + 0.0),
    }
}

// --- the Scheme face ---------------------------------------------------------------

/// `(%dump->stencil x-extent y-extent expr)`: the stencil of a reference's
/// data.
pub(crate) fn from_dump(cx: &mut Cx, args: &[Value]) -> R {
    let s = read(cx, args[0], args[1], args[2])?;
    wrap(cx, s)
}

/// `(%stencil->dump stencil)`: `(x-extent y-extent expr)`, as a reference
/// writes it.
pub(crate) fn to_dump(cx: &mut Cx, args: &[Value]) -> R {
    let Some(&s) = cx.heap().foreign::<Stencil>(args[0]) else {
        return Err(cx.wrong_type("%stencil->dump", 1, args[0], "Stencil"));
    };
    Ok(dump(cx.heap_mut(), &s))
}

/// `(%dump-forget-offset! grob a)`: the grob's offset on `a` no longer
/// cached, as before `get_offset` first read it ([`PRELUDE`]'s
/// `%dump-offset`).
pub(crate) fn forget_offset(cx: &mut Cx, args: &[Value]) -> R {
    let Some((arena, id)) = grob_of(cx.heap(), args[0]) else {
        return Err(cx.wrong_type("%dump-forget-offset!", 1, args[0], "Grob"));
    };
    let a = crate::axis_arg(cx, "%dump-forget-offset!", 2, args[1])?;
    grob::with_dim(&arena, id, a, |d| d.offset = None);
    Ok(Value::UNSPECIFIED)
}

/// The reads of the dump (MS3 step 28). `%grob-try-callback` sets a
/// property `calculation-in-progress` while its callback runs and gives
/// it the value after; when the callback throws, as LilyPond's
/// `Grob::try_callback` the property is left so, and any later read of it
/// is a cyclic dependency. LilyPond's run ends with the error; the dump's
/// goes on to the next grob. So `%dump-read` calls `proc` on `args` with
/// `%dump-try` in place of `%grob-try-callback`, which keeps each callback
/// it is in (innermost first, `%dump-pending`) with the property's data
/// as it was, and puts the data back when the callback throws, before the
/// throw goes on: a callback's own `catch` sees the property as it was. It
/// gives `proc`'s value, or `#f` if it threw. A stop for the host cancels
/// the run past every `catch`; `%dump-undo` then puts back what the
/// callbacks left, and `%grob-try-callback`. An offset is `get_offset`'s
/// cache, set to 0 before its property is read (MS3 step 33):
/// `%dump-offset`, in place of `%grob-offset`, forgets it again when the
/// read throws, so that a later read of it is not 0 but unreached.
/// `%dump-springs-and-rods` is [`read_columns`]' read, `#t` once the
/// grob's `springs-and-rods` is read, whatever its value; and
/// [`lay_out`]'s two, each `#t` once it is done ([`HOSTS`]):
/// `%dump-prebroken-pieces` once the System's breakable items' pieces are
/// made, and `%dump-pre-processing` once the rest of its `pre_processing`
/// is.
pub(crate) const PRELUDE: &str = r#"
(define %dump-pending '())
(define %dump-grob-try-callback %grob-try-callback)
(define (%dump-try store grob sym proc default)
  (let ((data (if (eq? store %grob-set-property-data!) (ly:grob-property-data grob sym) proc)))
    (set! %dump-pending (cons (list store grob sym data) %dump-pending))
    (let ((value (catch #t
                   (lambda () (%dump-grob-try-callback store grob sym proc default))
                   (lambda (key . args)
                     (store grob sym data)
                     (set! %dump-pending (cdr %dump-pending))
                     (apply throw key args)))))
      (set! %dump-pending (cdr %dump-pending))
      value)))
(define %dump-grob-offset %grob-offset)
(define (%dump-offset grob a)
  (or (%grob-offset-begin grob a)
      (begin
        (set! %dump-pending (cons (list %dump-forget-offset! grob a) %dump-pending))
        (let ((value (catch #t
                       (lambda () (%grob-offset-end grob a (ly:grob-property grob (if (eqv? a 0) 'X-offset 'Y-offset))))
                       (lambda (key . args)
                         (%dump-forget-offset! grob a)
                         (set! %dump-pending (cdr %dump-pending))
                         (apply throw key args)))))
          (set! %dump-pending (cdr %dump-pending))
          value))))
(define (%dump-undo)
  (for-each (lambda (p) (apply (car p) (cdr p))) %dump-pending)
  (set! %dump-pending '())
  (set! %grob-try-callback %dump-grob-try-callback)
  (set! %grob-offset %dump-grob-offset))
(define (%dump-springs-and-rods grob)
  (ly:grob-property grob 'springs-and-rods)
  #t)
(define (%dump-prebroken-pieces sys)
  (%host-call 'dump-prebroken-pieces sys)
  #t)
(define (%dump-pre-processing sys)
  (%host-call 'dump-pre-processing sys)
  #t)
(define (%dump-read proc . args)
  (set! %grob-try-callback %dump-try)
  (set! %grob-offset %dump-offset)
  (let ((value (catch #t (lambda () (apply proc args)) (lambda (key . args) #f))))
    (%dump-undo)
    value))
"#;

/// What `%host-call` runs for [`PRELUDE`] (D127): the parts of a
/// System's layout that [`lay_out`] reads one by one, as a per-edit
/// compile runs them.
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "dump-prebroken-pieces",
        f: host_prebroken_pieces,
    },
    HostHandler {
        what: "dump-pre-processing",
        f: host_pre_processing,
    },
];

/// [`system::make_prebroken_pieces`] of the System `args[1]`:
/// `pre_processing`'s first three scopes.
fn host_prebroken_pieces(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let made = system::make_prebroken_pieces(vm, args[1]);
    keep_thrown(vm, args[1], made)
}

/// [`system::read_before_breaking`] of the System `args[1]`:
/// `pre_processing`'s last two scopes.
fn host_pre_processing(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let read = system::read_before_breaking(vm, args[1]);
    keep_thrown(vm, args[1], read)
}

/// The System's object that keeps what its processing threw in a traced
/// compile, `(key . args)`, for [`lay_out`] to take once `%dump-read` has
/// caught it (B33): interned only then, so the image holds no more.
const THROWN: &str = "%dump-thrown";

/// `done`, its error kept on the System `system` ([`THROWN`]).
fn keep_thrown(
    vm: &mut Vm,
    system: Value,
    done: Result<(), Exception>,
) -> Result<Value, Exception> {
    if let Err(e) = done {
        let heap = vm.heap_mut();
        let sym = heap.intern(THROWN);
        let thrown = heap.cons(e.key, e.args);
        grob::set_object(heap, system, sym, thrown);
        return Err(e);
    }
    Ok(Value::UNSPECIFIED)
}
