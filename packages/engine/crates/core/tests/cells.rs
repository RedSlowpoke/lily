//! What Y7 gives back when a mark, a line or a column's reads are at stake
//! (D260): each compile against one from scratch, its pages and its traces
//! byte for byte, each column's skylines among them (`skylines.tsv`).
//!
//! Scores of the kinds the seeded edits do not reach: a staff that takes a
//! mark from the line that carried it, and one that comes to life above it.
//!
//! The tests run one at a time ([`TURN`]): the count of translations read is
//! the process's, and another thread's compile moving it refuses a box. That
//! costs reuse and nothing else, but these tests count what is reused.

use lily_engine_core::{Engine, FILE, MODULE, NEEDS, write_image};
use lily_engine_layout::cell::{Report, set_check};
use lily_engine_parser::source::DATADIR;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

/// Held for as long as a test runs.
static TURN: Mutex<()> = Mutex::new(());

struct Run {
    lily: PathBuf,
    datadir: PathBuf,
    image: Vec<u8>,
    _turn: MutexGuard<'static, ()>,
}

/// None, and a line saying why, without LilyPond or the built packs.
fn run() -> Option<Run> {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return None;
    };
    if !fonts().join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return None;
    }
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    set_check(true);
    let mut vm = support::library_with(&lily, lily_engine_core::install).unwrap();
    let scheme = vm.write_image().expect("the VM is at rest");
    Some(Run {
        datadir: lily.join("../.."),
        lily,
        image: write_image(&scheme),
        _turn: turn,
    })
}

fn fonts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts")
}

impl Run {
    fn engine(&self) -> Engine {
        assert!(self.lily.exists());
        let mut engine = Engine::start(&self.image).expect("the image restores");
        engine.set_trace(true);
        engine
    }

    fn compile(&self, engine: &mut Engine, text: &str) -> (Vec<(String, Vec<u8>)>, Vec<Report>) {
        let root = "/s/a.ly";
        engine.put(FILE, root, Some(text.as_bytes()));
        engine.compile(1, root);
        while engine.step(1e9) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            let data = if kind == MODULE {
                std::fs::read(fonts().join(format!("{name}.pack"))).ok()
            } else {
                let rest = name.strip_prefix(DATADIR).unwrap_or(&name);
                std::fs::read(self.datadir.join(rest.trim_start_matches('/'))).ok()
            };
            engine.put(kind, &name, data.as_deref());
            engine.compile(1, root);
        }
        (engine.outputs().to_vec(), engine.cell_reports().to_vec())
    }

    /// `texts` compiled in turn in one engine, each against one from
    /// scratch; the reports of each.
    fn chain(&self, texts: &[String]) -> Vec<Report> {
        self.chain_but(texts, &[])
    }

    /// [`Run::chain`], the outputs `known` left out of the comparison.
    fn chain_but(&self, texts: &[String], known: &[&str]) -> Vec<Report> {
        let mut engine = self.engine();
        let mut reports = Vec::new();
        for (n, text) in texts.iter().enumerate() {
            let (outputs, report) = self.compile(&mut engine, text);
            let (fresh, _) = self.compile(&mut self.engine(), text);
            let differs: Vec<&str> = outputs
                .iter()
                .filter(|o| !fresh.contains(o) && !known.contains(&o.0.as_str()))
                .map(|o| o.0.as_str())
                .collect();
            assert!(
                differs.is_empty() && outputs.len() == fresh.len(),
                "compile {n} differs from scratch in {differs:?}:\n{text}"
            );
            let report = report.into_iter().next().unwrap_or_default();
            assert_eq!(report.lines_wrong, 0, "compile {n}: {report:?}");
            reports.push(report);
        }
        reports
    }
}

/// Three systems of two staves, a bar number at each system's start and a
/// tempo mark on the second; `top` and `low` the first bars of the second
/// system on each staff.
fn two_staves(top: &str, low: &str) -> String {
    format!(
        "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ c''1 d'' \\break \\tempo 4 = 80 {top} f'' \\break g'' a'' }}
  \\new Staff {{ \\clef bass c1 d \\break {low} f \\break g a }}
>> \\layout {{ }} }}
"
    )
}

/// A cell carries what its group moved after the line breaks: a line whose
/// group places a bar number or a tempo mark is reused, not written across.
#[test]
fn a_cell_carries_the_marks_its_group_moves() {
    let Some(run) = run() else { return };
    let plain = two_staves("e''1", "e1");
    let reports = run.chain(&[
        plain.clone(),
        plain.clone(),
        plain.clone(),
        two_staves("e''1", "b,1"),
        plain,
    ]);
    assert_eq!(reports[0].coupled, 0, "{reports:?}");
    assert_eq!(reports[2].reused, reports[2].cells, "{reports:?}");
    assert!(reports[3].reused > 0, "{reports:?}");
}

/// A staff that starts on the second system, above the line that carried
/// the marks, takes them: the first state of the contexts and the columns
/// are as they were, so only the staves in the cell's key can tell it.
#[test]
fn a_staff_above_takes_the_marks_from_the_cell_that_carried_them() {
    let Some(run) = run() else { return };
    let score = |second: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new Staff = \"low\" {{ c'1 d' \\break \\tempo 4 = 80 {second} f' \\break g' a' }} \\layout {{ }} }}
"
        )
    };
    let plain = score("e'1");
    let above = score("<< e'1 \\new Staff \\with { alignAboveContext = \"low\" } { e''1 } >>");
    run.chain(&[plain.clone(), plain.clone(), above.clone(), plain]);
}

/// Systems of staves that die where they are empty: the mark goes past the
/// dead one to the line below, and a note that moves no column brings the
/// staff to life above that line, so that nothing of the line's own changes.
#[test]
fn a_staff_that_comes_to_life_above_takes_the_mark() {
    let Some(run) = run() else { return };
    let score = |top: &str| {
        format!(
            "\\version \"2.26.0\"
\\layout {{ \\context {{ \\Staff \\RemoveEmptyStaves }} }}
\\score {{ <<
  \\new Staff {{ c''1 d'' \\break {top} s1 \\break e'' f'' }}
  \\new Staff {{ c'1 d' \\break \\tempo 4 = 80 e'1 f' \\break g' a' }}
>> \\layout {{ }} }}
"
        )
    };
    let (dead, alive) = (score("s1"), score("e''1"));
    // OPEN B26: as the staff dies again, the mark passes it to the line below,
    // whose cell is given back as kept without the mark: its group's height
    // and skylines are computed again with it, and that cell is not kept, or
    // the compile where the staff comes to life again would get them.
    run.chain(&[
        dead.clone(),
        dead.clone(),
        alive.clone(),
        alive.clone(),
        dead.clone(),
        dead,
        alive,
    ]);
}

/// A staff's spacing to the next live staff (OPEN B25): the staff below it in
/// its group dies on the second system where it is empty, and the next staff is
/// then one outside the group, spaced otherwise. A note that moves no column
/// brings it to life, so that nothing of the staff above changes but what it
/// read of the next one's axis group.
#[test]
fn a_staff_spaced_to_the_next_live_staff_reads_what_that_staff_holds() {
    let Some(run) = run() else { return };
    let score = |middle: &str| {
        format!(
            "\\version \"2.26.0\"
\\layout {{ \\context {{ \\Staff \\RemoveEmptyStaves }} }}
\\score {{ <<
  \\new StaffGroup <<
    \\new Staff {{ c''1 d'' \\break e'' f'' \\break g'' a'' }}
    \\new Staff {{ c'1 d' \\break {middle} s1 \\break e' f' }}
  >>
  \\new Staff {{ \\clef bass c1 d \\break e f \\break g a }}
>> \\layout {{ }} }}
"
        )
    };
    let (dead, alive) = (score("s1"), score("e'1"));
    run.chain(&[
        dead.clone(),
        dead.clone(),
        alive.clone(),
        alive.clone(),
        dead,
        alive,
    ]);
}

/// A note that moves the staves below it: the columns' skylines are made
/// again from what stayed of their boxes, not computed; and with a line
/// whose offsets read the lines' places (a cross-staff beam, a dynamic
/// between the staves), as computed.
#[test]
fn the_lines_moved_columns_are_made_from_their_boxes() {
    let Some(run) = run() else { return };
    let score = |top: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ c''4 d'' e'' f'' g''1 {top} g'' \\break c''4 d'' e'' f'' g''1 g'' g'' }}
  \\new Staff {{ \\clef bass c4 d e f g1 g g \\break c4 d e f g1 g g }}
>> \\layout {{ }} }}
"
        )
    };
    let (low, high) = (score("g''1"), score("c''''''1"));
    let reports = run.chain(&[low.clone(), low.clone(), high.clone(), low]);
    assert!(reports[2].skylines_moved > 0, "{reports:?}");

    let piano = |top: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new PianoStaff <<
  \\new Staff = \"up\" {{ c''8[ d'' \\change Staff = \"down\" c e] {top} g'' a'' b'' \\break c'''1 }}
  \\new Dynamics {{ s1\\f s\\p s }}
  \\new Staff = \"down\" {{ \\clef bass c1 c c }}
>> \\layout {{ }} }}
"
        )
    };
    let (low, high) = (piano("f''4"), piano("c''''''4"));
    run.chain(&[low.clone(), low.clone(), high.clone(), low]);
}

/// A column holding a mark below the staves, whose pure offset reads where
/// the line it follows is put: its box moves as no box of a line does, so
/// the column is computed. Given back as a line's, its skylines are wrong.
#[test]
fn a_column_whose_mark_reads_a_line_is_computed() {
    let Some(run) = run() else { return };
    let score = |top: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ c''1 {top} \\break c'' c'' }}
  \\new Staff {{ c'1 c' \\break c' c' }}
>> \\layout {{
  \\context {{ \\Score
    \\override BarNumber.direction = #DOWN
    \\override BarNumber.break-visibility = #all-visible
    barNumberVisibility = #all-bar-numbers-visible
  }}
}} }}
"
        )
    };
    let (low, high) = (score("c''1"), score("c''''''1"));
    run.chain(&[low.clone(), low.clone(), high.clone(), low]);
}

/// Dots on a staff line, put in a space by their column after a column's
/// skylines read them (OPEN B27, D268): an edit below them computes their
/// column again on their line given back, which holds the dots where they
/// ended; and an edit that moves their staff makes the columns after it
/// from their boxes, kept as the dots were read. Each column's skylines as
/// from scratch.
#[test]
fn dots_moved_after_their_column_read_them_are_read_where_they_were() {
    let Some(run) = run() else { return };
    let below = |low: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ d''2. d''4 b'2. b'4 \\break d''2. d''4 b'2. b'4 }}
  \\new Staff {{ \\clef bass {low} c4 e2. c4 \\break e2. c4 e2. c4 }}
>> \\layout {{ }} }}
"
        )
    };
    let (e, g) = (below("e2."), below("g2."));
    let reports = run.chain(&[e.clone(), e.clone(), g, e]);
    assert!(reports.iter().any(|r| r.before_places > 0), "{reports:?}");

    let above = |top: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ c''4 d'' e'' f'' g''1 {top} g'' \\break c''4 d'' e'' f'' g''1 g'' g'' }}
  \\new Staff {{ \\clef bass d2. d4 b,2. b,4 b,2. b,4 b,1 \\break d2. d4 b,2. b,4 b,2. b,4 b,1 }}
>> \\layout {{ }} }}
"
        )
    };
    let (g, high) = (above("g''1"), above("c1"));
    let reports = run.chain(&[g.clone(), g.clone(), high, g]);
    assert!(reports.iter().any(|r| r.skylines_moved > 0), "{reports:?}");
}

/// A column whose skylines put what they read taken from the last compile
/// while its staff is laid out again (OPEN B27): the dots of `f''4.` on the
/// top line are put in a space by their column as its skylines read them,
/// and the bar line after them, its next note edited, reads its neighbours'
/// pure heights for its `extra-spacing-height` after that. The column is
/// computed, as from scratch, and the bar line reads the dots put.
#[test]
fn a_column_that_puts_its_dots_is_computed_where_their_staff_is_not_given_back() {
    let Some(run) = run() else { return };
    let score = |next: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new Staff {{ c''4 c'' c'' c'' | f''4. d''8 c''4 b' | {next} | c''1 }} \\layout {{ }} }}
"
        )
    };
    let (e, d) = (score("e'1"), score("d'1"));
    let reports = run.chain(&[e.clone(), e.clone(), d, e]);
    assert!(reports[2].skylines_reused > 0, "{reports:?}");
}

/// A beam's stems down across two staves (OPEN B27): the pure height the
/// first stem keeps for the others reaches the other staff, so it moves when
/// an edit elsewhere on the upper staff moves the lower one away, and a
/// column holding such a stem is computed, not made from its boxes.
#[test]
fn a_stem_whose_height_another_stem_kept_across_the_staves_moves_with_them() {
    let Some(run) = run() else { return };
    let score = |note: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new PianoStaff <<
  \\new Staff = \"up\" {{ \\stemDown c''8[ d'' \\change Staff = \"down\" g e] \\change Staff = \"up\" e''4 f'' | {note} g'' a'' b'' }}
  \\new Staff = \"down\" {{ \\clef bass c1 | c1 }}
>> \\layout {{ }} }}
"
        )
    };
    let (high, low) = (score("c''4"), score("c4"));
    let reports = run.chain(&[high.clone(), high.clone(), low, high]);
    assert!(reports[2].skylines_reused > 0, "{reports:?}");
}

/// An edit in two places, the first not music (OPEN B31, D268): a comment
/// added above and a note changed below, as a replace-all or a second cursor
/// makes them. The music between them is kept by the resume, its places moved
/// with the comment's line: the page's links as from scratch.
#[test]
fn an_edit_in_two_places_moves_the_places_between_them() {
    let Some(run) = run() else { return };
    let score = |comment: &str, last: &str| {
        format!(
            "\\version \"2.26.0\"
{comment}\\score {{ \\new Staff {{
  c''4 d'' e'' f'' | g''1 |
  a''4 b'' c''' d''' | g''1 |
  {last} | c''1 |
}} \\layout {{ }} }}
"
        )
    };
    let (plain, edited) = (score("", "e''1"), score("% one more line\n", "f''1"));
    run.chain(&[plain.clone(), plain.clone(), edited, plain]);
    // Two on one line: a space taken out before a note and one put in after.
    let line = |bar: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new Staff {{ c''1 | d''1 | {bar} | g''1 | a''1 | b''1 }} \\layout {{ }} }}
"
        )
    };
    let (two, one) = (line("e''1  f''1"), line("e''1 f''1 "));
    run.chain(&[two.clone(), two.clone(), one, two]);
}
