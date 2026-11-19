//! The staff symbol and the staff positions of its referencers (MS3 step
//! 27), and what their callbacks call of other parts, ported first:
//!
//! - `hara-kiri-group-spanner.cc`'s removal of empty groups: whether a
//!   group is empty between two columns (`request_suicide`) and once the
//!   lines are broken (`unpure_request_suicide`), each by itself and by its
//!   `make-dead-when` and `keep-alive-with`; and `consider_suicide`, which
//!   kills an empty group with all its children.
//! - `page-layout-problem.cc`'s `is_spaceable`.
//! - `staff-symbol.cc`'s callbacks: the staff symbol's line positions, its
//!   height and its stencil, whose ends `paper-column.cc`'s
//!   `break_align_width` gives at a bound with an X extent.
//! - `staff-symbol-referencer.cc` and its Scheme face: a grob's staff
//!   position, its Y offset by `ly:staff-symbol-referencer::callback`, and
//!   the staff space, line thickness and staff radius read through it.
//! - `self-alignment-interface.cc`'s five callbacks: a grob aligned on its
//!   own extent, pure and not, centered on its X parent's, and aligned on
//!   its X parent's, a paper column's note columns, its
//!   `X-alignment-extent` or a note column's `main-extent`.
//! - `ly:pure-from-neighbor-interface::calc-pure-relevant-grobs`: a grob's
//!   neighbors, or its original's, ordered by `outside-staff-priority`.
//! - The axis groups' Scheme face: `ly:relative-group-extent` and
//!   `ly:generic-bound-extent`, the union of the elements' extents.
//! - The staff grouper: the extremal live staff meeting an interval, and
//!   whether a child is within the group.
//! - `page-layout-problem.cc`'s spacing specs (MS3 step 28): a number of a
//!   spec, the spec between two staves or loose lines, and the spacing
//!   `alignment-distances` forces, with the spanner's pure property cache.
//! - The axis groups' extents and pure heights (MS3 step 28): a group's
//!   extent from its elements', its common refpoints, its pure height
//!   from its relevant grobs or, as an alignment's child, from its
//!   adjacent pure heights by break, and those heights, with an
//!   outside-staff grob put beyond the staff and the cyclic dependency
//!   T29 shows.
//! - The axis groups' skylines and staff spacing (MS3 step 28): the
//!   elements' skylines combined at their offsets; a group's skylines with
//!   its outside-staff grobs placed beyond the staff and each other, by
//!   priority, in the order of the placement directive, with their riders,
//!   and the warnings; the move that places one; and the staff-staff
//!   spacing of the grouper or of the group, pure and not.
//! - `VerticalAxisGroup`'s hara-kiri callbacks (MS3 step 28): an empty
//!   group removed with its elements before its extent, skylines or offset
//!   is read, and its pure height empty between columns, on the staves of
//!   `fixtures/y1-unreached.ly`'s suicide (`keep-alive-with`,
//!   `make-dead-when`), as LilyPond 2.26.0 removes them there.
//! - `layout.tsv`'s Y extent (MS3 step 28, `dump.rs`): the extent as
//!   `layout.ly` writes it, `?` when the read does not complete, and no
//!   property left `calculation-in-progress` by a callback that threw or
//!   stopped for the host, so that no later read is a cyclic dependency.
//! - The separation items' skylines (MS3 step 30): the skylines left and
//!   right of the boxes of a note column's elements, each widened by its
//!   extra spacing and the whole padded, as LilyPond 2.26.0 makes them.
//! - The minimum distance between columns (MS3 step 30), which has no
//!   Scheme face: the skylines of a spacing grob's left and right
//!   separation items, each at its elements' pure Y coordinate and taken
//!   as broken as the grob, with the right ones' conditional elements and
//!   the accidentals `get_relevant_accidentals` keeps of them, and the
//!   distance between them, as their lines give them.
//! - The paper column's when, system, musical, used and extraneous
//!   columns and the minimum distance between two columns' own skylines
//!   (MS3 step 30), which have no Scheme face, as their lines give them.
//! - `ly:paper-column::print` (MS3 step 30): its rank and when, and its
//!   arrows for springs and minimum distances, as LilyPond 2.26.0 draws
//!   them on columns given the same (a score run by the test, with
//!   Emmentaler's and the text fonts' packs), and the columns it draws no
//!   arrow to. Skipped without LilyPond 2.26.0, and then unverified.
//! - The break-aligned groups' X offsets within a column (MS3 step 30):
//!   their order by `break-align-orders`, and
//!   `ly:break-alignment-interface::calc-positioning-done` as LilyPond
//!   2.26.0 positions the groups of the break alignments of a score run
//!   by the test, given the same groups, extents and space-alists, with
//!   its programming errors where an entry is missing. Skipped without
//!   LilyPond 2.26.0, and then unverified.
//! - The break-aligned groups' anchors and visibility (MS3 step 30):
//!   `ly:break-aligned-interface::calc-average-anchor`,
//!   `::calc-joint-anchor-alignment`, `::calc-extent-aligned-anchor` and
//!   `::calc-break-visibility` on what LilyPond reads through a null or
//!   no number, and as LilyPond 2.26.0 computes them for the groups and
//!   items of a score run by the test, given the same extents and
//!   properties. Skipped without LilyPond 2.26.0, and then unverified.
//! - A spacing grob's columns (MS3 step 31), which have no Scheme face:
//!   the column of lowest rank of its right items, its own column, the
//!   note columns of its items and of their separation items, and the
//!   break-aligned grob sticking out furthest towards it, with its
//!   extent, as their lines give them.
//! - The spacing options (MS3 step 31), which have no Scheme face: read
//!   from a grob's properties as `init_from_grob` reads them, and the
//!   space of a duration, linear below the shortest and logarithmic above,
//!   with the platform's `log2` as LilyPond 2.26.0 computes it on macOS.
//! - The staff spacing and the separation items' distances (MS3 step 31),
//!   which have no Scheme face: the spring after a non-musical column's
//!   prefatory matter by each type of its `space-alist` entry, the gap
//!   past the columns' minimum distance and the optical correction for a
//!   down-stem after a bar line; and the rod between two separation items,
//!   with the right one's conditional elements, and their emptiness; all
//!   hand-computed.
//! - The spacing spanner (MS3 step 31): its columns, the root system's
//!   used ones up to its last breakable one;
//!   `ly:spacing-spanner::calc-common-shortest-duration` and `::set-springs`
//!   through `%host-call` (D127), the duration most measures start, the
//!   columns' neighbours, their springs by duration, their rods from the
//!   skylines reaching over columns, the loose columns pruned between
//!   their neighbours, uniform, packed and grace spacing, the broken
//!   pieces' springs, and whether a column fills its measure; all
//!   hand-computed.
//! - `columns.tsv` (MS3 step 31, D128, `dump.rs`): after the spacer's
//!   `springs-and-rods` is read, each column and each of its pieces after
//!   it, named by its line (a piece by its original's) and break
//!   direction, with its spacing wishes, its springs newest first and its
//!   rods, as `layout.ly` writes them; the springs and rods `?` when the
//!   read throws, the property put back; no line without a spacer.
//! - The Paper_score's break indices and ranks (MS3 step 32): its used
//!   columns, the breakable ones among them broken on both sides, the
//!   first and the last needing no piece, kept once found; and
//!   `Output_def::c_variable`.
//! - The Paper_score's lines (MS3 step 32, `Constrained_breaking`): each
//!   line's force between two breaks, at `(j, i)`, the ragged last line,
//!   the system spacings from the spacing specs, and the breaks each
//!   start begins at.
//!
//! - The staff symbol's stencil on Y1's five: `pages.scm`'s (stencils.ly)
//!   between bounds built from LilyPond's `layout.tsv`, skipped without
//!   `ref/` (`tools/corpus/fetch.mjs`, `capture.mjs`).
//! - The note head's callbacks (MS3 step 37): its stencil and its stem
//!   attachment for each stem direction, by duration log, style and
//!   glyph name, with LilyPond's warnings, and `ly:note-head::stem-attachment`
//!   of a scaled and an unscaled font, as LilyPond 2.26.0 gives them for
//!   the heads of a score run by the test (skipped without it, and then
//!   unverified); the mensural styles off the staff lines throwing by name
//!   until the ledgers (step 39); the X shift, which reads the stem's
//!   positioning; the ledger-line height and the tab stem attachment, all
//!   hand-computed; and the print of each NoteHead of Y1's five,
//!   `pages.scm`'s glyph with `layout.tsv`'s extents.
//! - Side positions (MS3 step 40): `ly:side-position-interface::x-aligned-side`,
//!   `::y-aligned-side` and `::pure-y-aligned-side` of grobs of a score
//!   run by the test (texts, fingerings with accidental placements among
//!   their supports and quantized, string numbers, arpeggios, the system
//!   start bar), rebuilt with fresh grobs, each offset LilyPond 2.26.0's
//!   bit for bit (skipped without it, and then unverified); the
//!   improbable offset's programming error and error; and
//!   `::move-to-extremal-staff` on a grouper and on a system, hand-computed.
//! - Clefs and key signatures (MS3 step 40): `ly:clef::calc-glyph-name`,
//!   `ly:clef::print` and `ly:key-signature-interface::print` of the
//!   clefs, key signatures and cancellations of a score run by the test
//!   (at the start of a line, in mid-line and at its end; changed,
//!   full-size, resized, unknown; keys of every count of sharps and flats
//!   in four clefs, cancelled, by octave, quarter tones, with padding,
//!   padding pairs and positions set, a glyph missing from the font or
//!   from the alist; a staff of 1.5 spaces), rebuilt with fresh grobs,
//!   each glyph name and stencil LilyPond 2.26.0's bit for bit (skipped
//!   without it, and then unverified); and the warnings, a clef without a
//!   glyph killed and one without a glyph name printing nothing,
//!   hand-computed.
//! - Clef modifiers (MS3 step 42): `ly:clef-modifier::calc-parent-alignment`
//!   of a score's clef modifier given each of 18 cases of glyph,
//!   direction and `clef-alignments` (below, above, centred and none; an
//!   entry no pair, a list, missing, twice; `clefs.` absent, twice,
//!   around another), rebuilt with fresh grobs, each alignment LilyPond
//!   2.26.0's (skipped without it, and then unverified); and the values
//!   its type checks keep from the callback, Guile's type errors, and a
//!   modifier without a clef, by hand.
//! - The system start delimiter (MS3 step 40):
//!   `ly:system-start-delimiter::print` of the bars, brackets, braces and
//!   square brackets of scores run by the test (over one staff, two and
//!   four, nested; thickness and collapse height set; staves of 1.5, 2/3
//!   and 0.5 spaces; staves removed when empty; an unknown style; three
//!   lines each), rebuilt with fresh grobs from the Y extents and staff
//!   spaces of their elements, each stencil LilyPond 2.26.0's bit for bit
//!   or the delimiter killed as LilyPond's (skipped without it, and then
//!   unverified), the brace's markup LilyPond's stencil of it at the size
//!   asked for; and the elements left out, an item and a spanner bound
//!   elsewhere, hand-computed. And the break-alignable interface's
//!   `ly:break-alignable-interface::find-parent` and `::self-align-callback`,
//!   hand-computed.
//! - Flags (MS3 step 41): the six `ly:flag::` callbacks of the flags of a
//!   score run by the test (an eighth to a 128th, up and down; longer and
//!   thicker stems, an invisible one; mensural on lines and spaces, on a
//!   staff of 1.5 spaces and on one of four uneven lines; no-flag,
//!   stacked; grace strokes, a style's and the plain one's, an unknown
//!   stroke, an empty one; an unknown glyph name; a font size), rebuilt
//!   with fresh grobs from their stems' directions, duration logs and
//!   extents, each glyph name, stencil, width and offset LilyPond
//!   2.26.0's bit for bit (skipped without it, and then unverified); and
//!   a glyph name that is no string and a flag without a stem,
//!   hand-computed.
//! - Note collisions (MS3 step 41):
//!   `ly:note-collision-interface::calc-positioning-done`, layout's
//!   primitive, and a collision with nothing to place, hand-computed; its
//!   columns' shifts are `tools/compare/test/layout.test.mjs`'
//!   (fixtures/collisions.ly, LilyPond's natively and in wasm).
//!
//! On grobs a Scheme engraver makes, skipped without LilyPond 2.26.0's
//! music types, and then unverified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_layout::accidental_placement::get_relevant_accidentals;
use lily_engine_layout::break_substitution;
use lily_engine_layout::constrained_breaking::{
    ConstrainedBreaking, LineDetails, LineShape, VPOS, min_permission,
};
use lily_engine_layout::dump::{offset_field, system_columns, y_extent_field};
use lily_engine_layout::grob_array::GrobArray;
use lily_engine_layout::hara_kiri_group_spanner::{
    consider_suicide, request_suicide, unpure_request_suicide,
};
use lily_engine_layout::note_spacing::{get_spacing, stem_dir_correction};
use lily_engine_layout::page_breaking::{
    BreakPosition, PageBreaking, compress_lines, uncompress_solution,
};
use lily_engine_layout::page_layout_problem::is_spaceable;
use lily_engine_layout::paper_book::PaperBook;
use lily_engine_layout::paper_score::{self, PaperScore};
use lily_engine_layout::simple_spacer::{
    ColumnXPositions, get_line_configuration, get_line_forces,
};
use lily_engine_layout::spacing_basic::{note_spacing, standard_breakable_column_spacing};
use lily_engine_layout::spacing_interface::{
    extremal_break_aligned_grob, left_column, left_note_columns, minimum_distance, right_column,
    right_note_columns, skylines,
};
use lily_engine_layout::spacing_loose_columns::set_loose_columns;
use lily_engine_layout::spacing_options::SpacingOptions;
use lily_engine_layout::spacing_spanner::{fills_measure, get_columns};
use lily_engine_layout::spacing_spec::{get_fixed_spacing, get_spacing_spec, read_spacing_spec};
use lily_engine_layout::spring::Spring;
use lily_engine_layout::staff_grouper::{get_extremal_staff, maybe_pure_within_group};
use lily_engine_layout::staff_spacing::bar_y_positions;
use lily_engine_layout::{
    Interval, Stencil, direction, grob, item, note_column, paper_column, separation_item,
    spaceable, spanner, staff_spacing, stem, system, unpure,
};
use lily_engine_music::{Host, Moment, Rational, Score};
use lily_engine_scheme::{Exception, Hosted, Value, Vm};

/// `scm/lily/` of LilyPond 2.26.0, as `build.rs` finds it.
fn lily_scm() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LILY_SCM") {
        return Some(PathBuf::from(dir));
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    Some(binary.parent()?.join("../share/lilypond/2.26.0/scm/lily"))
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// Global with thirteen grob types: `Group`, a hara-kiri axis group
/// spanner, `Thing`, an item, `Column`, a paper column, `Staff`, a staff
/// symbol spanner, `Note`, a note column item, `Align`, an alignment
/// spanner, `Stem`, a stem item, `Bundle`, an axis group item, `Sep`, a
/// separation item, `Placement`, an accidental placement item, `Bit`, an
/// item, `Head`, a rhythmic head item, `Piece`, an item, `Bar`, a bar
/// line item, `Foot`, a footnote item, `Delim`, a system-start
/// delimiter spanner, and `Span`, a footnote spanner; and a Voice whose
/// `Maker`
/// makes [`MADE`] of each note.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Group (meta (name . Group) (classes Spanner)
                     (interfaces grob-interface spanner-interface axis-group-interface
                                 hara-kiri-group-spanner-interface)))
        (Thing (meta (name . Thing) (classes Item) (interfaces grob-interface item-interface)))
        (Column (meta (name . Column) (classes Paper_column)
                      (interfaces grob-interface item-interface paper-column-interface)))
        (Staff (meta (name . Staff) (classes Spanner)
                     (interfaces grob-interface spanner-interface staff-symbol-interface)))
        (Note (meta (name . Note) (classes Item)
                    (interfaces grob-interface item-interface note-column-interface)))
        (Align (meta (name . Align) (classes Spanner)
                     (interfaces grob-interface spanner-interface align-interface
                                 axis-group-interface)))
        (Stem (meta (name . Stem) (classes Item)
                    (interfaces grob-interface item-interface stem-interface)))
        (Bundle (meta (name . Bundle) (classes Item)
                      (interfaces grob-interface item-interface axis-group-interface)))
        (Sep (meta (name . Sep) (classes Item)
                   (interfaces grob-interface item-interface separation-item-interface)))
        (Placement (meta (name . Placement) (classes Item)
                         (interfaces grob-interface item-interface
                                     accidental-placement-interface)))
        (Bit (meta (name . Bit) (classes Item) (interfaces grob-interface item-interface)))
        (Head (meta (name . Head) (classes Item)
                    (interfaces grob-interface item-interface rhythmic-head-interface)))
        (Piece (meta (name . Piece) (classes Item) (interfaces grob-interface item-interface)))
        (Bar (meta (name . Bar) (classes Item)
                   (interfaces grob-interface item-interface bar-line-interface)))
        (Foot (meta (name . Foot) (classes Item)
                    (interfaces grob-interface item-interface footnote-interface)))
        (Delim (meta (name . Delim) (classes Spanner)
                     (interfaces grob-interface spanner-interface
                                 system-start-delimiter-interface)))
        (Span (meta (name . Span) (classes Spanner)
                    (interfaces grob-interface spanner-interface footnote-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Maker))",
];

/// What `Maker` makes: `(group n)`, `(thing n)`, `(column n)`,
/// `(staff n)`, `(note n)`, `(align n)`, `(stem n)`, `(bundle n)`,
/// `(sep n)`, `(placement n)`, `(bit n)`, `(head n)`, `(piece n)`,
/// `(bar n)`, `(foot n)`, `(delim n)` and `(span n)` are the `n`th of
/// each.
const MADE: &str = "'((Group . 6) (Thing . 6) (Column . 4) (Staff . 2) (Note . 3) (Align . 1) \
                    (Stem . 3) (Bundle . 1) (Sep . 4) (Placement . 2) (Bit . 8) \
                    (Head . 4) (Piece . 24) (Bar . 1) (Foot . 4) (Delim . 1) (Span . 1))";

/// A VM with the music crate's and layout's functions and the music types,
/// translated over one note (see [`DEFS`]). `None` without LilyPond
/// 2.26.0's music types: skipped, and then unverified.
fn grobs_vm() -> Option<Vm> {
    grobs_vm_of(DEFS, MADE)
}

/// A callback that throws as a placeholder does (D67): `%throws` calls
/// `ly:music-iterator::constructor`, a placeholder no step ports (the
/// iterators are the engine's own). The tests' was
/// `ly:kievan-ligature::print` until MS8 step 9's part 2 ported it, and
/// every placeholder of `grob.rs` with it.
const THROWS: &str = "(define-public (%throws grob) (ly:music-iterator::constructor))";

/// [`grobs_vm`] with the contexts `defs` and what `Maker` makes, `made`.
fn grobs_vm_of(defs: &[&str], made: &str) -> Option<Vm> {
    if lily_engine_music::TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    eval(&mut vm, "(set-current-module (resolve-module '(lily)))");
    eval(
        &mut vm,
        "(define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
    );
    let callbacks = lily_scm().unwrap().join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    eval(
        &mut vm,
        &format!(
            "(define-public (ly:make-event-class class) (list class))
             (define-public made '())
             (ly:register-translator
               (lambda (context)
                 (let ((heard #f))
                   `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                     (process-music
                      . ,(lambda (t)
                           (when heard
                             (for-each
                              (lambda (kind)
                                (do ((i 0 (1+ i))) ((= i (cdr kind)))
                                  (set! made (append made (list (ly:engraver-make-grob t (car kind) heard))))))
                              {made})
                             (set! heard #f)))))))
               'Maker)"
        ),
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let layout = vm.heap_mut().wrap(lily_engine_music::OutputDef::new(scope));
    vm.heap_mut().root(layout);
    for text in defs {
        let mods = eval(&mut vm, &format!("'{text}"));
        let mut items = Vec::new();
        let mut p = mods;
        while let (Some(m), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            items.push(m);
            p = rest;
        }
        let (def, unknown) =
            lily_engine_music::make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
        assert_eq!(unknown, 0, "{text}");
        let name = vm.heap().car(vm.heap().cdr(items[0]).unwrap()).unwrap();
        let name = vm.heap().name(name).unwrap().to_string();
        vm.module_define(scope, &name, def);
    }
    let music = eval(
        &mut vm,
        "(make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2))",
    );
    vm.heap_mut().root(music);
    let done = lily_engine_music::run_translator_with(&mut vm, music, layout, None, &[])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    eval(
        &mut vm,
        &format!(
            "(define (made-of kind n)
               (let loop ((m made) (i 0) (k {made}))
                 (if (eq? (caar k) kind) (list-ref made (+ i n))
                     (loop m (+ i (cdar k)) (cdr k)))))
             (define (group n) (made-of 'Group n))
             (define (thing n) (made-of 'Thing n))
             (define (column n) (made-of 'Column n))
             (define (staff n) (made-of 'Staff n))
             (define (note n) (made-of 'Note n))
             (define (align n) (made-of 'Align n))
             (define (stem n) (made-of 'Stem n))
             (define (bundle n) (made-of 'Bundle n))
             (define (sep n) (made-of 'Sep n))
             (define (placement n) (made-of 'Placement n))
             (define (bit n) (made-of 'Bit n))
             (define (head n) (made-of 'Head n))
             (define (piece n) (made-of 'Piece n))
             (define (bar n) (made-of 'Bar n))
             (define (foot n) (made-of 'Foot n))
             (define (delim n) (made-of 'Delim n))
             (define (span n) (made-of 'Span n))
             (define (arr . gs) (ly:grob-list->grob-array gs))
             (define (put! g sym v) (%grob-set-property-data! g sym v))"
        ),
    );
    Some(vm)
}

/// The grob `text` evaluates to.
fn grob_of(vm: &mut Vm, text: &str) -> Value {
    eval(vm, text)
}

/// The column `(column n)` given the rank `rank`, and the things `things`
/// put on it (their X parent).
fn put_on(vm: &mut Vm, n: usize, rank: u64, things: &[usize]) {
    let col = grob_of(vm, &format!("(column {n})"));
    paper_column::set_rank(vm.heap(), col, rank);
    for t in things {
        eval(
            vm,
            &format!("(ly:grob-set-parent! (thing {t}) 0 (column {n}))"),
        );
    }
}

/// `v` as `display` writes it.
fn shown(vm: &Vm, v: Value) -> String {
    let mut out = String::new();
    vm.heap().display(v, &mut out);
    out
}

fn live(vm: &mut Vm, text: &str) -> bool {
    let g = grob_of(vm, text);
    grob::is_live_grob(vm.heap_mut(), g)
}

#[test]
fn a_staff_is_spaceable_without_a_number_as_its_staff_affinity() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(put! (thing 1) 'staff-affinity 1)
         (put! (thing 2) 'staff-affinity -0.5)
         (put! (thing 3) 'staff-affinity 'up)",
    );
    let spaceable: Vec<bool> = (0..4)
        .map(|n| {
            let g = grob_of(&mut vm, &format!("(thing {n})"));
            is_spaceable(&mut vm, g).unwrap()
        })
        .collect();
    assert_eq!(spaceable, [true, false, false, true]);
}

#[test]
fn a_group_is_empty_between_columns_without_worthy_ranks() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 0 and 1 on the columns of ranks 3 and 7, thing 2 on none;
    // (group 1), from rank 2 to 4, worthy too.
    put_on(&mut vm, 0, 3, &[0]);
    put_on(&mut vm, 1, 7, &[1]);
    put_on(&mut vm, 2, 2, &[]);
    put_on(&mut vm, 3, 4, &[]);
    let (g0, g1) = (grob_of(&mut vm, "(group 0)"), grob_of(&mut vm, "(group 1)"));
    let (c2, c3) = (
        grob_of(&mut vm, "(column 2)"),
        grob_of(&mut vm, "(column 3)"),
    );
    spanner::set_bound(&mut vm, g1, -1, c2).unwrap();
    spanner::set_bound(&mut vm, g1, 1, c3).unwrap();
    // Not without remove-empty; nor at the first column without
    // remove-first.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'items-worth-living (arr (thing 0) (thing 1) (thing 2)))",
    );
    assert!(!request_suicide(&mut vm, g0, 4, 6).unwrap());
    eval(&mut vm, "(put! (group 0) 'remove-empty #t)");
    assert!(!request_suicide(&mut vm, g0, 0, 6).unwrap());
    // The ranks are made the first time, sorted, each once.
    assert!(request_suicide(&mut vm, g0, 4, 6).unwrap());
    let ranks = eval(
        &mut vm,
        "(ly:grob-property-data (group 0) 'important-column-ranks)",
    );
    assert_eq!(shown(&vm, ranks), "#(3 7)");
    // Strictly between start and end.
    for (start, end, empty) in [
        (2, 4, false),
        (3, 7, true),
        (6, 8, false),
        (4, 5, true),
        (5, 4, true),
        (1, 1, true),
    ] {
        assert_eq!(
            request_suicide(&mut vm, g0, start, end).unwrap(),
            empty,
            "{start} {end}"
        );
    }
    // remove-first: from rank 0 too; an end under 2 finds none.
    eval(&mut vm, "(put! (group 0) 'remove-first #t)");
    assert!(!request_suicide(&mut vm, g0, 0, 6).unwrap());
    assert!(request_suicide(&mut vm, g0, 0, 1).unwrap());
    // A spanner worth living adds each column it spans.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 2) 'items-worth-living (arr (group 1) (thing 1)))
         (put! (group 2) 'remove-empty #t)",
    );
    let g2 = grob_of(&mut vm, "(group 2)");
    assert!(request_suicide(&mut vm, g2, 4, 6).unwrap());
    let ranks = eval(
        &mut vm,
        "(ly:grob-property-data (group 2) 'important-column-ranks)",
    );
    assert_eq!(shown(&vm, ranks), "#(2 3 4 7)");
    // No worthy grob: no ranks, empty.
    let g3 = grob_of(&mut vm, "(group 3)");
    eval(&mut vm, "(put! (group 3) 'remove-empty #t)");
    assert!(request_suicide(&mut vm, g3, 1, 9).unwrap());
    let ranks = eval(
        &mut vm,
        "(ly:grob-property-data (group 3) 'important-column-ranks)",
    );
    assert_eq!(shown(&vm, ranks), "#()");
    // A rank that is no size is Guile's error, as it is reached.
    eval(
        &mut vm,
        "(put! (group 4) 'remove-empty #t)
                   (put! (group 4) 'important-column-ranks (vector 1 'x))",
    );
    let g4 = grob_of(&mut vm, "(group 4)");
    let e = request_suicide(&mut vm, g4, 1, 9).unwrap_err();
    assert_eq!(shown(&vm, e.key), "wrong-type-arg");
    eval(
        &mut vm,
        "(put! (group 4) 'important-column-ranks (vector 1 -1))",
    );
    let e = request_suicide(&mut vm, g4, 1, 9).unwrap_err();
    assert_eq!(shown(&vm, e.key), "out-of-range");
}

#[test]
fn make_dead_when_and_keep_alive_with_decide_for_the_group() {
    let Some(mut vm) = grobs_vm() else { return };
    // (group 0) empty, (group 1) and (group 2) not, (group 3) dead.
    put_on(&mut vm, 0, 5, &[0]);
    eval(
        &mut vm,
        "(for-each (lambda (n) (put! (group n) 'remove-empty #t)) '(0 1 2 3))
         (for-each (lambda (n) (ly:grob-set-object! (group n) 'items-worth-living (arr (thing 0))))
                   '(1 2 3))
         (ly:grob-suicide! (group 3))",
    );
    let g = |vm: &mut Vm, n: usize| grob_of(vm, &format!("(group {n})"));
    let (g0, g1) = (g(&mut vm, 0), g(&mut vm, 1));
    assert!(request_suicide(&mut vm, g0, 1, 9).unwrap());
    assert!(!request_suicide(&mut vm, g1, 1, 9).unwrap());
    // A live friend that is not empty keeps it alive; a dead one does not.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'keep-alive-with (arr (group 3)))",
    );
    assert!(request_suicide(&mut vm, g0, 1, 9).unwrap());
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'keep-alive-with (arr (group 3) (group 2)))",
    );
    assert!(!request_suicide(&mut vm, g0, 1, 9).unwrap());
    // A live foe that is not empty kills it, before its friends are asked;
    // a dead one does not.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 1) 'make-dead-when (arr (group 3)))",
    );
    assert!(!request_suicide(&mut vm, g1, 1, 9).unwrap());
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 1) 'make-dead-when (arr (group 3) (group 2)))",
    );
    assert!(request_suicide(&mut vm, g1, 1, 9).unwrap());
    // The same, unpure, by the worthy grobs' liveness: (group 4) is empty
    // once its worthy thing dies.
    eval(
        &mut vm,
        "(put! (group 4) 'remove-empty #t)
         (put! (group 4) 'remove-first #t)
         (ly:grob-set-object! (group 4) 'items-worth-living (arr (thing 1)))",
    );
    let g4 = g(&mut vm, 4);
    assert!(!unpure_request_suicide(&mut vm, g4).unwrap());
    eval(&mut vm, "(ly:grob-suicide! (thing 1))");
    assert!(unpure_request_suicide(&mut vm, g4).unwrap());
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 4) 'keep-alive-with (arr (group 2)))",
    );
    assert!(!unpure_request_suicide(&mut vm, g4).unwrap());
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 4) 'keep-alive-with (arr))
         (put! (group 4) 'remove-empty #f)
         (ly:grob-set-object! (group 4) 'make-dead-when (arr (group 2)))",
    );
    assert!(unpure_request_suicide(&mut vm, g4).unwrap());
}

#[test]
fn unpure_the_first_column_is_kept_without_remove_first() {
    let Some(mut vm) = grobs_vm() else { return };
    put_on(&mut vm, 0, 0, &[]);
    put_on(&mut vm, 1, 3, &[]);
    eval(&mut vm, "(put! (group 0) 'remove-empty #t)");
    let (g0, c0, c1) = (
        grob_of(&mut vm, "(group 0)"),
        grob_of(&mut vm, "(column 0)"),
        grob_of(&mut vm, "(column 1)"),
    );
    // No left bound, and a bound of rank 0: kept.
    assert!(!unpure_request_suicide(&mut vm, g0).unwrap());
    spanner::set_bound(&mut vm, g0, -1, c0).unwrap();
    assert!(!unpure_request_suicide(&mut vm, g0).unwrap());
    spanner::set_bound(&mut vm, g0, -1, c1).unwrap();
    assert!(unpure_request_suicide(&mut vm, g0).unwrap());
}

#[test]
fn an_empty_group_dies_with_all_its_children() {
    let Some(mut vm) = grobs_vm() else { return };
    // (group 0) holds (thing 0) and (group 1), which holds (thing 1); the
    // things' elements are not followed, as they are no axis groups.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'elements (arr (thing 0) (group 1)))
         (ly:grob-set-object! (group 1) 'elements (arr (thing 1)))
         (ly:grob-set-object! (thing 0) 'elements (arr (thing 2)))
         (for-each (lambda (n) (put! (group n) 'remove-empty #t) (put! (group n) 'remove-first #t))
                   '(0 2))
         (ly:grob-set-object! (group 0) 'items-worth-living (arr (thing 3)))
         (ly:grob-set-object! (group 2) 'elements (arr (thing 4)))
         (ly:grob-set-object! (group 2) 'items-worth-living (arr (thing 3)))",
    );
    let all = [
        "(group 0)",
        "(thing 0)",
        "(group 1)",
        "(thing 1)",
        "(thing 2)",
        "(group 2)",
        "(thing 4)",
    ];
    let lives = |vm: &mut Vm| all.map(|g| live(vm, g));
    // Not while a worthy grob lives.
    for n in [0, 2] {
        let g = grob_of(&mut vm, &format!("(group {n})"));
        consider_suicide(&mut vm, g).unwrap();
    }
    assert_eq!(lives(&mut vm), [true; 7]);
    eval(&mut vm, "(ly:grob-suicide! (thing 3))");
    let g0 = grob_of(&mut vm, "(group 0)");
    consider_suicide(&mut vm, g0).unwrap();
    assert_eq!(
        lives(&mut vm),
        [false, false, false, false, true, true, true]
    );
}

/// What `text` throws, as its text, or what it gives, written.
fn caught(vm: &mut Vm, text: &str) -> String {
    match vm.eval_text(text, None) {
        Ok(v) => {
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
        Err(e) => e.text(vm.heap()),
    }
}

/// The staff symbol's callbacks, each of the grob alone.
const STAFF_SYMBOL_CALLBACKS: [&str; 3] = [
    "ly:staff-symbol::calc-line-positions",
    "ly:staff-symbol::height",
    "ly:staff-symbol::print",
];

#[test]
fn the_staff_symbol_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in STAFF_SYMBOL_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let expecting = if name.ends_with("print") {
            "Spanner): #<Grob Thing >"
        } else {
            "Grob): x"
        };
        let arg = if name.ends_with("print") {
            "(thing 0)"
        } else {
            "'x"
        };
        let got = caught(&mut vm, &format!("({name} {arg})"));
        assert!(
            got.contains(name)
                && got.contains(&format!(
                    "Wrong type argument in position 1 (expecting {expecting}"
                )),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn line_positions_go_down_from_the_top_line_by_two() {
    let Some(mut vm) = grobs_vm() else { return };
    // An int, or 0 (from_scm<int> (…, 0)); none for a negative count.
    for (count, positions) in [
        ("5", "(4.0 2.0 0.0 -2.0 -4.0)"),
        ("4", "(3.0 1.0 -1.0 -3.0)"),
        ("1", "(0.0)"),
        ("0", "()"),
        ("-2", "()"),
        ("5.0", "()"),
        ("'five", "()"),
        ("(expt 2 40)", "()"),
    ] {
        eval(&mut vm, &format!("(put! (staff 0) 'line-count {count})"));
        assert_eq!(
            caught(&mut vm, "(ly:staff-symbol::calc-line-positions (staff 0))"),
            positions,
            "{count}"
        );
    }
}

/// The layout's `line-thickness` 0.1 and `staff-space` 1.
fn set_layout(vm: &mut Vm) {
    eval(
        vm,
        "(let ((layout (ly:grob-layout (staff 0))))
           (ly:output-def-set-variable! layout 'line-thickness 0.1)
           (ly:output-def-set-variable! layout 'staff-space 1))",
    );
}

#[test]
fn the_height_spans_the_lines_and_half_their_thickness() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    let height = |vm: &mut Vm| caught(vm, "(ly:staff-symbol::height (staff 0))");
    // Five lines through line-positions' callback.
    eval(
        &mut vm,
        "(put! (staff 0) 'line-count 5)
         (put! (staff 0) 'line-positions ly:staff-symbol::calc-line-positions)",
    );
    assert_eq!(height(&mut vm), "(-2.05 . 2.05)");
    // Unsorted positions; the staff's own staff-space and thickness.
    eval(&mut vm, "(put! (staff 0) 'line-positions '(3 -1 1/2))");
    assert_eq!(height(&mut vm), "(-0.55 . 1.55)");
    eval(&mut vm, "(put! (staff 0) 'staff-space 2)");
    assert_eq!(height(&mut vm), "(-1.05 . 3.05)");
    eval(&mut vm, "(put! (staff 0) 'thickness 3)");
    assert_eq!(height(&mut vm), "(-1.15 . 3.15)");
    // A negative staff space turns the interval.
    eval(
        &mut vm,
        "(put! (staff 0) 'staff-space -1) (put! (staff 0) 'thickness 'x)",
    );
    assert_eq!(height(&mut vm), "(-1.55 . 0.55)");
    // No lines: (0 . 0).
    eval(&mut vm, "(put! (staff 0) 'line-positions '())");
    assert_eq!(height(&mut vm), "(0.0 . 0.0)");
    // A position that is no real is Guile's error; the layout's
    // dimension too.
    eval(&mut vm, "(put! (staff 0) 'line-positions '(1 x))");
    assert!(height(&mut vm).contains("Wrong type (expecting real number): x"));
    eval(
        &mut vm,
        "(put! (staff 0) 'line-positions '(1))
         (ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'line-thickness 'thin)",
    );
    assert!(height(&mut vm).contains("Wrong type (expecting real number): thin"));
}

/// `(translate-stencil (0.0 . y) line)` for each `y`, in a
/// `combine-stencil` from the last to the first, as `add_stencil` makes
/// it, with `line` `(draw-line t left 0 right 0)`.
fn staff_lines(t: &str, left: &str, right: &str, ys: &[&str]) -> String {
    let line = format!("(draw-line {t} {left} 0 {right} 0)");
    let items: Vec<String> = ys
        .iter()
        .rev()
        .map(|y| format!("(translate-stencil (0.0 . {y}) {line})"))
        .collect();
    format!("(combine-stencil {})", items.join(" "))
}

#[test]
fn the_stencil_draws_each_line_between_the_bounds() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    // Columns 0 and 2 at 1, 1 and 3 at 11; staff 0 bounded by columns 0
    // and 1, staff 1 by 2 and 3, each at 1, as its left bound is its X
    // parent (set_bound). Thickness 5 (0.5), staff-space 2: a position is
    // 1.
    eval(
        &mut vm,
        "(for-each (lambda (n x) (ly:grob-translate-axis! (column n) x 0))
                   '(0 1 2 3) '(1 11 1 11))
         (for-each (lambda (n)
                     (put! (staff n) 'thickness 5)
                     (put! (staff n) 'staff-space 2)
                     (put! (staff n) 'line-count 5)
                     (put! (staff n) 'line-positions ly:staff-symbol::calc-line-positions))
                   '(0 1))",
    );
    for (st, left, right) in [(0, 0, 1), (1, 2, 3)] {
        let st = grob_of(&mut vm, &format!("(staff {st})"));
        let c0 = grob_of(&mut vm, &format!("(column {left})"));
        let c1 = grob_of(&mut vm, &format!("(column {right})"));
        spanner::set_bound(&mut vm, st, -1, c0).unwrap();
        spanner::set_bound(&mut vm, st, 1, c1).unwrap();
    }
    let print = |vm: &mut Vm, n: usize| {
        let expr = caught(
            vm,
            &format!("(ly:stencil-expr (ly:staff-symbol::print (staff {n})))"),
        );
        let ext = caught(
            vm,
            &format!(
                "(let ((s (ly:staff-symbol::print (staff {n}))))
                   (list (ly:stencil-extent s 0) (ly:stencil-extent s 1)))"
            ),
        );
        (expr, ext)
    };
    let five = ["4.0", "2.0", "0.0", "-2.0", "-4.0"];
    // Bounds of no X extent: their coordinates, less half a line's
    // thickness inside, relative to the staff.
    assert_eq!(
        print(&mut vm, 0),
        (
            staff_lines("0.5", "0.25", "9.75", &five),
            "((0.25 . 9.75) (-4.25 . 4.25))".into()
        )
    );
    // A width ends it, from the staff's common refpoint with its bounds.
    eval(&mut vm, "(put! (staff 0) 'width 5)");
    assert_eq!(
        print(&mut vm, 0).0,
        staff_lines("0.5", "0.25", "3.75", &five)
    );
    // Bounds with an X extent end it at their break alignment: the left at
    // the whole alignment's, the right at the first non-empty group of
    // break-align-symbols; at the column itself without one.
    let two = ["1.0", "-1.0"];
    eval(
        &mut vm,
        "(put! (staff 1) 'line-positions '(1 -1))
         (put! (column 2) 'X-extent '(0 . 1))
         (put! (column 3) 'X-extent '(0 . 1))
         (for-each (lambda (n c) (ly:grob-set-parent! (thing n) 0 (column c)))
                   '(0 2 3 4) '(2 3 3 3))",
    );
    assert_eq!(
        print(&mut vm, 1).0,
        staff_lines("0.5", "0.25", "9.75", &two)
    );
    eval(
        &mut vm,
        "(ly:grob-set-object! (column 2) 'break-alignment (thing 0))
         (put! (thing 0) 'X-extent '(-0.5 . 0.5))
         (ly:grob-set-object! (column 3) 'break-alignment (thing 2))
         (put! (thing 2) 'X-extent '(-4 . 2))
         (ly:grob-set-object! (thing 2) 'elements (arr (thing 3) (thing 4)))
         (put! (thing 3) 'break-align-symbol 'key-signature)
         (put! (thing 3) 'X-extent '(1 . 0))
         (put! (thing 4) 'break-align-symbol 'staff-bar)
         (put! (thing 4) 'X-extent '(-2 . -1))",
    );
    // The right: no break-align-symbols, the column.
    assert_eq!(
        print(&mut vm, 1).0,
        staff_lines("0.5", "-0.25", "9.75", &two)
    );
    // The empty key signature passed over for the staff bar; the whole
    // alignment after a missing symbol; the column after none.
    for (symbols, right) in [
        ("'(key-signature staff-bar break-alignment)", "8.75"),
        ("'(clef break-alignment)", "11.75"),
        ("'staff-bar", "8.75"),
        ("'key-signature", "9.75"),
    ] {
        eval(
            &mut vm,
            &format!("(put! (staff 1) 'break-align-symbols {symbols})"),
        );
        assert_eq!(
            print(&mut vm, 1).0,
            staff_lines("0.5", "-0.25", right, &two),
            "{symbols}"
        );
    }
    // A musical column is the column itself, after a programming error;
    // an inexact duration is none (from_scm<Rational>).
    eval(&mut vm, "(put! (staff 1) 'break-align-symbols 'staff-bar)");
    for (duration, right) in [("1/4", "9.75"), ("+inf.0", "9.75"), ("0.25", "8.75")] {
        eval(
            &mut vm,
            &format!("(put! (column 3) 'shortest-starter-duration {duration})"),
        );
        assert_eq!(
            print(&mut vm, 1).0,
            staff_lines("0.5", "-0.25", right, &two),
            "{duration}"
        );
    }
    // No lines: an empty stencil.
    eval(&mut vm, "(put! (staff 1) 'line-positions '())");
    assert_eq!(
        print(&mut vm, 1),
        ("()".into(), "((+inf.0 . -inf.0) (+inf.0 . -inf.0))".into())
    );
}

/// `fetch.mjs`' corpus directory: `LILY_CORPUS`, or `~/.cache/lily-engine`.
fn corpus() -> PathBuf {
    std::env::var_os("LILY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache/lily-engine")
        })
}

/// Y1's five, whose staves `ref/<id>/` holds (MS3 step 25).
const FIVE: [&str; 5] = ["T01", "T03", "T14", "T16", "T19"];

/// The first balanced form of `text` from `start`.
fn form(text: &str, start: usize) -> &str {
    let mut depth = 0;
    for (i, c) in text[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 1 => return &text[start..=start + i],
            ')' => depth -= 1,
            _ => {}
        }
    }
    panic!("unbalanced from {start}")
}

/// Whether two expressions are the same, token for token, their numbers
/// within `layout.mjs`' TOLERANCE.
fn same_within(a: &str, b: &str) -> bool {
    let tokens = |s: &str| {
        s.replace('(', " ( ")
            .replace(')', " ) ")
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
    };
    let (a, b) = (tokens(a), tokens(b));
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            x == y
                || matches!((x.parse::<f64>(), y.parse::<f64>()),
                    (Ok(x), Ok(y)) if (x - y).abs() <= 1e-4 + 1e-9)
        })
}

#[test]
fn the_stencil_is_lilypond_s_on_y1_s_five_between_its_bounds() {
    // On the five, LilyPond's staff symbol is drawn between the columns
    // its line was broken at, which the engine neither spaces nor breaks
    // yet (steps 32–33): its bounds are built here from LilyPond's
    // layout.tsv, each column at its X with its X extent, its break
    // alignment and the staff bar's group, to four decimals; the stencil
    // is pages.scm's (stencils.ly) within layout.mjs' TOLERANCE, and its
    // extents the staff's in layout.tsv.
    let refs = corpus().join("ref");
    if FIVE
        .iter()
        .any(|id| !refs.join(id).join("layout.tsv").exists())
    {
        eprintln!("skipped: no layout.tsv in {}", refs.display());
        return;
    }
    for id in FIVE {
        let Some(mut vm) = grobs_vm() else { return };
        set_layout(&mut vm);
        let tsv = std::fs::read_to_string(refs.join(id).join("layout.tsv")).unwrap();
        let lines: Vec<Vec<&str>> = tsv.lines().map(|l| l.split('\t').collect()).collect();
        let of = |ty: &str, columns: &str| -> Vec<&Vec<&str>> {
            lines
                .iter()
                .filter(|l| l[0] == ty && l[3] == columns)
                .collect()
        };
        let staff: Vec<_> = lines.iter().filter(|l| l[0] == "StaffSymbol").collect();
        assert_eq!(staff.len(), 1, "{id}");
        let staff = staff[0];
        let extent = |e: &str| match e.split_once(' ') {
            Some((l, r)) => format!("'({l} . {r})"),
            None => "'(+inf.0 . -inf.0)".into(),
        };
        // Columns 0 and 1 the bounds; things 0 and 1 their break
        // alignments, thing 2 the staff bar's group of the right one.
        for (n, columns) in staff[3].split(';').enumerate() {
            let col = of("NonMusicalPaperColumn", columns)[0];
            let x: f64 = col[8].parse().unwrap();
            eval(
                &mut vm,
                &format!("(ly:grob-translate-axis! (column {n}) {x} 0)"),
            );
            if col[10] != "-" {
                eval(
                    &mut vm,
                    &format!("(put! (column {n}) 'X-extent {})", extent(col[10])),
                );
            }
            let Some(&ba) = of("BreakAlignment", columns).first() else {
                continue;
            };
            let bx: f64 = ba[8].parse().unwrap();
            eval(
                &mut vm,
                &format!(
                    "(ly:grob-set-parent! (thing {n}) 0 (column {n}))
                     (ly:grob-translate-axis! (thing {n}) {} 0)
                     (put! (thing {n}) 'X-extent {})
                     (ly:grob-set-object! (column {n}) 'break-alignment (thing {n}))",
                    bx - x,
                    extent(ba[10])
                ),
            );
            let bar = of("BarLine", columns).first().copied();
            if let Some(group) = bar.and_then(|bar| {
                of("BreakAlignGroup", columns)
                    .into_iter()
                    .find(|g| g[8] == bar[8])
            }) {
                let gx: f64 = group[8].parse().unwrap();
                eval(
                    &mut vm,
                    &format!(
                        "(ly:grob-set-parent! (thing 2) 0 (thing {n}))
                         (ly:grob-translate-axis! (thing 2) {} 0)
                         (put! (thing 2) 'X-extent {})
                         (put! (thing 2) 'break-align-symbol 'staff-bar)
                         (ly:grob-set-object! (thing {n}) 'elements (arr (thing 2)))",
                        gx - bx,
                        extent(group[10])
                    ),
                );
            }
        }
        // define-grobs.scm's StaffSymbol.
        eval(
            &mut vm,
            "(put! (staff 0) 'line-count 5)
             (put! (staff 0) 'line-positions ly:staff-symbol::calc-line-positions)
             (put! (staff 0) 'break-align-symbols '(staff-bar break-alignment))",
        );
        let st = grob_of(&mut vm, "(staff 0)");
        for (d, n) in [(-1, 0), (1, 1)] {
            let c = grob_of(&mut vm, &format!("(column {n})"));
            spanner::set_bound(&mut vm, st, d, c).unwrap();
        }
        let got = caught(
            &mut vm,
            "(let ((s (ly:staff-symbol::print (staff 0))))
               (list (ly:stencil-expr s) (ly:stencil-extent s 0)
                     (ly:staff-symbol::height (staff 0))))",
        );
        let pages = std::fs::read_to_string(refs.join(id).join("pages.scm")).unwrap();
        let lily = "(combine-stencil (translate-stencil (0.0 . -2.0) (draw-line ";
        assert_eq!(pages.matches(lily).count(), 1, "{id}");
        let expected = format!(
            "({} {} {})",
            form(&pages, pages.find(lily).unwrap()),
            extent(staff[10]),
            extent(staff[11])
        )
        .replace('\'', "");
        assert!(same_within(&got, &expected), "{id}:\n{got}\n{expected}");
        // A line moved by twice the tolerance is another.
        assert!(!same_within(&got, &expected.replacen("-2.0", "-2.0002", 1)));
    }
}

/// The referencer's Scheme face, each of the grob alone.
const REFERENCER_FUNCTIONS: [&str; 5] = [
    "ly:staff-symbol-referencer::callback",
    "ly:grob-staff-position",
    "ly:staff-symbol-line-thickness",
    "ly:staff-symbol-staff-space",
    "ly:staff-symbol-staff-radius",
];

#[test]
fn the_referencer_s_functions_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in REFERENCER_FUNCTIONS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let got = caught(&mut vm, &format!("({name} 1)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): 1"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

/// The things and staves Y-children of `(thing 5)`, the referencers' and
/// staves' common refpoint, and `staff` the `staff-symbol` of each thing
/// of `things`.
fn on_staff(vm: &mut Vm, staff: usize, things: &[usize]) {
    for t in things {
        eval(
            vm,
            &format!(
                "(ly:grob-set-parent! (thing {t}) 1 (thing 5))
                 (ly:grob-set-object! (thing {t}) 'staff-symbol (staff {staff}))"
            ),
        );
    }
    eval(
        vm,
        &format!("(ly:grob-set-parent! (staff {staff}) 1 (thing 5))"),
    );
}

#[test]
fn a_referencer_is_offset_by_half_a_staff_space_a_position() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    // Things 0 to 3 have only the referencer's Y callback; 0 to 2 are on
    // staff 0, whose staff space is 2 and which is 1 up.
    on_staff(&mut vm, 0, &[0, 1, 2]);
    eval(
        &mut vm,
        "(for-each (lambda (n) (put! (thing n) 'Y-offset ly:staff-symbol-referencer::callback))
                   '(0 1 2 3))
         (put! (staff 0) 'staff-space 2)
         (put! (staff 0) 'Y-offset 1)
         (put! (thing 0) 'staff-position 3)
         (put! (thing 1) 'staff-position 1/2)
         (put! (thing 2) 'staff-position 'x)
         (put! (thing 3) 'staff-position 3)",
    );
    let y = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!("(%grob-relative-coordinate (thing {n}) '() 1)"),
        )
    };
    // Half a staff space a position; none without a number; 1 without a
    // staff symbol.
    assert_eq!(
        (0..4).map(|n| y(&mut vm, n)).collect::<Vec<_>>(),
        ["3.0", "0.5", "0.0", "1.5"]
    );
    // The position from the staff symbol, exact when it is an integer.
    let pos = |vm: &mut Vm, n: usize| caught(vm, &format!("(ly:grob-staff-position (thing {n}))"));
    assert_eq!(pos(&mut vm, 0), "2");
    assert_eq!(pos(&mut vm, 1), "-0.5");
    assert_eq!(pos(&mut vm, 2), "-1");
    // Without a staff symbol, twice its offset from its Y parent: 3; its
    // absolute coordinate without one.
    assert_eq!(pos(&mut vm, 3), "3");
    eval(&mut vm, "(ly:grob-set-parent! (thing 3) 1 (thing 4))");
    eval(&mut vm, "(put! (thing 4) 'Y-offset 2)");
    assert_eq!(pos(&mut vm, 3), "3");
    assert_eq!(pos(&mut vm, 4), "4");
    // A complex staff-position is Guile's error.
    eval(&mut vm, "(put! (thing 4) 'staff-position 1+i)");
    assert!(
        caught(&mut vm, "(ly:staff-symbol-referencer::callback (thing 4))")
            .contains("Wrong type (expecting real number): 1.0+1.0i")
    );
}

/// MS3 step 46 (D205): a Rust read is the image's. A callback of the
/// callback table runs in Rust, one the score makes through the VM, each
/// once, its value kept as `try_callback_on_alist` keeps it; the slots of
/// a grob's own properties follow its alist as it is set, deleted and set
/// again; and the image's coordinates, which call the Rust, give what the
/// Rust gives.
#[test]
fn a_rust_read_is_the_image_s_with_the_callback_table_and_the_slots() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    on_staff(&mut vm, 0, &[0, 1]);
    eval(
        &mut vm,
        "(define calls 0)
         (put! (staff 0) 'staff-space 2)
         (put! (thing 0) 'Y-offset ly:staff-symbol-referencer::callback)
         (put! (thing 0) 'staff-position 3)
         (put! (thing 1) 'Y-offset (lambda (g) (set! calls (1+ calls)) 0.25))",
    );
    let [t0, t1, t5] = ["(thing 0)", "(thing 1)", "(thing 5)"].map(|g| grob_of(&mut vm, g));
    for _ in 0..2 {
        let y0 = grob::relative_coordinate(&mut vm, t0, t5, lily_engine_layout::Y).unwrap();
        let y1 = grob::relative_coordinate(&mut vm, t1, t5, lily_engine_layout::Y).unwrap();
        assert_eq!((y0, y1), (3.0, 0.25));
    }
    assert_eq!(caught(&mut vm, "calls"), "1");
    assert_eq!(
        caught(&mut vm, "(%grob-relative-coordinate (thing 0) (thing 5) 1)"),
        "3.0"
    );
    // The offsets are kept, their properties given up (`get_offset`).
    assert_eq!(
        caught(&mut vm, "(ly:grob-property-data (thing 1) 'Y-offset)"),
        "()"
    );
    // A computed property is set on the grob, read again without its
    // callback; an extent is the Rust's and the image's alike.
    eval(
        &mut vm,
        "(put! (thing 1) 'Y-extent (lambda (g) (set! calls (1+ calls)) '(-1 . 2)))",
    );
    let e = grob::extent(&mut vm, t1, t5, lily_engine_layout::Y).unwrap();
    assert_eq!((e.left, e.right), (-0.75, 2.25));
    assert_eq!(
        caught(&mut vm, "(%grob-extent (thing 1) (thing 5) 1)"),
        "(-0.75 . 2.25)"
    );
    assert_eq!(
        caught(&mut vm, "(ly:grob-property (thing 1) 'Y-extent)"),
        "(-1 . 2)"
    );
    assert_eq!(caught(&mut vm, "calls"), "2");
    // The slots: set, set in place, deleted, set again behind another.
    for (text, value) in [
        ("(put! (thing 2) 'foo 1)", "1"),
        ("(put! (thing 2) 'foo 2)", "2"),
        ("(%grob-del-property! (thing 2) 'foo)", "()"),
        ("(put! (thing 2) 'bar 3) (put! (thing 2) 'foo 4)", "4"),
    ] {
        eval(&mut vm, text);
        assert_eq!(
            caught(&mut vm, "(ly:grob-property (thing 2) 'foo)"),
            value,
            "{text}"
        );
    }
    assert_eq!(caught(&mut vm, "(ly:grob-property (thing 2) 'bar)"), "3");
}

#[test]
fn the_staff_position_is_the_property_without_a_common_refpoint() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    // Staff 1 and things 0 and 1 have no Y parents.
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 0) 'staff-symbol (staff 1))
         (ly:grob-set-object! (thing 1) 'staff-symbol (staff 1))
         (put! (thing 0) 'staff-position 5/2)
         (put! (thing 0) 'Y-offset 7)
         (put! (thing 1) 'staff-position 'x)",
    );
    let pos = |vm: &mut Vm, n: usize| caught(vm, &format!("(ly:grob-staff-position (thing {n}))"));
    assert_eq!(pos(&mut vm, 0), "2.5");
    assert_eq!(pos(&mut vm, 1), "0");
    // A staff space of 0 puts it at 0 where there is one.
    on_staff(&mut vm, 1, &[2]);
    eval(
        &mut vm,
        "(put! (staff 1) 'staff-space 0) (put! (thing 2) 'Y-offset 3)",
    );
    assert_eq!(pos(&mut vm, 2), "0");
    // Within 10^-6 of an integer it is the integer.
    eval(
        &mut vm,
        "(put! (staff 1) 'staff-space 1) (put! (thing 2) 'Y-offset 0)
         (put! (thing 3) 'Y-offset 1.0000004)",
    );
    on_staff(&mut vm, 1, &[3]);
    assert_eq!(pos(&mut vm, 3), "2");
}

#[test]
fn the_pure_position_takes_the_pure_y_offset() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    on_staff(&mut vm, 0, &[0, 1, 2]);
    // A procedure has no pure offset; the image's container calls the
    // callback for it.
    eval(
        &mut vm,
        "(put! (thing 0) 'Y-offset ly:staff-symbol-referencer::callback)
         (put! (thing 1) 'Y-offset (ly:make-unpure-pure-container ly:staff-symbol-referencer::callback))
         (put! (thing 0) 'staff-position 4)
         (put! (thing 1) 'staff-position 4)",
    );
    // Pure first: the pure coordinate takes the offset once it is known.
    let both = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!(
                "(let* ((pure (%staff-position (thing {n}) #t))
                        (pure-rounded (%rounded-staff-position (thing {n}) #t))
                        (unpure (%staff-position (thing {n}) #f)))
                   (list pure pure-rounded unpure (%rounded-staff-position (thing {n}) #f)
                         (%staff-position (thing {n}) #t)))"
            ),
        )
    };
    assert_eq!(both(&mut vm, 0), "(0.0 0 4.0 4 4.0)");
    assert_eq!(both(&mut vm, 1), "(4.0 4 4.0 4 4.0)");
    // The orders by position: thing 2 at 5.
    eval(&mut vm, "(put! (thing 2) 'Y-offset 5/2)");
    assert_eq!(
        caught(
            &mut vm,
            "(list (%staff-position<? (thing 0) (thing 2)) (%staff-position<? (thing 2) (thing 0))
                   (%pure-staff-position<? (thing 0) (thing 2))
                   (%staff-position-compare (thing 0) (thing 2))
                   (%staff-position-compare (thing 2) (thing 0))
                   (%staff-position-compare (thing 0) (thing 0)))"
        ),
        "(#t #f #t -1 1 0)"
    );
}

#[test]
fn set_position_moves_it_by_the_positions_to_go() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    on_staff(&mut vm, 0, &[0]);
    eval(
        &mut vm,
        "(put! (staff 0) 'staff-space 2)
         (put! (staff 0) 'Y-offset 1)
         (put! (thing 0) 'Y-offset ly:staff-symbol-referencer::callback)
         (put! (thing 0) 'staff-position 3)",
    );
    // At 2 (3.0 less the staff's 1, in half spaces of 2): 3 more to 5.
    assert_eq!(caught(&mut vm, "(ly:grob-staff-position (thing 0))"), "2");
    eval(&mut vm, "(%set-staff-position! (thing 0) 5 #f)");
    assert_eq!(caught(&mut vm, "(ly:grob-staff-position (thing 0))"), "5");
    assert_eq!(
        caught(
            &mut vm,
            "(ly:grob-relative-coordinate (thing 0) (thing 5) 1)"
        ),
        "6.0"
    );
    // Without a staff symbol, by the half spaces of 1 from 0.
    eval(
        &mut vm,
        "(put! (thing 1) 'Y-offset 10) (%grob-offset (thing 1) 1)
         (%set-staff-position! (thing 1) 3 #f)",
    );
    assert_eq!(caught(&mut vm, "(%grob-offset (thing 1) 1)"), "11.5");
}

#[test]
fn the_extent_in_the_staff_is_from_the_staff_symbol() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    on_staff(&mut vm, 0, &[0]);
    eval(
        &mut vm,
        "(put! (staff 0) 'Y-offset 1)
         (put! (thing 0) 'Y-offset 3)
         (put! (thing 0) 'Y-extent '(-1 . 1))
         (put! (thing 1) 'Y-extent '(-1 . 1))",
    );
    assert_eq!(caught(&mut vm, "(%staff-extent (thing 0))"), "(1.0 . 3.0)");
    assert_eq!(
        caught(&mut vm, "(%staff-extent (thing 1))"),
        "(+inf.0 . -inf.0)"
    );
}

#[test]
fn space_thickness_and_radius_are_read_through_the_staff_symbol() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    on_staff(&mut vm, 0, &[0]);
    let face = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!(
                "(list (ly:staff-symbol-staff-space (thing {n}))
                       (ly:staff-symbol-line-thickness (thing {n}))
                       (ly:staff-symbol-staff-radius (thing {n}))
                       (%staff-span (thing {n})))"
            ),
        )
    };
    // Without a staff symbol: 1, the layout's line-thickness, no lines.
    assert_eq!(face(&mut vm, 1), "(1.0 0.1 0.0 (+inf.0 . -inf.0))");
    // Five lines: a radius of 2 staff spaces, measured in positions.
    eval(
        &mut vm,
        "(put! (staff 0) 'staff-space 1.5)
         (put! (staff 0) 'thickness 2)
         (put! (staff 0) 'line-count 5)
         (put! (staff 0) 'line-positions ly:staff-symbol::calc-line-positions)",
    );
    assert_eq!(face(&mut vm, 0), "(1.5 0.2 2.0 (-4.0 . 4.0))");
    // One line, and none: 0.
    eval(&mut vm, "(put! (staff 0) 'line-positions '(3))");
    assert_eq!(face(&mut vm, 0), "(1.5 0.2 0.0 (3.0 . 3.0))");
    eval(&mut vm, "(put! (staff 0) 'line-positions '())");
    assert_eq!(face(&mut vm, 0), "(1.5 0.2 0.0 (1.0 . -1.0))");
    // A staff-symbol that is no grob is none.
    eval(&mut vm, "(ly:grob-set-object! (thing 2) 'staff-symbol 'x)");
    assert_eq!(face(&mut vm, 2), "(1.0 0.1 0.0 (+inf.0 . -inf.0))");
}

/// Self-alignment's five callbacks, each with the arguments it takes
/// after the grob.
const SELF_ALIGNMENT_CALLBACKS: [(&str, &str); 5] = [
    ("ly:self-alignment-interface::x-aligned-on-self", ""),
    ("ly:self-alignment-interface::y-aligned-on-self", ""),
    (
        "ly:self-alignment-interface::pure-y-aligned-on-self",
        " 0 0",
    ),
    ("ly:self-alignment-interface::centered-on-x-parent", ""),
    ("ly:self-alignment-interface::aligned-on-x-parent", ""),
];

#[test]
fn the_self_alignment_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, rest) in SELF_ALIGNMENT_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let got = caught(&mut vm, &format!("({name} 1{rest})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): 1"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x 'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn a_grob_is_aligned_on_its_own_extent() {
    let Some(mut vm) = grobs_vm() else { return };
    // Thing 0 is 4 wide and 2 high, read through callbacks, and offset:
    // its own extent is relative to itself.
    eval(
        &mut vm,
        "(put! (thing 0) 'X-extent (lambda (g) '(-1 . 3)))
         (put! (thing 0) 'Y-extent '(-2 . 0))
         (put! (thing 0) 'X-offset 5)
         (put! (thing 0) 'Y-offset 7)",
    );
    let aligned = |vm: &mut Vm, x: &str, y: &str| {
        caught(
            vm,
            &format!(
                "(begin (put! (thing 0) 'self-alignment-X {x})
                        (put! (thing 0) 'self-alignment-Y {y})
                        (list (ly:self-alignment-interface::x-aligned-on-self (thing 0))
                              (ly:self-alignment-interface::y-aligned-on-self (thing 0))))"
            ),
        )
    };
    // Minus the left edge at -1, the center at 0, the right edge at 1, and
    // beyond; LilyPond's minus 0 at the top.
    assert_eq!(aligned(&mut vm, "-1", "-1"), "(1.0 2.0)");
    assert_eq!(aligned(&mut vm, "0", "0"), "(-1.0 1.0)");
    assert_eq!(aligned(&mut vm, "1", "1"), "(-3.0 -0.0)");
    assert_eq!(aligned(&mut vm, "1/2", "3"), "(-2.0 -2.0)");
    // No number, no alignment.
    assert_eq!(aligned(&mut vm, "'()", "'x"), "(0.0 0.0)");
    // An empty extent is not aligned; a complex alignment is Guile's error.
    eval(&mut vm, "(put! (thing 1) 'X-extent '(1 . -1))");
    eval(&mut vm, "(put! (thing 1) 'self-alignment-X -1)");
    assert_eq!(
        caught(
            &mut vm,
            "(ly:self-alignment-interface::x-aligned-on-self (thing 1))"
        ),
        "0.0"
    );
    assert!(aligned(&mut vm, "1+i", "0").contains("Wrong type (expecting real number): 1.0+1.0i"));
}

#[test]
fn the_pure_alignment_takes_the_pure_extent_between_the_columns() {
    let Some(mut vm) = grobs_vm() else { return };
    // The pure height of each thing is (start + 1 . end), its height
    // (1 . 4); each thing's pure height is kept once computed.
    eval(
        &mut vm,
        "(for-each
           (lambda (n)
             (put! (thing n) 'Y-extent
                   (ly:make-unpure-pure-container (lambda (g) '(1 . 4))
                                                  (lambda (g s e) (cons (+ s 1) e)))))
           '(0 1 2 3 4 5))
         (for-each (lambda (n) (put! (thing n) 'self-alignment-Y -1)) '(0 1 2))
         (for-each (lambda (n) (put! (thing n) 'self-alignment-Y 1)) '(3 4 5))",
    );
    let pure = |vm: &mut Vm, n: usize, start: &str, end: &str| {
        caught(
            vm,
            &format!(
                "(ly:self-alignment-interface::pure-y-aligned-on-self (thing {n}) {start} {end})"
            ),
        )
    };
    // Minus the bottom: start + 1; start 0 for what is no vsize.
    assert_eq!(pure(&mut vm, 0, "3", "9"), "-4.0");
    assert_eq!(pure(&mut vm, 1, "-2", "9"), "-1.0");
    assert_eq!(pure(&mut vm, 2, "1.0", "9"), "-1.0");
    // Minus the top: end; INT_MAX for what is no vsize.
    assert_eq!(pure(&mut vm, 3, "0", "'x"), "-2147483647.0");
    assert_eq!(pure(&mut vm, 4, "0", "(expt 2 40)"), "-1099511627776.0");
    assert_eq!(pure(&mut vm, 5, "0", "-1"), "-2147483647.0");
    // The unpure alignment takes the height; the pure height is kept.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:self-alignment-interface::y-aligned-on-self (thing 0))
                   (ly:self-alignment-interface::pure-y-aligned-on-self (thing 0) 7 7))"
        ),
        "(-1.0 -4.0)"
    );
}

#[test]
fn the_pure_height_in_rust_is_the_scheme_s() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 0–2 alike: a pure height of (start + 1 . end) at least
    // (0 . 10), a pure offset of 1/2 under thing 4's 2.
    eval(
        &mut vm,
        "(for-each
           (lambda (n)
             (put! (thing n) 'Y-extent
                   (ly:make-unpure-pure-container (lambda (g) '(1 . 4))
                                                  (lambda (g s e) (cons (+ s 1) e))))
             (put! (thing n) 'minimum-Y-extent '(0 . 10))
             (put! (thing n) 'Y-offset
                   (ly:make-unpure-pure-container (lambda (g) 7) (lambda (g s e) 1/2)))
             (ly:grob-set-parent! (thing n) 1 (thing 4)))
           '(0 1 2))
         (put! (thing 4) 'Y-offset 2)",
    );
    let [t0, t5] = ["(thing 0)", "(thing 5)"].map(|g| grob_of(&mut vm, g));
    let iv = unpure::pure_y_extent(&mut vm, t0, t5, 3, 9).unwrap();
    assert_eq!((iv.left, iv.right), (2.5, 12.5));
    let y = unpure::pure_relative_y_coordinate(&mut vm, t0, t5, 3, 9).unwrap();
    assert_eq!(y, 2.5);
    for text in [
        "(%grob-scheme-pure-y-extent (thing 1) (thing 5) 3 9)",
        "(ly:grob-pure-height (thing 2) (thing 5) 3 9)",
        // The heights kept; columns past i64 through the Scheme.
        "(%grob-pure-y-extent (thing 0) (thing 5) 3 (expt 2 63))",
        "(%grob-scheme-pure-y-extent (thing 2) (thing 5) 4 9)",
    ] {
        assert_eq!(caught(&mut vm, text), "(2.5 . 12.5)", "{text}");
    }
    // The offsets forgotten, their flag given up, nothing kept unpure.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:grob-property-data (thing 0) 'pure-Y-offset-in-progress)
                   (%grob-relative-coordinate (thing 0) '() 1))"
        ),
        "(() 9.0)"
    );
}

#[test]
fn the_translations_are_found_by_their_keys_as_assoc_finds_them() {
    let Some(mut vm) = grobs_vm() else { return };
    // Each alist in turn, as an alignment's grows and is replaced: the
    // index's entry is assoc's (eq?), its error assoc's.
    let found = |vm: &mut Vm, key: &str, alist: &str| {
        caught(
            vm,
            &format!(
                "(let* ((key {key}) (alist {alist})
                        (f (lambda (p) (catch #t (lambda () (p key alist))
                                         (lambda (k . args) k)))))
                   (let ((index (f (lambda (k a) (%align-translations-assoc (align 0) k a))))
                         (walk (f assoc)))
                     (list (eq? index walk) (if (pair? index) (cdr index) index))))"
            ),
        )
    };
    eval(
        &mut vm,
        "(define a1 (list (cons '(0 . 5) '(1.0))))
         (define a2 (cons (cons '(0 . 5) '(2.0)) a1))
         (define a3 (cons (cons '(1 . 5) '(3.0)) a2))",
    );
    for (key, alist, expected) in [
        ("'(0 . 5)", "'()", "(#t #f)"),
        ("'(0 . 5)", "a1", "(#t (1.0))"),
        ("'(0 . 5)", "a2", "(#t (2.0))"),
        ("'(1 . 5)", "a3", "(#t (3.0))"),
        ("'(0 . 5)", "a3", "(#t (2.0))"),
        ("'(0 . 6)", "a3", "(#t #f)"),
        // Another list read again; keys that are no fixnums' never found.
        (
            "'(0 . 5)",
            "(list '(x . 1) '((0 . 5.0) 4.0) '((0 . 5) 5.0) '((0 . 5) 6.0))",
            "(#t (5.0))",
        ),
        ("'(0 . 5.0)", "(list '((0 . 5.0) 4.0))", "(#t (4.0))"),
        ("'(0 . 5)", "a1", "(#t (1.0))"),
        // What is no alist: assoc's answer.
        ("'(0 . 5)", "(list 5 '((0 . 5) 1.0))", "(#t wrong-type-arg)"),
        ("'(9 . 9)", "(cons '((0 . 5) 1.0) 7)", "(#t wrong-type-arg)"),
    ] {
        assert_eq!(found(&mut vm, key, alist), expected, "{key} {alist}");
    }
}

#[test]
fn centered_on_the_x_parent_is_the_center_of_its_extent() {
    let Some(mut vm) = grobs_vm() else { return };
    // Thing 0, 4 wide and offset, is the X parent of thing 1; thing 2,
    // empty, of thing 3.
    eval(
        &mut vm,
        "(put! (thing 0) 'X-extent '(-1 . 3))
         (put! (thing 0) 'X-offset 5)
         (put! (thing 2) 'X-extent '(1 . -1))
         (ly:grob-set-parent! (thing 1) 0 (thing 0))
         (ly:grob-set-parent! (thing 3) 0 (thing 2))",
    );
    let centered = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!("(ly:self-alignment-interface::centered-on-x-parent (thing {n}))"),
        )
    };
    assert_eq!(centered(&mut vm, 1), "1.0");
    // An empty extent is its reference point.
    assert_eq!(centered(&mut vm, 3), "0.0");
    // Without an X parent, Guile's type error.
    assert!(centered(&mut vm, 4).contains("Wrong type argument"));
}

#[test]
fn aligned_on_the_x_parent_combines_both_extents() {
    let Some(mut vm) = grobs_vm() else { return };
    // Thing 0, 4 wide, is the X parent of things 1 to 3, each 2 wide.
    eval(
        &mut vm,
        "(put! (thing 0) 'X-extent '(0 . 4))
         (for-each (lambda (n)
                     (put! (thing n) 'X-extent '(-1 . 1))
                     (ly:grob-set-parent! (thing n) 0 (thing 0)))
                   '(1 2 3))
         (put! (thing 1) 'self-alignment-X 0)
         (put! (thing 2) 'self-alignment-X -1)
         (put! (thing 2) 'parent-alignment-X 1)
         (put! (thing 3) 'self-alignment-X 'x)
         (put! (thing 3) 'parent-alignment-X 1)",
    );
    let aligned = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!("(ly:self-alignment-interface::aligned-on-x-parent (thing {n}))"),
        )
    };
    // The parent's alignment is its own without one; either, without a
    // number, is not aligned.
    assert_eq!(aligned(&mut vm, 1), "2.0");
    assert_eq!(aligned(&mut vm, 2), "5.0");
    assert_eq!(aligned(&mut vm, 3), "4.0");
    eval(&mut vm, "(put! (thing 2) 'parent-alignment-X 'x)");
    assert_eq!(aligned(&mut vm, 2), "1.0");
    // Without an X parent, Guile's type error.
    assert!(aligned(&mut vm, 4).contains("Wrong type argument"));
}

#[test]
fn on_a_paper_column_it_is_aligned_on_the_note_columns() {
    let Some(mut vm) = grobs_vm() else { return };
    // Column 0 has notes 0 and 1, each offset, and thing 4 among its
    // elements; column 1 thing 5 only. Things 1 and 2, 2 wide, centered on
    // the center, are on them; the columns' own extents are not read.
    eval(
        &mut vm,
        "(put! (note 0) 'X-extent '(1 . 3))
         (put! (note 0) 'X-offset 10)
         (put! (note 1) 'X-extent '(2 . 6))
         (put! (thing 4) 'X-extent '(-10 . 10))
         (put! (thing 5) 'X-extent '(-10 . 10))
         (ly:grob-set-object! (column 0) 'elements (arr (note 0) (thing 4) (note 1)))
         (ly:grob-set-object! (column 1) 'elements (arr (thing 5)))
         (put! (column 0) 'X-extent (lambda (g) (error \"read\")))
         (put! (column 1) 'X-extent (lambda (g) (error \"read\")))
         (for-each (lambda (n c)
                     (put! (thing n) 'X-extent '(-1 . 1))
                     (put! (thing n) 'self-alignment-X 0)
                     (ly:grob-set-parent! (thing n) 0 (column c)))
                   '(1 2) '(0 1))",
    );
    let aligned = |vm: &mut Vm, n: usize| {
        caught(
            vm,
            &format!("(ly:self-alignment-interface::aligned-on-x-parent (thing {n}))"),
        )
    };
    // The notes' union is (1 . 6).
    assert_eq!(aligned(&mut vm, 1), "3.5");
    // Without note columns, the column's X-alignment-extent, a number
    // pair; else not aligned on it.
    assert_eq!(aligned(&mut vm, 2), "0.0");
    eval(&mut vm, "(put! (column 1) 'X-alignment-extent '(2 . x))");
    assert_eq!(aligned(&mut vm, 2), "0.0");
    eval(&mut vm, "(put! (column 1) 'X-alignment-extent '(2 . 4))");
    assert_eq!(aligned(&mut vm, 2), "3.0");
    eval(&mut vm, "(put! (column 1) 'X-alignment-extent '(2 . 1+i))");
    assert!(aligned(&mut vm, 2).contains("Wrong type (expecting real number): 1.0+1.0i"));
}

#[test]
fn on_a_note_column_it_may_be_aligned_on_the_main_note_heads() {
    let Some(mut vm) = grobs_vm() else { return };
    // Note 2, 10 wide with main heads 2 wide, is the X parent of thing 1,
    // left-aligned on its right; thing 0, on the same terms, of thing 2.
    eval(
        &mut vm,
        "(put! (note 2) 'X-extent '(0 . 10))
         (put! (note 2) 'main-extent '(0 . 2))
         (put! (thing 0) 'X-extent '(0 . 10))
         (put! (thing 0) 'main-extent '(0 . 2))
         (ly:grob-set-parent! (thing 1) 0 (note 2))
         (ly:grob-set-parent! (thing 2) 0 (thing 0))
         (for-each (lambda (n)
                     (put! (thing n) 'X-extent '(-1 . 1))
                     (put! (thing n) 'self-alignment-X -1)
                     (put! (thing n) 'parent-alignment-X 1))
                   '(1 2))",
    );
    let aligned = |vm: &mut Vm, n: usize, main: &str| {
        caught(
            vm,
            &format!(
                "(begin (put! (thing {n}) 'X-align-on-main-noteheads {main})
                        (ly:self-alignment-interface::aligned-on-x-parent (thing {n})))"
            ),
        )
    };
    // Only #t takes the main heads, and only of a note column.
    assert_eq!(aligned(&mut vm, 1, "#t"), "3.0");
    assert_eq!(aligned(&mut vm, 1, "1"), "11.0");
    assert_eq!(aligned(&mut vm, 1, "#f"), "11.0");
    assert_eq!(aligned(&mut vm, 2, "#t"), "11.0");
}

#[test]
fn the_pure_relevant_grobs_are_the_neighbors_by_priority() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:pure-from-neighbor-interface::calc-pure-relevant-grobs";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(music.iter().all(|p| p.name != name), "no placeholder");
    // LilyPond's name of the callback in its type error.
    let got = caught(&mut vm, &format!("({name} 1)"));
    assert!(
        got.contains("Wrong type argument in position 1 (expecting Grob): 1")
            && got.contains("ly:calc-pure-relevant-grobs")
            && !got.contains("pure-from-neighbor"),
        "{got}"
    );
    // Twelve neighbors of note 0, with LilyPond 2.26.0's order for these
    // priorities: -infinity without one, ties as they came (fewer than
    // 24 are sorted by insertion).
    eval(
        &mut vm,
        "(define twelve (append (map thing '(0 1 2 3 4 5)) (map group '(0 1 2 3 4 5))))
         (for-each (lambda (g p) (if (not (eq? p 'none)) (put! g 'outside-staff-priority p)))
                   twelve '(3 -inf.0 1 3 none 2 1 none 3 1/2 1 -2))
         (ly:grob-set-object! (note 0) 'neighbors (ly:grob-list->grob-array twelve))
         (define (indices arr)
           (map (lambda (g) (let loop ((l twelve) (i 0)) (if (eq? (car l) g) i (loop (cdr l) (1+ i)))))
                (ly:grob-array->list arr)))",
    );
    let order = |vm: &mut Vm, n: usize| caught(vm, &format!("(indices ({name} (note {n})))"));
    assert_eq!(order(&mut vm, 0), "(1 4 7 11 9 2 6 10 5 0 3 8)");
    // A new array; the neighbors keep theirs.
    assert_eq!(
        caught(
            &mut vm,
            &format!(
                "(let ((r ({name} (note 0))))
                   (list (eq? r (ly:grob-object (note 0) 'neighbors))
                         (indices (ly:grob-object (note 0) 'neighbors))))"
            )
        ),
        "(#f (0 1 2 3 4 5 6 7 8 9 10 11))"
    );
    // A broken piece of an item is left out, of a spanner kept.
    let (t2, t0, g1, g0) = (
        grob_of(&mut vm, "(thing 2)"),
        grob_of(&mut vm, "(thing 0)"),
        grob_of(&mut vm, "(group 1)"),
        grob_of(&mut vm, "(group 0)"),
    );
    grob::with_grob_mut(vm.heap(), t2, |g| g.original = Some(t0));
    grob::with_grob_mut(vm.heap(), g1, |g| g.original = Some(g0));
    assert_eq!(order(&mut vm, 0), "(1 4 7 11 9 6 10 5 0 3 8)");
    // Note 1, broken from note 0, takes note 0's neighbors into its own
    // array while note 0 lives; not without an array of its own.
    let (n0, n1) = (grob_of(&mut vm, "(note 0)"), grob_of(&mut vm, "(note 1)"));
    grob::with_grob_mut(vm.heap(), n1, |g| g.original = Some(n0));
    assert_eq!(order(&mut vm, 1), "()");
    eval(
        &mut vm,
        "(ly:grob-set-object! (note 1) 'neighbors (ly:grob-list->grob-array (list (thing 5))))",
    );
    assert_eq!(order(&mut vm, 1), "(1 4 7 11 9 6 10 5 0 3 8)");
    assert_eq!(
        caught(&mut vm, "(indices (ly:grob-object (note 1) 'neighbors))"),
        "(0 1 2 3 4 5 6 7 8 9 10 11)"
    );
    // A dead original's are not taken.
    eval(
        &mut vm,
        "(ly:grob-set-object! (note 1) 'neighbors (ly:grob-list->grob-array (list (thing 5) (thing 4))))
         (ly:grob-suicide! (note 0))",
    );
    assert_eq!(order(&mut vm, 1), "(4 5)");
}

#[test]
fn the_relative_group_extent_unites_the_elements_extents() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things on column 0 at their X-offsets: 0 at 1 (-1 . 1), 1 at 4
    // (0 . 2), 2 cross-staff, 3 empty.
    eval(
        &mut vm,
        "(for-each (lambda (n off ext)
                     (ly:grob-set-parent! (thing n) 0 (column 0))
                     (put! (thing n) 'X-offset off)
                     (put! (thing n) 'X-extent ext))
                   '(0 1 2 3) '(1 4 0 0) '((-1 . 1) (0 . 2) (-9 . 9) (1 . -1)))
         (put! (thing 2) 'cross-staff #t)",
    );
    let rel = |vm: &mut Vm, args: &str| caught(vm, &format!("(ly:relative-group-extent {args})"));
    let all = "(map thing '(0 1 2 3))";
    assert_eq!(rel(&mut vm, &format!("{all} (column 0) 0")), "(0.0 . 6.0)");
    assert_eq!(
        rel(
            &mut vm,
            &format!("(ly:grob-list->grob-array {all}) (column 0) 0")
        ),
        "(0.0 . 6.0)"
    );
    // Only #t is cross-staff; no elements, empty.
    eval(&mut vm, "(put! (thing 2) 'cross-staff 1)");
    assert_eq!(rel(&mut vm, &format!("{all} (column 0) 0")), "(-9.0 . 9.0)");
    assert_eq!(rel(&mut vm, "'() (column 0) 1"), "(+inf.0 . -inf.0)");
    // A list's other things are left out, with the warning.
    assert_eq!(
        rel(&mut vm, "(list (thing 0) 5) (column 0) 0"),
        "(0.0 . 2.0)"
    );
    // LilyPond's errors, the first one's by its C++ name.
    for (args, error) in [
        (
            "3 (column 0) 0",
            "Wrong type argument in position 1 (expecting list or Grob_array): 3",
        ),
        (
            "'() 3 0",
            "Wrong type argument in position 2 (expecting Grob): 3",
        ),
        (
            "'() (column 0) 2",
            "Wrong type argument in position 3 (expecting axis): 2",
        ),
        (
            "'() (column 0) 1.0",
            "Wrong type argument in position 3 (expecting axis): 1.0",
        ),
    ] {
        let got = rel(&mut vm, args);
        assert!(got.contains(error), "{args}: {got}");
    }
    assert!(rel(&mut vm, "3 (column 0) 0").contains("ly_relative_group_extent"));
}

#[test]
fn the_generic_bound_extent_takes_the_elements_of_its_interfaces() {
    let Some(mut vm) = grobs_vm() else { return };
    // Group 0 holds group 1, note 0 and thing 0; group 1 things 1 and 2.
    eval(
        &mut vm,
        "(for-each (lambda (g off ext)
                     (ly:grob-set-parent! g 0 (column 0))
                     (put! g 'X-offset off)
                     (put! g 'X-extent ext))
                   (list (thing 0) (thing 1) (thing 2) (note 0) (group 0) (group 1))
                   '(0 0 0 0 7 0) '((-5 . -4) (1 . 2) (3 . 4) (0 . 1) () (-1 . 0)))
         (ly:grob-set-object! (group 0) 'elements (arr (group 1) (note 0) (thing 0)))
         (ly:grob-set-object! (group 1) 'elements (arr (thing 1) (thing 2)))
         (put! (group 1) 'bound-alignment-interfaces '(item-interface))",
    );
    let bound = |vm: &mut Vm, ifaces: &str| {
        caught(
            vm,
            &format!(
                "(begin (put! (group 0) 'bound-alignment-interfaces {ifaces})
                        (ly:generic-bound-extent (group 0) (column 0)))"
            ),
        )
    };
    // The note column; an axis group by its own interfaces.
    assert_eq!(bound(&mut vm, "'(note-column-interface)"), "(0.0 . 1.0)");
    assert_eq!(
        bound(&mut vm, "'(axis-group-interface note-column-interface)"),
        "(0.0 . 4.0)"
    );
    assert_eq!(bound(&mut vm, "'(item-interface)"), "(-5.0 . 1.0)");
    // None: the group's robust extent, its offset for no extent; what is
    // no symbol, or the end of an improper list, names none.
    assert_eq!(bound(&mut vm, "'()"), "(7.0 . 7.0)");
    assert_eq!(
        bound(
            &mut vm,
            "'(\"note-column-interface\" . note-column-interface)"
        ),
        "(7.0 . 7.0)"
    );
    // On Y, the vertical skylines are read first.
    eval(
        &mut vm,
        "(define skylines-read #f)
         (put! (group 0) 'vertical-skylines (lambda (g) (set! skylines-read #t) '()))
         (put! (group 0) 'Y-extent '(1 . 2))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (%generic-bound-extent (group 0) (column 0) 0) skylines-read)"
        ),
        "((7.0 . 7.0) #f)"
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (%generic-bound-extent (group 0) (column 0) 1) skylines-read)"
        ),
        "((1.0 . 2.0) #t)"
    );
    // Without a common refpoint, that of the elements with the group.
    assert_eq!(
        caught(
            &mut vm,
            "(begin (put! (group 0) 'bound-alignment-interfaces '(item-interface))
                                (%generic-bound-extent (group 0) '() 0))"
        ),
        "(-5.0 . 1.0)"
    );
    for (args, error) in [
        ("1 (column 0)", "position 1 (expecting Grob): 1"),
        ("(group 0) 'x", "position 2 (expecting Grob): x"),
    ] {
        let got = caught(&mut vm, &format!("(ly:generic-bound-extent {args})"));
        assert!(
            got.contains(error) && got.contains("ly:generic-bound-extent"),
            "{got}"
        );
    }
}

#[test]
fn the_extremal_staff_is_the_furthest_live_one_meeting_the_interval() {
    let Some(mut vm) = grobs_vm() else { return };
    // Thing 5 aligns groups 0, 1 and 2, from 0 to 2, 3 to 5 and 1 to 4 on
    // column 0; group 0 hara-kiri, its worthy thing 3.
    eval(
        &mut vm,
        "(for-each (lambda (n ext)
                     (ly:grob-set-parent! (group n) 0 (column 0))
                     (put! (group n) 'X-extent ext))
                   '(0 1 2) '((0 . 2) (3 . 5) (1 . 4)))
         (ly:grob-set-object! (thing 5) 'elements (arr (group 0) (group 1) (group 2)))
         (put! (group 0) 'remove-empty #t)
         (put! (group 0) 'remove-first #t)
         (ly:grob-set-object! (group 0) 'items-worth-living (arr (thing 3)))",
    );
    let (me, col) = (
        grob_of(&mut vm, "(thing 5)"),
        grob_of(&mut vm, "(column 0)"),
    );
    let names = ["(group 0)", "(group 1)", "(group 2)"].map(|g| grob_of(&mut vm, g));
    let extremal = |vm: &mut Vm, dir: i32, left: f64, right: f64| {
        get_extremal_staff(vm, me, col, dir, Interval { left, right })
            .unwrap()
            .map(|g| names.iter().position(|&n| n == g).unwrap())
    };
    assert_eq!(extremal(&mut vm, 1, 0.0, 1.0), Some(0));
    assert_eq!(extremal(&mut vm, -1, 0.0, 1.0), Some(2));
    assert_eq!(extremal(&mut vm, 1, 3.0, 6.0), Some(1));
    assert_eq!(extremal(&mut vm, -1, 3.0, 6.0), Some(2));
    // Touching meets; an empty interval meets none.
    assert_eq!(extremal(&mut vm, -1, 5.0, 6.0), Some(1));
    assert_eq!(extremal(&mut vm, 1, 1.0, 0.0), None);
    assert_eq!(extremal(&mut vm, 1, 10.0, 11.0), None);
    // Group 0 empty once its worthy thing dies: it removes itself first.
    eval(&mut vm, "(ly:grob-suicide! (thing 3))");
    assert_eq!(extremal(&mut vm, 1, 0.0, 1.0), Some(2));
    assert!(!live(&mut vm, "(group 0)"));
    // No elements, no staff.
    let none = grob_of(&mut vm, "(thing 4)");
    assert_eq!(
        get_extremal_staff(
            &mut vm,
            none,
            col,
            1,
            Interval {
                left: 0.0,
                right: 9.0
            }
        )
        .unwrap(),
        None
    );
}

#[test]
fn a_child_is_within_the_group_when_the_next_staff_is_the_group_s() {
    let Some(mut vm) = grobs_vm() else { return };
    // Thing 5 groups groups 0 to 3; group 1 a loose line, group 2 thing
    // 4's, group 3 thing 5's.
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 5) 'elements (arr (group 0) (group 1) (group 2) (group 3)))
         (put! (group 1) 'staff-affinity 1)
         (for-each (lambda (n g) (ly:grob-set-object! (group n) 'staff-grouper g))
                   '(1 2 3) (list (thing 5) (thing 4) (thing 5)))",
    );
    let me = grob_of(&mut vm, "(thing 5)");
    let g = [
        "(group 0)",
        "(group 1)",
        "(group 2)",
        "(group 3)",
        "(thing 0)",
    ]
    .map(|t| grob_of(&mut vm, t));
    let within = |vm: &mut Vm, child: usize, pure: bool, start: u64, end: u64| {
        maybe_pure_within_group(vm, me, g[child], pure, start, end).unwrap()
    };
    // Past the loose line, group 2 is no staff of thing 5's.
    assert!(!within(&mut vm, 0, false, 0, 0));
    assert!(!within(&mut vm, 1, false, 0, 0));
    assert!(within(&mut vm, 2, false, 0, 0));
    // The last, and no element, are not.
    assert!(!within(&mut vm, 3, false, 0, 0));
    assert!(!within(&mut vm, 4, false, 0, 0));
    // Pure: group 2, empty but at rank 3, is alive between 0 and 6, not
    // between 4 and 9.
    eval(
        &mut vm,
        "(put! (group 2) 'remove-empty #t)
         (put! (group 2) 'remove-first #t)
         (put! (group 2) 'important-column-ranks #(3))",
    );
    assert!(!within(&mut vm, 0, true, 0, 6));
    assert!(within(&mut vm, 0, true, 4, 9));
    // Not pure: group 2 dead, group 3 decides.
    assert!(!within(&mut vm, 0, false, 4, 9));
    eval(&mut vm, "(ly:grob-suicide! (group 2))");
    assert!(within(&mut vm, 0, false, 0, 0));
    // A staff-grouper that is no grob is not the group.
    eval(&mut vm, "(ly:grob-set-object! (group 3) 'staff-grouper 5)");
    assert!(!within(&mut vm, 0, false, 0, 0));
}

#[test]
fn a_spacing_spec_s_number_is_its_first_entry_s_if_a_number() {
    let Some(mut vm) = grobs_vm() else { return };
    let spec = eval(
        &mut vm,
        "'(5 (padding . 1) (basic-distance . 3/2) (minimum-distance . x)
          (padding . 2) (stretchability . +inf.0) . 7)",
    );
    let read = |vm: &mut Vm, sym: &str| read_spacing_spec(vm.heap_mut(), spec, sym).unwrap();
    // What is no pair is skipped, the first entry counts, an exact number
    // is made inexact.
    assert_eq!(read(&mut vm, "padding"), Some(1.0));
    assert_eq!(read(&mut vm, "basic-distance"), Some(1.5));
    assert_eq!(read(&mut vm, "stretchability"), Some(f64::INFINITY));
    // No number, no entry, no list: none.
    assert_eq!(read(&mut vm, "minimum-distance"), None);
    assert_eq!(read(&mut vm, "space"), None);
    let none = eval(&mut vm, "#f");
    assert_eq!(
        read_spacing_spec(vm.heap_mut(), none, "padding").unwrap(),
        None
    );
}

/// The spacing spec from `before` to `after`, written; pure between the
/// columns 2 and 5.
fn spec(vm: &mut Vm, before: Option<&str>, after: Option<&str>, pure: bool) -> String {
    let before = before.map(|t| grob_of(vm, t));
    let after = after.map(|t| grob_of(vm, t));
    let v = get_spacing_spec(vm, before, after, pure, 2, 5).unwrap();
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

/// The warnings the test's `ly:warning` took, the last first.
fn warnings(vm: &mut Vm) -> String {
    let v = eval(vm, "warnings");
    shown(vm, v)
}

#[test]
fn the_spacing_spec_is_chosen_by_the_lines_staff_affinities() {
    let Some(mut vm) = grobs_vm() else { return };
    // Each group's specs name it; the staff-staff spacing a pure part.
    eval(
        &mut vm,
        "(define warnings '())
         (define (ly:warning fmt . args) (set! warnings (cons fmt warnings)))
         (for-each
          (lambda (n)
            (for-each
             (lambda (sym) (put! (group n) sym (list (cons sym n))))
             '(nonstaff-relatedstaff-spacing nonstaff-unrelatedstaff-spacing
               nonstaff-nonstaff-spacing)))
          '(0 1 2 3))
         (put! (group 0) 'staff-staff-spacing
               (ly:make-unpure-pure-container
                (lambda (g) '((basic-distance . 9)))
                (lambda (g start end) (list (cons 'start start) (cons 'end end)))))",
    );
    // No line on a side: a huge stretch.
    assert_eq!(
        spec(&mut vm, None, Some("(group 0)"), false),
        "((stretchability . 1.0e8))"
    );
    assert_eq!(
        spec(&mut vm, Some("(group 0)"), None, false),
        "((stretchability . 1.0e8))"
    );
    // Two staves: before's, pure between the columns until the unpure
    // lookup keeps its value.
    assert_eq!(
        spec(&mut vm, Some("(group 0)"), Some("(group 1)"), true),
        "((start . 2) (end . 5))"
    );
    assert_eq!(
        spec(&mut vm, Some("(group 0)"), Some("(group 1)"), false),
        "((basic-distance . 9))"
    );
    assert_eq!(
        spec(&mut vm, Some("(group 0)"), Some("(group 1)"), true),
        "((basic-distance . 9))"
    );
    // A loose line below a staff: related unless it turns down, which
    // adds a large stretch unless the spec has one.
    for (affinity, got) in [
        ("1", "((nonstaff-relatedstaff-spacing . 2))"),
        ("0", "((nonstaff-relatedstaff-spacing . 2))"),
        ("0.5", "((nonstaff-relatedstaff-spacing . 2))"),
        (
            "-1",
            "((stretchability . 1000000.0) (nonstaff-unrelatedstaff-spacing . 2))",
        ),
    ] {
        eval(
            &mut vm,
            &format!("(put! (group 2) 'staff-affinity {affinity})"),
        );
        assert_eq!(
            spec(&mut vm, Some("(group 0)"), Some("(group 2)"), false),
            got,
            "{affinity}"
        );
    }
    eval(
        &mut vm,
        "(put! (group 2) 'nonstaff-unrelatedstaff-spacing '((stretchability . 3) (padding . 1)))",
    );
    assert_eq!(
        spec(&mut vm, Some("(group 0)"), Some("(group 2)"), false),
        "((stretchability . 3) (padding . 1))"
    );
    // A loose line above a staff: unrelated when it turns up.
    eval(&mut vm, "(put! (group 3) 'staff-affinity 1)");
    assert_eq!(
        spec(&mut vm, Some("(group 3)"), Some("(group 0)"), false),
        "((stretchability . 1000000.0) (nonstaff-unrelatedstaff-spacing . 3))"
    );
    eval(&mut vm, "(put! (group 3) 'staff-affinity -1)");
    assert_eq!(
        spec(&mut vm, Some("(group 3)"), Some("(group 0)"), false),
        "((nonstaff-relatedstaff-spacing . 3))"
    );
    assert_eq!(warnings(&mut vm), "()");
    // Two loose lines: before's, unrelated only from UP to DOWN; the
    // warning once, and never pure, when the affinity grows.
    for (b, a, got) in [
        (
            "1",
            "-1",
            "((stretchability . 1000000.0) (nonstaff-unrelatedstaff-spacing . 3))",
        ),
        ("1", "1", "((nonstaff-nonstaff-spacing . 3))"),
        ("0", "-1", "((nonstaff-nonstaff-spacing . 3))"),
    ] {
        eval(
            &mut vm,
            &format!("(put! (group 3) 'staff-affinity {b}) (put! (group 2) 'staff-affinity {a})"),
        );
        assert_eq!(
            spec(&mut vm, Some("(group 3)"), Some("(group 2)"), false),
            got,
            "{b} {a}"
        );
    }
    assert_eq!(warnings(&mut vm), "()");
    eval(
        &mut vm,
        "(put! (group 3) 'staff-affinity -1) (put! (group 2) 'staff-affinity 0)",
    );
    spec(&mut vm, Some("(group 3)"), Some("(group 2)"), true);
    assert_eq!(warnings(&mut vm), "()");
    for _ in 0..2 {
        assert_eq!(
            spec(&mut vm, Some("(group 3)"), Some("(group 2)"), false),
            "((nonstaff-nonstaff-spacing . 3))"
        );
    }
    assert_eq!(warnings(&mut vm), "(staff-affinities should only decrease)");
}

/// `ly:get-spacing-spec` (page-layout-problem-scheme.cc, MS3 step 63): the
/// spec between two grobs, never pure, and the type error for what is no
/// grob, at its position.
#[test]
fn ly_get_spacing_spec_is_the_unpure_spec_between_two_grobs() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(put! (group 0) 'staff-staff-spacing
               (ly:make-unpure-pure-container
                (lambda (g) '((basic-distance . 9)))
                (lambda (g start end) '((basic-distance . 1)))))
         (put! (group 2) 'staff-affinity -1)
         (put! (group 2) 'nonstaff-unrelatedstaff-spacing '((padding . 1)))",
    );
    assert_eq!(
        evaluated(&mut vm, "(ly:get-spacing-spec (group 0) (group 1))"),
        "((basic-distance . 9))"
    );
    assert_eq!(
        evaluated(&mut vm, "(ly:get-spacing-spec (group 0) (group 2))"),
        "((stretchability . 1000000.0) (padding . 1))"
    );
    for (call, text) in [
        (
            "(ly:get-spacing-spec 1 (group 0))",
            "ly:get-spacing-spec: Wrong type argument in position 1 (expecting Grob): 1",
        ),
        (
            "(ly:get-spacing-spec (group 0) 'x)",
            "ly:get-spacing-spec: Wrong type argument in position 2 (expecting Grob): x",
        ),
    ] {
        let e = vm.eval_text(call, None).unwrap_err();
        assert_eq!(shown(&vm, e.key), "wrong-type-arg", "{call}");
        assert!(
            e.text(vm.heap()).contains(text),
            "{call}: {}",
            e.text(vm.heap())
        );
    }
}

#[test]
fn the_fixed_spacing_is_the_system_s_alignment_distance() {
    let Some(mut vm) = grobs_vm() else { return };
    // (group 5) the system, from (column 0); groups 0 to 2 lines on it.
    let sys = grob_of(&mut vm, "(group 5)");
    lily_engine_layout::item::set_class(
        vm.heap_mut(),
        sys,
        lily_engine_layout::item::Class::System,
    );
    let cols = ["(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, sys, -1, cols[0]).unwrap();
    for c in cols {
        grob::with_grob_mut(vm.heap(), c, |g| g.column.system = Some(sys));
    }
    let g = ["(group 0)", "(group 1)", "(group 2)", "(group 3)"].map(|t| grob_of(&mut vm, t));
    for &l in &g[..3] {
        spanner::set_bound(&mut vm, l, -1, cols[0]).unwrap();
        spanner::set_bound(&mut vm, l, 1, cols[1]).unwrap();
    }
    let fixed =
        |vm: &mut Vm, b: usize, a: usize, index: i64, pure: bool, start: usize, end: usize| {
            get_fixed_spacing(vm, g[b], g[a], index, pure, start, end)
                .map_err(|e| e.text(vm.heap()))
        };
    let ninf = f64::NEG_INFINITY;
    // No details: none forced.
    assert_eq!(fixed(&mut vm, 0, 1, 1, false, 0, 0), Ok(ninf));
    eval(
        &mut vm,
        "(put! (column 0) 'line-break-system-details '((alignment-distances 3 9/2 x)))",
    );
    // The index − 1th distance, the last past the end, the first below 0;
    // what is no number, none.
    for (index, got) in [
        (1, 3.0),
        (2, 4.5),
        (3, ninf),
        (9, ninf),
        (0, 3.0),
        (-4, 3.0),
    ] {
        assert_eq!(fixed(&mut vm, 0, 1, index, false, 0, 0), Ok(got), "{index}");
    }
    // A loose line on either side, or a line of no system: none.
    eval(&mut vm, "(put! (group 2) 'staff-affinity 1)");
    assert_eq!(fixed(&mut vm, 0, 2, 1, false, 0, 0), Ok(ninf));
    assert_eq!(fixed(&mut vm, 2, 0, 1, false, 0, 0), Ok(ninf));
    assert_eq!(fixed(&mut vm, 3, 1, 1, false, 0, 0), Ok(ninf));
    // No distances, or details that are no alist.
    eval(
        &mut vm,
        "(put! (column 0) 'line-break-system-details '((alignment-distances)))",
    );
    assert_eq!(fixed(&mut vm, 0, 1, 1, false, 0, 0), Ok(ninf));
    for (details, error) in [
        (
            "5",
            "Wrong type argument in position 2 (expecting association list): 5",
        ),
        (
            "'(1)",
            "Wrong type argument in position 2 (expecting association list): (1)",
        ),
    ] {
        eval(
            &mut vm,
            &format!("(put! (column 0) 'line-break-system-details {details})"),
        );
        let got = fixed(&mut vm, 0, 1, 1, false, 0, 0).unwrap_err();
        assert!(
            got.contains(error) && got.contains("assq"),
            "{details}: {got}"
        );
    }
    // Pure: the system is the root of before's X parents, none yet; the
    // result kept from the start, whatever the end.
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 2, 4), Ok(ninf));
    assert_eq!(fixed(&mut vm, 0, 2, 1, true, 2, 4), Ok(ninf));
    eval(&mut vm, "(ly:grob-set-parent! (column 0) 0 (group 5))");
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 2, 9), Ok(ninf));
    // The pure bound is the column of the break ranked `start`: none for
    // a system in no Paper_score.
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 4, 9), Ok(ninf));
    // Columns 0 and 1 the breaks ranked 3 and 9, the first broken on the
    // right and the last on the left.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 5) 'columns (arr (column 0) (column 1)))
         (put! (column 0) 'line-break-system-details '((alignment-distances 3 9/2 x)))
         (put! (column 0) 'line-break-permission 'allow)
         (put! (column 1) 'line-break-permission 'allow)",
    );
    for (c, rank, piece) in [(0, 3, 0), (1, 9, 1)] {
        paper_column::set_rank(vm.heap(), cols[c], rank);
        let piece = grob_of(&mut vm, &format!("(piece {piece})"));
        grob::with_grob_mut(vm.heap(), cols[c], |g| g.broken[1 - c] = Some(piece));
    }
    paper_score_of(&mut vm, sys);
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 3, 9), Ok(3.0));
    // Column 1, with no details, and no break ranked 5: none.
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 9, 3), Ok(ninf));
    assert_eq!(fixed(&mut vm, 0, 1, 1, true, 5, 9), Ok(ninf));
    // Not read for a loose line.
    assert_eq!(fixed(&mut vm, 0, 2, 1, true, 3, 9), Ok(ninf));
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The axis groups' callbacks of MS3 step 28, each with the C++ method
/// that names its type error (LilyPond 2.26.0's, session 2's
/// `scratch/s2-errs.ly`) and its arity; `None` checks no argument.
const AXIS_GROUP_CALLBACKS: [(&str, Option<&str>, usize); 8] = [
    ("ly:axis-group-interface::width", Some("ly:width"), 1),
    ("ly:axis-group-interface::height", Some("ly:height"), 1),
    (
        "ly:axis-group-interface::pure-height",
        Some("ly:pure-height"),
        3,
    ),
    (
        "ly:axis-group-interface::calc-pure-relevant-grobs",
        Some("ly:calc-pure-relevant-grobs"),
        1,
    ),
    (
        "ly:axis-group-interface::calc-pure-y-common",
        Some("ly:calc-pure-y-common"),
        1,
    ),
    ("ly:axis-group-interface::calc-x-common", None, 1),
    ("ly:axis-group-interface::calc-y-common", None, 1),
    (
        "ly:axis-group-interface::adjacent-pure-heights",
        Some("ly:adjacent-pure-heights"),
        1,
    ),
];

/// The texts of the diagnostics given since the last call, in order.
fn diagnosed(vm: &mut Vm) -> String {
    let v = eval(
        vm,
        "(let ((d (reverse (map (lambda (v) (vector-ref v 2)) (car %diagnostics)))))
           (set-car! %diagnostics '())
           d)",
    );
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

#[test]
fn the_axis_group_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, method, arity) in AXIS_GROUP_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let rest = " 0".repeat(arity - 1);
        let got = caught(&mut vm, &format!("({name} 'x{rest})"));
        let subr = method.unwrap_or("ly:grob-object");
        assert!(
            got.contains(subr)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", &" 'x".repeat(arity + 1)] {
            let got = caught(&mut vm, &format!("({name}{args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn the_group_extent_is_its_elements_less_its_own_coordinate() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 0 and 1 and stem 0 in group 0, on column 0 on X; on Y in
    // group 0 itself. Each records the reading of its vertical skylines.
    eval(
        &mut vm,
        "(define read '())
         (for-each (lambda (g n xo xe yo ye)
                     (ly:grob-set-parent! g 0 (column 0))
                     (ly:grob-set-parent! g 1 (group 0))
                     (put! g 'X-offset xo)
                     (put! g 'X-extent xe)
                     (put! g 'Y-offset yo)
                     (put! g 'Y-extent ye)
                     (put! g 'vertical-skylines (lambda (x) (set! read (cons n read)) '())))
                   (list (thing 0) (thing 1) (stem 0)) '(0 1 s)
                   '(1 4 0) '((-1 . 1) (0 . 2) (-9 . 9)) '(1 -3 0) '((-1 . 1) (0 . 1) (5 . 6)))
         (ly:grob-set-parent! (group 0) 0 (column 0))
         (put! (group 0) 'X-offset 2)
         (put! (stem 0) 'cross-staff #t)
         (ly:grob-set-object! (group 0) 'elements (arr (thing 0) (thing 1) (stem 0)))",
    );
    // On X, relative to column 0, less the group's 2; no skylines read.
    // A cross-staff grob is left out.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:axis-group-interface::width (group 0)) read)"
        ),
        "((-2.0 . 4.0) ())"
    );
    // On Y each element's skylines are read first, but a cross-staff
    // stem's.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:axis-group-interface::height (group 0)) (reverse read))"
        ),
        "((-3.0 . 2.0) (0 1))"
    );
    eval(&mut vm, "(set! read '()) (put! (stem 0) 'cross-staff 1)");
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:axis-group-interface::height (group 0)) read)"
        ),
        "((-3.0 . 6.0) (s))"
    );
    // No elements: empty.
    assert_eq!(
        caught(&mut vm, "(ly:axis-group-interface::width (group 1))"),
        "(+inf.0 . -inf.0)"
    );
}

#[test]
fn the_common_refpoints_are_the_group_s_with_its_elements() {
    let Some(mut vm) = grobs_vm() else { return };
    // Group 0 and its things 0 and 1 on column 0; the things in it on Y.
    // Thing 2 on column 1, which shares no parent with column 0.
    eval(
        &mut vm,
        "(for-each (lambda (g) (ly:grob-set-parent! g 0 (column 0))) (list (group 0) (thing 0) (thing 1)))
         (for-each (lambda (g) (ly:grob-set-parent! g 1 (group 0))) (list (thing 0) (thing 1)))
         (ly:grob-set-parent! (thing 2) 0 (column 1))
         (ly:grob-set-object! (group 0) 'elements (arr (thing 0) (thing 1)))
         (ly:grob-set-object! (group 0) 'pure-relevant-grobs (arr (thing 1)))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (eq? (ly:axis-group-interface::calc-x-common (group 0)) (column 0))
                   (eq? (ly:axis-group-interface::calc-y-common (group 0)) (group 0))
                   (eq? (ly:axis-group-interface::calc-pure-y-common (group 0)) (group 0))
                   (eq? (ly:axis-group-interface::calc-x-common (group 1)) (group 1)))"
        ),
        "(#t #t #t #t)"
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // None: '(), with the programming error.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'elements (arr (thing 0) (thing 2)))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(ly:axis-group-interface::calc-x-common (group 0))"
        ),
        "()"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"No common parent found in calc_common axis.\")"
    );
    // The pure one: an alignment other than the group is the group, with
    // the programming error; none, '().
    eval(
        &mut vm,
        "(for-each (lambda (g) (ly:grob-set-parent! g 1 (align 0))) (list (group 1) (group 2)))
         (ly:grob-set-object! (group 1) 'pure-relevant-grobs (arr (group 2)))
         (ly:grob-set-object! (group 3) 'pure-relevant-grobs (arr (thing 0)))
         (ly:grob-set-object! (group 4) 'pure-relevant-grobs (arr (thing 0) (group 5)))
         (ly:grob-set-object! (align 0) 'pure-relevant-grobs (arr (group 2)))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (eq? (ly:axis-group-interface::calc-pure-y-common (group 1)) (group 1))
                   (eq? (ly:axis-group-interface::calc-pure-y-common (align 0)) (align 0))
                   (ly:axis-group-interface::calc-pure-y-common (group 3)))"
        ),
        "(#t #t ())"
    );
    // Past a grob with none, the next starts again
    // (`common_refpoint_of_array`).
    assert_eq!(
        caught(
            &mut vm,
            "(eq? (ly:axis-group-interface::calc-pure-y-common (group 4)) (group 5))"
        ),
        "#t"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"My pure_y_common is a VerticalAlignment, which might contain several staves.\" \
         \"No common parent found in calc_pure_y_common.\")"
    );
    // The pure-relevant grobs of the elements.
    assert_eq!(
        caught(
            &mut vm,
            "(equal? (ly:grob-array->list (ly:axis-group-interface::calc-pure-relevant-grobs (group 0)))
                     (list (thing 0) (thing 2)))"
        ),
        "#t"
    );
}

/// `(ly:axis-group-interface::pure-height g start end)`, written.
fn pure_height(vm: &mut Vm, g: &str, start: &str, end: &str) -> String {
    caught(
        vm,
        &format!("(ly:axis-group-interface::pure-height {g} {start} {end})"),
    )
}

#[test]
fn a_group_s_pure_height_unites_its_relevant_grobs_between_the_columns() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 0 and 1 on the columns of ranks 3 and 7, stem 0 cross-staff
    // on rank 5, thing 3 on none; all in group 2 on Y.
    put_on(&mut vm, 0, 3, &[0]);
    put_on(&mut vm, 1, 7, &[1]);
    put_on(&mut vm, 2, 5, &[]);
    eval(
        &mut vm,
        "(ly:grob-set-parent! (stem 0) 0 (column 2))
         (for-each (lambda (g yo ye)
                     (ly:grob-set-parent! g 1 (group 2))
                     (put! g 'Y-offset yo)
                     (put! g 'Y-extent ye))
                   (list (thing 0) (thing 1) (stem 0) (thing 3))
                   '(1 -3 0 0) '((-1 . 1) (0 . 1) (5 . 6) (9 . 9)))
         (put! (stem 0) 'cross-staff #t)
         (ly:grob-set-object! (group 2) 'pure-Y-common (group 2))
         (ly:grob-set-object! (group 2) 'pure-relevant-grobs
                              (arr (thing 0) (thing 1) (stem 0) (thing 3)))",
    );
    assert_eq!(pure_height(&mut vm, "(group 2)", "0", "10"), "(-3.0 . 2.0)");
    // What is no size is 0 or INT_MAX.
    assert_eq!(
        pure_height(&mut vm, "(group 2)", "'a", "-1"),
        "(-3.0 . 2.0)"
    );
    // Spanning a column from start to end.
    assert_eq!(
        pure_height(&mut vm, "(group 2)", "4", "10"),
        "(-3.0 . -2.0)"
    );
    // On the end's column an item is its piece at the end of the line, on
    // the start's at the beginning: none before breaking.
    assert_eq!(pure_height(&mut vm, "(group 2)", "0", "7"), "(0.0 . 2.0)");
    assert_eq!(
        pure_height(&mut vm, "(group 2)", "3", "10"),
        "(-3.0 . -2.0)"
    );
    // Thing 4, thing 0's piece at the beginning of a line, while visible
    // there.
    let (t0, t4, t5) = (
        grob_of(&mut vm, "(thing 0)"),
        grob_of(&mut vm, "(thing 4)"),
        grob_of(&mut vm, "(thing 5)"),
    );
    grob::with_grob_mut(vm.heap(), t0, |g| g.broken = [Some(t5), Some(t4)]);
    grob::with_grob_mut(vm.heap(), t4, |g| g.original = Some(t0));
    eval(
        &mut vm,
        "(ly:grob-set-parent! (thing 4) 0 (column 0))
         (ly:grob-set-parent! (thing 4) 1 (group 2))
         (put! (thing 4) 'Y-extent '(2 . 3))",
    );
    assert_eq!(pure_height(&mut vm, "(group 2)", "3", "10"), "(-3.0 . 3.0)");
    eval(&mut vm, "(put! (thing 4) 'break-visibility #(#t #t #f))");
    assert_eq!(
        pure_height(&mut vm, "(group 2)", "3", "10"),
        "(-3.0 . -2.0)"
    );
    eval(&mut vm, "(put! (thing 4) 'break-visibility #(#f #f #t))");
    assert_eq!(pure_height(&mut vm, "(group 2)", "3", "10"), "(-3.0 . 3.0)");
    // Only #t is cross-staff.
    eval(&mut vm, "(put! (stem 0) 'cross-staff 1)");
    assert_eq!(pure_height(&mut vm, "(group 2)", "0", "10"), "(-3.0 . 6.0)");
    // Less the group's pure coordinate on its common refpoint: group 4 at
    // 1 to 2 in group 3, which is 2 up thing 5.
    eval(
        &mut vm,
        "(ly:grob-set-parent! (group 3) 1 (thing 5))
         (put! (group 3) 'Y-offset 2)
         (ly:grob-set-parent! (group 4) 1 (group 3))
         (put! (group 4) 'Y-extent '(1 . 2))
         (ly:grob-set-object! (group 3) 'pure-Y-common (thing 5))
         (ly:grob-set-object! (group 3) 'pure-relevant-grobs (arr (group 4)))",
    );
    assert_eq!(pure_height(&mut vm, "(group 3)", "0", "1"), "(1.0 . 2.0)");
    // No common refpoint: empty, with the programming error.
    assert_eq!(diagnosed(&mut vm), "()");
    assert_eq!(
        pure_height(&mut vm, "(group 5)", "0", "1"),
        "(+inf.0 . -inf.0)"
    );
    assert_eq!(diagnosed(&mut vm), "(\"no pure Y common refpoint\")");
}

#[test]
fn an_aligned_group_s_pure_height_is_its_adjacent_heights_by_break() {
    let Some(mut vm) = grobs_vm() else { return };
    // Groups 0 and 1 and thing 0 children of align 0.
    eval(
        &mut vm,
        "(for-each (lambda (g)
                     (ly:grob-set-parent! g 1 (align 0))
                     (ly:grob-set-object! g 'pure-Y-common g))
                   (list (group 0) (group 1) (thing 0)))
         (put! (group 0) 'adjacent-pure-heights
               (cons #((0 . 1) (2 . 3) (-1 . 0)) #((5 . 6) (7 . 8) (-4 . -3))))",
    );
    // No root system, so no break ranks: nothing between them.
    assert_eq!(caught(&mut vm, "(%break-ranks (group 0))"), "#()");
    assert_eq!(
        pure_height(&mut vm, "(group 0)", "1", "3"),
        "(+inf.0 . -inf.0)"
    );
    eval(&mut vm, "(define (%break-ranks me) #(0 4 8 12))");
    // At the beginning of the line from 4 to 5, the measure from 4; in
    // the rest from 4 to 12, those from 4 and from 8.
    assert_eq!(pure_height(&mut vm, "(group 0)", "4", "12"), "(-4.0 . 8.0)");
    assert_eq!(
        caught(
            &mut vm,
            "(list (%spanner-cached-pure-property (group 0) 'begin-of-line-pure-height 4 5)
                   (%spanner-cached-pure-property (group 0) 'rest-of-line-pure-height 4 12))"
        ),
        "((2.0 . 3.0) (-4.0 . 8.0))"
    );
    // Kept: new heights change only what was not computed.
    eval(
        &mut vm,
        "(put! (group 0) 'adjacent-pure-heights
               (cons #((10 . 11) (12 . 13) (14 . 15)) #((20 . 21) (22 . 23) (24 . 25))))",
    );
    assert_eq!(pure_height(&mut vm, "(group 0)", "4", "12"), "(-4.0 . 8.0)");
    assert_eq!(pure_height(&mut vm, "(group 0)", "0", "4"), "(10.0 . 21.0)");
    // From 3: no break from 3 before 4 at the beginning of the line.
    assert_eq!(pure_height(&mut vm, "(group 0)", "3", "9"), "(22.0 . 25.0)");
    // No pair, or no vector in it: (0 . 0); no spanner: (0 . 0).
    eval(&mut vm, "(put! (group 1) 'adjacent-pure-heights 5)");
    assert_eq!(pure_height(&mut vm, "(group 1)", "0", "4"), "(0.0 . 0.0)");
    eval(
        &mut vm,
        "(put! (group 2) 'adjacent-pure-heights (cons 1 #((5 . 6))))
         (ly:grob-set-parent! (group 2) 1 (align 0))
         (ly:grob-set-object! (group 2) 'pure-Y-common (group 2))",
    );
    assert_eq!(pure_height(&mut vm, "(group 2)", "0", "4"), "(0.0 . 6.0)");
    assert_eq!(pure_height(&mut vm, "(thing 0)", "0", "4"), "(0.0 . 0.0)");
}

/// `(ly:axis-group-interface::adjacent-pure-heights g)`, written.
fn adjacent(vm: &mut Vm, g: &str) -> String {
    caught(
        vm,
        &format!("(ly:axis-group-interface::adjacent-pure-heights {g})"),
    )
}

#[test]
fn adjacent_pure_heights_unite_each_measure_s_grobs_by_line_position() {
    let Some(mut vm) = grobs_vm() else { return };
    // Breaks at 0, 4 and 8. Things 0, 2, 3 and 5 on the column of rank 1,
    // thing 1 on rank 4, its piece at the beginning of a line thing 4; all
    // in group 0 on Y. Thing 2 outside the staff up, 0.5 from it; thing 3
    // down, by default, with LilyPond 2.26.0's staff and TextScript of
    // `{ c''4_\markup "xyz" c''2. }` (session 2's `scratch/s2-aph.ly`).
    put_on(&mut vm, 0, 1, &[0, 2, 3, 5]);
    put_on(&mut vm, 1, 4, &[1, 4]);
    let (t1, t4) = (grob_of(&mut vm, "(thing 1)"), grob_of(&mut vm, "(thing 4)"));
    grob::with_grob_mut(vm.heap(), t1, |g| g.broken = [None, Some(t4)]);
    grob::with_grob_mut(vm.heap(), t4, |g| g.original = Some(t1));
    eval(
        &mut vm,
        "(define (%break-ranks me) #(0 4 8))
         (for-each (lambda (n ye)
                     (ly:grob-set-parent! (thing n) 1 (group 0))
                     (put! (thing n) 'Y-extent ye))
                   '(0 1 2 3 4 5)
                   '((-3 . 2.05) (0 . 2) (1 . 2) (-3.82849249507874 . -2.350060623769685)
                     (3 . 4) (9 . 9)))
         (put! (thing 2) 'outside-staff-priority 1)
         (put! (thing 2) 'outside-staff-padding 1/2)
         (put! (thing 3) 'outside-staff-priority 450)
         (put! (thing 3) 'direction -1)
         (put! (thing 5) 'cross-staff #t)
         (ly:grob-set-object! (group 0) 'pure-Y-common (group 0))
         (ly:grob-set-object! (group 1) 'pure-Y-common (group 0))
         (ly:grob-set-object! (group 0) 'pure-relevant-grobs
                              (arr (thing 0) (thing 1) (thing 2) (thing 3) (thing 5)))
         (ly:grob-set-object! (group 1) 'pure-relevant-grobs (arr (thing 0) (thing 3)))",
    );
    // Thing 1 at the beginning of the line from 4 by its piece, in the
    // rest of the one from 0 by itself; thing 2 moved up 1.55 to be 0.5
    // above the staff's 2.05.
    assert_eq!(
        adjacent(&mut vm, "(group 0)"),
        "(#((+inf.0 . -inf.0) (3.0 . 4.0)) . #((-3.0 . 3.55) (+inf.0 . -inf.0)))"
    );
    // Down from the staff, LilyPond 2.26.0's `unite_disjoint` moves the
    // TextScript up, into the staff: the staff's heights, as LilyPond's.
    assert_eq!(
        adjacent(&mut vm, "(group 1)"),
        "(#((+inf.0 . -inf.0) (+inf.0 . -inf.0)) . #((-3.0 . 2.05) (+inf.0 . -inf.0)))"
    );
    // Visible at the next break: thing 1's piece hidden at the beginning
    // of a line.
    eval(&mut vm, "(put! (thing 4) 'break-visibility #(#f #f #f))");
    assert_eq!(
        adjacent(&mut vm, "(group 0)"),
        "(#((+inf.0 . -inf.0) (+inf.0 . -inf.0)) . #((-3.0 . 3.55) (+inf.0 . -inf.0)))"
    );
    // No break: two empty vectors.
    eval(&mut vm, "(define (%break-ranks me) #())");
    assert_eq!(adjacent(&mut vm, "(group 0)"), "(#() . #())");
}

#[test]
fn reading_adjacent_pure_heights_while_computing_them_is_a_cyclic_dependency() {
    let Some(mut vm) = grobs_vm() else { return };
    // As T29's, and session 2's `scratch/s2-cycle.ly`: things 0 and 1 in
    // group 0, an alignment's child, whose pure height their pure heights
    // read. The first reads the adjacent heights twice while they are
    // computed, each the programming error, and gets (0 . 0), which the
    // group keeps; the second gets that.
    put_on(&mut vm, 0, 1, &[0]);
    put_on(&mut vm, 1, 3, &[1]);
    eval(
        &mut vm,
        "(define (%break-ranks me) #(0 4))
         (define inner '())
         (for-each (lambda (g)
                     (ly:grob-set-parent! g 1 (group 0))
                     (put! g 'Y-extent
                           (ly:make-unpure-pure-container
                            ly:grob::stencil-height
                            (lambda (g s e)
                              (set! inner (cons (ly:axis-group-interface::pure-height (group 0) s e) inner))
                              '(-1 . 1)))))
                   (list (thing 0) (thing 1)))
         (ly:grob-set-parent! (group 0) 1 (align 0))
         (put! (group 0) 'adjacent-pure-heights ly:axis-group-interface::adjacent-pure-heights)
         (ly:grob-set-object! (group 0) 'pure-Y-common (group 0))
         (ly:grob-set-object! (group 0) 'pure-relevant-grobs (arr (thing 0) (thing 1)))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:grob-property (group 0) 'adjacent-pure-heights) inner)"
        ),
        "((#((+inf.0 . -inf.0)) . #((-1.0 . 1.0))) ((0.0 . 0.0) (0.0 . 0.0)))"
    );
    let cyclic = "\"cyclic dependency: calculation-in-progress encountered for \
                  Group.adjacent-pure-heights\"";
    assert_eq!(diagnosed(&mut vm), format!("({cyclic} {cyclic})"));
    assert_eq!(pure_height(&mut vm, "(group 0)", "0", "4"), "(0.0 . 0.0)");
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The axis groups' skylines and staff spacing callbacks of MS3 step 28,
/// each with the C++ method that names its type error (LilyPond 2.26.0's,
/// session 3's `scratch/s3-errs.ly`) and its arity; `None` checks no
/// argument.
const AXIS_GROUP_SKYLINE_CALLBACKS: [(&str, Option<&str>, usize); 4] = [
    (
        "ly:axis-group-interface::calc-skylines",
        Some("ly:calc-skylines"),
        1,
    ),
    (
        "ly:axis-group-interface::combine-skylines",
        Some("ly:combine-skylines"),
        1,
    ),
    ("ly:axis-group-interface::calc-staff-staff-spacing", None, 1),
    (
        "ly:axis-group-interface::calc-pure-staff-staff-spacing",
        None,
        3,
    ),
];

#[test]
fn the_axis_group_skyline_and_spacing_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, method, arity) in AXIS_GROUP_SKYLINE_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let rest = " 0".repeat(arity - 1);
        let got = caught(&mut vm, &format!("({name} 'x{rest})"));
        let subr = method.unwrap_or("ly:grob-object");
        assert!(
            got.contains(subr)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", &" 'x".repeat(arity + 1)] {
            let got = caught(&mut vm, &format!("({name}{args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
    // The pure one's columns are scm_to_ulong's, the start first, before
    // the grob is read.
    let pure = "ly:axis-group-interface::calc-pure-staff-staff-spacing";
    let got = caught(&mut vm, &format!("({pure} 'x 'a -1)"));
    assert!(
        got.contains("Wrong type (expecting exact integer): a"),
        "{got}"
    );
    let got = caught(&mut vm, &format!("({pure} 'x 0 -1)"));
    assert!(
        got.contains("Value out of range 0 to< 18446744073709551615: -1"),
        "{got}"
    );
}

/// `(skyline . points)` of the skyline pair `text`, down then up.
fn sky_points(vm: &mut Vm, text: &str) -> String {
    caught(
        vm,
        &format!(
            "(let ((p {text}))
               (list (ly:skyline->points (car p) 0) (ly:skyline->points (cdr p) 0)))"
        ),
    )
}

/// Down's points and up's of the box `x` by `y`, as `ly:skyline->points`
/// gives them.
fn box_points(x: (f64, f64), y: (f64, f64)) -> String {
    format!(
        "(((-inf.0 . +inf.0) ({x0:?} . +inf.0) ({x0:?} . {y0:?}) ({x1:?} . {y0:?}) ({x1:?} . +inf.0) \
         (+inf.0 . +inf.0)) \
         ((-inf.0 . -inf.0) ({x0:?} . -inf.0) ({x0:?} . {y1:?}) ({x1:?} . {y1:?}) ({x1:?} . -inf.0) \
         (+inf.0 . -inf.0)))",
        x0 = x.0,
        x1 = x.1,
        y0 = y.0,
        y1 = y.1
    )
}

#[test]
fn combined_skylines_are_the_elements_at_their_offsets() {
    let Some(mut vm) = grobs_vm() else { return };
    // Groups 0 and 1 in align 0, on column 0 on X, at 1 and -4 on Y; the
    // alignment at 0.5 on X. Group 2 has no skylines.
    eval(
        &mut vm,
        "(for-each (lambda (g xo yo sky)
                     (ly:grob-set-parent! g 0 (column 0))
                     (ly:grob-set-parent! g 1 (align 0))
                     (put! g 'X-offset xo)
                     (put! g 'Y-offset yo)
                     (put! g 'vertical-skylines sky))
                   (list (group 0) (group 1) (group 2)) '(0 1 0) '(1 -4 0)
                   (list (%box-skylines '(0 . 2) '(-1 . 1) 0)
                         (%box-skylines '(0 . 2) '(-1 . 1) 0)
                         '()))
         (ly:grob-set-parent! (align 0) 0 (column 0))
         (put! (align 0) 'X-offset 0.5)
         (ly:grob-set-object! (align 0) 'elements (arr (group 0) (group 1) (group 2)))",
    );
    let got = sky_points(
        &mut vm,
        "(ly:axis-group-interface::combine-skylines (align 0))",
    );
    assert_eq!(
        got,
        "(((-inf.0 . +inf.0) (-0.5 . +inf.0) (-0.5 . -0.0) (0.5 . -0.0) (0.5 . -5.0) (2.5 . -5.0) \
         (2.5 . +inf.0) (+inf.0 . +inf.0)) \
         ((-inf.0 . -inf.0) (-0.5 . -inf.0) (-0.5 . 2.0) (1.5 . 2.0) (1.5 . -3.0) (2.5 . -3.0) \
         (2.5 . -inf.0) (+inf.0 . -inf.0)))"
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // An element outside it on Y: LilyPond's programming error, the
    // skylines still combined.
    eval(&mut vm, "(ly:grob-set-parent! (group 1) 1 (thing 0))");
    let got = sky_points(
        &mut vm,
        "(ly:axis-group-interface::combine-skylines (align 0))",
    );
    assert!(got.starts_with("(((-inf.0 . +inf.0)"), "{got}");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"combining skylines that don't belong to me\")"
    );
    // No elements: the empty pair.
    eval(&mut vm, "(ly:grob-set-object! (align 0) 'elements (arr))");
    assert_eq!(
        sky_points(
            &mut vm,
            "(ly:axis-group-interface::combine-skylines (align 0))"
        ),
        "(((-inf.0 . +inf.0) (+inf.0 . +inf.0)) ((-inf.0 . -inf.0) (+inf.0 . -inf.0)))"
    );
}

/// Group 0 on column 0, at 0.5 on X; things 0 and 1 in it, in the staff:
/// the boxes (0, 2) by (-1, 1) at 0 and (0, 1) by (-2, 2) at 4. Thing 2,
/// outside the staff up by 100, (0, 2) by (0, 1) at 1, padded 0.5, with
/// thing 3, a rider, (0, 0.5) by (-0.5, 0.5) at 1 on it; thing 4 outside
/// down by 100, (0, 1) by (-1, 0) at 0. Thing 5 is cross-staff.
const SPACED: &str = "(ly:grob-set-parent! (group 0) 0 (column 0))
     (put! (group 0) 'X-offset 0.5)
     (put! (group 0) 'outside-staff-placement-directive 'left-to-right-polite)
     (for-each (lambda (g xo xe ye)
                 (ly:grob-set-parent! g 0 (column 0))
                 (ly:grob-set-parent! g 1 (group 0))
                 (put! g 'X-offset xo)
                 (put! g 'Y-offset 0)
                 (put! g 'X-extent xe)
                 (put! g 'vertical-skylines (%box-skylines xe ye 0)))
               (list (thing 0) (thing 1) (thing 2) (thing 3) (thing 4) (thing 5))
               '(0 4 1 1 0 0)
               '((0 . 2) (0 . 1) (0 . 2) (0 . 0.5) (0 . 1) (0 . 9))
               '((-1 . 1) (-2 . 2) (0 . 1) (-0.5 . 0.5) (-1 . 0) (-9 . 9)))
     (ly:grob-set-parent! (thing 3) 1 (thing 2))
     (put! (thing 2) 'outside-staff-priority 100)
     (put! (thing 2) 'outside-staff-padding 0.5)
     (put! (thing 2) 'direction 1)
     (put! (thing 4) 'outside-staff-priority 100)
     (put! (thing 4) 'direction -1)
     (put! (thing 5) 'cross-staff #t)
     (ly:grob-set-object! (group 0) 'elements
                          (arr (thing 0) (thing 1) (thing 2) (thing 3) (thing 4) (thing 5)))";

/// The Y offsets of things 2 and 4 and their priorities.
fn placed(vm: &mut Vm) -> String {
    caught(
        vm,
        "(map (lambda (g) (cons (ly:grob-relative-coordinate g (group 0) 1)
                                (ly:grob-property g 'outside-staff-priority)))
              (list (thing 2) (thing 4)))",
    )
}

#[test]
fn the_skylines_place_the_outside_staff_grobs_beyond_the_staff() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(&mut vm, SPACED);
    let got = sky_points(
        &mut vm,
        "(ly:axis-group-interface::calc-skylines (group 0))",
    );
    // Thing 2 up 2: 1.5 above the staff's 1 at 1 to 2, by its rider's
    // -0.5 and its padding; thing 4 down 1.46 below the staff's -1, by
    // the default padding 0.46. Thing 5 is left out; all shifted by the
    // group's -0.5. Down, thing 2's bottom where thing 0 ends.
    assert_eq!(placed(&mut vm), "((2.0 . #f) (-1.46 . #f))");
    assert_eq!(
        got,
        "(((-inf.0 . +inf.0) (-0.5 . +inf.0) (-0.5 . -2.46) (0.5 . -2.46) (0.5 . -1.0) (1.5 . -1.0) \
         (1.5 . 2.0) (2.5 . 2.0) (2.5 . +inf.0) (3.5 . +inf.0) (3.5 . -2.0) (4.5 . -2.0) \
         (4.5 . +inf.0) (+inf.0 . +inf.0)) \
         ((-inf.0 . -inf.0) (-0.5 . -inf.0) (-0.5 . 1.0) (0.5 . 1.0) (0.5 . 3.0) (2.5 . 3.0) \
         (2.5 . -inf.0) (3.5 . -inf.0) (3.5 . 2.0) (4.5 . 2.0) (4.5 . -inf.0) (+inf.0 . -inf.0)))"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_skylines_take_the_vertical_skyline_elements_and_warn() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(&mut vm, SPACED);
    // Only thing 0 and thing 2, whose Y parent, group 1, has a priority:
    // the warning, and thing 2 is group 1's rider, out of the staff's
    // skylines. An invalid directive is warned of where a priority's
    // grobs are placed; none here.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'vertical-skyline-elements (arr (thing 0) (thing 2)))
         (ly:grob-set-parent! (group 1) 1 (group 0))
         (ly:grob-set-parent! (thing 2) 1 (group 1))
         (put! (group 1) 'outside-staff-priority 5)
         (put! (group 0) 'outside-staff-placement-directive 'bogus)",
    );
    let got = sky_points(
        &mut vm,
        "(ly:axis-group-interface::calc-skylines (group 0))",
    );
    assert_eq!(placed(&mut vm), "((0.0 . #f) (0.0 . 100))");
    assert_eq!(got, box_points((-0.5, 1.5), (-1.0, 1.0)));
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Cannot set outside-staff-priority for element and elements' Y parent.\")"
    );
    // With an outside-staff grob: the directive's warning, and polite.
    // A grob of no direction is warned of and goes up.
    eval(
        &mut vm,
        "(ly:grob-set-parent! (thing 2) 1 (group 0))
         (put! (thing 2) 'outside-staff-priority 100)
         (put! (thing 2) 'direction 0)",
    );
    sky_points(
        &mut vm,
        "(ly:axis-group-interface::calc-skylines (group 0))",
    );
    assert_eq!(placed(&mut vm), "((1.5 . #f) (0.0 . 100))");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"\\\"bogus\\\" is not a valid outside-staff-placement-directive\" \
         \"an outside-staff object should have a direction, defaulting to up\")"
    );
    // An element outside the group on Y: the programming error, the group
    // its own common refpoint still.
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 0) 'vertical-skyline-elements (arr (thing 0) (group 2)))",
    );
    sky_points(
        &mut vm,
        "(ly:axis-group-interface::calc-skylines (group 0))",
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Some of my vertical-skyline-elements are outside my VerticalAxisGroup.\")"
    );
}

/// Group 0 on column 0 with the staff (0, 10) by (-1, 1), thing 0, and
/// three grobs of priority 100 up, of height 1: A, thing 1, (0, 2) at 0;
/// B, thing 2, (0, 2) at 1; C, thing 3, (0, 1.5) at 2.5; their Y offsets
/// once placed by `directive`.
fn placed_by(directive: &str) -> Option<String> {
    let mut vm = grobs_vm()?;
    eval(
        &mut vm,
        &format!(
            "(ly:grob-set-parent! (group 0) 0 (column 0))
             (put! (group 0) 'outside-staff-placement-directive '{directive})
             (for-each (lambda (g xo xe ye)
                         (ly:grob-set-parent! g 0 (column 0))
                         (ly:grob-set-parent! g 1 (group 0))
                         (put! g 'X-offset xo)
                         (put! g 'Y-offset 0)
                         (put! g 'X-extent xe)
                         (put! g 'outside-staff-padding 0)
                         (put! g 'direction 1)
                         (put! g 'vertical-skylines (%box-skylines xe ye 0)))
                       (list (thing 0) (thing 1) (thing 2) (thing 3))
                       '(0 0 1 2.5) '((0 . 10) (0 . 2) (0 . 2) (0 . 1.5))
                       '((-1 . 1) (0 . 1) (0 . 1) (0 . 1)))
             (for-each (lambda (g) (put! g 'outside-staff-priority 100))
                       (list (thing 1) (thing 2) (thing 3)))
             (ly:grob-set-object! (group 0) 'elements
                                  (arr (thing 0) (thing 3) (thing 2) (thing 1)))
             (ly:axis-group-interface::calc-skylines (group 0))"
        ),
    );
    Some(caught(
        &mut vm,
        "(map (lambda (g) (ly:grob-relative-coordinate g (group 0) 1))
              (list (thing 1) (thing 2) (thing 3)))",
    ))
}

#[test]
fn the_directive_orders_the_grobs_of_one_priority() {
    // Sorted by their left ends: A, B, C. Polite, a grob that meets the
    // last placed one waits for the next pass; greedy, none waits.
    for (directive, got) in [
        ("left-to-right-polite", "(1.0 2.0 1.0)"),
        ("right-to-left-polite", "(1.0 2.0 1.0)"),
        ("left-to-right-greedy", "(1.0 2.0 3.0)"),
        ("right-to-left-greedy", "(3.0 2.0 1.0)"),
    ] {
        let Some(placed) = placed_by(directive) else {
            return;
        };
        assert_eq!(placed, got, "{directive}");
    }
}

#[test]
fn the_move_is_the_nearest_allowed_shift_that_way() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(define (b x y) (%box-skylines x y 0))
         (define (move v others dir . hpads)
           (let ((hpad (if (pair? hpads) (car hpads) 0.0)))
             (%outside-staff-move v 0.0 hpad others
                                  (map (lambda (o) 0.0) others)
                                  (map (lambda (o) hpad) others) dir)))",
    );
    // Against (0, 2) by (-1, 1) and, apart, (3, 4) by (2, 3): up 1, down
    // -1 from the first; the second does not meet it.
    let one = "(b '(0 . 1) '(0 . 0))";
    let others = "(list (b '(0 . 2) '(-1 . 1)) (b '(3 . 4) '(2 . 3)))";
    assert_eq!(caught(&mut vm, &format!("(move {one} {others} 1)")), "1.0");
    assert_eq!(
        caught(&mut vm, &format!("(move {one} {others} -1)")),
        "-1.0"
    );
    // Over both, the gap between them is too small: up past the second.
    let wide = "(b '(0 . 4) '(0 . 0.5))";
    let others = "(list (b '(0 . 2) '(-1 . 1)) (b '(3 . 4) '(1.3 . 3)))";
    assert_eq!(caught(&mut vm, &format!("(move {wide} {others} 1)")), "3.0");
    // Where 0 is allowed, no move; none against nothing.
    let others = "(list (b '(5 . 6) '(-1 . 1)))";
    assert_eq!(caught(&mut vm, &format!("(move {one} {others} 1)")), "0.0");
    assert_eq!(caught(&mut vm, &format!("(move {one} '() -1)")), "0.0");
    // A negative padding along the horizon, the larger of the grob's and
    // the other's: Skyline::padded's warning for each distance.
    let others = "(list (b '(0 . 2) '(-1 . 1)))";
    assert_eq!(
        caught(&mut vm, &format!("(move {one} {others} 1 -1.0)")),
        "1.0"
    );
    let junk = "\"Cannot have negative horizon padding.  Junking.\"";
    assert_eq!(diagnosed(&mut vm), format!("({junk} {junk})"));
}

#[test]
fn the_staff_staff_spacing_is_the_grouper_s_or_the_group_s() {
    let Some(mut vm) = grobs_vm() else { return };
    // Groups 0 and 1 staves of thing 5's; group 2 none.
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 5) 'elements (arr (group 0) (group 1)))
         (for-each (lambda (g) (ly:grob-set-object! g 'staff-grouper (thing 5)))
                   (list (group 0) (group 1)))
         (put! (thing 5) 'staff-staff-spacing '((basic-distance . 1)))
         (put! (thing 5) 'staffgroup-staff-spacing
               (ly:make-unpure-pure-container (lambda (g) '((basic-distance . 2)))
                                              (lambda (g s e) (list 'pure s e))))
         (put! (group 2) 'default-staff-staff-spacing '((basic-distance . 3)))",
    );
    let unpure = |vm: &mut Vm, g: &str| {
        caught(
            vm,
            &format!("(ly:axis-group-interface::calc-staff-staff-spacing {g})"),
        )
    };
    let pure = |vm: &mut Vm, g: &str| {
        caught(
            vm,
            &format!("(ly:axis-group-interface::calc-pure-staff-staff-spacing {g} 4 9)"),
        )
    };
    // Within the group, the grouper's staff-staff-spacing; the last, its
    // staffgroup-staff-spacing; no grouper, the group's own default. The
    // pure value first: the unpure one is kept once read.
    assert_eq!(pure(&mut vm, "(group 1)"), "(pure 4 9)");
    assert_eq!(pure(&mut vm, "(group 2)"), "((basic-distance . 3))");
    assert_eq!(unpure(&mut vm, "(group 0)"), "((basic-distance . 1))");
    assert_eq!(unpure(&mut vm, "(group 2)"), "((basic-distance . 3))");
    // Pure, group 1 is empty between 4 and 9: group 0 is not within.
    eval(
        &mut vm,
        "(put! (group 1) 'remove-empty #t)
         (put! (group 1) 'important-column-ranks #(3))",
    );
    assert_eq!(pure(&mut vm, "(group 0)"), "(pure 4 9)");
    assert_eq!(unpure(&mut vm, "(group 0)"), "((basic-distance . 1))");
    assert_eq!(unpure(&mut vm, "(group 1)"), "((basic-distance . 2))");
    assert_eq!(pure(&mut vm, "(group 1)"), "((basic-distance . 2))");
}

/// `VerticalAxisGroup`'s hara-kiri callbacks of MS3 step 28, each with the
/// C++ method that names its type error (LilyPond 2.26.0's, session 4's
/// `scratch/s4-errs.ly`) and its arity.
const HARA_KIRI_CALLBACKS: [(&str, &str, usize); 4] = [
    ("ly:hara-kiri-group-spanner::y-extent", "ly:y-extent", 1),
    (
        "ly:hara-kiri-group-spanner::calc-skylines",
        "ly:calc-skylines",
        1,
    ),
    (
        "ly:hara-kiri-group-spanner::pure-height",
        "ly:pure-height",
        3,
    ),
    (
        "ly:hara-kiri-group-spanner::force-hara-kiri-callback",
        "ly:force-hara-kiri-callback",
        1,
    ),
];

#[test]
fn the_hara_kiri_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, method, arity) in HARA_KIRI_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let rest = " 0".repeat(arity - 1);
        let got = caught(&mut vm, &format!("({name} 'x{rest})"));
        assert!(
            got.contains(method)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", &" 'x".repeat(arity + 1)] {
            let got = caught(&mut vm, &format!("({name}{args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn the_hara_kiri_callbacks_remove_an_empty_group_first() {
    let Some(mut vm) = grobs_vm() else { return };
    // The staves of fixtures/y1-unreached.ly's suicide, each group removed
    // when empty (remove-empty and remove-first), its thing an element on
    // Y, all on the column of rank 1: group 0 of rests (thing 3, not
    // worthy) kept alive by group 1 of notes (worthy thing 0), as each
    // other's keep-alive-with; group 3 of notes (thing 2) made dead by
    // group 2 of notes (thing 1) of a lower layer; group 4 of skips
    // (thing 4) alone.
    put_on(&mut vm, 0, 1, &[0, 1, 2, 3, 4]);
    eval(
        &mut vm,
        "(for-each (lambda (n t)
                     (put! (group n) 'remove-empty #t)
                     (put! (group n) 'remove-first #t)
                     (ly:grob-set-parent! (thing t) 1 (group n))
                     (put! (thing t) 'Y-offset 0.5)
                     (put! (thing t) 'Y-extent (cons (- n) 1))
                     (ly:grob-set-object! (group n) 'elements (arr (thing t)))
                     (ly:grob-set-object! (group n) 'pure-Y-common (group n))
                     (ly:grob-set-object! (group n) 'pure-relevant-grobs (arr (thing t))))
                   '(0 1 2 3 4) '(3 0 1 2 4))
         (for-each (lambda (n t)
                     (ly:grob-set-object! (group n) 'items-worth-living (arr (thing t))))
                   '(1 2 3) '(0 1 2))
         (ly:grob-set-object! (group 0) 'keep-alive-with (arr (group 1)))
         (ly:grob-set-object! (group 1) 'keep-alive-with (arr (group 0)))
         (ly:grob-set-object! (group 3) 'make-dead-when (arr (group 2)))",
    );
    let hk = |vm: &mut Vm, cb: &str, g: usize, args: &str| {
        caught(
            vm,
            &format!("(ly:hara-kiri-group-spanner::{cb} (group {g}){args})"),
        )
    };
    // Pure: empty between the columns for groups 3 and 4, the others'
    // pure heights; what is no size is 0 or INT_MAX. Nothing is killed.
    let heights: Vec<String> = (0..5)
        .map(|g| hk(&mut vm, "pure-height", g, " 0 1000"))
        .collect();
    assert_eq!(
        heights,
        [
            "(0.5 . 1.5)",
            "(-0.5 . 1.5)",
            "(-1.5 . 1.5)",
            "(+inf.0 . -inf.0)",
            "(+inf.0 . -inf.0)"
        ]
    );
    assert_eq!(hk(&mut vm, "pure-height", 0, " 'a -1"), "(0.5 . 1.5)");
    // No column strictly between 1 and 2: all empty.
    assert_eq!(hk(&mut vm, "pure-height", 1, " 1 2"), "(+inf.0 . -inf.0)");
    let lives = |vm: &mut Vm| {
        [
            "(group 0)",
            "(group 1)",
            "(group 2)",
            "(group 3)",
            "(group 4)",
        ]
        .map(|g| live(vm, g))
    };
    assert_eq!(lives(&mut vm), [true; 5]);
    // Each callback removes an empty group with its elements first:
    // group 3's extent is empty, group 4's skylines are, and the offset
    // is 0.0 whether the group dies or not.
    assert_eq!(hk(&mut vm, "y-extent", 3, ""), "(+inf.0 . -inf.0)");
    assert!(!live(&mut vm, "(thing 2)"));
    assert_eq!(
        caught(
            &mut vm,
            "(%skyline-pair-empty? (ly:hara-kiri-group-spanner::calc-skylines (group 4)))"
        ),
        "#t"
    );
    assert!(!live(&mut vm, "(thing 4)"));
    for g in 0..5 {
        assert_eq!(hk(&mut vm, "force-hara-kiri-callback", g, ""), "0.0");
    }
    assert_eq!(lives(&mut vm), [true, true, true, false, false]);
    assert!(live(&mut vm, "(thing 3)"));
    // The live groups' extents; a dead one's pure height stays empty.
    assert_eq!(hk(&mut vm, "y-extent", 0, ""), "(0.5 . 1.5)");
    assert_eq!(hk(&mut vm, "y-extent", 2, ""), "(-1.5 . 1.5)");
    assert_eq!(
        hk(&mut vm, "pure-height", 4, " 0 1000"),
        "(+inf.0 . -inf.0)"
    );
    // A live group's skylines are the axis group's: its thing's at its
    // offset.
    eval(
        &mut vm,
        "(put! (thing 1) 'vertical-skylines (%box-skylines '(0 . 2) '(-2 . 1) 0))",
    );
    let own = sky_points(
        &mut vm,
        "(ly:hara-kiri-group-spanner::calc-skylines (group 2))",
    );
    assert_eq!(own, box_points((0.0, 2.0), (-1.5, 1.5)));
    assert_eq!(
        own,
        sky_points(
            &mut vm,
            "(ly:axis-group-interface::calc-skylines (group 2))"
        )
    );
    // Group 0 dies with its friend's worthy thing.
    eval(&mut vm, "(ly:grob-suicide! (thing 0))");
    assert_eq!(hk(&mut vm, "force-hara-kiri-callback", 0, ""), "0.0");
    assert!(!live(&mut vm, "(group 0)") && !live(&mut vm, "(thing 3)"));
}

/// The alignment's callbacks of MS3 step 28, with the C++ method that
/// names their type errors (LilyPond 2.26.0's, session 5's
/// `scratch/s5-errs.ly`).
const ALIGN_CALLBACKS: [(&str, &str); 2] = [
    (
        "ly:align-interface::align-to-minimum-distances",
        "ly:align-to-minimum-distances",
    ),
    (
        "ly:align-interface::align-to-ideal-distances",
        "ly:align-to-ideal-distances",
    ),
];

#[test]
fn the_alignment_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, method) in ALIGN_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let got = caught(&mut vm, &format!("({name} 5)"));
        assert!(
            got.contains(method)
                && got.contains("Wrong type argument in position 1 (expecting Grob): 5"),
            "{name}: {got}"
        );
        for args in ["", " 1 2"] {
            let got = caught(&mut vm, &format!("({name}{args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn the_primitive_solver_takes_no_springs_and_refuses_insane_ones() {
    let Some(mut vm) = grobs_vm() else { return };
    // `%spacer-solve`'s forces and positions are checked against
    // LilyPond's in tests/spacer.rs. No springs: nothing to solve. Insane
    // values are refused with LilyPond's programming errors.
    assert_eq!(
        caught(&mut vm, "(%spacer-solve '() 3 #f)"),
        "(3000000.0 #t (0.0) 0.0)"
    );
    eval(
        &mut vm,
        "(define sp (%make-spring -1 +inf.0))
         (%spring-set! sp 'inverse-stretch-strength -2)
         (%spring-set! sp 'ensure-min-distance 0.5)",
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"insane spring distance requested, ignoring it\" \
         \"insane spring min_distance requested, ignoring it\" \"insane spring constant\")"
    );
    assert_eq!(
        caught(&mut vm, "(caddr (%spacer-solve (list sp) 0 #f))"),
        "(0.0 1.0)"
    );
    assert!(caught(&mut vm, "(%spring-set! sp 'bogus 1)").contains("spring setter"));
}

/// Groups 0 to 2 the elements of align 0, the alignment of the system
/// group 5 on columns 0 and 1: two staves, the first's skylines a box
/// from -2 to `top`, and between them a loose line (`staff-affinity`
/// UP) from -1 to 1, the second staff from -3 to 1, all from 0 to 20 on
/// X; their spacing specs LilyPond's defaults for a Staff's and a
/// Lyrics' VerticalAxisGroup, the system's left column's
/// `line-break-system-details` `details`. As session 5's
/// `scratch/s5-align.ily` gives them to LilyPond 2.26.0.
fn aligned_system(vm: &mut Vm, top: f64, details: &str) {
    let sys = grob_of(vm, "(group 5)");
    lily_engine_layout::item::set_class(
        vm.heap_mut(),
        sys,
        lily_engine_layout::item::Class::System,
    );
    let cols = ["(column 0)", "(column 1)"].map(|t| grob_of(vm, t));
    for c in cols {
        grob::with_grob_mut(vm.heap(), c, |g| g.column.system = Some(sys));
    }
    for t in [
        "(group 5)",
        "(align 0)",
        "(group 0)",
        "(group 1)",
        "(group 2)",
    ] {
        let g = grob_of(vm, t);
        spanner::set_bound(vm, g, -1, cols[0]).unwrap();
        spanner::set_bound(vm, g, 1, cols[1]).unwrap();
    }
    eval(
        vm,
        &format!(
            "(let ((layout (ly:grob-layout (group 5))))
               (ly:output-def-set-variable! layout 'indent 8.535826771653543)
               (ly:output-def-set-variable! layout 'line-width 102.4299212598425))
             (put! (column 0) 'line-break-system-details '{details})
             (put! (group 5) 'skyline-horizontal-padding 1.0)
             (ly:grob-set-object! (group 5) 'vertical-alignment (align 0))
             (ly:grob-set-parent! (align 0) 1 (group 5))
             (put! (align 0) 'axes '(1))
             (put! (align 0) 'stacking-dir -1)
             (ly:grob-set-object! (align 0) 'elements (arr (group 0) (group 1) (group 2)))
             (for-each (lambda (g y)
                         (ly:grob-set-parent! g 1 (align 0))
                         (put! g 'vertical-skylines (%box-skylines '(0 . 20) y 0))
                         (put! g 'Y-extent y)
                         (put! g 'staff-staff-spacing '((basic-distance . 9) (minimum-distance . 8) (padding . 1)))
                         (put! g 'default-staff-staff-spacing '((basic-distance . 9) (minimum-distance . 8) (padding . 1)))
                         (put! g 'nonstaff-unrelatedstaff-spacing '((padding . 0.5))))
                       (list (group 0) (group 1) (group 2))
                       (list (cons -2 {top}) '(-1 . 1) '(-3 . 1)))
             (put! (group 1) 'staff-affinity 1)
             (put! (group 1) 'nonstaff-relatedstaff-spacing '((basic-distance . 5.5) (padding . 0.5) (stretchability . 1)))
             (put! (group 1) 'nonstaff-nonstaff-spacing
                   '((basic-distance . 0) (minimum-distance . 2.8) (padding . 0.2) (stretchability . 0)))
             (put! (group 1) 'nonstaff-unrelatedstaff-spacing '((padding . 1.5) (padding . 0.5)))"
        ),
    );
}

/// Each group's Y coordinate in the alignment and its `system-Y-offset`.
fn aligned(vm: &mut Vm) -> String {
    caught(
        vm,
        "(map (lambda (g) (cons (ly:grob-relative-coordinate g (align 0) 1)
                                (ly:grob-property g 'system-Y-offset)))
              (list (group 0) (group 1) (group 2)))",
    )
}

#[test]
fn the_minimum_distances_stack_the_lines_by_skylines_and_specs() {
    for (details, got) in [
        ("()", "((-2.5) (-6.0) (-10.5))"),
        // The staff placed at least its fixed spacing past the loose
        // line, as LilyPond places it.
        ("((alignment-distances 12))", "((-2.5) (-6.0) (-18.0))"),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, details);
        assert_eq!(
            caught(
                &mut vm,
                "(ly:align-interface::align-to-minimum-distances (align 0))"
            ),
            "#t"
        );
        // LilyPond 2.26.0's (`scratch/s5-align-min.ly`,
        // `s5-align-details-min.ly`).
        assert_eq!(aligned(&mut vm), got, "{details}");
        assert_eq!(
            caught(&mut vm, "(ly:grob-property (align 0) 'positioning-done)"),
            "#t"
        );
        assert_eq!(diagnosed(&mut vm), "()");
    }
    let Some(mut vm) = grobs_vm() else { return };
    aligned_system(&mut vm, 2.5, "()");
    let translations = |vm: &mut Vm, elements: &str| {
        caught(
            vm,
            &format!("(%align-minimum-translations (align 0) (list {elements}) 1 #t #f 0 0)"),
        )
    };
    // Lines without skylines before the first take its translation, and
    // one after a line the line's; stacked up, the other way.
    eval(&mut vm, "(put! (group 3) 'vertical-skylines '())");
    assert_eq!(
        translations(&mut vm, "(group 3) (group 0) (group 3) (group 2)"),
        "(-2.5 -2.5 -2.5 -10.5)"
    );
    // Stacked up, the first line's skyline below is 2 under its
    // reference point, which keeps it at 0.
    eval(&mut vm, "(put! (align 0) 'stacking-dir 1)");
    assert_eq!(translations(&mut vm, "(group 0) (group 2)"), "(0.0 8.0)");
    // The alignment's padding without a spec's.
    eval(
        &mut vm,
        "(put! (align 0) 'stacking-dir -1) (put! (align 0) 'padding 3)
         (put! (group 0) 'staff-staff-spacing '())",
    );
    assert_eq!(translations(&mut vm, "(group 0) (group 2)"), "(-5.5 -11.5)");
    // An alignment of no system, unpure on Y.
    let col = grob_of(&mut vm, "(column 1)");
    grob::with_grob_mut(vm.heap(), col, |g| g.column.system = None);
    translations(&mut vm, "(group 0)");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"vertical alignment called before line-breaking\")"
    );
}

#[test]
fn the_ideal_distances_are_the_page_layout_problem_s() {
    // LilyPond 2.26.0's (`scratch/s5-align-ideal.ly`,
    // `s5-align-details.ly`, `s5-align-tall.ly`): the loose line between
    // the staves by its springs, the staves fixed at their alignment
    // distance, and an over-full page compressed.
    for (top, details, got, warned) in [
        (
            2.5,
            "()",
            "((-2.5 . -3.552713678800501e-15) (-7.999999999999996 . -3.552713678800501e-15) \
             (-11.5 . -3.552713678800501e-15))",
            "()",
        ),
        (
            2.5,
            "((alignment-distances 12))",
            "((-2.5 . -3.552713678800501e-15) (-8.0000054999945 . -3.552713678800501e-15) \
             (-14.5 . -3.552713678800501e-15))",
            "()",
        ),
        (
            100.0,
            "()",
            "((-100.0 . -0.0) (-103.5 . -0.0) (-102.5 . -0.0))",
            "(\"compressing over-full page by 11.0 staff-spaces\")",
        ),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, top, details);
        assert_eq!(
            caught(
                &mut vm,
                "(ly:align-interface::align-to-ideal-distances (align 0))"
            ),
            "#t"
        );
        assert_eq!(aligned(&mut vm), got, "{top} {details}");
        assert_eq!(diagnosed(&mut vm), warned, "{top} {details}");
        assert_eq!(
            caught(&mut vm, "(ly:grob-property (align 0) 'positioning-done)"),
            "#t"
        );
        assert_eq!(
            caught(&mut vm, "(ly:grob-property (group 1) 'staff-affinity)"),
            "1"
        );
    }
    // A system of one loose line: it is spaced as a staff.
    let Some(mut vm) = grobs_vm() else { return };
    aligned_system(&mut vm, 2.5, "()");
    eval(
        &mut vm,
        "(ly:grob-set-object! (align 0) 'elements (arr (group 1)))",
    );
    caught(
        &mut vm,
        "(ly:align-interface::align-to-ideal-distances (align 0))",
    );
    assert_eq!(
        caught(&mut vm, "(ly:grob-property (group 1) 'staff-affinity)"),
        "#f"
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // No system: LilyPond's programming error, nothing placed.
    let Some(mut vm) = grobs_vm() else { return };
    caught(
        &mut vm,
        "(ly:align-interface::align-to-ideal-distances (align 0))",
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"vertical alignment called before line breaking\")"
    );
}

/// The page-layout problem of `systems` on the paper `paper` and the
/// page `page`, Scheme (`#f` for none; the page an alist, made a `Prob`),
/// solved; its offsets, then its diagnostics.
fn page_solution(vm: &mut Vm, paper: &str, page: &str, systems: &str, ragged: bool) -> String {
    page_layout(vm, "%page-layout-solution", paper, page, systems, ragged)
}

/// `page_solution`'s problem solved by `solve`. Each of `systems` without
/// a `footnote-stencil` is first given the empty one, as
/// `add_footnotes_to_lines` leaves a line without footnotes before
/// `Page_breaking::draw_page` makes the problem.
fn page_layout(
    vm: &mut Vm,
    solve: &str,
    paper: &str,
    page: &str,
    systems: &str,
    ragged: bool,
) -> String {
    let page = match page {
        "#f" => Value::FALSE,
        alist => {
            let (type_, init) = (eval(vm, "'page"), eval(vm, alist));
            lily_engine_music::make_prob(vm.heap_mut(), type_, init)
        }
    };
    vm.heap_mut().root(page);
    let ragged = if ragged { "#t" } else { "#f" };
    let solve = eval(
        vm,
        &format!(
            "(lambda (page)
               (let ((systems {systems}))
                 (for-each (lambda (l)
                             (cond ((and (ly:grob? l) (null? (ly:grob-property l 'footnote-stencil)))
                                    (%grob-set-property-data! l 'footnote-stencil (ly:make-stencil '())))
                                   ((and (ly:prob? l) (null? (ly:prob-property l 'footnote-stencil)))
                                    (ly:prob-set-property! l 'footnote-stencil (ly:make-stencil '())))))
                           systems)
                 ({solve} {paper} page systems {ragged})))"
        ),
    );
    let got = match Host::call(vm, solve, &[page]) {
        Ok(v) => shown(vm, v),
        Err(e) => e.text(vm.heap()),
    };
    format!("{got} {}", diagnosed(vm))
}

/// A paper with the variables `vars`, a list of `(name . value)`.
fn paper_with(vars: &str) -> String {
    format!(
        "(let ((paper (ly:make-output-def)))
           (for-each (lambda (v) (ly:output-def-set-variable! paper (car v) (cdr v))) {vars})
           paper)"
    )
}

#[test]
fn the_page_s_head_foot_and_height_and_the_paper_s_specs_space_its_systems() {
    // From page-layout-problem.cc:406–564 on `aligned_system`'s system:
    // the springs' minimum distances, the first staff's top 2.5 under the
    // head, the staves 8 apart and the last's bottom 3 above the foot,
    // add to 13.5, and a page not as high is compressed by the rest.
    const FOOTNOTES: &str =
        "\"A page layout problem has been initiated that cannot accommodate footnotes.\"";
    let over = |by: &str| format!("\"compressing over-full page by {by} staff-spaces\"");
    let tall = "(list '(paper-height . 10))";
    let stencils = "(list '(paper-height . 10)
                          (cons 'head-stencil (ly:make-stencil '() '(0 . 1) '(-1 . 3)))
                          (cons 'foot-stencil (ly:make-stencil '() '(0 . 1) '(0 . 2))))";
    let empty = paper_with("'()");
    for (paper, page, warned) in [
        // No paper: footnotes cannot be placed; the page's height read.
        ("#f", tall, format!("({FOOTNOTES} {})", over("3.5"))),
        ("#f", "#f", "()".into()),
        (&empty, tall, format!("({})", over("3.5"))),
        // A height that is no number is 100.
        (&empty, "(list '(paper-height . high))", "()".into()),
        // The head's height (4) keeps the first staff under it, the foot's
        // (2) the last above it; what is no stencil has none.
        (&empty, stencils, format!("({})", over("9.5"))),
        (
            &empty,
            "(list '(paper-height . 10) '(head-stencil . 1) '(foot-stencil . #f))",
            format!("({})", over("3.5")),
        ),
        // The margins come off the height.
        (
            &paper_with("'((top-margin . 1) (bottom-margin . 0.5))"),
            tall,
            format!("({})", over("5.0")),
        ),
        // The last spring's padding (3 + 1.5), and its minimum distance.
        (
            &paper_with("'((last-bottom-spacing (padding . 1.5)))"),
            tall,
            format!("({})", over("5.0")),
        ),
        (
            &paper_with("'((last-bottom-spacing (minimum-distance . 6) (padding . 1.5)))"),
            tall,
            format!("({})", over("6.5")),
        ),
        // The first system's spec is the top's: 2.5 + 4, or 5.
        (
            &paper_with(
                "'((top-system-spacing (padding . 4)) (system-system-spacing (padding . 9))
                   (score-system-spacing (padding . 9)))",
            ),
            tall,
            format!("({})", over("7.5")),
        ),
        (
            &paper_with("'((top-system-spacing (minimum-distance . 5) (padding . 1)))"),
            tall,
            format!("({})", over("6.0")),
        ),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        let got = page_solution(&mut vm, paper, page, "(list (group 5))", false);
        assert!(
            got.ends_with(&format!(" {warned}")),
            "{paper} {page}: {got}"
        );
    }

    // Ragged on a page high enough, the system where its springs' ideal
    // lengths put it: its first staff 2.5 under the top, 4 more under the
    // head, or 9 under the top with a basic distance of 9 (the last bits
    // the skylines' distance's, shifted by the indent, as in the
    // alignment's test).
    let high = "(list '(paper-height . 100)
                      (cons 'head-stencil (ly:make-stencil '() '(0 . 1) '(-1 . 3))))";
    for (paper, page, got) in [
        ("#f", "#f", "(3.552713678800501e-15) ()"),
        (&empty, high, "(4.0000000000000036) ()"),
        (
            &paper_with("'((top-system-spacing (basic-distance . 9)))"),
            "(list '(paper-height . 100))",
            "(6.5) ()",
        ),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        assert_eq!(
            page_solution(&mut vm, paper, page, "(list (group 5))", true),
            got,
            "{paper} {page}"
        );
    }

    // A second system: the score's spec when its left bound is the column
    // of rank 0, else the system's. Its top 5.5 under the first's bottom
    // (2.5 + 3), here at least 20 or 30: 2.5 + 8 + 20 + 8 + 3, or 30 for
    // 20.
    let specs = paper_with(
        "'((score-system-spacing (minimum-distance . 20))
           (system-system-spacing (minimum-distance . 30))
           (markup-system-spacing (minimum-distance . 40)))",
    );
    for (rank, by) in [(0, "31.5"), (1, "41.5")] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        let col = grob_of(&mut vm, "(column 0)");
        paper_column::set_rank(vm.heap(), col, rank);
        let got = page_solution(&mut vm, &specs, tall, "(list (group 5) (group 5))", false);
        assert!(got.ends_with(&format!(" ({})", over(by))), "{rank}: {got}");
    }

    // The in-note stencil (3 high) raised by the paper's padding (1) and
    // direction (DOWN) under the last staff's bottom: 13.5 + 4; without a
    // paper, 0.5 and UP over the first's top: 13.5 + 3.5.
    for (paper, page, set, by) in [
        (
            paper_with("'((in-note-system-padding . 1) (in-note-direction . -1))"),
            tall,
            "(1.0 -1)",
            "7.5",
        ),
        (
            paper_with("'((in-note-system-padding . none) (in-note-direction . 0.5))"),
            tall,
            "(0.5 1)",
            "7.0",
        ),
        ("#f".into(), "#f", "(0.5 1)", ""),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        eval(
            &mut vm,
            "(put! (group 5) 'in-note-stencil (ly:make-stencil '() '(0 . 1) '(0 . 3)))",
        );
        let got = page_solution(&mut vm, &paper, page, "(list (group 5))", false);
        if !by.is_empty() {
            assert!(got.ends_with(&format!(" ({})", over(by))), "{paper}: {got}");
        }
        assert_eq!(
            caught(
                &mut vm,
                "(list (ly:grob-property (group 5) 'in-note-system-padding)
                       (ly:grob-property (group 5) 'in-note-direction))"
            ),
            set,
            "{paper}"
        );
    }
}

#[test]
fn the_pure_translations_stack_the_pure_heights_and_are_kept() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 0 and 1, items from -1 to 1, in align 0 (no system's).
    eval(
        &mut vm,
        "(put! (align 0) 'axes '(1))
         (put! (align 0) 'stacking-dir -1)
         (ly:grob-set-object! (align 0) 'elements (arr (thing 0) (thing 1)))
         (for-each (lambda (g) (ly:grob-set-parent! g 1 (align 0)) (put! g 'Y-extent '(-1 . 1)))
                   (list (thing 0) (thing 1)))
         (put! (thing 0) 'staff-staff-spacing
               '((basic-distance . 6) (minimum-distance . 3) (padding . 0.5)))",
    );
    let pure = |vm: &mut Vm, start: &str, end: &str| {
        caught(
            vm,
            &format!(
                "(%align-pure-minimum-translations (align 0) (list (thing 0) (thing 1)) 1 {start} {end})"
            ),
        )
    };
    // At least the minimum distance; the basic one between 0 and INT_MAX.
    assert_eq!(pure(&mut vm, "0", "1"), "(-1.0 -4.0)");
    assert_eq!(pure(&mut vm, "0", "2147483647"), "(-1.0 -7.0)");
    // Kept by the columns.
    eval(
        &mut vm,
        "(put! (thing 0) 'staff-staff-spacing '((minimum-distance . 5)))",
    );
    assert_eq!(pure(&mut vm, "0", "1"), "(-1.0 -4.0)");
    assert_eq!(pure(&mut vm, "1", "2"), "(-1.0 -6.0)");
    assert_eq!(
        caught(
            &mut vm,
            "(ly:grob-property (align 0) 'minimum-translations-alist)"
        ),
        "(((1 . 2) -1.0 -6.0) ((0 . 2147483647) -1.0 -7.0) ((0 . 1) -1.0 -4.0))"
    );
    // The pure coordinate of a child is its translation.
    let (align, thing) = (grob_of(&mut vm, "(align 0)"), grob_of(&mut vm, "(thing 1)"));
    assert_eq!(
        lily_engine_layout::align::get_pure_child_y_translation(&mut vm, align, thing, 0, 1)
            .unwrap(),
        -4.0
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // A kept one read with no VM call (MS4 step 21), as the Scheme reads
    // it: the entry its key finds, not computed again.
    eval(
        &mut vm,
        "(put! (align 0) 'minimum-translations-alist '(((5 . 6) 1.5 -2) ((0 . 1) 3/2 x)))",
    );
    let pure_child = |vm: &mut Vm, ch: Value, start: usize, end: usize| {
        lily_engine_layout::align::get_pure_child_y_translation(vm, align, ch, start, end)
            .map_err(|e| e.text(vm.heap()))
    };
    assert_eq!(pure_child(&mut vm, thing, 5, 6), Ok(-2.0));
    let first = grob_of(&mut vm, "(thing 0)");
    assert_eq!(pure_child(&mut vm, first, 5, 6), Ok(1.5));
    // The Scheme's where the list holds what is no fixnum or flonum: a
    // symbol its error.
    let wrong = pure_child(&mut vm, first, 0, 1);
    assert!(
        wrong.as_ref().is_err_and(|e| e.contains("real number")),
        "{wrong:?}"
    );
    // And for no element of the alignment, its programming error.
    let group = grob_of(&mut vm, "(group 0)");
    assert_eq!(pure_child(&mut vm, group, 5, 6), Ok(0.0));
    assert_eq!(
        diagnosed(&mut vm),
        "(\"tried to get a translation for something that is no child of mine\")"
    );
    // A hara-kiri group that would die has no skylines; an axis group's
    // are its pure heights at the beginning of the line and in the rest.
    eval(
        &mut vm,
        "(ly:grob-set-object! (align 0) 'elements (arr (thing 0) (group 0)))
         (put! (group 0) 'remove-empty #t)
         (put! (group 0) 'Y-extent '(-2 . 2))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(%align-pure-minimum-translations (align 0) (list (thing 0) (group 0)) 1 3 4)"
        ),
        "(-1.0 -1.0)"
    );
}

/// `layout.tsv`'s Y extent of `(thing n)`.
fn y_extent(vm: &mut Vm, n: usize) -> String {
    let g = grob_of(vm, &format!("(thing {n})"));
    y_extent_field(vm, g).unwrap()
}

#[test]
fn the_y_extent_is_written_as_layout_ly_writes_it() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(put! (thing 0) 'Y-extent '(-1.25 . 2.5))
         (put! (thing 1) 'Y-extent '(+inf.0 . -inf.0))
         (put! (thing 2) 'Y-extent (lambda (g) (cons -inf.0 (ly:grob-property g 'top))))
         (put! (thing 2) 'top (lambda (g) +inf.0))
         (put! (thing 3) 'Y-extent '(-0.0 . 0.00004))
         (put! (thing 4) 'Y-extent '(1.0 . 3.0))
         (put! (thing 4) 'minimum-Y-extent '(0.0 . 5.0))
         (put! (thing 5) 'Y-extent (lambda (g) (ly:grob-property g 'none)))",
    );
    let fields: Vec<String> = (0..6).map(|n| y_extent(&mut vm, n)).collect();
    assert_eq!(
        fields,
        [
            "-1.2500 2.5000",
            "-",
            "-inf +inf",
            "0.0000 0.0000",
            "0.0000 5.0000",
            "-"
        ]
    );
    // A callback's value is kept, as a read keeps it.
    let top = eval(&mut vm, "(ly:grob-property-data (thing 2) 'top)");
    assert_eq!(shown(&vm, top), "+inf.0");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_offsets_are_written_relative_to_the_system_as_layout_ly_writes_them() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(&mut vm, THROWS);
    // (thing 1) under (thing 0) on both axes; (thing 2)'s Y offset a
    // placeholder's.
    eval(
        &mut vm,
        "(for-each (lambda (a) (ly:grob-set-parent! (thing 1) a (thing 0))) '(0 1))
         (put! (thing 0) 'X-offset 1.25)
         (put! (thing 0) 'Y-offset -0.0)
         (put! (thing 1) 'X-offset 0.00004)
         (put! (thing 1) 'Y-offset (lambda (g) -2.05))
         (ly:grob-set-parent! (thing 2) 1 (thing 0))
         (put! (thing 2) 'stencil %throws)
         (put! (thing 2) 'Y-offset ly:grob::stencil-height)",
    );
    let [t0, t1, t2] = ["(thing 0)", "(thing 1)", "(thing 2)"].map(|t| grob_of(&mut vm, t));
    let fields: Vec<String> = [(t1, t0, 0), (t1, t0, 1), (t0, t0, 1), (t2, t0, 1)]
        .into_iter()
        .map(|(g, sys, a)| offset_field(&mut vm, g, sys, a).unwrap())
        .collect();
    assert_eq!(fields, ["0.0000", "-2.0500", "0.0000", "?"]);
    // The read that threw left neither a cyclic dependency nor the 0
    // `get_offset` caches before it reads the property: read again, and
    // through a grob under it, it is unreached, not 0.
    eval(&mut vm, "(ly:grob-set-parent! (thing 3) 1 (thing 2))");
    let t3 = grob_of(&mut vm, "(thing 3)");
    assert_eq!(offset_field(&mut vm, t2, t0, 1).unwrap(), "?");
    assert_eq!(offset_field(&mut vm, t3, t0, 1).unwrap(), "?");
    assert_eq!(diagnosed(&mut vm), "()");
    // Outside the dump, as LilyPond's: the 0 is kept.
    caught(
        &mut vm,
        "(ly:grob-relative-coordinate (thing 2) (thing 0) 1)",
    );
    assert_eq!(
        evaluated(
            &mut vm,
            "(ly:grob-relative-coordinate (thing 2) (thing 0) 1)"
        ),
        "0.0"
    );
}

#[test]
fn a_read_that_throws_is_unreached_and_leaves_no_cyclic_dependency() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(&mut vm, THROWS);
    // A placeholder under the stencil the extent is read from.
    eval(
        &mut vm,
        "(for-each (lambda (n)
                     (put! (thing n) 'stencil %throws)
                     (put! (thing n) 'Y-extent ly:grob::stencil-height))
                   '(0 1))",
    );
    // Caught outside the dump, as LilyPond leaves it: the next read is a
    // cyclic dependency.
    assert_eq!(
        caught(
            &mut vm,
            "(catch #t (lambda () (ly:grob-extent (thing 1) (thing 1) 1)) (lambda _ 'thrown))"
        ),
        "thrown"
    );
    caught(&mut vm, "(ly:grob-property (thing 1) 'stencil)");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"cyclic dependency: calculation-in-progress encountered for Thing.stencil\")"
    );
    // The dump's read: unreached, each callback put back, twice alike.
    for _ in 0..2 {
        assert_eq!(y_extent(&mut vm, 0), "?");
        let back = eval(
            &mut vm,
            "(list (eq? (ly:grob-property-data (thing 0) 'stencil) %throws)
                   (eq? (ly:grob-property-data (thing 0) 'Y-extent) ly:grob::stencil-height)
                   (eq? %grob-try-callback %dump-grob-try-callback)
                   %dump-pending)",
        );
        assert_eq!(shown(&vm, back), "(#t #t #t ())");
    }
    assert!(
        caught(&mut vm, "(ly:grob-property (thing 0) 'stencil)")
            .contains("ly:music-iterator::constructor")
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // A callback that catches its own callback's error reads the property
    // again: the callback is called again, no cyclic dependency.
    eval(
        &mut vm,
        "(put! (thing 2) 'stencil %throws)
         (put! (thing 2) 'Y-extent
               (lambda (g)
                 (catch #t (lambda () (ly:grob-property g 'stencil)) (lambda _ #f))
                 (catch #t (lambda () (ly:grob-property g 'stencil)) (lambda _ #f))
                 '(0 . 1)))",
    );
    assert_eq!(y_extent(&mut vm, 2), "0.0000 1.0000");
    assert_eq!(diagnosed(&mut vm), "()");
    // An unpure-pure container is put back as it was, not its unpure part.
    eval(
        &mut vm,
        "(put! (thing 3) 'stencil %throws)
         (put! (thing 3) 'Y-extent
               (ly:make-unpure-pure-container ly:grob::stencil-height (lambda (g s e) '(0 . 0))))",
    );
    assert_eq!(y_extent(&mut vm, 3), "?");
    let back = eval(
        &mut vm,
        "(ly:unpure-pure-container? (ly:grob-property-data (thing 3) 'Y-extent))",
    );
    assert_eq!(shown(&vm, back), "#t");
}

#[test]
fn a_read_that_stops_for_the_host_is_unreached_and_leaves_no_cyclic_dependency() {
    let Some(mut vm) = grobs_vm() else { return };
    // A part's file the host must be asked for: the run is cancelled past
    // every catch.
    vm.set_part_source(Some(Box::new(|_| Hosted::Ask)));
    eval(
        &mut vm,
        "(put! (thing 0) 'stencil (lambda (g) (%part-source \"part.scm\")))
         (put! (thing 0) 'Y-extent ly:grob::stencil-height)",
    );
    assert_eq!(y_extent(&mut vm, 0), "?");
    // The wait is the compile's to clear (compile.rs).
    assert!(vm.take_waiting());
    let back = eval(
        &mut vm,
        "(list (procedure? (ly:grob-property-data (thing 0) 'stencil))
               (eq? (ly:grob-property-data (thing 0) 'Y-extent) ly:grob::stencil-height)
               (eq? %grob-try-callback %dump-grob-try-callback)
               %dump-pending)",
    );
    assert_eq!(shown(&vm, back), "(#t #t #t ())");
    // Given the file, the same read completes.
    vm.set_part_source(Some(Box::new(|_| Hosted::Text(String::from("x")))));
    eval(
        &mut vm,
        "(put! (thing 0) 'stencil
               (lambda (g) (%part-source \"part.scm\") (ly:make-stencil '() '(0 . 1) '(-2 . 3))))",
    );
    assert_eq!(y_extent(&mut vm, 0), "-2.0000 3.0000");
    assert!(!vm.waiting());
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_separation_item_s_skylines_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:separation-item::calc-skylines";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(
        music.iter().all(|p| p.name != name),
        "{name} is no placeholder"
    );
    // LilyPond does not check the grob and crashes on what is no item.
    for arg in ["'x", "(group 0)"] {
        let got = caught(&mut vm, &format!("({name} {arg})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Item)"),
            "{arg}: {got}"
        );
    }
    for args in ["", "'x 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
}

/// The items of a separation item's case: `(x-extent y-extent x-offset
/// y-offset extra-spacing-width extra-spacing-height)` each, the extra
/// spacing `#f` where it is not set.
struct SeparationCase {
    padding: &'static str,
    items: &'static [&'static str],
    /// The bundle, an axis group, on the note's column among the elements.
    with_group: bool,
    /// A thing on another column among the elements.
    with_other: bool,
    /// LilyPond 2.26.0's points of the skylines left and right, along Y.
    points: &'static str,
}

/// `scratch/s1-separation.ly`'s cases (MS3 step 30, session 1), where a
/// NoteColumn's elements are made items with these extents, offsets and
/// extra spacing, its padding this. LilyPond's `ly:grob-set-property!`
/// rejected case 2's non-pairs and case 4's non-real padding there, so
/// the column read its defaults: here the non-pairs are set and read as
/// none, and case 4's padding is NoteColumn's 0.15.
const SEPARATION_CASES: &[SeparationCase] = &[
    SeparationCase {
        padding: "0.0",
        items: &["'(0 . 1) '(0 . 2) 0 0 #f #f"],
        with_group: false,
        with_other: false,
        points: "(((+inf.0 -inf.0) (+inf.0 0.0) (-0.1 0.0) (-0.1 2.0) (+inf.0 2.0) (+inf.0 +inf.0)) \
                  ((-inf.0 -inf.0) (-inf.0 0.0) (1.1 0.0) (1.1 2.0) (-inf.0 2.0) (-inf.0 +inf.0)))",
    },
    SeparationCase {
        padding: "0.25",
        items: &[
            "'(-1 . 1) '(-2 . 2) 0.5 1 #f #f",
            "'(0 . 3) '(1 . 4) 1 -1 '(0 . 0) '(-0.5 . 0.5)",
        ],
        with_group: false,
        with_other: false,
        points: "(((+inf.0 -inf.0) (+inf.0 -1.5) (-0.3500000000000001 -1.5) \
                   (-0.6000000000000001 -1.25) (-0.6 -1.25) (-0.6 -1.0) (-0.6 -1.0) (-0.6 3.0) \
                   (-0.6 3.0) (-0.6 3.25) (-0.6000000000000001 3.25) (-0.3500000000000001 3.5) \
                   (1.0 3.5) (1.0 3.75) (1.0 3.75) (1.25 4.0) (+inf.0 4.0) (+inf.0 +inf.0)) \
                  ((-inf.0 -inf.0) (-inf.0 -1.5) (1.35 -1.5) (1.6 -1.25) (1.6 -1.25) (1.6 -1.0) \
                   (3.75 -1.0) (4.0 -0.75) (4.0 -0.75) (4.0 -0.5) (4.0 -0.5) (4.0 3.5) (4.0 3.5) \
                   (4.0 3.75) (4.0 3.75) (3.75 4.0) (-inf.0 4.0) (-inf.0 +inf.0)))",
    },
    SeparationCase {
        padding: "0.1",
        items: &[
            "'(0 . 1) '(0 . 1) 0 0 '(-inf.0 . +inf.0) '(0 . 0)",
            "'(2 . 3) '(-1 . 0) 0 0 '(0.2 . 0.3) '(-inf.0 . +inf.0)",
            "'(1 . 2) '(3 . 5) 0 2 'x '(1 . x)",
        ],
        with_group: true,
        with_other: true,
        points: "(((2.2 -inf.0) (2.2 -0.2) (-inf.0 -0.2) (-inf.0 -0.1) (-inf.0 -0.1) (-inf.0 0.0) \
                   (-inf.0 0.0) (-inf.0 1.0) (-inf.0 1.0) (-inf.0 1.1) (-inf.0 1.1) \
                   (-inf.0 1.2000000000000002) (2.2 1.2000000000000002) (2.2 4.8) (1.0 4.8) \
                   (0.9 4.9) (0.9 4.9) (0.9 5.0) (0.9 5.0) (0.9 7.0) (0.9 7.0) (0.9 7.1) (0.9 7.1) \
                   (1.0 7.199999999999999) (2.2 7.199999999999999) (2.2 +inf.0)) \
                  ((3.3 -inf.0) (3.3 -0.2) (+inf.0 -0.2) (+inf.0 -0.1) (+inf.0 -0.1) (+inf.0 0.0) \
                   (+inf.0 0.0) (+inf.0 1.0) (+inf.0 1.0) (+inf.0 1.1) (+inf.0 1.1) \
                   (+inf.0 1.2000000000000002) (3.3 1.2000000000000002) (3.3 +inf.0)))",
    },
    SeparationCase {
        padding: "-0.5",
        items: &[
            "'(0 . 1) '(0 . 2) 0 0 #f #f",
            "'(1 . 0) '(0 . 2) 0 0 #f #f",
            "'(0 . 1) '(0 . -1) 0 0 '(0.5 . -0.5) #f",
        ],
        with_group: false,
        with_other: false,
        points: "(((+inf.0 -inf.0) (+inf.0 0.0) (-0.1 0.0) (-0.1 2.0) (+inf.0 2.0) (+inf.0 +inf.0)) \
                  ((-inf.0 -inf.0) (-inf.0 0.0) (1.1 0.0) (1.1 2.0) (-inf.0 2.0) (-inf.0 +inf.0)))",
    },
    SeparationCase {
        padding: "0.15",
        items: &[
            "'(0 . 1.5) '(0.25 . 2.75) 0.125 0.375 '(-0.3 . 0.7) '(0.1 . -0.1)",
            "'(0.75 . 1) '(-3 . -2.5) 0 0 #f '(-1 . 1)",
        ],
        with_group: false,
        with_other: false,
        points: "(((+inf.0 -inf.0) (+inf.0 -4.3) (0.8 -4.3) (0.65 -4.15) (0.65 -4.15) (0.65 -4.0) \
                   (0.65 -4.0) (0.65 -1.5) (0.65 -1.5) (0.65 -1.35) (0.6499999999999999 -1.35) \
                   (0.7999999999999999 -1.2000000000000002) (+inf.0 -1.2000000000000002) \
                   (+inf.0 0.425) (-0.025000000000000005 0.425) (-0.17500000000000002 0.575) \
                   (-0.175 0.575) (-0.175 0.725) (-0.175 0.725) (-0.175 3.025) (-0.175 3.025) \
                   (-0.175 3.175) (-0.17499999999999993 3.175) (-0.024999999999999918 3.3249999999999997) \
                   (+inf.0 3.3249999999999997) (+inf.0 +inf.0)) \
                  ((-inf.0 -inf.0) (-inf.0 -4.3) (0.95 -4.3) (1.0999999999999999 -4.15) (1.1 -4.15) \
                   (1.1 -4.0) (1.1 -4.0) (1.1 -1.5) (1.1 -1.5) (1.1 -1.35) (1.1 -1.35) \
                   (0.9500000000000001 -1.2000000000000002) (-inf.0 -1.2000000000000002) \
                   (-inf.0 0.425) (2.1750000000000003 0.425) (2.325 0.575) (2.325 0.575) \
                   (2.325 0.725) (2.325 0.725) (2.325 3.025) (2.325 3.025) (2.325 3.175) \
                   (2.325 3.175) (2.1750000000000003 3.3249999999999997) \
                   (-inf.0 3.3249999999999997) (-inf.0 +inf.0)))",
    },
    SeparationCase {
        padding: "0.0",
        items: &[],
        with_group: false,
        with_other: false,
        points: "(((+inf.0 -inf.0) (+inf.0 +inf.0)) ((-inf.0 -inf.0) (-inf.0 +inf.0)))",
    },
];

#[test]
fn a_separation_item_s_skylines_are_lilypond_s() {
    for (k, case) in SEPARATION_CASES.iter().enumerate() {
        let Some(mut vm) = grobs_vm() else { return };
        // Note 0 on column 0 is the separation item; its items are things
        // 0 to 2 on the same column, each with thing 5 as its Y parent.
        eval(
            &mut vm,
            "(define (on! g col)
               (ly:grob-set-parent! g 0 col)
               (ly:grob-set-parent! g 1 (thing 5)))
             (define (item! g x y xo yo w h)
               (on! g (column 0))
               (put! g 'X-extent x)
               (put! g 'Y-extent y)
               (put! g 'X-offset xo)
               (put! g 'Y-offset yo)
               (if w (put! g 'extra-spacing-width w))
               (if h (put! g 'extra-spacing-height h))
               g)
             (define (points sky)
               (map (lambda (p) (list (car p) (cdr p))) (ly:skyline->points sky 1)))
             (on! (note 0) (column 0))
             (item! (bundle 0) '(-5 . 5) '(-5 . 5) 0 0 #f #f)
             (on! (thing 4) (column 1))
             (put! (thing 4) 'X-extent '(-5 . 5))
             (put! (thing 4) 'Y-extent '(-5 . 5))",
        );
        let mut elts: Vec<String> = case
            .items
            .iter()
            .enumerate()
            .map(|(i, d)| format!("(item! (thing {i}) {d})"))
            .collect();
        if case.with_group {
            elts.push(String::from("(bundle 0)"));
        }
        if case.with_other {
            elts.push(String::from("(thing 4)"));
        }
        let got = caught(
            &mut vm,
            &format!(
                "(begin
                   (ly:grob-set-object! (note 0) 'elements (arr {}))
                   (put! (note 0) 'skyline-vertical-padding {})
                   (let ((sp (ly:separation-item::calc-skylines (note 0))))
                     (list (points (car sp)) (points (cdr sp)))))",
                elts.join(" "),
                case.padding
            ),
        );
        assert_eq!(got, case.points, "case {k}");
        // A negative padding: Skyline::padded's warning for each side.
        let junk = "\"Cannot have negative horizon padding.  Junking.\"";
        let warned = if case.padding.starts_with('-') {
            format!("({junk} {junk})")
        } else {
            String::from("()")
        };
        assert_eq!(diagnosed(&mut vm), warned, "case {k}");
    }
}

/// The spacing grob `(thing 0)` on column 0, whose separation items are
/// `left` and `right`, Scheme lists of them, with `setup` evaluated
/// first. Every grob here has X and Y offsets of 0 and its X parent's
/// column, and the separation items and their elements have `(thing 5)`,
/// whose Y offset is 0, as their Y parent unless `setup` says otherwise;
/// each element's box is widened by `(-0.5 . 0.5)`.
fn spacing_vm(setup: &str, left: &str, right: &str) -> Option<Vm> {
    let mut vm = grobs_vm()?;
    eval(
        &mut vm,
        "(define (place! g col ypar)
           (ly:grob-set-parent! g 0 col)
           (ly:grob-set-parent! g 1 ypar)
           (put! g 'X-offset 0)
           (put! g 'Y-offset 0)
           g)
         (define (box! g col x y . ypar)
           (place! g col (if (pair? ypar) (car ypar) (thing 5)))
           (put! g 'X-extent x)
           (put! g 'Y-extent y)
           (put! g 'extra-spacing-width '(-0.5 . 0.5))
           g)
         (define (sep! s col ypar . elts)
           (place! s col ypar)
           (ly:grob-set-object! s 'elements (apply arr elts))
           (put! s 'horizontal-skylines ly:separation-item::calc-skylines)
           s)
         (for-each (lambda (t) (put! (thing t) 'Y-offset 0)) '(3 4 5))
         (place! (thing 0) (column 0) (thing 5))",
    );
    eval(&mut vm, setup);
    eval(
        &mut vm,
        &format!(
            "(ly:grob-set-object! (thing 0) 'left-items (apply arr {left}))
             (ly:grob-set-object! (thing 0) 'right-items (apply arr {right}))"
        ),
    );
    Some(vm)
}

/// The left skyline's rightmost point and the right one's leftmost, and
/// the minimum distance, from `(thing 0)` to column 1.
fn spaced(vm: &mut Vm) -> (f64, f64, f64) {
    let me = grob_of(vm, "(thing 0)");
    let right = grob_of(vm, "(column 1)");
    let [l, r] = skylines(vm, me, right).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let d = minimum_distance(vm, me, right).unwrap();
    (l.max_height(), r.max_height(), d)
}

/// Sep 0 on column 0 with a box over `(0 . 2)`, sep 1 on column 1 with
/// one over `(-1 . 1)`, both along `(0 . 1)`.
const TWO_SEPS: &str =
    "(sep! (sep 0) (column 0) (thing 5) (box! (bit 0) (column 0) '(0 . 2) '(0 . 1)))
     (sep! (sep 1) (column 1) (thing 5) (box! (bit 1) (column 1) '(-1 . 1) '(0 . 1)))";

#[test]
fn the_minimum_distance_is_between_the_separation_items_skylines() {
    let Some(mut vm) = spacing_vm(TWO_SEPS, "(list (sep 0))", "(list (sep 1))") else {
        return;
    };
    // 2 + 0.5 on the left, -1 - 0.5 on the right.
    assert_eq!(spaced(&mut vm), (2.5, -1.5, 4.0));
    assert_eq!(diagnosed(&mut vm), "()");

    // No left items: no right skyline either side faces, so 0.
    let Some(mut vm) = spacing_vm(TWO_SEPS, "'()", "(list (sep 1))") else {
        return;
    };
    assert_eq!(spaced(&mut vm), (f64::NEG_INFINITY, -1.5, 0.0));

    // A right box above the left one: they never meet.
    let Some(mut vm) = spacing_vm(
        &format!("{TWO_SEPS} (put! (bit 1) 'Y-extent '(2 . 3))"),
        "(list (sep 0))",
        "(list (sep 1))",
    ) else {
        return;
    };
    assert_eq!(spaced(&mut vm), (2.5, -1.5, 0.0));
}

#[test]
fn only_separation_items_in_the_side_s_column_count() {
    // Left: a spanner, an item without the interface, and a separation
    // item on column 2, each with a box that would reach further; right:
    // a separation item on column 0.
    let setup = format!(
        "{TWO_SEPS}
         (box! (bit 2) (column 0) '(0 . 9) '(0 . 1))
         (sep! (sep 2) (column 2) (thing 5) (box! (bit 3) (column 2) '(0 . 9) '(0 . 1)))
         (sep! (sep 3) (column 0) (thing 5) (box! (bit 4) (column 0) '(-9 . -8) '(0 . 1)))
         (ly:grob-set-object! (group 0) 'elements (arr (bit 2)))"
    );
    let Some(mut vm) = spacing_vm(
        &setup,
        "(list (group 0) (bit 2) (sep 2) (sep 0))",
        "(list (sep 3) (sep 1))",
    ) else {
        return;
    };
    assert_eq!(spaced(&mut vm), (2.5, -1.5, 4.0));
}

#[test]
fn each_separation_item_is_at_its_elements_pure_y_coordinate() {
    // Sep 2 on column 0 reaches to 4.5, its elements' common refpoint,
    // thing 4, 5 up; the right box is along (0 . 1) relative to thing 3.
    let setup = |up: &str| {
        format!(
            "{TWO_SEPS}
             (put! (thing 4) 'Y-offset 5)
             (put! (thing 3) 'Y-offset {up})
             (sep! (sep 2) (column 0) (thing 4) (box! (bit 2) (column 0) '(0 . 4) '(0 . 1) (thing 4)))
             (place! (sep 1) (column 1) (thing 3))
             (place! (bit 1) (column 1) (thing 3))"
        )
    };
    for (up, want) in [("0", 4.0), ("5", 6.0), ("2.5", 0.0)] {
        let Some(mut vm) = spacing_vm(&setup(up), "(list (sep 0) (sep 2))", "(list (sep 1))")
        else {
            return;
        };
        assert_eq!(spaced(&mut vm), (4.5, -1.5, want), "right box {up} up");
    }
}

#[test]
fn a_spacing_grob_s_piece_takes_its_items_pieces() {
    // Thing 1 is thing 0 broken, at the beginning of a line; sep 2, sep
    // 0 broken there, reaches to 6.5. The items are thing 0's.
    let Some(mut vm) = spacing_vm(
        &format!(
            "{TWO_SEPS}
             (place! (thing 1) (column 0) (thing 5))
             (sep! (sep 2) (column 0) (thing 5) (box! (bit 2) (column 0) '(0 . 6) '(0 . 1)))"
        ),
        "(list (sep 0))",
        "(list (sep 1))",
    ) else {
        return;
    };
    let g: Vec<Value> = ["(thing 0)", "(thing 1)", "(sep 0)", "(sep 2)"]
        .iter()
        .map(|t| grob_of(&mut vm, t))
        .collect();
    let heap = vm.heap();
    grob::with_grob_mut(heap, g[0], |x| x.broken = [None, Some(g[1])]);
    grob::with_grob_mut(heap, g[1], |x| x.original = Some(g[0]));
    grob::with_grob_mut(heap, g[2], |x| x.broken = [None, Some(g[3])]);
    grob::with_grob_mut(heap, g[3], |x| x.original = Some(g[2]));
    let right = grob_of(&mut vm, "(column 1)");
    let d = minimum_distance(&mut vm, g[1], right).unwrap();
    assert_eq!(d, 8.0);
    // Thing 0 itself takes sep 0.
    assert_eq!(minimum_distance(&mut vm, g[0], right).unwrap(), 4.0);
}

/// Placement 0's accidentals, bits 4 and 5 on column 1, and placement 1's,
/// bits 6 and 7; bits 5 and 7 have a tie, and `forced` `force5` and
/// `#t`. Bit 3 on column 1 is another conditional element of sep 1.
fn accidentals(force5: &str) -> String {
    format!(
        "(box! (bit 3) (column 1) '(-2 . -1) '(0 . 1))
         (box! (bit 4) (column 1) '(-4 . -3) '(0 . 1))
         (box! (bit 5) (column 1) '(-9 . -8) '(0 . 1))
         (box! (bit 6) (column 1) '(-5 . -4) '(5 . 6))
         (box! (bit 7) (column 1) '(-6 . -5) '(5 . 6))
         (ly:grob-set-object! (bit 5) 'tie (stem 0))
         (ly:grob-set-object! (bit 7) 'tie (stem 0))
         (put! (bit 5) 'forced {force5})
         (put! (bit 7) 'forced #t)
         (ly:grob-set-object! (placement 0) 'accidental-grobs
                              (list (cons '(0 . 1) (list (bit 4) (bit 5)))))
         (ly:grob-set-object! (placement 1) 'accidental-grobs
                              (list (cons '(1 . 1) (list (bit 6))) (cons '(2 . 1) (list (bit 7)))))"
    )
}

#[test]
fn the_right_items_conditional_elements_are_given_the_first_left_item() {
    let cases = [
        // Bit 3 alone, to -2.5.
        ("(arr (bit 3))", "#f", 5.0),
        // Placement 0's accidental bit 4, to -4.5; bit 5 is a break
        // reminder, `forced` not #t, and sep 0 begins no line.
        ("(arr (placement 0) (bit 3))", "#f", 7.0),
        ("(arr (placement 0) (bit 3))", "'yes", 7.0),
        // Bit 5 forced, to -9.5.
        ("(arr (bit 3) (placement 0))", "#t", 12.0),
    ];
    for (conditional, force5, want) in cases {
        let Some(mut vm) = spacing_vm(
            &format!(
                "{TWO_SEPS} {}
                 (ly:grob-set-object! (sep 1) 'conditional-elements {conditional})",
                accidentals(force5)
            ),
            "(list (sep 0))",
            "(list (sep 1))",
        ) else {
            return;
        };
        assert_eq!(
            spaced(&mut vm),
            (2.5, 2.5 - want, want),
            "{conditional} {force5}"
        );
    }
}

#[test]
fn relevant_accidentals_keep_break_reminders_at_the_beginning_of_a_line() {
    let Some(mut vm) = spacing_vm(&accidentals("#f"), "'()", "'()") else {
        return;
    };
    let g: Vec<Value> = (0..4)
        .map(|i| grob_of(&mut vm, &format!("(thing {i})")))
        .collect();
    let heap = vm.heap();
    // Things 1 and 2: thing 0 broken at the end and the beginning of a line.
    grob::with_grob_mut(heap, g[0], |x| x.broken = [Some(g[1]), Some(g[2])]);
    grob::with_grob_mut(heap, g[1], |x| x.original = Some(g[0]));
    grob::with_grob_mut(heap, g[2], |x| x.original = Some(g[0]));
    let placements = [
        grob_of(&mut vm, "(placement 0)"),
        grob_of(&mut vm, "(placement 1)"),
    ];
    let bits: Vec<Value> = (0..8)
        .map(|i| grob_of(&mut vm, &format!("(bit {i})")))
        .collect();
    let named = |got: Vec<Value>| -> Vec<usize> {
        got.iter()
            .map(|v| bits.iter().position(|b| b == v).unwrap())
            .collect()
    };
    // The lists are kept from one placement to the next, as LilyPond's.
    for (left, want) in [
        (g[0], vec![4, 4, 6, 7]),
        (g[1], vec![4, 4, 6, 7]),
        (g[2], vec![4, 5, 4, 6, 7, 5]),
    ] {
        let got = get_relevant_accidentals(&mut vm, &placements, left).unwrap();
        assert_eq!(named(got), want);
    }
    assert!(
        get_relevant_accidentals(&mut vm, &[], g[2])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_separation_item_without_skylines_is_a_programming_error() {
    let Some(mut vm) = spacing_vm(
        &format!("{TWO_SEPS} (put! (sep 0) 'horizontal-skylines 'none)"),
        "(list (sep 0))",
        "(list (sep 1))",
    ) else {
        return;
    };
    assert_eq!(spaced(&mut vm), (f64::NEG_INFINITY, -1.5, 0.0));
    assert_eq!(
        diagnosed(&mut vm),
        "(\"separation item has no skyline\" \"separation item has no skyline\")"
    );
    // Its skylines the wrong way round: from_scm's error.
    let Some(mut vm) = spacing_vm(
        &format!(
            "{TWO_SEPS} (let ((sp (ly:separation-item::calc-skylines (sep 0))))
                          (put! (sep 0) 'horizontal-skylines (cons (cdr sp) (car sp))))"
        ),
        "(list (sep 0))",
        "(list (sep 1))",
    ) else {
        return;
    };
    let me = grob_of(&mut vm, "(thing 0)");
    let right = grob_of(&mut vm, "(column 1)");
    let e = minimum_distance(&mut vm, me, right).unwrap_err();
    assert!(
        e.text(vm.heap())
            .contains("direction of first skyline in skyline pair must be DOWN/LEFT"),
        "{}",
        e.text(vm.heap())
    );
}

#[test]
fn a_column_s_when_is_its_moment_or_0() {
    let Some(mut vm) = grobs_vm() else { return };
    let col = grob_of(&mut vm, "(column 0)");
    let when = |vm: &mut Vm| paper_column::when_mom(vm, col).unwrap();
    let zero = Moment::new(Rational::ZERO, Rational::ZERO);
    assert_eq!(when(&mut vm), zero);
    eval(&mut vm, "(put! (column 0) 'when (ly:make-moment 3/4 -1/8))");
    assert_eq!(
        when(&mut vm),
        Moment::new(Rational::new(3, 4), Rational::new(-1, 8))
    );
    eval(&mut vm, "(put! (column 0) 'when 3/4)");
    assert_eq!(when(&mut vm), zero);
}

#[test]
fn a_column_is_put_on_its_system() {
    let Some(mut vm) = grobs_vm() else { return };
    put_on(&mut vm, 0, 0, &[0]);
    let [col, sys, thing] = ["(column 0)", "(group 0)", "(thing 0)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(item::get_system(vm.heap_mut(), thing), None);
    paper_column::set_system(vm.heap(), col, sys);
    assert_eq!(paper_column::system(vm.heap(), col), Some(sys));
    assert_eq!(item::get_system(vm.heap_mut(), thing), Some(sys));
}

#[test]
fn a_column_is_musical_by_its_shortest_starter_duration() {
    let Some(mut vm) = grobs_vm() else { return };
    let col = grob_of(&mut vm, "(column 0)");
    for (st, musical) in [
        ("'()", false),
        ("1/4", true),
        ("0", false),
        ("+inf.0", true),
        ("0.5", false),
        ("(ly:make-moment 1/4)", false),
    ] {
        eval(
            &mut vm,
            &format!("(put! (column 0) 'shortest-starter-duration {st})"),
        );
        assert_eq!(
            paper_column::is_musical(&mut vm, col).unwrap(),
            musical,
            "{st}"
        );
    }
}

#[test]
fn a_column_is_used_by_its_grobs_breaks_and_labels() {
    let Some(mut vm) = grobs_vm() else { return };
    let col = grob_of(&mut vm, "(column 0)");
    assert!(!paper_column::is_used(&mut vm, col).unwrap());
    for (set, used) in [
        ("(ly:grob-set-object! (column 0) 'elements (arr))", false),
        (
            "(ly:grob-set-object! (column 0) 'elements (arr (thing 0)))",
            true,
        ),
        (
            "(ly:grob-set-object! (column 0) 'bounded-by-me (arr (group 0)))",
            true,
        ),
        ("(put! (column 0) 'line-break-permission 'allow)", true),
        ("(put! (column 0) 'line-break-permission #t)", false),
        ("(put! (column 0) 'used #t)", true),
        ("(put! (column 0) 'used 'yes)", false),
        ("(put! (column 0) 'labels '(mark))", true),
        ("(put! (column 0) 'labels '())", false),
    ] {
        eval(
            &mut vm,
            &format!(
                "(ly:grob-set-object! (column 0) 'elements '())
                 (ly:grob-set-object! (column 0) 'bounded-by-me '())
                 (for-each (lambda (p) (put! (column 0) p '()))
                           '(line-break-permission used labels))
                 {set}"
            ),
        );
        assert_eq!(paper_column::is_used(&mut vm, col).unwrap(), used, "{set}");
    }
}

/// Columns 0 and 1, each with `horizontal-skylines` from its `elements`,
/// bits 0 and 1 over `(0 . 2)` and `(-1 . 1)` along `(0 . 1)` widened by
/// `(-0.5 . 0.5)`, with `setup` evaluated after.
fn columns_vm(setup: &str) -> Option<Vm> {
    spacing_vm(
        &format!(
            "(define (col! c . elts)
               (ly:grob-set-parent! c 1 (thing 5))
               (put! c 'X-offset 0)
               (put! c 'Y-offset 0)
               (ly:grob-set-object! c 'elements (apply arr elts))
               (put! c 'horizontal-skylines ly:separation-item::calc-skylines)
               c)
             (col! (column 0) (box! (bit 0) (column 0) '(0 . 2) '(0 . 1)))
             (col! (column 1) (box! (bit 1) (column 1) '(-1 . 1) '(0 . 1)))
             {setup}"
        ),
        "'()",
        "'()",
    )
}

/// The minimum distance from column 0 to column 1.
fn column_distance(vm: &mut Vm) -> f64 {
    let [left, right] = ["(column 0)", "(column 1)"].map(|t| grob_of(vm, t));
    paper_column::minimum_distance(vm, left, right)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
}

#[test]
fn the_minimum_distance_is_between_the_columns_skylines() {
    for (setup, want) in [
        // 2 + 0.5 on the left, -1 - 0.5 on the right.
        ("", 4.0),
        // The right box above the left one: they never meet.
        ("(put! (bit 1) 'Y-extent '(2 . 3))", 0.0),
        // Either side without a skyline pair has an empty skyline, with
        // no programming error.
        ("(put! (column 0) 'horizontal-skylines 'none)", 0.0),
        ("(put! (column 1) 'horizontal-skylines '())", 0.0),
        // Column 1's conditional element, to -3.5.
        (
            "(box! (bit 3) (column 1) '(-3 . -2) '(0 . 1))
             (ly:grob-set-object! (column 1) 'conditional-elements (arr (bit 3)))",
            6.0,
        ),
        // Column 0's conditional elements are not its own.
        (
            "(box! (bit 3) (column 0) '(0 . 9) '(0 . 1))
             (ly:grob-set-object! (column 0) 'conditional-elements (arr (bit 3)))",
            4.0,
        ),
    ] {
        let Some(mut vm) = columns_vm(setup) else {
            return;
        };
        assert_eq!(column_distance(&mut vm), want, "{setup}");
        assert_eq!(diagnosed(&mut vm), "()", "{setup}");
    }
    // Column 0's accidental placement among column 1's conditional
    // elements: its accidental bit 4, to -4.5, given column 0.
    let Some(mut vm) = columns_vm(
        "(box! (bit 4) (column 1) '(-4 . -3) '(0 . 1))
         (ly:grob-set-object! (placement 0) 'accidental-grobs
                              (list (cons '(0 . 1) (list (bit 4)))))
         (ly:grob-set-object! (column 1) 'conditional-elements (arr (placement 0)))",
    ) else {
        return;
    };
    assert_eq!(column_distance(&mut vm), 7.0);
}

#[test]
fn a_column_s_skylines_the_wrong_way_round_are_from_scm_s_error() {
    let Some(mut vm) = columns_vm(
        "(let ((sp (ly:separation-item::calc-skylines (column 1))))
           (put! (column 1) 'horizontal-skylines (cons (cdr sp) (car sp))))",
    ) else {
        return;
    };
    let [left, right] = ["(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let e = paper_column::minimum_distance(&mut vm, left, right).unwrap_err();
    assert!(
        e.text(vm.heap())
            .contains("direction of first skyline in skyline pair must be DOWN/LEFT"),
        "{}",
        e.text(vm.heap())
    );
}

#[test]
fn a_musical_column_whose_heads_are_elsewhere_is_extraneous() {
    let Some(mut vm) = grobs_vm() else { return };
    let col = grob_of(&mut vm, "(column 0)");
    eval(
        &mut vm,
        "(ly:grob-set-parent! (head 0) 0 (column 1))
         (ly:grob-set-parent! (head 1) 0 (column 0))
         (ly:grob-set-parent! (bit 0) 0 (column 1))",
    );
    for (st, elements, extraneous) in [
        // Not musical.
        ("0", "(arr (head 0))", false),
        // No elements, or no rhythmic heads among them.
        ("1/4", "(arr)", false),
        ("1/4", "(arr (bit 0))", false),
        // Its head is column 1's.
        ("1/4", "(arr (bit 0) (head 0))", true),
        // One head is its own.
        ("1/4", "(arr (head 0) (head 1))", false),
        ("1/4", "(arr (head 1) (head 0))", false),
    ] {
        eval(
            &mut vm,
            &format!(
                "(put! (column 0) 'shortest-starter-duration {st})
                 (ly:grob-set-object! (column 0) 'elements {elements})"
            ),
        );
        assert_eq!(
            paper_column::is_extraneous_column_from_ligature(&mut vm, col).unwrap(),
            extraneous,
            "{st} {elements}"
        );
    }
}

#[test]
fn the_paper_column_s_print_is_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:paper-column::print";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(music.iter().all(|p| p.name != name), "no placeholder");
    // What is no paper column, where LilyPond reads through a null.
    for arg in ["(thing 0)", "'x"] {
        let got = caught(&mut vm, &format!("({name} {arg})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Paper_column)"),
            "{arg}: {got}"
        );
    }
    for args in ["", "(column 0) 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
    // The when, as Moment::to_string writes it; `?/?` for no moment.
    for (when, written) in [
        ("(ly:make-moment 1/4)", "1/4"),
        ("(ly:make-moment 3)", "3"),
        ("(ly:make-moment 0)", "0"),
        ("(ly:make-moment 1 2 -1 8)", "1/2G-1/8"),
        ("(ly:make-moment -3 4 1 8)", "-3/4G1/8"),
        ("(ly:make-moment +inf.0)", "infinity"),
        ("(ly:make-moment -inf.0)", "-infinity"),
        ("'x", "?/?"),
        ("1/4", "?/?"),
    ] {
        assert_eq!(
            caught(&mut vm, &format!("(%paper-column-when-string {when})")),
            format!("\"{written}\""),
            "{when}"
        );
    }
    // A distance as Apple's libc writes it with %5.2lf (printf in C).
    for (x, written) in [
        ("0.125", " 0.12"),
        ("0.375", " 0.38"),
        ("2.675", " 2.67"),
        ("12.345", "12.35"),
        ("-0.004", "-0.00"),
        ("-0.0", "-0.00"),
        ("100.5", "100.50"),
        ("1.005", " 1.00"),
        ("0.615", " 0.61"),
        ("0.625", " 0.62"),
        ("123456.785", "123456.79"),
        ("5e-324", " 0.00"),
        ("3", " 3.00"),
        ("+inf.0", "  inf"),
        ("-inf.0", " -inf"),
        ("+nan.0", "  nan"),
    ] {
        assert_eq!(
            caught(&mut vm, &format!("(%paper-column-distance-string {x})")),
            format!("\"{written}\""),
            "{x}"
        );
    }
}

/// `tools/` of the engine.
fn tools() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools")
}

/// Emmentaler's packs and LilyPond's text packs (`tools/packs/build.mjs`,
/// `text.mjs`), built once; `None` without LilyPond 2.26.0.
fn print_packs() -> Option<&'static PathBuf> {
    static BUILT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-print");
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            for script in ["packs/build.mjs", "packs/text.mjs"] {
                let run = Command::new("node")
                    .arg(tools().join(script))
                    .arg("--out")
                    .arg(&dir)
                    .output()
                    .unwrap_or_else(|e| panic!("node runs tools/{script}: {e}"));
                assert!(
                    run.status.success(),
                    "tools/{script} failed: {}",
                    String::from_utf8_lossy(&run.stderr)
                );
            }
            Some(dir)
        })
        .as_ref()
}

/// The cases of [`PRINT_PROBE`]: each column's `when`, the ideal
/// distances of its springs and its minimum distances, all to itself.
const PRINT_CASES: &str = "(list
  (list (ly:make-moment 1/4) '(3.5 2) '(1.25))
  (list (ly:make-moment 1 2 -1 8) '(0.125) '(0.375 -0.004))
  (list (ly:make-moment 3 8) '() '())
  (list (ly:make-moment +inf.0) '(12.345) '(2.675))
  (list (ly:make-moment -3 4 1 8) '(0 7.5 1.005) '(100.5 0.615 3)))";

/// A score whose first columns are given [`PRINT_CASES`]' and printed by
/// `ly:paper-column::print`, each written as a line: its rank, a tab, and
/// its stencil as `tools/compare/stencils.ly`'s datum `(x y expr)`, font
/// metrics written as `(%font file name magnification)`.
const PRINT_PROBE: &str = r#"\version "2.26.0"
#(define cases CASES)
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define n 0)
#(define (probe g)
  (if (< n (length cases))
      (let ((c (list-ref cases n)))
        (set! n (1+ n))
        (ly:grob-set-property! g 'when (car c))
        (ly:grob-set-object! g 'ideal-distances
                             (map (lambda (d) (cons (ly:make-spring d d) g)) (cadr c)))
        (ly:grob-set-object! g 'minimum-distances (map (lambda (d) (cons g d)) (caddr c)))
        (let ((s (ly:paper-column::print g)))
          (format #t "~a\t" (car (ly:grob-spanned-column-rank-interval g)))
          (write (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s))))
          (newline)
          s))
      #f))
\layout { \context { \Score
  \override PaperColumn.stencil = #probe
  \override NonMusicalPaperColumn.stencil = #probe } }
{ c'4 d' e' f' g' a' }
"#;

/// `text` with each string naming an `.otf` file cut to the file's name:
/// a capture's path is the installed LilyPond's.
fn otf_names(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let end = start + 1 + rest[start + 1..].find('"').unwrap();
        let s = &rest[start + 1..end];
        out.push_str(&rest[..=start]);
        if s.ends_with(".otf") {
            out.push_str(s.rsplit('/').next().unwrap());
        } else {
            out.push_str(s);
        }
        out.push('"');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// [`grobs_vm`] given the packs but the braces', with `(column 0)` a column as
/// `define-grobs.scm`'s at staff size 20 (its `font-size` and the fonts of
/// LilyPond's `property-defaults`), on the system `(group 0)`, and the
/// installed `color.scm` and `stencil-with-color`.
fn print_vm(packs: &PathBuf) -> Option<Vm> {
    print_vm_of(packs, DEFS, MADE)
}

/// [`print_vm`] with the contexts `defs` and what `Maker` makes, `made`
/// (see [`grobs_vm_of`]).
fn print_vm_of(packs: &PathBuf, defs: &[&str], made: &str) -> Option<Vm> {
    let mut vm = grobs_vm_of(defs, made)?;
    for entry in std::fs::read_dir(packs).unwrap() {
        let path = entry.unwrap().path();
        let brace = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("emmentaler-brace"));
        if path.extension().is_some_and(|e| e == "pack") && !brace {
            lily_engine_layout::font::put_pack(&mut vm, &std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {}", path.display(), e.text(vm.heap())));
        }
    }
    eval(&mut vm, "(use-modules (srfi srfi-1))");
    let color = lily_scm().unwrap().join("color.scm");
    let text = std::fs::read_to_string(&color).unwrap();
    vm.eval_text(&text, color.to_str())
        .unwrap_or_else(|e| panic!("color.scm: {}", e.text(vm.heap())));
    let stencil = std::fs::read_to_string(lily_scm().unwrap().join("stencil.scm")).unwrap();
    let start = stencil.find("(define-public (stencil-with-color").unwrap();
    let mut heap = lily_engine_scheme::Heap::new();
    let form = lily_engine_scheme::read(&mut heap, &stencil, start, None).unwrap();
    eval(
        &mut vm,
        &format!(
            "(define-public X 0) (define-public Y 1)
             (ly:register-stencil-expression 'color)
             {}
             (define-public feta-design-size-mapping
               '((11 . 11.22) (13 . 12.60) (14 . 14.14) (16 . 15.87) (18 . 17.82) (20 . 20)
                 (23 . 22.45) (26 . 25.20)))
             (let ((layout (ly:grob-layout (column 0))) (staff-height (* 20 (ly:pt 1))))
               (ly:output-def-set-variable! layout 'staff-height staff-height)
               (ly:output-def-set-variable! layout 'output-scale (/ staff-height 4))
               (ly:output-def-set-variable! layout 'text-font-size 11.0))
             (put! (column 0) 'font-size -7.5)
             (put! (column 0) 'fonts '((serif . \"LilyPond Serif\") (sans . \"LilyPond Sans Serif\")
                                       (typewriter . \"LilyPond Monospace\") (music . \"emmentaler\")))
             (define cases {PRINT_CASES})
             (define (print-case n)
               (let ((c (list-ref cases n)) (g (column 0)))
                 (put! g 'when (car c))
                 (ly:grob-set-object! g 'ideal-distances
                                      (map (lambda (d) (cons (ly:make-spring d d) g)) (cadr c)))
                 (ly:grob-set-object! g 'minimum-distances (map (lambda (d) (cons g d)) (caddr c)))
                 (%stencil->dump (ly:paper-column::print g))))",
            &stencil[form.start..form.end]
        ),
    );
    let col = grob_of(&mut vm, "(column 0)");
    let system = grob_of(&mut vm, "(group 0)");
    paper_column::set_system(vm.heap(), col, system);
    Some(vm)
}

#[test]
fn a_paper_column_s_print_is_lilypond_s() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is unverified against it");
        return;
    };
    let Some(mut vm) = print_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-print-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("print.ly"),
        PRINT_PROBE.replace("CASES", PRINT_CASES),
    )
    .unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages", "print.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 5, "{lilypond}");
    let col = grob_of(&mut vm, "(column 0)");
    for (n, line) in lines.iter().enumerate() {
        let (rank, want) = line.split_once('\t').unwrap();
        paper_column::set_rank(vm.heap(), col, rank.parse().unwrap());
        let got = caught(&mut vm, &format!("(print-case {n})"));
        assert_eq!(otf_names(&got), otf_names(want), "case {n}, rank {rank}");
    }
}

#[test]
fn the_print_draws_arrows_only_to_columns_on_a_system() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = print_vm(packs) else {
        return;
    };
    // Column 1, on no system, printed as column 0 is.
    eval(
        &mut vm,
        "(put! (column 1) 'font-size -7.5)
         (put! (column 1) 'fonts (ly:grob-property (column 0) 'fonts))
         (define (arrows me ideal minimum)
           (ly:grob-set-object! me 'ideal-distances ideal)
           (ly:grob-set-object! me 'minimum-distances minimum)
           (ly:stencil-expr (ly:paper-column::print me)))",
    );
    let arrows = |vm: &mut Vm, me: &str, ideal: &str, minimum: &str| {
        caught(vm, &format!("(arrows {me} {ideal} {minimum})"))
            .matches("(draw-line ")
            .count()
    };
    let spring = "(ly:make-spring 2 2)";
    // A spring's to a grob with a system, whichever; a distance's to a
    // grob of the column's own system, none for none.
    for (me, ideal, minimum, drawn) in [
        ("(column 0)", "'()", "'()", 0),
        (
            "(column 0)",
            &format!(
                "(list (cons {spring} (column 0)) (cons {spring} (thing 0)) (cons {spring} 'x))"
            ),
            "'()",
            1,
        ),
        (
            "(column 0)",
            "'()",
            "(list (cons (column 0) 1) (cons (column 1) 1) (cons 'x 1))",
            1,
        ),
        (
            "(column 1)",
            &format!("(list (cons {spring} (column 0)) (cons {spring} (column 1)))"),
            "(list (cons (column 0) 1) (cons (column 1) 1) (cons (thing 0) 1))",
            3,
        ),
    ] {
        assert_eq!(
            arrows(&mut vm, me, ideal, minimum),
            drawn,
            "{me} {ideal} {minimum}"
        );
    }
    // A spring that is none is read only when drawn, where LilyPond reads
    // through a null; a distance that is no real always (from_scm<double>).
    assert_eq!(
        arrows(&mut vm, "(column 0)", "(list (cons 'x (thing 0)))", "'()"),
        0
    );
    let got = caught(
        &mut vm,
        "(arrows (column 0) (list (cons 'x (column 0))) '())",
    );
    assert!(
        got.contains("%spring-ideal-distance")
            && got.contains("Wrong type argument in position 1 (expecting Spring): x"),
        "{got}"
    );
    let got = caught(&mut vm, "(arrows (column 0) '() (list (cons 'x 'far)))");
    assert!(
        got.contains("Wrong type (expecting real number): far"),
        "{got}"
    );
}

/// Break alignments of pieces: `(aligned c)` makes `(piece 0)` a break
/// alignment of the case `c`, `(dir orders groups)` as
/// [`POSITIONING_PROBE`] writes it, each group `(symbol extent cause
/// elements)` a piece whose `X-extent` is `extent`, whose `cause` is a
/// piece named `cause` (`#f` a dead one, `""` none), and whose
/// `elements` are pieces with each `(symbol space-alist)`; `(positions)`
/// positions them and gives their X offsets within it. Its break direction
/// is [`break_dir`]'s, `(piece 1)` its original.
const ALIGNED: &str = "(define next-piece 0)
  (define (fresh) (let ((p (piece next-piece))) (set! next-piece (1+ next-piece)) p))
  (define (aligned c)
    (let ((me (fresh)))
      (fresh)
      (put! me 'break-align-orders (cadr c))
      (ly:grob-set-object!
       me 'elements
       (apply arr
              (map (lambda (g)
                     (let ((grp (fresh)))
                       (ly:grob-set-parent! grp 0 me)
                       (put! grp 'break-align-symbol (car g))
                       (put! grp 'X-extent (cadr g))
                       (if (not (equal? (caddr g) \"\"))
                           (let ((cause (fresh)))
                             (put! cause 'meta (if (caddr g) `((name . ,(caddr g))) '()))
                             (put! grp 'cause cause)))
                       (ly:grob-set-object!
                        grp 'elements
                        (apply arr (map (lambda (e)
                                          (let ((elt (fresh)))
                                            (put! elt 'break-align-symbol (car e))
                                            (put! elt 'space-alist (cadr e))
                                            elt))
                                        (cadddr g))))
                       grp))
                   (caddr c))))
      me))
  (define (positions)
    (let ((me (piece 0)))
      (ly:break-alignment-interface::calc-positioning-done me)
      (map (lambda (g) (ly:grob-relative-coordinate g me 0)) (%grob-set me 'elements))))";

/// `(piece 0)` given the break direction `dir`: a piece broken from
/// `(piece 1)`, its left piece for -1.
fn break_dir(vm: &mut Vm, dir: i64) {
    let (me, original) = (grob_of(vm, "(piece 0)"), grob_of(vm, "(piece 1)"));
    if dir != 0 {
        grob::with_grob_mut(vm.heap(), me, |g| g.original = Some(original));
    }
    if dir == -1 {
        grob::with_grob_mut(vm.heap(), original, |g| g.broken[0] = Some(me));
    }
}

/// [`grobs_vm`] with [`ALIGNED`].
fn aligned_vm() -> Option<Vm> {
    let mut vm = grobs_vm()?;
    eval(&mut vm, ALIGNED);
    Some(vm)
}

/// The texts of the diagnostics given since the last call, each ended by
/// a newline.
fn diagnosed_lines(vm: &mut Vm) -> String {
    let v = eval(
        vm,
        "(let ((d (reverse (car %diagnostics))))
           (set-car! %diagnostics '())
           (apply string-append (map (lambda (v) (string-append (vector-ref v 2) \"\\n\")) d)))",
    );
    shown(vm, v)
}

#[test]
fn the_break_alignment_s_positioning_is_layout_s() {
    let Some(mut vm) = aligned_vm() else { return };
    let subr = "ly:break-alignment-interface::calc-positioning-done";
    for (arg, what) in [("'x", "x"), ("(staff 0)", "#<Grob Staff >")] {
        let got = caught(&mut vm, &format!("({subr} {arg})"));
        assert!(
            got.contains(subr)
                && got.contains(&format!(
                    "Wrong type argument in position 1 (expecting Item): {what}"
                )),
            "{got}"
        );
    }
    assert!(
        caught(&mut vm, &format!("({subr})")).contains("Wrong number of arguments"),
        "arity"
    );
    // No group: positioned, and nothing moved.
    assert_eq!(caught(&mut vm, &format!("({subr} (piece 0))")), "#t");
    assert_eq!(
        caught(&mut vm, "(ly:grob-property (piece 0) 'positioning-done)"),
        "#t"
    );
    assert_eq!(diagnosed_lines(&mut vm), "");
}

#[test]
fn the_groups_are_ordered_by_the_break_align_orders_of_their_break_direction() {
    // Groups a, b, a, of no symbol and c, pieces 2 to 6.
    let groups =
        "'((a (0 . 1) \"\" ()) (b (0 . 1) \"\" ()) (a (0 . 1) \"\" ()) (() (0 . 1) \"\" ())
                    (c (0 . 1) \"\" ()))";
    for (dir, orders, want) in [
        // As they are without a vector of three orders.
        (0, "()", "(2 3 4 5 6)"),
        (0, "#((a) (b))", "(2 3 4 5 6)"),
        // Each symbol's from the last to the first, once; those of no
        // symbol left out.
        (0, "#((c) (b a c a) (a))", "(3 4 2 6)"),
        (1, "#((c) (b a c a) (a))", "(4 2)"),
        (-1, "#((c) (b a c a) (a))", "(6)"),
        // As they are for #f; up to the end of an improper order.
        (0, "#(() #f ())", "(2 3 4 5 6)"),
        (0, "#(() (b . c) ())", "(3)"),
        (0, "#(() x ())", "()"),
    ] {
        let Some(mut vm) = aligned_vm() else { return };
        eval(
            &mut vm,
            &format!("(aligned (list {dir} '{orders} {groups}))"),
        );
        break_dir(&mut vm, dir);
        let got = caught(
            &mut vm,
            "(map (lambda (g) (let loop ((i 0)) (if (eq? (piece i) g) i (loop (1+ i)))))
                  (%break-alignment-ordered-elements (piece 0)))",
        );
        assert_eq!(got, want, "{dir} {orders}");
    }
}

#[test]
fn the_groups_are_spaced_by_their_space_alists_and_aligned_on_their_edge() {
    // A clef, a key signature and a time signature; the clef's space-alist
    // that of its last element that has one, the key signature's none.
    let case = |clef_alist: &str, key_symbol: &str| {
        format!(
            "'(0 () ((clef (0.0 . 2.0) Clef ((clef ((key-signature extra-space . 9.0)))
                                          (clef {clef_alist}) (clef 7)))
                     (key-signature (-1.0 . 1.0) KeySignature (({key_symbol} ())))
                     (time-signature (0.0 . 1.0) \"\" ((time-signature ((right-edge extra-space . 0.25)))))))"
        )
    };
    for (dir, clef_alist, key_symbol, want, errors) in [
        (
            0,
            "((key-signature minimum-space . 3.0))",
            "key-signature",
            "(0.0 3.0 5.0)",
            "No spacing entry from KeySignature to `time-signature'\n",
        ),
        // Aligned on its right with the last extra space at the end of a
        // line.
        (
            -1,
            "((key-signature minimum-space . 3.0))",
            "key-signature",
            "(-6.25 -3.25 -1.25)",
            "No spacing entry from KeySignature to `time-signature'\n",
        ),
        // A space of no known type is none; aligned on the left of all.
        (
            1,
            "((key-signature fixed-space . 3.0))",
            "key-signature",
            "(1.0 1.0 3.0)",
            "No spacing entry from KeySignature to `time-signature'\n",
        ),
        // No symbol on the right: no entry, named as nothing.
        (
            0,
            "((key-signature extra-space . 3.0))",
            "5",
            "(0.0 4.0 6.0)",
            "No spacing entry from Clef to `'\nNo spacing entry from KeySignature to `time-signature'\n",
        ),
    ] {
        let Some(mut vm) = aligned_vm() else { return };
        eval(
            &mut vm,
            &format!("(aligned {})", case(clef_alist, key_symbol)),
        );
        break_dir(&mut vm, dir);
        assert_eq!(caught(&mut vm, "(positions)"), want, "{clef_alist}");
        assert_eq!(diagnosed_lines(&mut vm), errors, "{clef_alist}");
    }
    // A distance that is no real: from_scm<double>'s error.
    let Some(mut vm) = aligned_vm() else { return };
    eval(
        &mut vm,
        &format!(
            "(aligned {})",
            case("((key-signature extra-space . far))", "key-signature")
        ),
    );
    let got = caught(&mut vm, "(positions)");
    assert!(
        got.contains("Wrong type (expecting real number): far"),
        "{got}"
    );
    // The edge is the last group whose left-edge is on the right of
    // another, though one before it was a left edge.
    let Some(mut vm) = aligned_vm() else { return };
    eval(
        &mut vm,
        "(aligned '(0 () ((left-edge (0.0 . 2.0) \"\" ((left-edge ((left-edge extra-space . 1.0)))))
                         (left-edge (0.0 . 1.0) \"\" ((left-edge ((right-edge extra-space . 0.5))))))))",
    );
    assert_eq!(caught(&mut vm, "(positions)"), "(-3.0 0.0)");
    // Aligned on the left edge, though a group reaches further left.
    let Some(mut vm) = aligned_vm() else { return };
    eval(
        &mut vm,
        "(aligned '(1 () ((left-edge (0.0 . 0.0) \"\" ((left-edge ((clef minimum-space . 1.0)))))
                         (clef (-2.0 . 2.0) \"\" ((clef ((right-edge extra-space . 0.5))))))))",
    );
    break_dir(&mut vm, 1);
    assert_eq!(caught(&mut vm, "(positions)"), "(0.0 1.0)");
    // Groups all empty: nothing to position, nothing moved.
    let Some(mut vm) = aligned_vm() else { return };
    eval(
        &mut vm,
        "(aligned '(-1 () ((clef (+inf.0 . -inf.0) Clef ((clef ())))
                          (key-signature (1.0 . 0.0) \"\" ()))))",
    );
    break_dir(&mut vm, -1);
    assert_eq!(caught(&mut vm, "(positions)"), "(0.0 0.0)");
    assert_eq!(diagnosed_lines(&mut vm), "");
}

/// A score whose break alignments are positioned by
/// `ly:break-alignment-interface::calc-positioning-done`, each written as
/// a line: `((dir orders groups) offsets)`, its break direction, its
/// `break-align-orders`, its groups as [`ALIGNED`] takes them (their
/// extents read before), and their X offsets within it after. Its
/// programming errors are LilyPond's on its standard error.
const POSITIONING_PROBE: &str = r#"\version "2.26.0"
#(define (elts g)
  (let ((a (ly:grob-object g 'elements))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (probe g)
  (let* ((groups (elts g))
         (input (list (ly:item-break-dir g)
                      (ly:grob-property g 'break-align-orders)
                      (map (lambda (grp)
                             (list (ly:grob-property grp 'break-align-symbol)
                                   (ly:grob-extent grp grp X)
                                   (let ((c (ly:grob-property grp 'cause)))
                                     (if (ly:grob? c) (grob::name c) ""))
                                   (map (lambda (e) (list (ly:grob-property e 'break-align-symbol)
                                                          (ly:grob-property e 'space-alist)))
                                        (elts grp))))
                           groups)))
         (r (ly:break-alignment-interface::calc-positioning-done g)))
    (write (list input (map (lambda (grp) (ly:grob-relative-coordinate grp g X)) groups)))
    (newline)
    r))
\layout { \context { \Score \override BreakAlignment.positioning-done = #probe } }
<<
  \new Staff \with { \consists Ambitus_engraver } {
    \clef bass \key es \major \time 3/4 c4 d e \breathe | \bar "||" \key a \major \clef treble
    f g a \break \time 2/4 b c' \bar "|." }
  \new Staff { \override Staff.TimeSignature.space-alist = #'() c'4 d' e' f' g' a' \break b' c'' }
>>
\score { { \override Staff.KeySignature.space-alist = #'() \key d \major c'1 \key f \major d'1 } }
\score {
  \layout { \context { \Score \override BreakAlignment.break-align-orders =
    #(vector '(clef staff-bar) '(time-signature clef key-signature staff-bar)) } }
  { \key d \major c'1 \key f \major \clef bass \time 3/4 d2. }
}
\score {
  \layout { \context { \Score \override BreakAlignment.break-align-orders =
    #(vector '(key-signature clef staff-bar) '(time-signature left-edge clef)
             '(staff-bar key-signature clef time-signature left-edge)) } }
  { \key d \major c'1 \key f \major \clef bass \time 3/4 d2. \break e2. \bar "|." }
}
"#;

#[test]
fn break_aligned_groups_are_positioned_as_lilypond_positions_them() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the positioning is unverified against it"
        );
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-positioning-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("positioning.ly"), POSITIONING_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages", "positioning.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 34, "{lilypond}");
    let want_errors: String = String::from_utf8(out.stderr)
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("programming error: "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(want_errors.lines().count(), 14, "{want_errors}");
    let (mut errors, mut dirs) = (String::new(), [0; 3]);
    for (n, line) in lines.iter().enumerate() {
        let Some(mut vm) = aligned_vm() else { return };
        eval(&mut vm, &format!("(define c '{line})"));
        let d = eval(&mut vm, "(caar c)").as_fixnum().unwrap();
        dirs[(d + 1) as usize] += 1;
        eval(&mut vm, "(aligned (car c))");
        break_dir(&mut vm, d);
        let got = caught(&mut vm, "(positions)");
        assert_eq!(got, caught(&mut vm, "(cadr c)"), "case {n}: {line}");
        errors.push_str(&diagnosed_lines(&mut vm));
    }
    // Ends of lines, unbroken columns and beginnings of lines.
    assert!(dirs.iter().all(|&k| k > 0), "{dirs:?}");
    assert_eq!(errors, want_errors);
}

/// The break-aligned interface's four callbacks.
const ANCHOR_CALLBACKS: [&str; 4] = [
    "ly:break-aligned-interface::calc-average-anchor",
    "ly:break-aligned-interface::calc-joint-anchor-alignment",
    "ly:break-aligned-interface::calc-extent-aligned-anchor",
    "ly:break-aligned-interface::calc-break-visibility",
];

/// Break-aligned pieces: `(anchored c)` makes a fresh piece of the case
/// `c` as [`ANCHORS_PROBE`] writes it, `(kind extent elements)` for a
/// group or `(extent extent alignment)` for an item, whose `X-extent` is
/// `extent`, and calls the callback of `kind` on it. An `average`
/// group's elements are each `(extent anchor)`, a `joint` group's
/// `(alignment)` and a `visibility` group's `(break-visibility)`.
const ANCHORED: &str = "(define (anchored c)
    (let ((me (fresh)) (kind (car c)))
      (put! me 'X-extent (cadr c))
      (if (eq? kind 'extent)
          (begin (put! me 'break-align-anchor-alignment (caddr c))
                 (ly:break-aligned-interface::calc-extent-aligned-anchor me))
          (begin
            (ly:grob-set-object!
             me 'elements
             (apply arr
                    (map (lambda (e)
                           (let ((elt (fresh)))
                             (case kind
                               ((average) (put! elt 'X-extent (car e))
                                          (put! elt 'break-align-anchor (cadr e)))
                               ((joint) (put! elt 'break-align-anchor-alignment (car e)))
                               (else (put! elt 'break-visibility (car e))))
                             elt))
                         (caddr c))))
            ((case kind
               ((average) ly:break-aligned-interface::calc-average-anchor)
               ((joint) ly:break-aligned-interface::calc-joint-anchor-alignment)
               (else ly:break-aligned-interface::calc-break-visibility))
             me)))))";

/// [`aligned_vm`] with [`ANCHORED`].
fn anchored_vm() -> Option<Vm> {
    let mut vm = aligned_vm()?;
    eval(&mut vm, ANCHORED);
    Some(vm)
}

#[test]
fn the_break_aligned_anchors_and_visibility_are_layout_s() {
    let Some(mut vm) = anchored_vm() else { return };
    for subr in ANCHOR_CALLBACKS {
        let got = caught(&mut vm, &format!("({subr} 'x)"));
        assert!(
            got.contains(subr)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{got}"
        );
        assert!(
            caught(&mut vm, &format!("({subr})")).contains("Wrong number of arguments"),
            "{subr}: arity"
        );
    }
    // A group of no elements.
    for (subr, want) in ANCHOR_CALLBACKS
        .iter()
        .zip(["0.0", "0", "0.0", "#(#f #f #f)"])
    {
        assert_eq!(
            caught(&mut vm, &format!("({subr} (piece 0))")),
            want,
            "{subr}"
        );
    }
}

#[test]
fn the_anchors_skip_what_is_no_number_and_clamp_to_the_elements_anchors() {
    for (case, want) in [
        // No real anchor, or a NaN: none.
        (
            "(average (0.0 . 4.0) (((0.0 . 2.0) x) ((0.0 . 2.0) +nan.0) ((0.0 . 2.0) ())))",
            "0.0",
        ),
        // An exact anchor; zero-width and empty extents normalize to no
        // finite point, so the center of the anchors.
        (
            "(average (0.0 . 4.0) (((1.0 . 1.0) 3) ((+inf.0 . -inf.0) -1.0)))",
            "1.0",
        ),
        // Unanimous after normalizing: the group's right end.
        (
            "(average (-1.0 . 5.0) (((0.0 . 2.0) 2.0) ((1.0 . 5.0) 5.0)))",
            "5.0",
        ),
        // Normalized -1 and 1 average to the group's center ...
        (
            "(average (0.0 . 4.0) (((0.0 . 2.0) 0.0) ((0.0 . 4.0) 4.0)))",
            "2.0",
        ),
        // ... clamped to the anchors' range, here its right end.
        (
            "(average (0.0 . 8.0) (((0.0 . 2.0) 0.0) ((0.0 . 2.0) 2.0)))",
            "2.0",
        ),
        // A group of no extent: NaN, which no clamp changes.
        ("(average (+inf.0 . -inf.0) (((0.0 . 2.0) 1.0)))", "+nan.0"),
        // What is no real is no alignment; the sign they agree on, else 0.
        ("(joint (0.0 . 1.0) ((x) (()) (+nan.0) (0)))", "0"),
        ("(joint (0.0 . 1.0) ((0) (0.25) (1)))", "1"),
        ("(joint (0.0 . 1.0) ((-0.5) (0) (-1)))", "-1"),
        ("(joint (0.0 . 1.0) ((-0.5) (2)))", "0"),
        // The extent's combination; 0 where both ends are infinite.
        ("(extent (1.0 . 3.0) 0.5)", "2.5"),
        ("(extent (1.0 . 3.0) x)", "2.0"),
        ("(extent (+inf.0 . -inf.0) 1)", "0.0"),
        ("(extent (-inf.0 . +inf.0) 1)", "0.0"),
        ("(extent (-inf.0 . 2.0) 1)", "+nan.0"),
        ("(extent (-inf.0 . 2.0) -1)", "-inf.0"),
        // Visible where one element is #t; what is no vector, or no #t,
        // is not.
        (
            "(visibility (0.0 . 1.0) ((#(#f #t #f)) (#(#t #f #f)) (all) (#(yes 1 ()))))",
            "#(#t #t #f)",
        ),
        ("(visibility (0.0 . 1.0) ((#(#f #f #t))))", "#(#f #f #t)"),
    ] {
        let Some(mut vm) = anchored_vm() else { return };
        assert_eq!(
            caught(&mut vm, &format!("(anchored '{case})")),
            want,
            "{case}"
        );
    }
    // A conflict ends the walk: the elements after it are not read.
    for (first, want) in [("-1", "0"), ("1", "read")] {
        let Some(mut vm) = anchored_vm() else { return };
        eval(
            &mut vm,
            &format!("(anchored '(joint (0.0 . 1.0) (({first}) (1) (0))))"),
        );
        eval(
            &mut vm,
            "(put! (piece 3) 'break-align-anchor-alignment (lambda (g) (error \"read\")))",
        );
        let got = caught(
            &mut vm,
            "(ly:break-aligned-interface::calc-joint-anchor-alignment (piece 0))",
        );
        assert!(got.contains(want), "{first}: {got}");
    }
    // A vector shorter than three: Guile's out-of-range error, as
    // scm_c_vector_ref's.
    let Some(mut vm) = anchored_vm() else { return };
    let got = caught(&mut vm, "(anchored '(visibility (0.0 . 1.0) ((#(#t #t)))))");
    assert!(got.contains("out of range"), "{got}");
}

/// A score whose break-aligned groups and items compute their anchors,
/// joint alignments and visibility by the four callbacks, each call
/// written as a line: `(input result)`, the input as [`ANCHORED`] takes
/// it (the extents and properties read before the call).
const ANCHORS_PROBE: &str = r#"\version "2.26.0"
#(define (elts g)
  (let ((a (ly:grob-object g 'elements))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (element name e)
  (case name
    ((average) (list (ly:grob-extent e e X) (ly:grob-property e 'break-align-anchor)))
    ((joint) (list (ly:grob-property e 'break-align-anchor-alignment)))
    (else (list (ly:grob-property e 'break-visibility)))))
#(define ((group-probe name proc) g)
  (let* ((input (list name (ly:grob-extent g g X) (map (lambda (e) (element name e)) (elts g))))
         (r (proc g)))
    (write (list input r))
    (newline)
    r))
#(define (extent-probe g)
  (let* ((input (list 'extent (ly:grob-extent g g X) (ly:grob-property g 'break-align-anchor-alignment)))
         (r (ly:break-aligned-interface::calc-extent-aligned-anchor g)))
    (write (list input r))
    (newline)
    r))
\layout {
  \context {
    \Score
    \override BreakAlignGroup.break-align-anchor =
      #(group-probe 'average ly:break-aligned-interface::calc-average-anchor)
    \override BreakAlignGroup.break-align-anchor-alignment =
      #(group-probe 'joint ly:break-aligned-interface::calc-joint-anchor-alignment)
    \override BreakAlignGroup.break-visibility =
      #(group-probe 'visibility ly:break-aligned-interface::calc-break-visibility)
    \override Clef.break-align-anchor = #extent-probe
    \override KeySignature.break-align-anchor = #extent-probe
    \override TimeSignature.break-align-anchor = #extent-probe
    \override LeftEdge.break-align-anchor = #extent-probe
  }
}
<<
  \new Staff {
    \override Staff.Clef.break-align-anchor-alignment = #LEFT
    \clef bass \key es \major \time 3/4 c4 d e \mark \default | \key a \major \clef treble
    f g a \break \time 2/4 b c' \bar "|." \mark \default }
  \new Staff {
    \override Staff.KeySignature.break-align-anchor-alignment = #CENTER
    \override Staff.Clef.break-visibility = #begin-of-line-visible
    c'4 d' e' \mark \default f' \once \omit Staff.Clef \clef alto g' a' \break b' c'' }
  \new Staff {
    \override Staff.Clef.break-align-anchor-alignment = #0.5
    \override Staff.TimeSignature.break-align-anchor-alignment = #-0.25
    \override Staff.KeySignature.break-align-anchor = #-2
    \key d \major c'4 d' e' f' g' a' \break b' c'' }
>>
\score { { \override Score.TimeSignature.break-align-anchor = #3 \key d \major c'1 \mark \default \key f \major d'1 } }
\score {
  <<
    \new Staff {
      \override Staff.Clef.break-align-anchor-alignment = #RIGHT
      \override Staff.TimeSignature.break-align-anchor-alignment = #LEFT
      \once \override Score.RehearsalMark.break-align-symbols = #'(clef)
      c'1 \mark \default \clef bass \time 3/4 d2.
      \once \override Score.RehearsalMark.break-align-symbols = #'(time-signature)
      \mark \default \clef treble \time 4/4 e'1
      \once \override Score.RehearsalMark.break-align-symbols = #'(key-signature)
      \mark \default \key b \major \clef "treble_8" f'1
      \once \override Score.RehearsalMark.break-align-symbols = #'(left-edge)
      \mark \default \break g'1 \mark \default
    }
    \new Staff {
      \override Staff.Clef.break-align-anchor-alignment = #LEFT
      \override Staff.TimeSignature.break-align-anchor-alignment = #0.75
      \override Staff.KeySignature.break-align-anchor-alignment = #-0.5
      c1 \clef alto d2. \clef "bass^15" \time 4/4 e1 \key b \major \clef tenor f1 g1 }
    \new Staff {
      \override Staff.Clef.break-align-anchor = #1.25
      \override Staff.KeySignature.break-align-anchor = #4
      c1 \clef percussion d2. \clef tab e1 \key b \major \clef soprano f1 g1 }
  >>
}
"#;

#[test]
fn break_aligned_anchors_and_visibility_are_lilypond_s() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the anchors are unverified against it"
        );
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-anchors-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anchors.ly"), ANCHORS_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages", "anchors.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    assert_eq!(lilypond.lines().count(), 232, "{lilypond}");
    // Each distinct call once, on fresh pieces: a VM's 24 hold five.
    let mut lines: Vec<&str> = lilypond.lines().collect();
    lines.sort_unstable();
    lines.dedup();
    let mut kinds = std::collections::BTreeMap::new();
    let mut vm = None;
    for (n, line) in lines.iter().enumerate() {
        if n % 5 == 0 {
            vm = anchored_vm();
        }
        let Some(vm) = vm.as_mut() else { return };
        eval(vm, &format!("(define c '{line})"));
        *kinds.entry(caught(vm, "(caar c)")).or_insert(0) += 1;
        let got = caught(vm, "(anchored (car c))");
        assert_eq!(got, caught(vm, "(cadr c)"), "case {n}: {line}");
    }
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        [
            ("average".to_string(), 12),
            ("extent".to_string(), 10),
            ("joint".to_string(), 12),
            ("visibility".to_string(), 61)
        ]
    );
}

#[test]
fn the_right_column_is_the_lowest_ranked_of_the_right_items_columns() {
    let Some(mut vm) = grobs_vm() else { return };
    // Things 1, 2 and 3 on columns ranked 3, 1 and 2; group 0, a spanner,
    // on column 3, ranked 0; thing 4 on no column.
    put_on(&mut vm, 0, 3, &[1]);
    put_on(&mut vm, 1, 1, &[2]);
    put_on(&mut vm, 2, 2, &[3]);
    put_on(&mut vm, 3, 0, &[]);
    eval(&mut vm, "(ly:grob-set-parent! (group 0) 0 (column 3))");
    let me = grob_of(&mut vm, "(thing 0)");
    let right = |vm: &mut Vm| right_column(vm, me).unwrap();
    assert_eq!(right(&mut vm), None);
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 0) 'right-items
           (arr (thing 1) (group 0) (thing 4) (thing 3) (thing 2)))",
    );
    assert_eq!(right(&mut vm), Some(grob_of(&mut vm, "(column 1)")));

    // Ranks from INT_MAX, VPOS's too, are none.
    put_on(&mut vm, 1, 2_147_483_647, &[]);
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 0) 'right-items (arr (thing 2) (thing 4)))",
    );
    assert_eq!(right(&mut vm), None);
    put_on(&mut vm, 1, 2_147_483_646, &[]);
    assert_eq!(right(&mut vm), Some(grob_of(&mut vm, "(column 1)")));
    put_on(&mut vm, 1, u64::MAX, &[]);
    assert_eq!(right(&mut vm), None);

    put_on(&mut vm, 1, 1, &[]);
    eval(&mut vm, "(ly:grob-suicide! (thing 0))");
    assert_eq!(right(&mut vm), None);
}

#[test]
fn the_left_column_is_a_live_item_s_column() {
    let Some(mut vm) = grobs_vm() else { return };
    put_on(&mut vm, 0, 0, &[1]);
    eval(&mut vm, "(ly:grob-set-parent! (group 0) 0 (column 0))");
    let left = |vm: &mut Vm, t: &str| {
        let g = grob_of(vm, t);
        left_column(vm, g)
    };
    let col = grob_of(&mut vm, "(column 0)");
    assert_eq!(left(&mut vm, "(thing 1)"), Some(col));
    assert_eq!(left(&mut vm, "(column 0)"), Some(col));
    assert_eq!(left(&mut vm, "(thing 4)"), None);
    assert_eq!(left(&mut vm, "(group 0)"), None);
    eval(&mut vm, "(ly:grob-suicide! (thing 1))");
    assert_eq!(left(&mut vm, "(thing 1)"), None);
}

#[test]
fn the_note_columns_are_the_items_and_their_separation_items() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(ly:grob-set-object! (sep 1) 'elements (arr (note 2)))
         (ly:grob-set-object! (sep 0) 'elements (arr (note 1) (bit 0) (sep 1)))
         (ly:grob-set-object! (thing 0) 'left-items (arr (note 0) (thing 1) (sep 0)))
         (ly:grob-set-object! (thing 0) 'right-items (arr (sep 1) (sep 2) (note 0)))",
    );
    let [me, other] = ["(thing 0)", "(thing 1)"].map(|t| grob_of(&mut vm, t));
    let notes = |vm: &mut Vm, ns: &[usize]| -> Vec<Value> {
        ns.iter()
            .map(|n| grob_of(vm, &format!("(note {n})")))
            .collect()
    };
    let want = notes(&mut vm, &[0, 1, 2]);
    assert_eq!(left_note_columns(&mut vm, me).unwrap(), want);
    let want = notes(&mut vm, &[2, 0]);
    assert_eq!(right_note_columns(&mut vm, me).unwrap(), want);
    assert_eq!(left_note_columns(&mut vm, other).unwrap(), []);
    assert_eq!(right_note_columns(&mut vm, other).unwrap(), []);
}

/// Column 0 at 10 on X and column 1, its piece at the beginning of a
/// line, at 20; things 1 to 4 on column 0 at 0 with the X extents
/// `exts`, all but thing 4 with a `space-alist`; bit 0 on column 0 with
/// an empty X extent and a `space-alist`. `(thing 0)` is the spacing
/// grob.
fn break_aligned_vm(exts: [&str; 4]) -> Option<Vm> {
    let mut vm = grobs_vm()?;
    eval(
        &mut vm,
        &format!(
            "(put! (column 0) 'X-offset 10)
             (put! (column 1) 'X-offset 20)
             (define (aligned! g ext alist)
               (ly:grob-set-parent! g 0 (column 0))
               (put! g 'X-offset 0)
               (put! g 'X-extent ext)
               (put! g 'space-alist alist)
               g)
             (define alist '((first-note fixed-space . 1)))
             (aligned! (thing 1) '{} alist)
             (aligned! (thing 2) '{} alist)
             (aligned! (thing 3) '{} alist)
             (aligned! (thing 4) '{} '())
             (aligned! (bit 0) '(1 . 0) alist)",
            exts[0], exts[1], exts[2], exts[3]
        ),
    );
    Some(vm)
}

/// [`extremal_break_aligned_grob`] of `(thing 0)` towards `d` for
/// `break_dir`, with `elts` as its `left-break-aligned` or
/// `right-break-aligned`.
fn extremal(vm: &mut Vm, d: i32, break_dir: i32, elts: &str) -> Option<(Value, Interval)> {
    let side = if d == -1 { "left" } else { "right" };
    eval(
        vm,
        &format!("(ly:grob-set-object! (thing 0) '{side}-break-aligned (arr {elts}))"),
    );
    let me = grob_of(vm, "(thing 0)");
    extremal_break_aligned_grob(vm, me, d, break_dir)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
}

#[test]
fn the_extremal_break_aligned_grob_sticks_out_furthest_towards_the_spacing() {
    let Some(mut vm) = break_aligned_vm(["(0 . 2)", "(-2 . 3)", "(-1 . 3)", "(-5 . 9)"]) else {
        return;
    };
    let [t1, t2, t3] = ["(thing 1)", "(thing 2)", "(thing 3)"].map(|t| grob_of(&mut vm, t));
    let all = "(thing 1) (thing 2) (thing 3) (thing 4) (bit 0)";
    // On the left, the furthest right; of things 2 and 3, the last; thing
    // 4 has no space-alist and bit 0 an empty extent. Relative to column 0.
    assert_eq!(
        extremal(&mut vm, -1, 0, all),
        Some((t3, Interval::new(-1.0, 3.0)))
    );
    assert_eq!(
        extremal(&mut vm, -1, 0, "(thing 1) (thing 4) (bit 0)"),
        Some((t1, Interval::new(0.0, 2.0)))
    );
    // On the right, the furthest left.
    assert_eq!(
        extremal(&mut vm, 1, 0, all),
        Some((t2, Interval::new(-2.0, 3.0)))
    );
    // None that counts, or none at all.
    assert_eq!(extremal(&mut vm, 1, 0, "(thing 4) (bit 0)"), None);
    assert_eq!(extremal(&mut vm, -1, 0, ""), None);
    // What is no item is skipped; first, it leaves no column, so the
    // extents are from the root, column 0's 10 added.
    assert_eq!(
        extremal(&mut vm, -1, 0, "(group 0) (thing 1) (group 1)"),
        Some((t1, Interval::new(10.0, 12.0)))
    );
}

#[test]
fn the_extremal_break_aligned_grob_is_taken_broken_as_the_break_direction() {
    let Some(mut vm) = break_aligned_vm(["(0 . 2)", "(-2 . 3)", "(-1 . 3)", "(-5 . 9)"]) else {
        return;
    };
    // Piece 0 is thing 1 at the beginning of a line, at 1 on column 0,
    // with thing 1's X extent and space-alist; thing 2 is not broken.
    let [t1, p0] = ["(thing 1)", "(piece 0)"].map(|t| grob_of(&mut vm, t));
    eval(
        &mut vm,
        "(aligned! (piece 0) '(0 . 2) alist)
         (put! (piece 0) 'X-offset 1)",
    );
    grob::with_grob_mut(vm.heap(), t1, |x| x.broken = [None, Some(p0)]);
    grob::with_grob_mut(vm.heap(), p0, |x| x.original = Some(t1));
    let both = "(thing 1) (thing 2)";
    // Column 0 is not broken: no column, extents from the root.
    assert_eq!(
        extremal(&mut vm, -1, 1, both),
        Some((p0, Interval::new(11.0, 13.0)))
    );
    // Piece 0 itself is at the beginning of a line.
    assert_eq!(
        extremal(&mut vm, -1, 1, "(piece 0) (thing 2)"),
        Some((p0, Interval::new(11.0, 13.0)))
    );
    // Column 1 is column 0 at the beginning of a line, piece 0 on it.
    let [c0, c1] = ["(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    grob::with_grob_mut(vm.heap(), c0, |x| x.broken = [None, Some(c1)]);
    grob::with_grob_mut(vm.heap(), c1, |x| x.original = Some(c0));
    eval(&mut vm, "(ly:grob-set-parent! (piece 0) 0 (column 1))");
    assert_eq!(
        extremal(&mut vm, -1, 1, both),
        Some((p0, Interval::new(1.0, 3.0)))
    );
    // At the end of a line, thing 1 has no piece.
    assert_eq!(extremal(&mut vm, -1, -1, both), None);
}

#[test]
fn the_spacing_options_are_read_from_the_grob_s_properties() {
    let Some(mut vm) = grobs_vm() else { return };
    let me = grob_of(&mut vm, "(thing 0)");
    let read = |vm: &mut Vm| {
        let mut o = SpacingOptions::default();
        o.init_from_grob(vm, me)
            .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
        o
    };
    let constructed = SpacingOptions {
        packed: false,
        stretch_uniformly: false,
        float_nonmusical_columns: false,
        float_grace_columns: false,
        shortest_duration_space: 2.0,
        increment: 1.2,
        global_shortest: Rational::new(1, 8),
    };
    assert_eq!(SpacingOptions::default(), constructed);
    // Unset numbers are 1, not the constructor's.
    assert_eq!(
        read(&mut vm),
        SpacingOptions {
            shortest_duration_space: 1.0,
            increment: 1.0,
            ..constructed
        }
    );
    eval(
        &mut vm,
        "(put! (thing 0) 'spacing-increment 1.5)
         (put! (thing 0) 'shortest-duration-space 3)
         (put! (thing 0) 'packed-spacing #t)
         (put! (thing 0) 'uniform-stretching 'yes)
         (put! (thing 0) 'strict-note-spacing #t)
         (put! (thing 0) 'strict-grace-spacing #t)
         (put! (thing 0) 'common-shortest-duration (ly:make-moment 3 16 1 32))",
    );
    let o = SpacingOptions {
        packed: true,
        stretch_uniformly: false,
        float_nonmusical_columns: true,
        float_grace_columns: true,
        shortest_duration_space: 3.0,
        increment: 1.5,
        global_shortest: Rational::new(3, 16),
    };
    assert_eq!(read(&mut vm), o);
    // A main part of 0 gives the grace part.
    eval(
        &mut vm,
        "(put! (thing 0) 'common-shortest-duration (ly:make-moment 0 -1/32))",
    );
    assert_eq!(
        read(&mut vm),
        SpacingOptions {
            global_shortest: Rational::new(-1, 32),
            ..o
        }
    );
    // What is no number or moment is read as its default.
    eval(
        &mut vm,
        "(put! (thing 0) 'spacing-increment \"1.5\")
         (put! (thing 0) 'uniform-stretching #t)
         (put! (thing 0) 'common-shortest-duration 1/4)",
    );
    assert_eq!(
        read(&mut vm),
        SpacingOptions {
            stretch_uniformly: true,
            increment: 1.0,
            global_shortest: Rational::new(1, 8),
            ..o
        }
    );
}

#[test]
fn the_duration_space_is_linear_below_the_shortest_and_logarithmic_above() {
    let mut vm = Vm::new();
    let o = SpacingOptions::default();
    let space = |vm: &mut Vm, o: &SpacingOptions, n: i64, d: i64| {
        o.get_duration_space(vm, Rational::new(n, d))
            .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
    };
    // (2 + ratio - 1) × 1.2 below 1/8, (2 + log₂ ratio) × 1.2 from it.
    assert_eq!(space(&mut vm, &o, 1, 16), 1.5 * 1.2);
    assert_eq!(space(&mut vm, &o, 1, 32), 1.25 * 1.2);
    assert_eq!(space(&mut vm, &o, 3, 32), 1.75 * 1.2);
    assert_eq!(space(&mut vm, &o, 1, 8), 2.0 * 1.2);
    assert_eq!(space(&mut vm, &o, 1, 4), 3.0 * 1.2);
    assert_eq!(space(&mut vm, &o, 1, 1), 5.0 * 1.2);
    assert_eq!(space(&mut vm, &o, 1, 0), f64::INFINITY);

    // Dots, double dots and tuplets from 1/1024 to 8 against the shortest
    // of each: the platform's log₂, on macOS LilyPond 2.26.0's.
    let factors = [
        (1, 1),
        (3, 2),
        (7, 4),
        (15, 8),
        (2, 3),
        (4, 5),
        (4, 7),
        (8, 9),
        (16, 15),
    ];
    let mut n = 0;
    for (a, b) in factors {
        for (c, d) in factors {
            for k in 0..=13 {
                let o = SpacingOptions {
                    global_shortest: Rational::new(c, d * 1024),
                    ..o
                };
                let dur = Rational::new(a << k, b * 1024);
                let ratio = ((a * d) << k) as f64 / (b * c) as f64;
                let want = if ratio < 1.0 {
                    (2.0 + ratio - 1.0) * 1.2
                } else {
                    (2.0 + ratio.log2()) * 1.2
                };
                let got = o.get_duration_space(&mut vm, dur).unwrap();
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{a}/{b} × 2^{k} over {c}/{d}"
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, 1134);

    // A ratio that does not fit is an overflow.
    let o = SpacingOptions {
        global_shortest: Rational::new(1, i64::MAX),
        ..o
    };
    let e = o
        .get_duration_space(&mut vm, Rational::new(i64::MAX, 1))
        .unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "Throw to key `numerical-overflow' with args \
         `(\"get_duration_space\" \"Numerical overflow\" () #f)'."
    );
}

/// A spring of LilyPond's fields: `ideal`, `min`, the inverse stretch and
/// compress strengths and the blocking force.
fn spring(ideal: f64, min: f64, stretch: f64, compress: f64, blocking: f64) -> Spring {
    Spring {
        ideal_distance: ideal,
        min_distance: min,
        inverse_stretch_strength: stretch,
        inverse_compress_strength: compress,
        blocking_force: blocking,
    }
}

/// The standard spring from column 0 to column 1 of [`columns_vm`]
/// (4 apart) after `setup`, of `(thing 1)`'s `spacing-increment`.
fn standard_spring(setup: &str, options: &SpacingOptions) -> Option<(Spring, String)> {
    let mut vm = columns_vm(setup)?;
    let [me, l, r] = ["(thing 1)", "(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let s = standard_breakable_column_spacing(&mut vm, me, l, r, options)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some((s, diagnosed(&mut vm)))
}

#[test]
fn the_standard_spring_is_by_the_measure_between_breakable_columns() {
    let o = SpacingOptions::default();
    let breakable = "(put! (column 0) 'line-break-permission 'allow)
                     (put! (column 1) 'line-break-permission 'force)";
    // 1.2 × (3/4 ÷ 1/8) × 0.8 past the minimum distance, stretching as
    // that alone, compressing as the room above the minimum.
    let space = 1.2 * 6.0 * 0.8;
    let Some(got) = standard_spring(
        &format!(
            "{breakable}
             (put! (column 0) 'measure-length (ly:make-moment 3/4))
             (put! (column 1) 'measure-length (ly:make-moment 1/16))
             (put! (thing 1) 'spacing-increment 1.2)"
        ),
        &o,
    ) else {
        return;
    };
    let ideal = 4.0 + space;
    assert_eq!(
        got,
        (spring(ideal, 4.0, space, ideal - 4.0, -1.0), "()".into())
    );

    // Without a measure length or an increment: 1 × 8 × 0.8, here by
    // whole notes.
    let whole = SpacingOptions {
        global_shortest: Rational::new(1, 1),
        ..o
    };
    let (got, _) = standard_spring(
        &format!("{breakable} (put! (column 0) 'measure-length 3/4)"),
        &whole,
    )
    .unwrap();
    assert_eq!(got, spring(4.8, 4.0, 0.8, 4.8 - 4.0, -1.0));

    // A space below 0 is insane: the spring keeps its 1, now below the
    // minimum distance, and stretches as long as it is.
    let (got, errors) = standard_spring(
        &format!("{breakable} (put! (thing 1) 'spacing-increment -1)"),
        &o,
    )
    .unwrap();
    assert_eq!(got, spring(1.0, 4.0, 1.0, 0.0, 3.0));
    assert_eq!(
        errors,
        "(\"insane spring distance requested, ignoring it\" \"insane spring constant\")"
    );

    // A ratio that does not fit is an overflow.
    let Some(mut vm) = columns_vm(&format!(
        "{breakable} (put! (column 0) 'measure-length (ly:make-moment 2))"
    )) else {
        return;
    };
    let [me, l, r] = ["(thing 1)", "(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let tiny = SpacingOptions {
        global_shortest: Rational::new(1, i64::MAX),
        ..o
    };
    let e = standard_breakable_column_spacing(&mut vm, me, l, r, &tiny).unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "Throw to key `numerical-overflow' with args \
         `(\"standard_breakable_column_spacing\" \"Numerical overflow\" () #f)'."
    );
}

#[test]
fn the_standard_spring_is_by_the_duration_between_other_columns() {
    let o = SpacingOptions::default();
    let one_breakable = "(put! (column 0) 'line-break-permission 'allow)
                         (put! (column 0) 'measure-length (ly:make-moment 1))";
    // At the same moment, 0.5 past the minimum distance, stretching as
    // long as it is.
    let Some((got, errors)) = standard_spring(one_breakable, &o) else {
        return;
    };
    assert_eq!(got, spring(4.5, 4.0, 4.5, 0.5, -1.0));
    assert_eq!(errors, "()");
    // A quarter later, the quarter's space, (2 + log₂ 2) × 1.2, past it;
    // the right column's moment first.
    for (when, space) in [
        ("(ly:make-moment 1/4)", (2.0 + 1.0) * 1.2),
        // Of no main part, the space of 0: (2 + 0 - 1) × 1.2.
        ("(ly:make-moment 0 -1/16)", 1.0 * 1.2),
        // The main part only.
        ("(ly:make-moment 1 16 1 32)", (2.0 + 0.5 - 1.0) * 1.2),
    ] {
        let (got, _) = standard_spring(
            &format!("{one_breakable} (put! (column 1) 'when {when})"),
            &o,
        )
        .unwrap();
        let ideal = 4.0 + space;
        assert_eq!(got, spring(ideal, 4.0, ideal, ideal - 4.0, -1.0), "{when}");
    }
    // Columns that never meet are 0 apart.
    let (got, _) = standard_spring(
        "(put! (bit 1) 'Y-extent '(2 . 3))
         (put! (column 0) 'when (ly:make-moment 1/2))
         (put! (column 1) 'when (ly:make-moment 1))",
        &o,
    )
    .unwrap();
    let ideal = (2.0 + 2.0) * 1.2;
    assert_eq!(got, spring(ideal, 0.0, ideal, ideal, -1.0));
}

/// Columns 0 to 3 of ranks 0 to 3 on `(group 0)`, its `columns`, after
/// `setup`; the note spring from column `l` to column `r` and the
/// programming errors.
fn note_spring(
    setup: &str,
    l: usize,
    r: usize,
    options: &SpacingOptions,
) -> Option<(Spring, String)> {
    let mut vm = grobs_vm()?;
    for n in 0..4 {
        let col = grob_of(&mut vm, &format!("(column {n})"));
        paper_column::set_rank(vm.heap(), col, n as u64);
    }
    eval(
        &mut vm,
        "(for-each (lambda (n) (ly:grob-set-parent! (column n) 0 (group 0))) '(0 1 2 3))
         (ly:grob-set-object! (group 0) 'columns
                              (arr (column 0) (column 1) (column 2) (column 3)))",
    );
    eval(&mut vm, setup);
    let [me, lc, rc] = [
        "(thing 1)",
        &format!("(column {l})"),
        &format!("(column {r})"),
    ]
    .map(|t| grob_of(&mut vm, t));
    let s = note_spacing(&mut vm, me, lc, rc, options)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some((s, diagnosed(&mut vm)))
}

#[test]
fn the_note_spring_is_the_part_of_the_shortest_playing_note_s() {
    let o = SpacingOptions::default();
    // Six sixteenths of a sixteenth's (2 + 1/2 - 1) × 1.2 against the
    // eighth, at least six times 1.2, stretching as six times the rest.
    let Some(got) = note_spring(
        "(put! (column 1) 'when (ly:make-moment 1/4))
         (put! (column 1) 'shortest-playing-duration 1/16)
         (put! (column 2) 'when (ly:make-moment 5/8))",
        1,
        2,
        &o,
    ) else {
        return;
    };
    let len = 1.5 * 1.2;
    let (ideal, min) = (6.0 * len, 6.0 * 1.2);
    let want = spring(ideal, min, 6.0 * (len - 1.2), ideal - min, -1.0);
    assert_eq!(got, (want, "()".into()));

    // The measure caps the time and the playing note: the nearest
    // measure length at or before the left column's rank, column 0's,
    // not column 2's. One quarter of a quarter's (2 + 1) × 1.2.
    let (got, _) = note_spring(
        "(put! (column 0) 'measure-length (ly:make-moment 1/4))
         (put! (column 2) 'measure-length (ly:make-moment 1/32))
         (put! (column 1) 'shortest-playing-duration 1/2)
         (put! (column 2) 'when (ly:make-moment 1/2))",
        1,
        2,
        &o,
    )
    .unwrap();
    let len = 3.0 * 1.2;
    assert_eq!(got, spring(len, 1.2, len - 1.2, len - 1.2, -1.0));
    // So does column 1's own before column 0's; and a rank beyond the
    // system's columns looks from the last of them down.
    for setup in [
        "(put! (column 0) 'measure-length (ly:make-moment 1/32))
         (put! (column 1) 'measure-length (ly:make-moment 1/4))",
        "(put! (column 0) 'measure-length (ly:make-moment 1/4))
         (ly:grob-set-object! (group 0) 'columns (arr (column 0)))",
    ] {
        let (got, _) = note_spring(
            &format!(
                "{setup}
                 (put! (column 1) 'shortest-playing-duration 1/2)
                 (put! (column 2) 'when (ly:make-moment 1/2))"
            ),
            1,
            2,
            &o,
        )
        .unwrap();
        assert_eq!(got, spring(len, 1.2, len - 1.2, len - 1.2, -1.0), "{setup}");
    }
    // Without a system nothing caps it: two halves of a half's, (2 + 2) ×
    // 1.2, stretching as twice the rest.
    let (got, _) = note_spring(
        "(put! (column 0) 'measure-length (ly:make-moment 1/4))
         (ly:grob-set-parent! (column 1) 0 (thing 2))
         (put! (column 1) 'shortest-playing-duration 1/2)
         (put! (column 2) 'when (ly:make-moment 1))",
        1,
        2,
        &o,
    )
    .unwrap();
    let len = 4.0 * 1.2;
    let (ideal, min) = (2.0 * len, 2.0 * 1.2);
    assert_eq!(
        got,
        spring(ideal, min, 2.0 * (len - 1.2), ideal - min, -1.0)
    );

    // Stretching as the part of the space above the increment: two
    // eighths of a quarter's (0.5 + 1) × 2 above 2; at least 0.1 times
    // the part, half an eighth's (0.5 + 0) × 2 below it.
    let wide = SpacingOptions {
        shortest_duration_space: 0.5,
        increment: 2.0,
        ..o
    };
    let (got, _) = note_spring(
        "(put! (column 1) 'shortest-playing-duration 1/4)
         (put! (column 2) 'when (ly:make-moment 1/16))",
        1,
        2,
        &wide,
    )
    .unwrap();
    let len = (0.5 + 1.0) * 2.0;
    let (ideal, min) = (0.25 * len, 0.25 * 2.0);
    assert_eq!(got, spring(ideal, min, 0.25 * 1.0, ideal - min, -1.0));
    let (got, _) = note_spring(
        "(put! (column 1) 'shortest-playing-duration 1/8)
         (put! (column 2) 'when (ly:make-moment 1/16))",
        1,
        2,
        &wide,
    )
    .unwrap();
    let len = 0.5 * 2.0;
    let (ideal, min) = (0.5 * len, 0.5 * 2.0);
    // The minimum above the ideal: it cannot compress, and blocks where
    // stretching 0.5 × 0.1 reaches it.
    assert_eq!(
        got,
        spring(ideal, min, 0.5 * 0.1, 0.0, (min - ideal) / (0.5 * 0.1))
    );
}

#[test]
fn a_note_spring_without_a_ruling_note_is_by_a_whole_one() {
    let o = SpacingOptions::default();
    // Not above 0, or no exact number: a programming error at the left
    // column's moment, and the whole note's (2 + 3) × 1.2, a quarter of it.
    for value in ["0", "-1/4", "0.25", "'x"] {
        let Some((got, errors)) = note_spring(
            &format!(
                "(put! (column 1) 'when (ly:make-moment 1/4 -1/8))
                 (put! (column 1) 'shortest-playing-duration {value})
                 (put! (column 2) 'when (ly:make-moment 1/2 -1/8))"
            ),
            1,
            2,
            &o,
        ) else {
            return;
        };
        // A left column among grace notes has no spring of its own.
        assert_eq!(got, Spring::default(), "{value}");
        assert_eq!(
            errors, "(\"cannot find a ruling note at: 1/4G-1/8\")",
            "{value}"
        );
    }
    let (got, errors) = note_spring(
        "(put! (column 1) 'when (ly:make-moment 1/4))
         (put! (column 2) 'when (ly:make-moment 1/2))",
        1,
        2,
        &o,
    )
    .unwrap();
    let len = 5.0 * 1.2;
    let (ideal, min) = (0.25 * len, 0.25 * 1.2);
    assert_eq!(
        got,
        spring(ideal, min, 0.25 * (len - 1.2), ideal - min, -1.0)
    );
    assert_eq!(errors, "(\"cannot find a ruling note at: 1/4\")");
}

#[test]
fn a_grace_note_spring_is_the_grace_spacing_s() {
    let o = SpacingOptions::default();
    let grace = "(put! (column 1) 'shortest-playing-duration 1/4)
                 (put! (column 1) 'when (ly:make-moment 0 -1/16))";
    // The grace spacing's space of the sixteenth, (1.6 + 1/2 - 1) × 0.8,
    // at least 0.8, stretching as 0.4.
    let Some((got, errors)) = note_spring(
        &format!(
            "{grace}
             (put! (thing 2) 'spacing-increment 0.8)
             (put! (thing 2) 'shortest-duration-space 1.6)
             (ly:grob-set-object! (column 1) 'grace-spacing (thing 2))"
        ),
        1,
        2,
        &o,
    ) else {
        return;
    };
    let len = (1.6 + 0.5 - 1.0) * 0.8;
    assert_eq!(got, spring(len, 0.8, 0.4, len - 0.8, -1.0));
    assert_eq!(errors, "()");
    // Without one, half the shortest note's, (2 × 1.2) / 2, at least 0.6.
    for setup in ["", "(ly:grob-set-object! (column 1) 'grace-spacing 'x)"] {
        let (got, _) = note_spring(&format!("{grace} {setup}"), 1, 2, &o).unwrap();
        assert_eq!(got, spring(1.2, 0.6, 1.2, 0.6, -1.0), "{setup}");
    }
    // Neither main nor grace time between them: `Spring ()`.
    let (got, _) =
        note_spring("(put! (column 1) 'shortest-playing-duration 1/4)", 1, 2, &o).unwrap();
    assert_eq!(got, Spring::default());
}

/// Scheme for the stems: `(on! g col)` puts `g` on `col` at X 0; `(head!
/// h col pos)` a head 1.3 wide on `col` at staff position `pos`; `(stem!
/// s note dir ext . heads)` the stem `s` of `note`, in its column, of
/// direction `dir`, a quarter's, along `ext` at Y 0, 1.5 thick, with the
/// heads `heads` (the note's too). The layout's line thickness is 0.1.
const STEMS: &str = "(define (on! g col)
                       (ly:grob-set-parent! g 0 col)
                       (put! g 'X-offset 0)
                       g)
                     (define (head! h col pos)
                       (on! h col)
                       (put! h 'Y-offset (exact->inexact (/ pos 2)))
                       (put! h 'X-extent '(0 . 1.3))
                       h)
                     (define (stem! s note dir ext . heads)
                       (ly:grob-set-object! note 'stem s)
                       (on! s (ly:grob-parent note 0))
                       (put! s 'direction dir)
                       (put! s 'duration-log 2)
                       (put! s 'Y-offset 0)
                       (put! s 'Y-extent ext)
                       (put! s 'thickness 1.5)
                       (ly:grob-set-object! s 'note-heads (apply arr heads))
                       (ly:grob-set-object! note 'note-heads (apply arr heads)))";

/// `f` of the grob `text` evaluates to, or LilyPond's error's text.
fn of<T>(vm: &mut Vm, text: &str, f: fn(&mut dyn Host, Value) -> Result<T, Exception>) -> T {
    let g = grob_of(vm, text);
    f(vm, g).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
}

/// A VM with [`STEMS`], note 0 on column 0 and note 1 on column 1, and
/// `setup` evaluated.
fn stem_vm(setup: &str) -> Option<Vm> {
    let mut vm = grobs_vm()?;
    set_layout(&mut vm);
    eval(&mut vm, STEMS);
    eval(
        &mut vm,
        "(on! (note 0) (column 0)) (on! (note 1) (column 1))",
    );
    eval(&mut vm, setup);
    Some(vm)
}

#[test]
fn the_first_head_is_the_stem_s_end_and_the_positions_its_heads() {
    // Heads 0, 1 and 2 at positions 2, -1.4 (rounding to -1) and -1: the
    // lowest is the first of the unison, the highest the last.
    let Some(mut vm) = stem_vm(
        "(stem! (stem 0) (note 0) 1 '(0 . 3) (head! (head 0) (column 0) 2)
                (head! (head 1) (column 0) -1.4) (head! (head 2) (column 0) -1))",
    ) else {
        return;
    };
    let [h0, h1, h2] = ["(head 0)", "(head 1)", "(head 2)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(
        of(&mut vm, "(stem 0)", stem::extremal_heads),
        [Some(h1), Some(h0)]
    );
    assert_eq!(of(&mut vm, "(stem 0)", stem::first_head), Some(h1));
    assert_eq!(of(&mut vm, "(note 0)", note_column::first_head), Some(h1));
    assert_eq!(
        of(&mut vm, "(stem 0)", stem::head_positions),
        Interval::new(-1.4, 2.0)
    );
    eval(&mut vm, "(put! (stem 0) 'direction -1)");
    assert_eq!(of(&mut vm, "(stem 0)", stem::first_head), Some(h0));
    for dir in ["0", "'()", "2"] {
        eval(&mut vm, &format!("(put! (stem 0) 'direction {dir})"));
        assert_eq!(of(&mut vm, "(stem 0)", stem::first_head), None, "{dir}");
    }
    // No heads, no stem.
    eval(&mut vm, "(ly:grob-set-object! (stem 0) 'note-heads (arr))");
    assert_eq!(of(&mut vm, "(stem 0)", stem::extremal_heads), [None, None]);
    assert!(of(&mut vm, "(stem 0)", stem::head_positions).is_empty());
    assert_eq!(of(&mut vm, "(note 1)", note_column::first_head), None);
    assert_eq!(of(&mut vm, "(note 1)", note_column::get_stem), None);
    eval(&mut vm, "(ly:grob-set-object! (note 1) 'stem (group 0))");
    assert_eq!(of(&mut vm, "(note 1)", note_column::get_stem), None);

    // A unison of three at -1: the first and the last.
    let mut vm = stem_vm(
        "(stem! (stem 0) (note 0) 1 '(0 . 3) (head! (head 0) (column 0) -1)
                (head! (head 1) (column 0) -1.4) (head! (head 2) (column 0) -1))",
    )
    .unwrap();
    assert_eq!(
        of(&mut vm, "(stem 0)", stem::extremal_heads),
        [Some(h0), Some(h2)]
    );
}

/// Where stem 0 attaches to head 0, 1.3 wide, in a [`stem_vm`] of an
/// up-stem, after `setup`; centered on an invisible stem if `center`.
fn attached(setup: &str, center: bool) -> Option<f64> {
    let mut vm = stem_vm(&format!(
        "(stem! (stem 0) (note 0) 1 '(0 . 3) (head! (head 0) (column 0) 0)) {setup}"
    ))?;
    let [st, h] = ["(stem 0)", "(head 0)"].map(|t| grob_of(&mut vm, t));
    let r = stem::internal_calc_stem_offset_from_head(&mut vm, st, h, center)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some(r)
}

#[test]
fn the_stem_attaches_at_the_head_s_attachment_less_half_its_thickness() {
    let at = |a: &str| format!("(put! (head 0) 'stem-attachment '{a})");
    let Some(got) = attached(&at("(1 . 0.5)"), false) else {
        return;
    };
    let rule = 1.5 * 0.1;
    // Up: the right edge less half the thickness.
    assert_eq!(got, 1.3 - rule * 0.5);
    // Down at the left edge: plus half of it.
    let down = format!("(put! (stem 0) 'direction -1) {}", at("(-1 . -0.5)"));
    assert_eq!(attached(&down, false).unwrap(), rule * 0.5);
    // A third of the way along a head from 0.2: the linear combination,
    // fused.
    let third = format!(
        "{} (put! (head 0) 'X-extent '(0.2 . 1.3))",
        at("(0.3 . 0.5)")
    );
    let comb = (1.0f64 - 0.3).mul_add(0.2, (0.3 + 1.0) * 1.3) * 0.5;
    assert_eq!(attached(&third, false).unwrap(), comb - rule * 0.5);
    // A mensural head's correction is a quarter of it the other way; the
    // petrucci styles' and neomensural's none, as one within 10⁻³ of the
    // middle. A style that is no symbol is the default.
    let styled = |style: &str| format!("{third} (put! (head 0) 'style {style})");
    assert_eq!(
        attached(&styled("'mensural"), false).unwrap(),
        comb + rule / 4.0
    );
    for style in [
        "'neomensural",
        "'petrucci",
        "'blackpetrucci",
        "'semipetrucci",
    ] {
        assert_eq!(attached(&styled(style), false).unwrap(), comb, "{style}");
    }
    assert_eq!(
        attached(&styled("\"mensural\""), false).unwrap(),
        comb - rule * 0.5
    );
    let near = format!(
        "{} (put! (head 0) 'X-extent '(0.2 . 1.3))",
        at("(0.001 . 0)")
    );
    let want = (1.0f64 - 0.001).mul_add(0.2, (0.001 + 1.0) * 1.3) * 0.5;
    assert_eq!(attached(&near, false).unwrap(), want);
    // No pair of numbers: the middle; an empty head: 0, with the
    // thickness's correction.
    assert_eq!(attached(&at("(1 . x)"), false).unwrap(), 0.65);
    assert_eq!(attached("", false).unwrap(), 0.65);
    let empty = format!(
        "{} (put! (head 0) 'X-extent '(+inf.0 . -inf.0))",
        at("(1 . 0.5)")
    );
    assert_eq!(attached(&empty, false).unwrap(), -rule * 0.5);
    // Centered on an invisible stem only if asked.
    let invisible = format!("{} (put! (stem 0) 'duration-log 0)", at("(1 . 0.5)"));
    assert_eq!(attached(&invisible, true).unwrap(), 0.65);
    assert_eq!(attached(&invisible, false).unwrap(), 1.3 - rule * 0.5);
    assert_eq!(attached(&at("(1 . 0.5)"), true).unwrap(), 1.3 - rule * 0.5);
    // The thickness in line thicknesses; Guile's error for no number.
    let mut vm = stem_vm("(stem! (stem 0) (note 0) 1 '(0 . 3))").unwrap();
    assert_eq!(of(&mut vm, "(stem 0)", stem::thickness), rule);
    eval(&mut vm, "(put! (stem 0) 'thickness 'x)");
    let st = grob_of(&mut vm, "(stem 0)");
    let e = stem::thickness(&mut vm, st).unwrap_err();
    assert_eq!(shown(&vm, e.key), "wrong-type-arg");
}

/// Stem 0's support head of heads 0, 1 and 2, `exts` wide, each
/// attached at `attach` unless `attaches` says otherwise, for the
/// direction `dir`, and the three heads.
fn supported(
    exts: [&str; 3],
    attach: &str,
    attaches: [&str; 3],
    dir: i32,
) -> Option<(Option<Value>, [Value; 3])> {
    let attaches = attaches.map(|a| if a.is_empty() { attach } else { a });
    let mut vm = stem_vm(&format!(
        "(stem! (stem 0) (note 0) {dir} '(0 . 3) (head! (head 0) (column 0) 0)
                (head! (head 1) (column 0) 2) (head! (head 2) (column 0) 4))
         (for-each (lambda (h ext at)
                     (put! h 'stem-attachment at)
                     (put! h 'X-extent ext))
                   (list (head 0) (head 1) (head 2)) '({} {} {}) '({} {} {}))",
        exts[0], exts[1], exts[2], attaches[0], attaches[1], attaches[2]
    ))?;
    let heads = ["(head 0)", "(head 1)", "(head 2)"].map(|t| grob_of(&mut vm, t));
    Some((of(&mut vm, "(stem 0)", stem::support_head), heads))
}

#[test]
fn the_support_head_has_the_widest_part_inside_the_stem() {
    let same = "(0 . 1.3)";
    let up = "(1 . 0.5)";
    let Some((got, [h0, h1, h2])) = supported([same; 3], up, ["", "", ""], 1) else {
        return;
    };
    // All alike: the first.
    assert_eq!(got, Some(h0));
    // The widest left of an up-stem's attachment, at the right edge.
    let wider = [same, "(0 . 1.5)", "(-0.1 . 1.3)"];
    assert_eq!(supported(wider, up, ["", "", ""], 1).unwrap().0, Some(h1));
    let widest = [same, "(0 . 1.5)", "(-0.3 . 1.3)"];
    assert_eq!(supported(widest, up, ["", "", ""], 1).unwrap().0, Some(h2));
    // Right of a down-stem's, at the left edge.
    let down = "(-1 . -0.5)";
    assert_eq!(
        supported(widest, down, ["", "", ""], -1).unwrap().0,
        Some(h2)
    );
    // Head 1 attached at its left edge, the others at their right: 0.075
    // inside an up-stem, 0.925 inside a down-stem; for a direction of 0
    // the right edge, 1 inside.
    let lefts = ["", down, ""];
    let unit = ["(0 . 1)"; 3];
    assert_eq!(supported(unit, up, lefts, 1).unwrap().0, Some(h0));
    assert_eq!(supported(unit, up, lefts, -1).unwrap().0, Some(h1));
    assert_eq!(supported(unit, up, lefts, 0).unwrap().0, Some(h1));
    // One head is it, whatever it is; none is the first head, none.
    let mut vm = stem_vm(
        "(stem! (stem 0) (note 0) 1 '(0 . 3))
         (ly:grob-set-object! (stem 0) 'note-heads (arr (bit 0)))",
    )
    .unwrap();
    let bit = grob_of(&mut vm, "(bit 0)");
    assert_eq!(of(&mut vm, "(stem 0)", stem::support_head), Some(bit));
    eval(&mut vm, "(ly:grob-set-object! (stem 0) 'note-heads (arr))");
    assert_eq!(of(&mut vm, "(stem 0)", stem::support_head), None);
}

#[test]
fn a_stem_is_invisible_without_a_duration_or_a_stemlet_and_beamed_by_a_spanner() {
    let Some(mut vm) = stem_vm(
        "(stem! (stem 0) (note 0) 1 '(0 . 3) (head! (head 0) (column 0) 0))
         (stem! (stem 1) (note 1) 1 '(0 . 3))",
    ) else {
        return;
    };
    let invisible = |vm: &mut Vm, s: &str| of(vm, s, stem::is_invisible);
    assert!(!invisible(&mut vm, "(stem 0)"));
    eval(&mut vm, "(put! (stem 0) 'duration-log 0)");
    assert!(invisible(&mut vm, "(stem 0)"));
    assert!(invisible(&mut vm, "(stem 1)"));
    eval(&mut vm, "(put! (stem 1) 'stemlet-length 'x)");
    assert!(invisible(&mut vm, "(stem 1)"));
    eval(&mut vm, "(put! (stem 1) 'stemlet-length 0.5)");
    assert!(!invisible(&mut vm, "(stem 1)"));

    assert_eq!(of(&mut vm, "(stem 0)", stem::get_beam), None);
    eval(&mut vm, "(ly:grob-set-object! (stem 0) 'beam (thing 0))");
    assert_eq!(of(&mut vm, "(stem 0)", stem::get_beam), None);
    eval(&mut vm, "(ly:grob-set-object! (stem 0) 'beam (group 0))");
    let g = grob_of(&mut vm, "(group 0)");
    assert_eq!(of(&mut vm, "(stem 0)", stem::get_beam), Some(g));
}

#[test]
fn a_note_column_s_accidentals_are_its_first_accidental_s_placement() {
    let Some(mut vm) =
        stem_vm("(ly:grob-set-object! (note 0) 'note-heads (arr (head 0) (head 1) (head 2)))")
    else {
        return;
    };
    let acc = |vm: &mut Vm| of(vm, "(note 0)", note_column::accidentals);
    assert_eq!(acc(&mut vm), None);
    eval(
        &mut vm,
        "(ly:grob-set-object! (head 0) 'accidental-grob 'x)
         (ly:grob-set-object! (head 1) 'accidental-grob (bit 1))
         (ly:grob-set-object! (head 2) 'accidental-grob (bit 2))",
    );
    let [b1, p0] = ["(bit 1)", "(placement 0)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(acc(&mut vm), Some(b1));
    eval(&mut vm, "(ly:grob-set-parent! (bit 1) 0 (thing 0))");
    assert_eq!(acc(&mut vm), Some(b1));
    eval(&mut vm, "(ly:grob-set-parent! (bit 1) 0 (placement 0))");
    assert_eq!(acc(&mut vm), Some(p0));
}

#[test]
fn a_bar_line_covers_its_extent_in_staff_spaces() {
    let Some(mut vm) = stem_vm(
        "(put! (bar 0) 'glyph-name \"|\")
         (put! (bar 0) 'Y-offset 1)
         (put! (bar 0) 'Y-extent '(-2 . 2))",
    ) else {
        return;
    };
    let bar = |vm: &mut Vm, t: &str| of(vm, t, bar_y_positions);
    // No staff symbol: from the root, in staff spaces of 1.
    assert_eq!(bar(&mut vm, "(bar 0)"), Interval::new(-1.0, 3.0));
    // From the staff symbol, 2 a staff space, 0.5 below the bar line.
    eval(
        &mut vm,
        "(ly:grob-set-object! (bar 0) 'staff-symbol (staff 0))
         (put! (staff 0) 'staff-space 2)
         (put! (staff 0) 'Y-offset 0.5)
         (ly:grob-set-parent! (bar 0) 1 (staff 0))",
    );
    assert_eq!(bar(&mut vm, "(bar 0)"), Interval::new(-0.5, 1.5));
    // In no common refpoint: from the root.
    eval(
        &mut vm,
        "(put! (thing 1) 'Y-offset 0) (ly:grob-set-parent! (bar 0) 1 (thing 1))",
    );
    assert_eq!(bar(&mut vm, "(bar 0)"), Interval::new(-0.5, 1.5));
    eval(&mut vm, "(put! (bar 0) 'glyph-name \".|:\")");
    assert_eq!(bar(&mut vm, "(bar 0)"), Interval::new(-0.5, 1.5));
    for glyph in ["\":|.\"", "\"\"", "'|"] {
        eval(&mut vm, &format!("(put! (bar 0) 'glyph-name {glyph})"));
        assert!(bar(&mut vm, "(bar 0)").is_empty(), "{glyph}");
    }
    eval(&mut vm, "(put! (thing 2) 'glyph-name \"|\")");
    assert!(bar(&mut vm, "(thing 2)").is_empty());
}

/// [`STEMS`]' spacing grob `(thing 0)` on column 0 between note 0 there,
/// its left item, and note 1 on column 1, its right one, each with an
/// up-stem along `(0 . 3.5)` of a head at position 0 (stem 0 and head 0,
/// stem 1 and head 1); its stem spacing correction 0.5, same direction
/// correction 0.25 and knee spacing correction 1. Then `setup`.
fn stems_vm(setup: &str) -> Option<Vm> {
    stem_vm(&format!(
        "(on! (thing 0) (column 0))
         (stem! (stem 0) (note 0) 1 '(0 . 3.5) (head! (head 0) (column 0) 0))
         (stem! (stem 1) (note 1) 1 '(0 . 3.5) (head! (head 1) (column 1) 0))
         (ly:grob-set-object! (thing 0) 'left-items (arr (note 0)))
         (ly:grob-set-object! (thing 0) 'right-items (arr (note 1)))
         (put! (thing 0) 'stem-spacing-correction 0.5)
         (put! (thing 0) 'same-direction-correction 0.25)
         (put! (thing 0) 'knee-spacing-correction 1.0)
         {setup}"
    ))
}

/// The correction [`stem_dir_correction`] makes to 0 from `(thing 0)` to
/// column 1 of [`stems_vm`] after `setup`, at an increment of 1.2.
fn corrected(setup: &str) -> Option<f64> {
    let mut vm = stems_vm(setup)?;
    let [me, right] = ["(thing 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let mut space = 0.0;
    stem_dir_correction(&mut vm, me, right, 1.2, &mut space)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some(space)
}

/// Stem 1 down, along `(-0.5 . 2)`: positions -1 to 4 against stem 0's 0
/// to 7.
const UP_DOWN: &str = "(put! (stem 1) 'direction -1) (put! (stem 1) 'Y-extent '(-0.5 . 2))";

#[test]
fn up_then_down_stems_get_more_space_by_their_overlap() {
    let Some(got) = corrected(UP_DOWN) else {
        return;
    };
    // An overlap of 4, a seventh of it at half.
    let up_down = (4.0f64 / 7.0).min(1.0) * 1.0 * 0.5;
    assert_eq!(got, up_down);
    // Down then up: as much less.
    let down_up = "(put! (stem 0) 'direction -1) (put! (stem 1) 'Y-extent '(-0.5 . 2))";
    assert_eq!(corrected(down_up).unwrap(), -(4.0 / 7.0) * 0.5);
    // At most the whole correction; none without an overlap.
    let long =
        format!("{UP_DOWN} (put! (stem 0) 'Y-extent '(0 . 5)) (put! (stem 1) 'Y-extent '(-3 . 5))");
    assert_eq!(corrected(&long).unwrap(), 0.5);
    let apart = format!(
        "{UP_DOWN} (put! (stem 0) 'Y-extent '(0 . 1)) (put! (stem 1) 'Y-extent '(1.5 . 2.5))"
    );
    assert_eq!(corrected(&apart).unwrap(), 0.0);
    // Positions in staff spaces: of 2, half as many.
    let wide = format!(
        "{UP_DOWN} (ly:grob-set-object! (stem 1) 'staff-symbol (staff 0))
         (put! (staff 0) 'staff-space 2)"
    );
    assert_eq!(corrected(&wide).unwrap(), 2.0 / 7.0 * 0.5);
    // The correction is the grob's.
    let none = format!("{UP_DOWN} (put! (thing 0) 'stem-spacing-correction 'x)");
    assert_eq!(corrected(&none).unwrap(), 0.0);
    // A left stem with a flag needs a beam; any beam but the right stem's.
    let flag = format!("{UP_DOWN} (put! (stem 0) 'duration-log 3)");
    assert_eq!(corrected(&flag).unwrap(), 0.0);
    let beamed = format!("{flag} (ly:grob-set-object! (stem 0) 'beam (group 1))");
    assert_eq!(corrected(&beamed).unwrap(), up_down);
}

#[test]
fn a_knee_s_stems_get_the_right_head_s_width_less_the_stem() {
    let knee = format!(
        "{UP_DOWN} (ly:grob-set-object! (stem 0) 'beam (group 0))
         (ly:grob-set-object! (stem 1) 'beam (group 0))"
    );
    let Some(got) = corrected(&knee) else {
        return;
    };
    let rule = 1.5 * 0.1;
    assert_eq!(got, -(1.3 - rule) * -1.0 * 1.0);
    // An empty head: the increment.
    let empty = format!("{knee} (put! (head 1) 'X-extent '(1 . 0))");
    assert_eq!(corrected(&empty).unwrap(), -(1.2 - rule) * -1.0 * 1.0);
    // The head in its own column, here column 0 at 2.
    let moved = format!("{knee} (on! (head 1) (column 0)) (put! (column 0) 'X-offset 2)");
    assert_eq!(corrected(&moved).unwrap(), -(1.3 - rule) * -1.0 * 1.0);
    // No head: the increment, the stem's thickness not taken.
    let headless = format!(
        "{knee} (ly:grob-set-object! (stem 1) 'note-heads (arr))
         (put! (stem 1) 'stemlet-length 1)"
    );
    assert_eq!(corrected(&headless).unwrap(), -1.2 * -1.0 * 1.0);
    // Two beams are no knee.
    let two = format!("{knee} (ly:grob-set-object! (stem 1) 'beam (group 1))");
    assert_eq!(corrected(&two).unwrap(), 4.0 / 7.0 * 0.5);
}

#[test]
fn stems_the_same_way_move_a_head_stepping_away_towards_the_lower() {
    // The left head at 4, the right at 0: less space.
    let Some(got) = corrected("(put! (head 0) 'Y-offset 2.0)") else {
        return;
    };
    assert_eq!(got, -0.25);
    assert_eq!(corrected("(put! (head 1) 'Y-offset 2.0)").unwrap(), 0.25);
    assert_eq!(
        corrected("(put! (stem 0) 'direction -1) (put! (stem 1) 'direction -1) (put! (head 1) 'Y-offset 2.0)")
            .unwrap(),
        0.25
    );
    // A step of 1 or less, or the same position: none.
    assert_eq!(corrected("(put! (head 1) 'Y-offset 0.5)").unwrap(), 0.0);
    assert_eq!(corrected("").unwrap(), 0.0);
    // Accidentals on the right, not on the left, take it away.
    let acc = |h: usize| {
        format!(
            "(put! (head 0) 'Y-offset 2.0)
             (ly:grob-set-object! (head {h}) 'accidental-grob (bit 0))"
        )
    };
    assert_eq!(corrected(&acc(1)).unwrap(), 0.0);
    assert_eq!(corrected(&acc(0)).unwrap(), -0.25);
}

#[test]
fn a_bar_line_stands_for_a_right_stem_at_half_the_correction() {
    let bar = "(on! (bar 0) (column 1))
               (put! (bar 0) 'X-extent '(0 . 0.5))
               (put! (bar 0) 'space-alist '((first-note fixed-space . 1)))
               (put! (bar 0) 'glyph-name \"|\")
               (put! (bar 0) 'Y-offset 0)
               (put! (bar 0) 'Y-extent '(-2 . 2))
               (ly:grob-set-object! (thing 0) 'right-break-aligned (arr (bar 0)))
               (ly:grob-set-object! (thing 0) 'right-items (arr))";
    // Positions -4 to 4 down against 0 to 7 up.
    let Some(got) = corrected(bar) else {
        return;
    };
    assert_eq!(got, 4.0 / 7.0 * 1.0 * 0.5 * 0.5);
    // With the right stem up too: the bar line's.
    let both = format!("{bar} (ly:grob-set-object! (thing 0) 'right-items (arr (note 1)))");
    assert_eq!(corrected(&both).unwrap(), 4.0 / 7.0 * 1.0 * 0.5 * 0.5);
    // Down: less.
    let down = format!("{bar} (put! (stem 0) 'direction -1)");
    assert_eq!(corrected(&down).unwrap(), -(4.0 / 7.0) * 0.5 * 0.5);
    // Not in the right column, or not a bar line's glyph: none.
    let away = format!("{bar} (on! (bar 0) (column 2))");
    assert_eq!(corrected(&away).unwrap(), 0.0);
    let colon = format!("{bar} (put! (bar 0) 'glyph-name \":\")");
    assert_eq!(corrected(&colon).unwrap(), 0.0);
}

#[test]
fn no_stem_correction_without_two_visible_stems_each_way() {
    for (setup, why) in [
        ("(ly:grob-set-object! (note 0) 'stem '())", "no left stem"),
        (
            "(ly:grob-set-object! (note 1) 'stem (group 0))",
            "no right stem",
        ),
        ("(ly:grob-suicide! (stem 0))", "a dead stem"),
        ("(put! (stem 1) 'duration-log 0)", "an invisible stem"),
        (
            "(put! (stem 1) 'direction 0)",
            "a right stem of no direction",
        ),
        (
            "(on! (note 2) (column 0))
             (stem! (stem 2) (note 2) -1 '(0 . 3.5) (head! (head 2) (column 0) 0))
             (ly:grob-set-object! (thing 0) 'left-items (arr (note 0) (note 2)))",
            "left stems both ways",
        ),
        (
            "(on! (note 1) (column 2))",
            "the right note column elsewhere",
        ),
        (
            "(ly:grob-set-object! (thing 0) 'right-items (arr (thing 1) (group 0)))",
            "no note column",
        ),
    ] {
        let Some(got) = corrected(&format!("{UP_DOWN} {setup}")) else {
            return;
        };
        assert_eq!(got, 0.0, "{why}");
    }
    // A right note column elsewhere is skipped, not an end.
    let skipped = format!(
        "{UP_DOWN} (on! (note 2) (column 2))
         (ly:grob-set-object! (thing 0) 'right-items (arr (note 2) (note 1)))"
    );
    assert_eq!(corrected(&skipped).unwrap(), 4.0 / 7.0 * 0.5);
}

/// The note spring from `(thing 0)` to column 1 of [`spacing_vm`] with
/// [`TWO_SEPS`] (4 apart) and [`STEMS`], note 0 a left item on column 0,
/// and `setup`, from `base` at an increment of 1.2; and the diagnostics.
fn note_spring_of(setup: &str, base: Spring) -> Option<(Spring, String)> {
    let mut vm = spacing_vm(TWO_SEPS, "(list (sep 0) (note 0))", "(list (sep 1))")?;
    set_layout(&mut vm);
    eval(&mut vm, STEMS);
    eval(&mut vm, "(on! (note 0) (column 0))");
    eval(&mut vm, setup);
    let [me, right] = ["(thing 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let s = get_spacing(&mut vm, me, right, base, 1.2)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some((s, diagnosed(&mut vm)))
}

/// Note 0's rest thing 1, on column 0 and `x` wide.
fn rest(x: &str) -> String {
    format!(
        "(ly:grob-set-object! (note 0) 'rest (thing 1))
         (on! (thing 1) (column 0)) (put! (thing 1) 'X-extent '(0 . {x}))"
    )
}

#[test]
fn the_note_spring_is_from_the_left_heads_end_and_apart_by_the_skylines() {
    let base = spring(3.0, 1.0, 3.0, 2.0, -1.0);
    // The rest's end, less the increment; the skylines 4 apart.
    let Some((got, errors)) = note_spring_of(&rest("1.25"), base) else {
        return;
    };
    let ideal = 3.0 - 1.2 + 1.25;
    assert_eq!(got, spring(ideal, 4.0, 3.0, 2.0, (4.0 - ideal) / 3.0));
    assert_eq!(errors, "()");
    // No note column's head: from 0.
    let (got, _) =
        note_spring_of("(ly:grob-set-object! (note 0) 'note-heads (arr))", base).unwrap();
    assert_eq!(got.ideal_distance, 3.0 - 1.2);
    // The first head of a stem's, the lowest of an up-stem.
    let heads = "(stem! (stem 0) (note 0) 1 '(0 . 3) (head! (head 0) (column 0) 2)
                        (head! (head 1) (column 0) 0))
                 (put! (head 1) 'X-extent '(0 . 1.5))";
    let (got, _) = note_spring_of(heads, base).unwrap();
    assert_eq!(got.ideal_distance, 3.0 - 1.2 + 1.5);
    // The rest before the heads; the last note column's.
    let (got, _) = note_spring_of(&format!("{heads} {}", rest("1.25")), base).unwrap();
    assert_eq!(got.ideal_distance, ideal);
    let two = format!(
        "{} (ly:grob-set-object! (thing 0) 'left-items (arr (sep 0) (note 0) (note 1)))
         (on! (note 1) (column 0)) (ly:grob-set-object! (note 1) 'rest (thing 2))
         (on! (thing 2) (column 0)) (put! (thing 2) 'X-extent '(0 . 0.75))",
        rest("1.25")
    );
    let (got, _) = note_spring_of(&two, base).unwrap();
    assert_eq!(got.ideal_distance, 3.0 - 1.2 + 0.75);
    // A rest in another column: LilyPond's programming error.
    let (got, errors) = note_spring_of(
        &format!("{} (on! (thing 1) (column 1))", rest("1.25")),
        base,
    )
    .unwrap();
    assert_eq!(got.ideal_distance, 3.0 - 1.2);
    assert_eq!(
        errors,
        "(\"Note_spacing::get_spacing (): Common refpoint incorrect\")"
    );
    // At least 0.
    let (got, _) = note_spring_of("", spring(0.5, 0.0, 0.5, 0.5, -1.0)).unwrap();
    assert_eq!((got.ideal_distance, got.min_distance), (0.0, 4.0));
    // The vertical padding: a right box above the left one meets it
    // only with it.
    let above = "(put! (bit 1) 'Y-extent '(1.5 . 2.5))";
    let (got, _) = note_spring_of(above, base).unwrap();
    assert_eq!(got.min_distance, 0.0);
    let padded = format!("{above} (put! (column 1) 'skyline-vertical-padding 1)");
    let (got, _) = note_spring_of(&padded, base).unwrap();
    assert_eq!(got.min_distance, 4.0);
}

#[test]
fn before_a_non_musical_column_the_note_spring_is_to_its_bar_line() {
    let base = spring(10.0, 1.0, 10.0, 9.0, -1.0);
    let ideal = 10.0 - 1.2 + 1.25;
    let to_bar = format!(
        "{} (put! (thing 0) 'space-to-barline #t) (put! (column 1) 'X-extent '(-1 . 2))",
        rest("1.25")
    );
    let Some((got, _)) = note_spring_of(&to_bar, base) else {
        return;
    };
    // To the column's right side.
    assert_eq!(got.ideal_distance, ideal - 2.0);
    // At least halfway from the ideal distance to the minimum.
    let (got, _) = note_spring_of(&to_bar, spring(3.0, 1.0, 3.0, 2.0, -1.0)).unwrap();
    assert_eq!(got.ideal_distance, (3.0 - 1.2 + 1.25 + 4.0) / 2.0);
    // To the staff bar's left edge, which the halfway does not bound.
    let staff_bar = format!(
        "{to_bar} (ly:grob-set-object! (column 1) 'break-alignment (thing 2))
         (ly:grob-set-object! (thing 2) 'elements (arr (bit 3) (bit 2)))
         (put! (bit 3) 'break-align-symbol 'clef)
         (put! (bit 3) 'X-extent '(-3 . 0))
         (put! (bit 2) 'break-align-symbol 'staff-bar)
         (on! (bit 2) (column 1))
         (put! (bit 2) 'X-extent '(0.5 . 1))"
    );
    let (got, _) = note_spring_of(&staff_bar, base).unwrap();
    assert_eq!(got.ideal_distance, ideal - 0.5);
    let (got, _) = note_spring_of(&staff_bar, spring(3.0, 1.0, 3.0, 2.0, -1.0)).unwrap();
    assert_eq!(got.ideal_distance, 3.0 - 1.2 + 1.25 - 0.5);
    // An empty staff bar, or a break alignment that is no item: the
    // column's.
    let empty = format!("{staff_bar} (put! (bit 2) 'X-extent '(1 . 0))");
    let (got, _) = note_spring_of(&empty, base).unwrap();
    assert_eq!(got.ideal_distance, ideal - 2.0);
    let spanner =
        format!("{staff_bar} (ly:grob-set-object! (column 1) 'break-alignment (group 0))");
    let (got, _) = note_spring_of(&spanner, base).unwrap();
    assert_eq!(got.ideal_distance, ideal - 2.0);
    // Not for a musical column, without the right skyline, or without
    // space-to-barline #t.
    for setup in [
        "(put! (column 1) 'shortest-starter-duration 1/8)",
        "(ly:grob-set-object! (thing 0) 'right-items (arr))",
        "(put! (thing 0) 'space-to-barline 1)",
    ] {
        let (got, _) = note_spring_of(&format!("{to_bar} {setup}"), base).unwrap();
        assert_eq!(got.ideal_distance, ideal, "{setup}");
    }
}

#[test]
fn the_note_spring_takes_the_stems_correction() {
    // Note 0 up, a quarter rest's width; note 1 on column 1 down.
    let setup = format!(
        "{} (stem! (stem 0) (note 0) 1 '(0 . 3.5) (head! (head 0) (column 0) 0))
         (ly:grob-set-object! (note 0) 'rest (thing 1))
         (on! (note 1) (column 1))
         (stem! (stem 1) (note 1) -1 '(-0.5 . 2) (head! (head 1) (column 1) 0))
         (ly:grob-set-object! (thing 0) 'right-items (arr (sep 1) (note 1)))
         (put! (thing 0) 'stem-spacing-correction 0.5)",
        rest("1.25")
    );
    let Some((got, _)) = note_spring_of(&setup, spring(3.0, 1.0, 3.0, 2.0, -1.0)) else {
        return;
    };
    assert_eq!(got.ideal_distance, 3.0 - 1.2 + 1.25 + 4.0 / 7.0 * 1.0 * 0.5);
}

/// [`columns_vm`] (columns 0 and 1 4 apart), with [`STEMS`], and the
/// staff spacing `(thing 0)` on column 0 after the clef `(thing 1)` there,
/// its `left-break-aligned`, along `(-1 . 5)` with the space-alist
/// `alist`; then `setup`.
fn staff_vm(alist: &str, setup: &str) -> Option<Vm> {
    let mut vm = columns_vm(&format!(
        "(put! (thing 1) 'X-offset 0)
         (ly:grob-set-parent! (thing 1) 0 (column 0))
         (put! (thing 1) 'X-extent '(-1 . 5))
         (put! (thing 1) 'space-alist '{alist})
         (ly:grob-set-object! (thing 0) 'left-break-aligned (arr (thing 1)))"
    ))?;
    set_layout(&mut vm);
    eval(&mut vm, STEMS);
    eval(&mut vm, setup);
    Some(vm)
}

/// The staff spring from `(thing 0)` to column 1 of [`staff_vm`] at the
/// situational space `situational`, and the diagnostics.
fn staff_spring_of(alist: &str, setup: &str, situational: f64) -> Option<(Spring, String)> {
    let mut vm = staff_vm(alist, setup)?;
    let [me, right] = ["(thing 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let s = staff_spacing::get_spacing(&mut vm, me, right, situational)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    Some((s, diagnosed(&mut vm)))
}

/// The staff spring of `alist` with no setup and no situational space.
fn staff_spring(alist: &str) -> Spring {
    staff_spring_of(alist, "", 0.0).unwrap().0
}

/// The error the staff spring of `alist` after `setup` throws, as text.
fn staff_spring_error(alist: &str, setup: &str) -> String {
    let mut vm = staff_vm(alist, setup).unwrap();
    let [me, right] = ["(thing 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let e = staff_spacing::get_spacing(&mut vm, me, right, 0.0).unwrap_err();
    e.text(vm.heap())
}

#[test]
fn the_staff_spring_is_by_the_type_of_the_first_note_entry() {
    // The clef ends at 5, is 6 long; the columns' minimum distance is 4.
    let Some((got, errors)) = staff_spring_of("((first-note fixed-space . 2))", "", 0.0) else {
        return;
    };
    assert_eq!(got, spring(7.0, 4.0, 0.0, 0.0, 0.0));
    assert_eq!(errors, "()");
    for (ty, distance, want) in [
        ("extra-space", 2, spring(7.0, 4.0, 2.0, 2.0, -1.5)),
        ("semi-fixed-space", 2, spring(7.0, 4.0, 1.0, 1.0, -3.0)),
        // From the left edge, at least the length.
        ("minimum-space", 8, spring(7.0, 4.0, 2.0, 2.0, -1.5)),
        ("minimum-space", 2, spring(5.0, 4.0, 0.0, 0.0, 0.0)),
        ("minimum-fixed-space", 8, spring(7.0, 4.0, 0.0, 0.0, 0.0)),
        ("minimum-fixed-space", 2, spring(5.0, 4.0, 0.0, 0.0, 0.0)),
        // Not stretchable.
        ("shrink-space", 2, spring(7.0, 4.0, 0.0, 2.0, -1.5)),
        ("semi-shrink-space", 2, spring(7.0, 4.0, 0.0, 1.0, -3.0)),
        // No type: 1 past the end, its distance unused.
        ("other", 2, spring(6.0, 4.0, 1.0, 1.0, -2.0)),
        ("#:fixed-space", 2, spring(6.0, 4.0, 1.0, 1.0, -2.0)),
        ("\"fixed-space\"", 2, spring(6.0, 4.0, 1.0, 1.0, -2.0)),
    ] {
        let alist = format!("((first-note {ty} . {distance}))");
        assert_eq!(staff_spring(&alist), want, "{ty} {distance}");
    }
    // The first entry of the key; what is no pair is skipped.
    assert_eq!(
        staff_spring("(x (first-note fixed-space . 2) (first-note extra-space . 2))"),
        spring(7.0, 4.0, 0.0, 0.0, 0.0)
    );
}

#[test]
fn the_situational_space_widens_the_ideal_distance_and_not_the_stretch() {
    let Some((got, _)) = staff_spring_of("((first-note extra-space . 2))", "", 0.5) else {
        return;
    };
    assert_eq!(got, spring(7.5, 4.0, 2.0, 2.5, (4.0 - 7.5) / 2.5));
    let (got, _) = staff_spring_of("((first-note fixed-space . 2))", "", 0.5).unwrap();
    assert_eq!(got, spring(7.5, 4.0, 0.0, 0.5, (4.0 - 7.5) / 0.5));
    // A minimum space within the clef's length: from its end, not its
    // left edge plus the distance.
    let (got, _) = staff_spring_of("((first-note minimum-space . 2))", "", 0.5).unwrap();
    assert_eq!(got, spring(5.5, 4.0, 0.0, 0.5, (4.0 - 5.5) / 0.5));
}

#[test]
fn the_fixed_distance_leaves_a_gap_past_the_minimum_distance() {
    // The clef ends at 1.5: fixed at 1.5 + 0.3 short of 4.
    let short = "(put! (thing 1) 'X-extent '(-1 . 1.5))";
    let Some((got, errors)) = staff_spring_of("((first-note extra-space . 3))", short, 0.0) else {
        return;
    };
    let fixed = 1.5 + (0.3 + 4.0 - 1.5);
    let ideal = 1.5 + 3.0;
    assert_eq!(
        got,
        spring(
            ideal,
            4.0,
            3.0,
            ideal - fixed,
            (4.0 - ideal) / (ideal - fixed)
        )
    );
    assert_eq!(errors, "()");
    // The ideal distance at least the fixed one.
    let (got, _) = staff_spring_of("((first-note fixed-space . 1))", short, 0.0).unwrap();
    let fixed = 2.5 + (0.3 + 4.0 - 2.5);
    assert_eq!(got, spring(fixed, 4.0, 0.0, 0.0, 0.0));
    // A minimum distance of 0, the right box above the left one: 0.3 at
    // least, and no less above it.
    let above = format!("{short} (put! (bit 1) 'Y-extent '(2 . 3))");
    let (got, _) = staff_spring_of("((first-note fixed-space . -1.4))", &above, 0.0).unwrap();
    let fixed = 1.5 + -1.4;
    let fixed = fixed + (0.3 + 0.0 - fixed);
    assert_eq!(got, spring(fixed, 0.0, 0.0, 0.0, 0.0));
    let (got, _) = staff_spring_of("((first-note fixed-space . -1))", &above, 0.0).unwrap();
    assert_eq!(got, spring(0.5, 0.0, 0.0, 0.0, 0.0));
}

#[test]
fn the_next_note_entry_is_for_an_unbroken_staff_spacing() {
    let both = "((first-note fixed-space . 2) (next-note extra-space . 2))";
    let Some((got, _)) = staff_spring_of(both, "", 0.0) else {
        return;
    };
    assert_eq!(got, spring(7.0, 4.0, 2.0, 2.0, -1.5));
    // Only it.
    assert_eq!(
        staff_spring("((next-note extra-space . 2))"),
        spring(7.0, 4.0, 2.0, 2.0, -1.5)
    );

    // Broken at the beginning of a line, thing 0 is thing 2's piece, and
    // piece 0, thing 1's, on column 0: the first note's.
    let mut vm = staff_vm(
        both,
        "(put! (piece 0) 'X-offset 0)
         (ly:grob-set-parent! (piece 0) 0 (column 0))
         (put! (piece 0) 'X-extent '(-1 . 5))
         (put! (piece 0) 'space-alist (ly:grob-property (thing 1) 'space-alist))",
    )
    .unwrap();
    let [me, t1, t2, p0, right] = [
        "(thing 0)",
        "(thing 1)",
        "(thing 2)",
        "(piece 0)",
        "(column 1)",
    ]
    .map(|t| grob_of(&mut vm, t));
    for (orig, piece) in [(t2, me), (t1, p0)] {
        grob::with_grob_mut(vm.heap(), orig, |x| x.broken = [None, Some(piece)]);
        grob::with_grob_mut(vm.heap(), piece, |x| x.original = Some(orig));
    }
    let got = staff_spacing::get_spacing(&mut vm, me, right, 0.0).unwrap();
    assert_eq!(got, spring(7.0, 4.0, 0.0, 0.0, 0.0));
    // No only next-note entry there: LilyPond's programming error.
    eval(
        &mut vm,
        "(put! (piece 0) 'space-alist '((next-note extra-space . 2)))",
    );
    let got = staff_spacing::get_spacing(&mut vm, me, right, 0.0).unwrap();
    assert_eq!(got, Spring::default());
    assert_eq!(diagnosed(&mut vm), "(\"unknown prefatory spacing\")");
}

#[test]
fn without_a_prefatory_entry_the_staff_spring_is_the_default_one() {
    let Some((got, errors)) = staff_spring_of(
        "((first-note fixed-space . 2))",
        "(ly:grob-set-object! (thing 0) 'left-break-aligned (arr))",
        0.0,
    ) else {
        return;
    };
    // No break-aligned grob: no warning.
    assert_eq!((got, errors.as_str()), (Spring::default(), "()"));
    // An improper space-alist.
    let (got, errors) = staff_spring_of("((first-note fixed-space . 2) . 3)", "", 0.0).unwrap();
    assert_eq!((got, errors.as_str()), (Spring::default(), "()"));
    // No entry of the first note, nor of the next.
    let (got, errors) = staff_spring_of("((clef extra-space . 2))", "", 0.0).unwrap();
    assert_eq!(
        (got, errors.as_str()),
        (Spring::default(), "(\"unknown prefatory spacing\")")
    );
}

#[test]
fn a_space_alist_entry_that_is_no_type_and_distance_is_guile_s_error() {
    if grobs_vm().is_none() {
        return;
    }
    let e = staff_spring_error("((first-note . 2))", "");
    assert!(
        e.contains("cdr") && e.contains("Wrong type argument (expecting pair): 2"),
        "{e}"
    );
    let e = staff_spring_error("((first-note fixed-space . x))", "");
    assert!(e.contains("Wrong type (expecting real number): x"), "{e}");
    // The next note's entry, a pair, is taken whatever its value.
    let e = staff_spring_error("((first-note fixed-space . 2) (next-note . x))", "");
    assert!(e.contains("Wrong type argument (expecting pair): x"), "{e}");
    // A staff spacing in no column: its minimum distance reads no grob.
    let e = staff_spring_error(
        "((first-note fixed-space . 2))",
        "(ly:grob-set-parent! (thing 0) 0 (thing 4))",
    );
    assert!(e.contains("ly:grob-property"), "{e}");
}

/// [`staff_vm`] with bar 0, a bar line along `(-2 . 2)` in staff spaces
/// of 1, as the break-aligned grob instead of the clef, and note 1 on
/// column 1, the staff spacing's right item, with a stem `stem` (as
/// [`STEMS`]' `stem!` makes it after the note) and `setup`.
fn barred(alist: &str, stem: &str, setup: &str) -> Option<(Spring, String)> {
    staff_spring_of(
        alist,
        &format!(
            "(put! (bar 0) 'X-offset 0)
             (ly:grob-set-parent! (bar 0) 0 (column 0))
             (put! (bar 0) 'X-extent '(-1 . 5))
             (put! (bar 0) 'space-alist (ly:grob-property (thing 1) 'space-alist))
             (put! (bar 0) 'glyph-name \"|\")
             (put! (bar 0) 'Y-offset 0)
             (put! (bar 0) 'Y-extent '(-2 . 2))
             (ly:grob-set-object! (thing 0) 'left-break-aligned (arr (bar 0)))
             (on! (note 1) (column 1))
             (stem! (stem 1) (note 1) {stem} (head! (head 1) (column 1) 0))
             (ly:grob-set-object! (thing 0) 'right-items (arr (note 1)))
             {setup}"
        ),
        0.0,
    )
}

const EXTRA: &str = "((first-note extra-space . 2))";

/// The fields of a spring (no blocking force).
fn fields(s: Spring) -> [f64; 4] {
    [
        s.ideal_distance,
        s.min_distance,
        s.inverse_stretch_strength,
        s.inverse_compress_strength,
    ]
}

/// The extra-space spring of [`EXTRA`] after the bar line, both distances
/// moved by the optical correction `o`.
fn optical(o: f64) -> [f64; 4] {
    let (fixed, ideal) = (5.0 + o, 5.0 + 2.0 + 0.0 + o);
    [ideal, 4.0, 2.0, ideal - fixed]
}

#[test]
fn a_down_stem_after_a_bar_line_moves_the_staff_spring_by_their_overlap() {
    // Along (-1 . 3.5), 3 of it beside the bar line.
    let Some((got, _)) = barred(EXTRA, "-1 '(-1 . 3.5)", "") else {
        return;
    };
    let o = (3.0f64 / 7.0).abs();
    assert_eq!(fields(got), optical(o));
    // By the stem spacing correction, at most 1.
    let (got, _) = barred(
        EXTRA,
        "-1 '(-1 . 3.5)",
        "(put! (thing 0) 'stem-spacing-correction 0.5)",
    )
    .unwrap();
    assert_eq!(fields(got), optical(o * 0.5));
    let (got, _) = barred(
        EXTRA,
        "-1 '(-9 . 9)",
        "(put! (bar 0) 'Y-extent '(-9 . 9))
         (put! (thing 0) 'stem-spacing-correction 3)",
    )
    .unwrap();
    assert_eq!(fields(got), optical(3.0));
    // The bar line in staff spaces: 2 a staff space, so (-1 . 1).
    let (got, _) = barred(
        EXTRA,
        "-1 '(-1 . 3.5)",
        "(ly:grob-set-object! (bar 0) 'staff-symbol (staff 0))
         (put! (staff 0) 'staff-space 2)
         (put! (staff 0) 'Y-offset 0)
         (ly:grob-set-parent! (bar 0) 1 (staff 0))",
    )
    .unwrap();
    assert_eq!(fields(got), optical((2.0f64 / 7.0).abs()));
    // The largest of the right note columns', note 0 up there too.
    let (got, _) = barred(
        EXTRA,
        "-1 '(-1 . 3.5)",
        "(on! (note 0) (column 1))
         (stem! (stem 0) (note 0) -1 '(-1 . 1) (head! (head 0) (column 1) 0))
         (ly:grob-set-object! (thing 0) 'right-items (arr (note 0) (note 1) (note 2)))",
    )
    .unwrap();
    assert_eq!(fields(got), optical(o));
}

#[test]
fn no_optical_correction_without_a_bar_line_and_a_normal_down_stem() {
    for (stem, setup) in [
        ("1 '(-1 . 3.5)", ""),
        // A whole note's, no head's, no stem.
        ("-1 '(-1 . 3.5)", "(put! (stem 1) 'duration-log 0)"),
        (
            "-1 '(-1 . 3.5)",
            "(ly:grob-set-object! (stem 1) 'note-heads (arr))",
        ),
        ("-1 '(-1 . 3.5)", "(ly:grob-set-object! (note 1) 'stem '())"),
        // A bar line covering nothing, or no bar line.
        ("-1 '(-1 . 3.5)", "(put! (bar 0) 'glyph-name \":|.\")"),
        ("-1 '(-1 . 3.5)", "(put! (bar 0) 'Y-extent '(1 . -1))"),
        (
            "-1 '(-1 . 3.5)",
            "(ly:grob-set-object! (thing 0) 'left-break-aligned (arr (thing 1)))",
        ),
        // Away from the bar line.
        ("-1 '(3 . 5)", ""),
        // No note column among the right items.
        (
            "-1 '(-1 . 3.5)",
            "(ly:grob-set-object! (thing 0) 'right-items (arr (thing 3)))",
        ),
    ] {
        let Some((got, _)) = barred(EXTRA, stem, setup) else {
            return;
        };
        assert_eq!(fields(got), optical(0.0), "{stem} {setup}");
    }
}

#[test]
fn the_optical_correction_is_of_the_original_s_right_note_columns() {
    // Thing 0 is thing 2's piece at the end of a line; bar 0, the bar
    // line on column 0, piece 0's there. Note 1 is thing 2's right item.
    let Some(mut vm) = staff_vm(
        EXTRA,
        "(put! (bar 0) 'X-offset 0)
         (ly:grob-set-parent! (bar 0) 0 (column 0))
         (put! (bar 0) 'X-extent '(-1 . 5))
         (put! (bar 0) 'space-alist (ly:grob-property (thing 1) 'space-alist))
         (put! (bar 0) 'glyph-name \"|\")
         (put! (bar 0) 'Y-offset 0)
         (put! (bar 0) 'Y-extent '(-2 . 2))
         (ly:grob-set-object! (thing 0) 'left-break-aligned (arr (piece 0)))
         (on! (note 1) (column 1))
         (stem! (stem 1) (note 1) -1 '(-1 . 3.5) (head! (head 1) (column 1) 0))
         (ly:grob-set-object! (thing 2) 'right-items (arr (note 1)))",
    ) else {
        return;
    };
    let [me, b0, t2, p0, right] = [
        "(thing 0)",
        "(bar 0)",
        "(thing 2)",
        "(piece 0)",
        "(column 1)",
    ]
    .map(|t| grob_of(&mut vm, t));
    for (orig, piece) in [(t2, me), (p0, b0)] {
        grob::with_grob_mut(vm.heap(), orig, |x| x.broken = [Some(piece), None]);
        grob::with_grob_mut(vm.heap(), piece, |x| x.original = Some(orig));
    }
    let got = staff_spacing::get_spacing(&mut vm, me, right, 0.0).unwrap();
    assert_eq!(fields(got), optical((3.0f64 / 7.0).abs()));
    // Thing 0's own right items do not count.
    eval(
        &mut vm,
        "(ly:grob-set-object! (thing 0) 'right-items (ly:grob-object (thing 2) 'right-items))
         (ly:grob-set-object! (thing 2) 'right-items (arr))",
    );
    let got = staff_spacing::get_spacing(&mut vm, me, right, 0.0).unwrap();
    assert_eq!(fields(got), optical(0.0));
}

/// The distance [`separation_item::set_distance`] gives from column 0 to
/// column 1 of [`columns_vm`] after `setup` at `padding`, and column 0's
/// `minimum-distances` then.
fn set_distance_of(setup: &str, padding: f64) -> Option<(f64, String)> {
    let mut vm = columns_vm(setup)?;
    let [l, r] = ["(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    let d = separation_item::set_distance(&mut vm, l, r, padding)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let mins = eval(
        &mut vm,
        "(map (lambda (e) (cons (eq? (car e) (column 1)) (cdr e)))
              (ly:grob-object (column 0) 'minimum-distances '()))",
    );
    Some((d, shown(&vm, mins)))
}

#[test]
fn the_separation_items_distance_is_padded_and_kept_as_a_rod() {
    // 2.5 against -1.5, 0.25 apart.
    let Some(got) = set_distance_of("", 0.25) else {
        return;
    };
    assert_eq!(got, (4.25, "((#t . 4.25))".to_string()));
    // No rod at 0 or below; 0 at least.
    assert_eq!(set_distance_of("", -4.0).unwrap(), (0.0, "()".to_string()));
    assert_eq!(set_distance_of("", -5.0).unwrap(), (0.0, "()".to_string()));
    // With the right one's conditional elements, to -3.5.
    let cond = "(box! (bit 3) (column 1) '(-3 . -2) '(0 . 1))
                (ly:grob-set-object! (column 1) 'conditional-elements (arr (bit 3)))";
    assert_eq!(
        set_distance_of(cond, 0.0).unwrap(),
        (6.0, "((#t . 6.0))".to_string())
    );
    // Without the right skyline pair, the conditional ones alone; without
    // the left one, none.
    let none = format!("{cond} (put! (column 1) 'horizontal-skylines 'none)");
    assert_eq!(
        set_distance_of(&none, 0.0).unwrap(),
        (6.0, "((#t . 6.0))".to_string())
    );
    let none = "(put! (column 0) 'horizontal-skylines '())";
    assert_eq!(set_distance_of(none, 1.0).unwrap(), (0.0, "()".to_string()));
    // The rod is the items' columns', each item's offset in it added.
    let Some(mut vm) = columns_vm(TWO_SEPS) else {
        return;
    };
    eval(
        &mut vm,
        "(put! (sep 0) 'X-offset 0.5) (put! (sep 1) 'X-offset -1)",
    );
    let [l, r] = ["(sep 0)", "(sep 1)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(
        separation_item::set_distance(&mut vm, l, r, 0.0).unwrap(),
        4.0
    );
    let mins = eval(
        &mut vm,
        "(map cdr (ly:grob-object (column 0) 'minimum-distances '()))",
    );
    assert_eq!(shown(&vm, mins), "(5.5)");
}

#[test]
fn a_separation_item_is_empty_without_a_skyline_either_side() {
    let Some(mut vm) = columns_vm(
        "(ly:grob-set-object! (column 2) 'elements (arr))
         (put! (column 2) 'horizontal-skylines ly:separation-item::calc-skylines)
         (put! (column 3) 'horizontal-skylines 'none)",
    ) else {
        return;
    };
    let empty = |vm: &mut Vm, t: &str| of(vm, t, separation_item::is_empty);
    assert!(!empty(&mut vm, "(column 0)"));
    assert!(empty(&mut vm, "(column 2)"));
    assert!(empty(&mut vm, "(column 3)"));
}

/// A line of `n` paper columns, `(col 0)` to `(col n-1)` (pieces 0 up,
/// ranks 0 up), each `used`, on the system `sys` (group 5), its
/// `columns`, and the spacing spanner `spacer` (group 4) from the first
/// to the last; after `setup`. `(sky! c . boxes)` gives a column
/// separation-item skylines of its `elements`, and `(box! g c x y)` a
/// box on it ([`spacing_vm`]).
fn line_vm(n: usize, setup: &str) -> Option<Vm> {
    let mut vm = spacing_vm("", "'()", "'()")?;
    let sys = grob_of(&mut vm, "(group 5)");
    item::set_class(vm.heap_mut(), sys, item::Class::System);
    let mut cols = Vec::new();
    for i in 0..n {
        let c = grob_of(&mut vm, &format!("(piece {i})"));
        paper_column::make(vm.heap_mut(), c);
        paper_column::set_rank(vm.heap(), c, i as u64);
        cols.push(format!("(piece {i})"));
    }
    eval(
        &mut vm,
        &format!(
            "(define sys (group 5))
             (define spacer (group 4))
             (define (col i) (piece i))
             (define cols (list {}))
             (for-each (lambda (c)
                         (ly:grob-set-parent! c 0 sys)
                         (ly:grob-set-parent! c 1 (thing 5))
                         (put! c 'X-offset 0)
                         (put! c 'Y-offset 0)
                         (put! c 'used #t))
                       cols)
             (ly:grob-set-object! sys 'columns (apply arr cols))
             (ly:grob-set-parent! spacer 1 sys)
             (define (sky! c . elts)
               (ly:grob-set-object! c 'elements (apply arr elts))
               (put! c 'horizontal-skylines ly:separation-item::calc-skylines))",
            cols.join(" ")
        ),
    );
    let [spacer, first, last] =
        ["spacer", "(col 0)", &format!("(col {})", n - 1)].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, spacer, -1, first).unwrap();
    spanner::set_bound(&mut vm, spacer, 1, last).unwrap();
    eval(&mut vm, setup);
    Some(vm)
}

/// The ranks of `cols`.
fn ranks(vm: &mut Vm, cols: &[Value]) -> Vec<u64> {
    cols.iter()
        .map(|&c| paper_column::rank(vm.heap(), c))
        .collect()
}

#[test]
fn the_root_system_s_used_columns_end_at_its_last_breakable_one() {
    // Column 0 and 3 breakable, 2 unused.
    let Some(mut vm) = line_vm(
        5,
        "(put! (col 0) 'line-break-permission 'allow)
         (put! (col 2) 'used '())
         (put! (col 3) 'line-break-permission 'force)",
    ) else {
        return;
    };
    let [spacer, sys, t0] = ["spacer", "sys", "(thing 0)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(system::get_root_system(vm.heap_mut(), spacer), Some(sys));
    assert_eq!(system::get_root_system(vm.heap_mut(), sys), Some(sys));
    // (thing 0)'s Y parent is (thing 5), no system.
    assert_eq!(system::get_root_system(vm.heap_mut(), t0), None);

    // Not past column 3, nor column 2.
    let used = system::used_columns_in_range(&mut vm, sys, 0, 5).unwrap();
    assert_eq!(ranks(&mut vm, &used), [0, 1, 3]);
    let used = system::used_columns_in_range(&mut vm, sys, 1, 3).unwrap();
    assert_eq!(ranks(&mut vm, &used), [1]);
    let used = system::used_columns_in_range(&mut vm, sys, 4, 2).unwrap();
    assert!(used.is_empty());
    // The spanner's, from its bounds: up to column 3.
    let cols = get_columns(&mut vm, spacer).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1, 3]);
    assert_eq!(diagnosed(&mut vm), "()");

    // The columns by rank, paper columns only.
    let c2 = grob_of(&mut vm, "(col 2)");
    assert_eq!(system::column(&mut vm, sys, 2).unwrap(), Some(c2));
    assert_eq!(system::column(&mut vm, sys, 5).unwrap(), None);
    assert_eq!(system::column(&mut vm, sys, u64::MAX).unwrap(), None);
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'columns (arr (col 0) (thing 0) (col 1) (col 3)))",
    );
    assert_eq!(system::column(&mut vm, sys, 1).unwrap(), None);
    let used = system::used_columns_in_range(&mut vm, sys, 0, 9).unwrap();
    assert_eq!(ranks(&mut vm, &used), [0, 1, 3]);

    // No breakable column: none.
    eval(
        &mut vm,
        "(put! (col 0) 'line-break-permission '())
         (put! (col 3) 'line-break-permission '())",
    );
    assert!(
        system::used_columns_in_range(&mut vm, sys, 0, 9)
            .unwrap()
            .is_empty()
    );

    // Bounds that are no columns: LilyPond's programming error, none.
    eval(&mut vm, "(put! (col 3) 'line-break-permission 'allow)");
    let thing = grob_of(&mut vm, "(thing 1)");
    spanner::set_bound(&mut vm, spacer, 1, thing).unwrap();
    assert!(get_columns(&mut vm, spacer).unwrap().is_empty());
    spanner::set_bound(&mut vm, spacer, -1, thing).unwrap();
    assert!(get_columns(&mut vm, spacer).unwrap().is_empty());
    assert_eq!(
        diagnosed(&mut vm),
        "(\"spanner's right bound is not a paper column\" \
          \"spanner's left bound is not a paper column\")"
    );
    // No system above the spanner: none.
    let [c0, c3] = ["(col 0)", "(col 3)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, spacer, -1, c0).unwrap();
    spanner::set_bound(&mut vm, spacer, 1, c3).unwrap();
    let cols = get_columns(&mut vm, spacer).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1, 3]);
    item::set_class(vm.heap_mut(), sys, item::Class::Spanner);
    assert!(get_columns(&mut vm, spacer).unwrap().is_empty());
    assert_eq!(diagnosed(&mut vm), "()");
}

/// A Paper_score of `sys`'s layout with `sys` its root system.
fn paper_score_of(vm: &mut Vm, sys: Value) -> Value {
    let layout = eval(vm, "(ly:grob-layout (staff 0))");
    let ps = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(ps);
    paper_score::typeset_system(vm.heap_mut(), ps, sys);
    ps
}

/// Breaks `(col c)` into pieces `(piece l)` on the left, `(piece r)` on
/// the right, either one left out for `None`.
fn break_col(vm: &mut Vm, c: usize, l: Option<usize>, r: Option<usize>) {
    let col = grob_of(vm, &format!("(col {c})"));
    let [lp, rp] = [l, r].map(|p| p.map(|n| grob_of(vm, &format!("(piece {n})"))));
    grob::with_grob_mut(vm.heap(), col, |g| g.broken = [lp, rp]);
}

#[test]
fn the_breaks_are_the_breakable_columns_broken_on_both_sides() {
    // Columns 0, 1, 3, 4 and 5 breakable, 2 unused. Column 1 is broken
    // only on the right and 4 not at all; 3 on both sides; the first, 0,
    // needs no piece on the left and the last, 5, none on the right.
    let Some(mut vm) = line_vm(
        6,
        "(for-each (lambda (c) (put! (col c) 'line-break-permission 'allow))
                   '(0 1 3 4 5))
         (put! (col 2) 'used '())",
    ) else {
        return;
    };
    break_col(&mut vm, 0, None, Some(18));
    break_col(&mut vm, 1, None, Some(10));
    break_col(&mut vm, 3, Some(11), Some(12));
    break_col(&mut vm, 5, Some(19), None);
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    // Each getter finds them the first time.
    assert_eq!(
        paper_score::get_break_ranks(&mut vm, ps).unwrap(),
        [0, 3, 5]
    );
    let cols = paper_score::get_columns(&mut vm, ps).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1, 3, 4, 5]);
    assert_eq!(
        paper_score::get_break_indices(&mut vm, ps).unwrap(),
        [0, 2, 4]
    );
    let ps2 = paper_score_of(&mut vm, sys);
    let cols = paper_score::get_columns(&mut vm, ps2).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1, 3, 4, 5]);
    assert_eq!(
        paper_score::get_break_ranks(&mut vm, ps2).unwrap(),
        [0, 3, 5]
    );

    // Once found, they are kept, as LilyPond's mutable members are, until
    // found again: column 4 broken on both sides is a break then.
    break_col(&mut vm, 4, Some(13), Some(14));
    assert_eq!(
        paper_score::get_break_indices(&mut vm, ps).unwrap(),
        [0, 2, 4]
    );
    assert_eq!(
        paper_score::get_break_ranks(&mut vm, ps).unwrap(),
        [0, 3, 5]
    );
    paper_score::find_break_indices(&mut vm, ps).unwrap();
    assert_eq!(
        paper_score::get_break_indices(&mut vm, ps).unwrap(),
        [0, 2, 3, 4]
    );
    assert_eq!(
        paper_score::get_break_ranks(&mut vm, ps).unwrap(),
        [0, 3, 4, 5]
    );

    // The first column with no piece on the right is no break, nor the
    // last with none on the left.
    break_col(&mut vm, 0, Some(17), None);
    break_col(&mut vm, 5, None, Some(16));
    paper_score::find_break_indices(&mut vm, ps).unwrap();
    assert_eq!(paper_score::get_break_ranks(&mut vm, ps).unwrap(), [3, 4]);
    assert_eq!(paper_score::get_break_indices(&mut vm, ps).unwrap(), [2, 3]);
    let cols = paper_score::get_columns(&mut vm, ps).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1, 3, 4, 5]);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn no_columns_and_no_breaks_without_a_breakable_column_or_a_system() {
    let Some(mut vm) = line_vm(3, "") else {
        return;
    };
    // No breakable column: no used columns, so nothing found, and each
    // getter looks again.
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    assert!(paper_score::get_columns(&mut vm, ps).unwrap().is_empty());
    assert!(
        paper_score::get_break_indices(&mut vm, ps)
            .unwrap()
            .is_empty()
    );
    // Column 1 breakable but not broken on the left: columns, no breaks.
    eval(&mut vm, "(put! (col 1) 'line-break-permission 'force)");
    assert!(
        paper_score::get_break_ranks(&mut vm, ps)
            .unwrap()
            .is_empty()
    );
    let cols = paper_score::get_columns(&mut vm, ps).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1]);
    // No breaks found, so looked for again: column 1 broken is one.
    break_col(&mut vm, 1, Some(10), None);
    assert_eq!(paper_score::get_break_ranks(&mut vm, ps).unwrap(), [1]);
    assert_eq!(paper_score::get_break_indices(&mut vm, ps).unwrap(), [1]);
    // A column whose line-break-permission is no symbol is not breakable.
    break_col(&mut vm, 0, None, Some(11));
    eval(&mut vm, "(put! (col 0) 'line-break-permission \"allow\")");
    paper_score::find_break_indices(&mut vm, ps).unwrap();
    assert_eq!(paper_score::get_break_indices(&mut vm, ps).unwrap(), [1]);
    eval(&mut vm, "(put! (col 0) 'line-break-permission 'allow)");
    paper_score::find_break_indices(&mut vm, ps).unwrap();
    assert_eq!(paper_score::get_break_indices(&mut vm, ps).unwrap(), [0, 1]);

    // No root system, which LilyPond reads through a null: no columns.
    let layout = eval(&mut vm, "(ly:grob-layout (staff 0))");
    let lone = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(lone);
    assert!(paper_score::get_columns(&mut vm, lone).unwrap().is_empty());
    assert!(
        paper_score::get_break_ranks(&mut vm, lone)
            .unwrap()
            .is_empty()
    );
    // Nor for what is no Paper_score.
    assert!(
        paper_score::get_break_indices(&mut vm, sys)
            .unwrap()
            .is_empty()
    );
    paper_score::find_break_indices(&mut vm, sys).unwrap();
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn scheme_reads_the_break_ranks_and_paper_systems_of_the_paper_score() {
    let Some(mut vm) = line_vm(
        4,
        "(for-each (lambda (c) (put! (col c) 'line-break-permission 'allow))
                   '(0 2 3))",
    ) else {
        return;
    };
    break_col(&mut vm, 0, None, Some(10));
    break_col(&mut vm, 2, Some(11), Some(12));
    break_col(&mut vm, 3, Some(13), None);
    // The spacer's root system (its Y parents') is in no Paper_score yet;
    // a column's Y parent is no system.
    assert_eq!(caught(&mut vm, "(%break-ranks spacer)"), "#()");
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    assert_eq!(caught(&mut vm, "(%break-ranks spacer)"), "#(0 2 3)");
    assert_eq!(caught(&mut vm, "(%break-ranks (col 0))"), "#()");

    // The pure bound: the column of the break ranked start on the left,
    // end on the right; #f for a rank no break has, and for what is no
    // rank.
    let col2 = grob_of(&mut vm, "(col 2)");
    let col3 = grob_of(&mut vm, "(col 3)");
    for (args, got) in [
        ("-1 2 3", Some(col2)),
        ("1 0 3", Some(col3)),
        ("1 0 2", Some(col2)),
        ("-1 1 3", None),
        ("1 0 1", None),
        ("-1 -2 3", None),
        ("-1 2.0 3", None),
    ] {
        let v = eval(&mut vm, &format!("(%system-pure-bound sys {args})"));
        assert_eq!(v, got.unwrap_or(Value::FALSE), "{args}");
    }
    // A system in no Paper_score has none.
    let spacer = grob_of(&mut vm, "spacer");
    item::set_class(vm.heap_mut(), spacer, item::Class::System);
    assert_eq!(caught(&mut vm, "(%system-pure-bound spacer -1 0 3)"), "#f");

    // The paper systems, once made, are kept; what is no Paper_score is
    // LY_ASSERT_SMOB's type error.
    let f = eval(&mut vm, "ly:paper-score-paper-systems");
    let got = caught(&mut vm, "(ly:paper-score-paper-systems sys)");
    assert!(
        got.contains("ly:paper-score-paper-systems")
            && got.contains("Wrong type argument in position 1 (expecting Paper_score)"),
        "{got}"
    );
    let systems = eval(&mut vm, "(vector 'a 'b)");
    vm.heap_mut()
        .foreign_mut::<PaperScore>(ps)
        .unwrap()
        .paper_systems = systems;
    assert_eq!(Host::call(&mut vm, f, &[ps]).unwrap(), systems);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn c_variable_is_the_layout_s_variable_or_undefined() {
    let Some(mut vm) = grobs_vm() else { return };
    let layout = eval(
        &mut vm,
        "(let ((layout (ly:grob-layout (staff 0))))
           (ly:output-def-set-variable! layout 'system-count 2)
           layout)",
    );
    let v = paper_score::c_variable(&mut vm, layout, "system-count");
    assert_eq!(shown(&vm, v), "2");
    let v = paper_score::c_variable(&mut vm, layout, "no-such-variable");
    assert_eq!(v, Value::UNDEFINED);
    // Through its parents, up to the root.
    let child = eval(&mut vm, "(ly:make-output-def)");
    lily_engine_music::set_parent(vm.heap_mut(), child, layout);
    let v = paper_score::c_variable(&mut vm, child, "system-count");
    assert_eq!(shown(&vm, v), "2");
}

/// [`line_vm`]'s four columns, 0, 2 and 3 breaks, each its own piece, with
/// springs (2, 1), (3, 1) and (4, 2) between neighbours, in a Paper_score
/// whose layout is 10 wide, indented 3, then 1, with `layout` set.
fn breaking_vm(layout: &str) -> Option<(Vm, Value)> {
    let mut vm = line_vm(
        4,
        "(for-each (lambda (c) (put! (col c) 'line-break-permission 'allow))
                   '(0 2 3))",
    )?;
    let c: Vec<Value> = (0..4)
        .map(|i| grob_of(&mut vm, &format!("(col {i})")))
        .collect();
    for (i, (l, r)) in [(false, true), (false, false), (true, true), (true, false)]
        .into_iter()
        .enumerate()
    {
        grob::with_grob_mut(vm.heap(), c[i], |g| {
            g.broken = [l.then_some(c[i]), r.then_some(c[i])]
        });
    }
    for (i, (ideal, min)) in [(2.0, 1.0), (3.0, 1.0), (4.0, 2.0)].into_iter().enumerate() {
        spaceable::add_spring(&mut vm, c[i], c[i + 1], &set_spring(ideal, min, None)).unwrap();
    }
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    eval(
        &mut vm,
        &format!(
            "(let ((l (ly:grob-layout (staff 0))))
               (ly:output-def-set-variable! l 'line-width 10)
               (ly:output-def-set-variable! l 'indent 3)
               (ly:output-def-set-variable! l 'short-indent 1)
               (for-each (lambda (v) (ly:output-def-set-variable! l (car v) (cdr v)))
                         (list {layout})))"
        ),
    );
    Some((vm, ps))
}

#[test]
fn the_lines_between_the_breaks_take_the_line_forces() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert_eq!(cb.pscore, Some(ps));
    assert_eq!(cb.breaks, [0, 2, 3]);
    assert_eq!(ranks(&mut vm, &cb.all), [0, 1, 2, 3]);
    assert!(!cb.ragged_right && !cb.ragged_last);
    // 9 long (10 less the short indent), the first line 2 shorter.
    let forces = get_line_forces(&mut vm, &cb.all, 9.0, 2.0, false).unwrap();
    // The line from break i to break j is at (j, i); (i, j) is left as
    // made, infinite.
    for (i, j) in [(0, 1), (0, 2), (1, 2)] {
        assert_eq!(cb.lines.at(j, i).force, forces[i * 3 + j], "{i} {j}");
        assert_eq!(cb.lines.at(i, j).force, f64::INFINITY, "{i} {j}");
    }
    // The first line, 7 long, of springs 5 then 9 long at rest: stretched,
    // then squeezed.
    assert!(cb.lines.at(1, 0).force > 0.0);
    assert!(cb.lines.at(2, 0).force < 0.0);
    // Each line that fits is filled in (fill_line_details): its last
    // column, and with no vertical alignment, empty shapes and refpoints
    // as (0, 0) and no spacing: an inverse Hooke's constant of 0; the
    // page permission unset, so none; (i, j) keeps the defaults.
    let c2 = grob_of(&mut vm, "(col 3)");
    let line = cb.lines.at(2, 1);
    assert_eq!(line.last_column, Some(c2));
    let zero = Interval::new(0.0, 0.0);
    assert_eq!((line.shape.begin, line.shape.rest), (zero, zero));
    assert_eq!((line.refpoint_extent, line.inverse_hooke), (zero, 0.0));
    let allow = vm.heap_mut().intern("allow");
    assert_eq!(line.break_permission, allow);
    assert_eq!(
        (line.page_permission, line.turn_permission),
        (Value::NULL, Value::NULL)
    );
    assert_eq!(cb.lines.at(1, 2), &LineDetails::new(vm.heap_mut()));
    // Started at the first column: break 0, with its state to fill.
    assert_eq!(cb.start, [0]);
    assert_eq!(cb.starting_breakpoints, [0]);
    assert_eq!(cb.state.len(), 1);
    assert_eq!(cb.state[0].rows(), 0);
    assert_eq!((cb.systems, cb.valid_systems), (0, 0));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_ragged_last_line_is_free_unless_squeezed_past_a_break() {
    let Some((mut vm, ps)) = breaking_vm("'(ragged-last . #t) '(ragged-right . 1)") else {
        return;
    };
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    // `#t` alone is true.
    assert!(cb.ragged_last && !cb.ragged_right);
    let forces = get_line_forces(&mut vm, &cb.all, 9.0, 2.0, false).unwrap();
    assert_eq!(cb.lines.at(1, 0).force, forces[1]);
    // Squeezed over two lines' breaks: infinite; stretched: 0.
    assert_eq!(cb.lines.at(2, 0).force, f64::INFINITY);
    assert!(forces[5] > 0.0);
    assert_eq!(cb.lines.at(2, 1).force, 0.0);

    // Ragged right: the forces as ragged, every last line free.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'ragged-right #t)",
    );
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert!(cb.ragged_right);
    let forces = get_line_forces(&mut vm, &cb.all, 9.0, 2.0, true).unwrap();
    assert_eq!(cb.lines.at(1, 0).force, forces[1]);
    assert_eq!(cb.lines.at(2, 1).force, 0.0);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_system_spacings_come_from_the_layout_s_spacing_specs() {
    let Some((mut vm, ps)) = breaking_vm(
        "(cons 'system-system-spacing
               '((basic-distance . 12) (padding . 1) (minimum-distance . 8)))
         (cons 'page-breaking-system-system-spacing '((padding . 2)))
         (cons 'score-system-spacing '((padding . 3) (minimum-distance . 14)))
         (cons 'score-markup-spacing
               '((basic-distance . 5) (padding . 1/2) (minimum-distance . 4)))",
    ) else {
        return;
    };
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    // The page-breaking spacing read after the system one, over it.
    assert_eq!(
        [
            cb.system_system_space,
            cb.system_markup_space,
            cb.system_system_padding,
            cb.score_system_padding,
            cb.score_markup_padding,
            cb.score_system_min_distance,
            cb.system_system_min_distance,
            cb.score_markup_min_distance,
        ],
        [12.0, 5.0, 2.0, 3.0, 0.5, 14.0, 8.0, 4.0]
    );
    // A dimension that is no real is Guile's type error.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'short-indent 'wide)",
    );
    let got = ConstrainedBreaking::new(&mut vm, Some(ps))
        .unwrap_err()
        .text(vm.heap());
    assert!(got.contains("real number"), "{got}");
}

#[test]
fn breaking_starts_at_the_first_break_not_before_each_start() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    // Breaks at columns 0, 2 and 3; a start past the end at the last.
    let cb = ConstrainedBreaking::with_starts(&mut vm, Some(ps), &[0, 1, 2, 2, 3, 9]).unwrap();
    assert_eq!(cb.starting_breakpoints, [0, 1, 1, 1, 2, 2]);
    assert_eq!(cb.start, [0, 2, 2, 2, 3, 3]);
    assert_eq!(cb.state.len(), 6);

    // No Paper_score, or what is none: nothing, not ragged.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'ragged-right #t)",
    );
    let sys = grob_of(&mut vm, "sys");
    for ps in [None, Some(sys)] {
        let cb = ConstrainedBreaking::new(&mut vm, ps).unwrap();
        assert_eq!(cb.pscore, None);
        assert!(!cb.ragged_right);
        assert!(cb.all.is_empty() && cb.breaks.is_empty() && cb.start.is_empty());
        assert_eq!(cb.lines.rows(), 0);
    }

    // No break (a Paper_score with no root system has no columns):
    // nothing to start at, where LilyPond reads past the end.
    let layout = eval(&mut vm, "(ly:grob-layout (staff 0))");
    let lone = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(lone);
    let cb = ConstrainedBreaking::with_starts(&mut vm, Some(lone), &[0, 4]).unwrap();
    assert_eq!(cb.pscore, Some(lone));
    assert!(cb.ragged_right);
    assert!(cb.all.is_empty() && cb.breaks.is_empty());
    assert!(cb.start.is_empty() && cb.starting_breakpoints.is_empty() && cb.state.is_empty());
    assert_eq!(diagnosed(&mut vm), "()");
}

/// `xs` within a rounding of `want`.
fn assert_close(xs: &[f64], want: &[f64]) {
    assert_eq!(xs.len(), want.len(), "{xs:?}");
    for (x, w) in xs.iter().zip(want) {
        assert!((x - w).abs() < 1e-12, "{xs:?} {want:?}");
    }
}

#[test]
fn a_line_is_spaced_between_its_breaks_from_its_indent() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    // The first line, columns 0 to 2: 7 long (10 less the indent), from
    // 3; springs of 2 and 3 stretched by 7/5, both inverse strengths
    // their lengths.
    let got = cb.space_line(&mut vm, 0, 1).unwrap();
    assert_eq!(ranks(&mut vm, &got.cols), [0, 1, 2]);
    assert!(got.loose_cols.is_empty());
    assert_close(&got.config, &[3.0, 5.8, 10.0]);
    assert!(got.satisfies_constraints);
    let want = get_line_configuration(&mut vm, &cb.all[0..=2], 7.0, 3.0, false).unwrap();
    assert_eq!((got.config, got.force), (want.config, want.force));
    // The last, columns 2 and 3: 9 long, from the short indent; not the
    // only line, so not ragged.
    let got = cb.space_line(&mut vm, 1, 2).unwrap();
    assert_eq!(ranks(&mut vm, &got.cols), [2, 3]);
    assert_eq!(got.config, [1.0, 10.0]);
    let want = get_line_configuration(&mut vm, &cb.all[2..=3], 9.0, 1.0, false).unwrap();
    assert_eq!(got.force, want.force);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_only_line_is_ragged_unless_raggedness_is_set() {
    let Some((mut vm, ps)) = breaking_vm("'(line-width . 20)") else {
        return;
    };
    // Columns 0 to 3, 17 long from 3; at rest 9.
    let at_rest = [3.0, 5.0, 8.0, 12.0];
    let stretched = [3.0, 3.0 + 2.0 * 17.0 / 9.0, 3.0 + 5.0 * 17.0 / 9.0, 20.0];
    let only_line = |vm: &mut Vm, set: &str, len: f64| {
        eval(
            vm,
            &format!(
                "(let ((l (ly:grob-layout (staff 0))))
                   (for-each (lambda (v) (ly:output-def-set-variable! l (car v) (cdr v)))
                             (list {set})))"
            ),
        );
        let cb = ConstrainedBreaking::new(vm, Some(ps)).unwrap();
        // Read where initialize never writes: infinite, not negative.
        assert_eq!(cb.lines.at(0, 2).force, f64::INFINITY);
        let got = cb.space_line(vm, 0, 2).unwrap();
        let mut want = |ragged| {
            get_line_configuration(vm, &cb.all, len, 3.0, ragged)
                .unwrap()
                .force
        };
        let ragged = got.force == want(true);
        assert!(ragged || got.force == want(false));
        (got.config, ragged)
    };
    // Neither set, or set to what is no boolean: ragged.
    let (config, ragged) = only_line(&mut vm, "", 17.0);
    assert!(ragged);
    assert_eq!(config, at_rest);
    let (_, ragged) = only_line(&mut vm, "'(ragged-right . 1) '(ragged-last . ())", 17.0);
    assert!(ragged);
    // Either a boolean: ragged only if true.
    let (config, ragged) = only_line(&mut vm, "'(ragged-right . #f)", 17.0);
    assert!(!ragged);
    assert_close(&config, &stretched);
    let (_, ragged) = only_line(&mut vm, "'(ragged-right . 1) '(ragged-last . #f)", 17.0);
    assert!(!ragged);
    let (config, ragged) = only_line(&mut vm, "'(ragged-last . #t)", 17.0);
    assert!(ragged);
    assert_eq!(config, at_rest);
    let (_, ragged) = only_line(&mut vm, "'(ragged-right . #t) '(ragged-last . #f)", 17.0);
    assert!(ragged);
    // Squeezed, the line from 0 to 2 has a negative force, but the one
    // read is infinite: still ragged with neither a boolean.
    let (_, ragged) = only_line(
        &mut vm,
        "'(ragged-right . 1) '(ragged-last . 1) '(line-width . 8)",
        5.0,
    );
    assert!(ragged);
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert!(cb.lines.at(2, 0).force < 0.0);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn spacing_a_line_reads_the_layout_again() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    // Not ragged when broken; ragged-right set after: ragged, at rest.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'ragged-right #t)",
    );
    let got = cb.space_line(&mut vm, 1, 2).unwrap();
    assert_eq!(got.config, [1.0, 5.0]);
    // A dimension that is no real is Guile's type error.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'short-indent 'wide)",
    );
    let got = cb.space_line(&mut vm, 1, 2).unwrap_err().text(vm.heap());
    assert!(got.contains("real number"), "{got}");
    // The first line reads the indent, not the short indent.
    assert_eq!(cb.space_line(&mut vm, 0, 1).unwrap().config[0], 3.0);
    // No Paper_score: no columns.
    let none = ConstrainedBreaking::new(&mut vm, None).unwrap();
    let got = none.space_line(&mut vm, 0, 1).unwrap();
    assert!(got.cols.is_empty() && got.config.is_empty());
    assert!(got.satisfies_constraints);
}

/// [`line_vm`]'s five columns, 1, 3 and 4 on `sys` at 2, 10 and 14,
/// column 1 one wide, and column 2, a quarter wide, hanging between
/// columns 1 and 3 with the spacer as its spacing; after `setup`. None
/// is musical or breakable, so a loose column's spring to the next is
/// 0.5 ideal and 0 at least: as wide as itself (0.25) at least.
fn loose_vm(setup: &str) -> Option<Vm> {
    let mut vm = line_vm(
        5,
        &format!(
            "(put! (col 1) 'X-offset 2)
             (put! (col 1) 'X-extent '(0 . 1))
             (put! (col 3) 'X-offset 10)
             (put! (col 4) 'X-offset 14)
             (put! (col 2) 'X-extent '(0 . 0.25))
             (ly:grob-set-object! (col 2) 'between-cols (cons (col 1) (col 3)))
             (ly:grob-set-object! (col 2) 'spacing spacer)
             {setup}"
        ),
    )?;
    let sys = grob_of(&mut vm, "sys");
    for t in ["(col 1)", "(col 3)", "(col 4)"] {
        let c = grob_of(&mut vm, t);
        paper_column::set_system(vm.heap(), c, sys);
    }
    Some(vm)
}

/// `set_loose_columns` on `sys` for the loose columns `loose`, then where
/// they are on `sys`.
fn place_loose(vm: &mut Vm, loose: &[&str]) -> Vec<f64> {
    let sys = grob_of(vm, "sys");
    let loose_cols: Vec<_> = loose.iter().map(|t| grob_of(vm, t)).collect();
    let posns = ColumnXPositions {
        loose_cols: loose_cols.clone(),
        ..ColumnXPositions::default()
    };
    set_loose_columns(vm, sys, &posns).unwrap();
    for &c in &loose_cols {
        assert_eq!(paper_column::system(vm.heap(), c), Some(sys));
    }
    loose
        .iter()
        .map(|t| {
            let v = eval(vm, &format!("(ly:grob-relative-coordinate {t} sys 0)"));
            vm.heap().to_f64(v).unwrap()
        })
        .collect()
}

#[test]
fn a_loose_column_hangs_its_spring_left_of_its_right_neighbour() {
    let Some(mut vm) = loose_vm("") else {
        return;
    };
    // Room enough (7 from column 1's right edge to column 3): the ideal
    // 0.5.
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [9.5]);
    assert_eq!(diagnosed(&mut vm), "()");
    // No loose columns: nothing moves, nothing is put on the system.
    let mut vm = loose_vm("").unwrap();
    assert!(place_loose(&mut vm, &[]).is_empty());
    let c2 = grob_of(&mut vm, "(col 2)");
    assert_eq!(paper_column::system(vm.heap(), c2), None);

    // Squeezed into 0.5 less the padding of 0.15: from the minimum 0.25
    // two fifths of the way to the ideal, with LilyPond's one rounding.
    let mut vm = loose_vm("(put! (col 3) 'X-offset 3.5)").unwrap();
    let scale = ((0.5f64 - 0.15) - 0.25) / (0.5 - 0.25);
    assert_eq!(
        place_loose(&mut vm, &["(col 2)"]),
        [3.5 - (0.5f64 - 0.25).mul_add(scale, 0.25)]
    );
    // Crammed into less than its minimum: its minimum.
    let mut vm = loose_vm("(put! (col 3) 'X-offset 3.2)").unwrap();
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [3.2 - 0.25]);

    // No spacing object: the default options, the same spring here.
    let mut vm = loose_vm("(ly:grob-set-object! (col 2) 'spacing '())").unwrap();
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [9.5]);
    assert_eq!(diagnosed(&mut vm), "(\"Column without spacing object\")");
}

#[test]
fn a_spaced_line_s_columns_are_moved_to_their_positions_on_the_system_then_its_loose_ones() {
    // Columns 0, 1, 3 and 4 at 0, 2, 10 and 14; column 2 loose between 1
    // and 3, hung 0.5 left of 3 as set_loose_columns hangs it.
    let Some(mut vm) = line_vm(
        5,
        "(put! (col 1) 'X-extent '(0 . 1))
         (put! (col 2) 'X-extent '(0 . 0.25))
         (ly:grob-set-object! (col 2) 'between-cols (cons (col 1) (col 3)))
         (ly:grob-set-object! (col 2) 'spacing spacer)",
    ) else {
        return;
    };
    let sys = grob_of(&mut vm, "sys");
    let cols: Vec<Value> = [0, 1, 3, 4]
        .map(|i| grob_of(&mut vm, &format!("(col {i})")))
        .into();
    let loose = grob_of(&mut vm, "(col 2)");
    let posns = ColumnXPositions {
        cols: cols.clone(),
        loose_cols: vec![loose],
        config: vec![0.0, 2.0, 10.0, 14.0],
        ..ColumnXPositions::default()
    };
    system::place_line(&mut vm, sys, &posns).unwrap();
    let at = |vm: &mut Vm, i: usize| {
        let v = eval(
            vm,
            &format!("(ly:grob-relative-coordinate (col {i}) sys 0)"),
        );
        vm.heap().to_f64(v).unwrap()
    };
    assert_eq!(
        (0..5).map(|i| at(&mut vm, i)).collect::<Vec<_>>(),
        [0.0, 2.0, 9.5, 10.0, 14.0]
    );
    for c in cols.into_iter().chain([loose]) {
        assert_eq!(paper_column::system(vm.heap(), c), Some(sys));
    }
    assert_eq!(diagnosed(&mut vm), "()");

    // An infinite position: LilyPond's programming error, the column not
    // moved but put on the system.
    let mut vm = line_vm(2, "").unwrap();
    let [sys, c0] = ["sys", "(col 0)"].map(|t| grob_of(&mut vm, t));
    let posns = ColumnXPositions {
        cols: vec![c0],
        config: vec![f64::INFINITY],
        ..ColumnXPositions::default()
    };
    system::place_line(&mut vm, sys, &posns).unwrap();
    assert_eq!(at(&mut vm, 0), 0.0);
    assert_eq!(paper_column::system(vm.heap(), c0), Some(sys));
    assert_eq!(diagnosed(&mut vm), "(\"Infinity or NaN encountered\")");
}

#[test]
fn calc_breaking_solves_into_the_system_count_or_the_best_number_of_lines() {
    // Lines 0-1 stretched (0.4), 0-2 squeezed (-0.4512), 1-2 stretched
    // (1.25): one line's demerits 2 × 0.4512² = 0.407, two lines' 2 ×
    // 0.4² + 1.25² + 0.85² = 2.605. The best is one line.
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    let got = paper_score::calc_breaking(&mut vm, ps).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(ranks(&mut vm, &got[0].cols), [0, 1, 2, 3]);
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    let want = cb.space_line(&mut vm, 0, 2).unwrap();
    assert_eq!((&got[0].config, got[0].force), (&want.config, want.force));
    assert_eq!(diagnosed(&mut vm), "()");

    // The layout's system-count: solve's two lines.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'system-count 2)",
    );
    let got = paper_score::calc_breaking(&mut vm, ps).unwrap();
    assert_eq!(got.len(), 2);
    let want = [(0, 1), (1, 2)].map(|(i, j)| cb.space_line(&mut vm, i, j).unwrap());
    for (got, want) in got.iter().zip(&want) {
        assert_eq!(ranks(&mut vm, &got.cols), ranks(&mut vm, &want.cols));
        assert_eq!((&got.config, got.force), (&want.config, want.force));
    }
    // A count that is no exact positive integer: the best.
    for count in ["0", "-2", "2.0", "'two"] {
        eval(
            &mut vm,
            &format!(
                "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'system-count {count})"
            ),
        );
        assert_eq!(
            paper_score::calc_breaking(&mut vm, ps).unwrap().len(),
            1,
            "{count}"
        );
    }
    assert_eq!(diagnosed(&mut vm), "()");

    // One break (column 0, the last breakable one): at most no systems,
    // so no line, as LilyPond's; and none for what is no Paper_score.
    let mut vm = line_vm(3, "(put! (col 0) 'line-break-permission 'allow)").unwrap();
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    eval(
        &mut vm,
        "(let ((l (ly:grob-layout (staff 0))))
           (for-each (lambda (v) (ly:output-def-set-variable! l v 0))
                     '(line-width indent short-indent)))",
    );
    assert_eq!(paper_score::get_break_indices(&mut vm, ps).unwrap(), [0]);
    assert!(paper_score::calc_breaking(&mut vm, ps).unwrap().is_empty());
    assert!(
        paper_score::calc_breaking(&mut vm, Value::FALSE)
            .unwrap()
            .is_empty()
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_breaker_chooses_lines_by_their_demerits_and_constraints() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    let mut cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert_eq!(cb.min_system_count(vm.heap_mut(), 0, VPOS), 1);
    assert_eq!(cb.max_system_count(0, VPOS), 2);
    // One system found in the programme prepare_solution sized for one.
    assert_eq!(
        (cb.valid_systems, cb.state[0].rows(), cb.state[0].columns()),
        (1, 3, 1)
    );
    let forces = |v: &[ColumnXPositions]| v.iter().map(|c| c.force).collect::<Vec<_>>();
    let one = cb.solve(&mut vm, 0, VPOS, 1).unwrap();
    assert_eq!(ranks(&mut vm, &one[0].cols), [0, 1, 2, 3]);
    let two = cb.solve(&mut vm, 0, VPOS, 2).unwrap();
    assert_eq!(forces(&two), [0.4, 1.25]);
    // The demerits: each line's force squared and its change from the
    // line before squared, the first's from 0.
    let dem = |cb: &ConstrainedBreaking, k: usize| cb.state[0].at(2, k - 1).demerits;
    assert!((dem(&cb, 1) - 2.0 * 0.4512f64.powi(2)).abs() < 1e-12);
    assert!((dem(&cb, 2) - 2.605).abs() < 1e-12);
    assert_eq!(cb.state[0].at(2, 1).prev, Some(1));
    // More systems than breaks: the most there are, unwarned.
    assert_eq!(forces(&cb.solve(&mut vm, 0, VPOS, 3).unwrap()), [0.4, 1.25]);
    assert!(cb.solve(&mut vm, 0, VPOS, 0).unwrap().is_empty());
    assert_eq!(cb.best_solution(&mut vm, 0, VPOS).unwrap().len(), 1);
    // The details of the lines solve gives, as the programme keeps them.
    let details = cb.line_details(&mut vm, 0, VPOS, 2).unwrap();
    assert_eq!(
        details,
        [cb.lines.at(1, 0).clone(), cb.lines.at(2, 1).clone()]
    );
    assert_eq!(diagnosed(&mut vm), "()");

    // A penalty on the break between: still one line. A bonus big enough:
    // two.
    for (penalty, lines) in [(10, 1), (-10, 2)] {
        eval(
            &mut vm,
            &format!("(put! (col 2) 'line-break-penalty {penalty})"),
        );
        let mut cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
        assert_eq!(cb.lines.at(1, 0).break_penalty, f64::from(penalty));
        assert_eq!(cb.best_solution(&mut vm, 0, VPOS).unwrap().len(), lines);
    }
    eval(&mut vm, "(put! (col 2) 'line-break-penalty '())");

    // A forced break (\break): no line over it, so two lines, the
    // fewest that fit.
    eval(&mut vm, "(put! (col 2) 'line-break-permission 'force)");
    let mut cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert_eq!(cb.lines.at(2, 0).force, f64::INFINITY);
    assert_eq!(cb.min_system_count(vm.heap_mut(), 0, VPOS), 2);
    // Grown three systems past the one it lacked.
    assert_eq!((cb.valid_systems, cb.state[0].columns()), (4, 4));
    assert_eq!(
        forces(&cb.best_solution(&mut vm, 0, VPOS).unwrap()),
        [0.4, 1.25]
    );
    // One system asked of it: no solution, the line from the break
    // before the last to the end, then the good part, with LilyPond's
    // warning.
    let got = cb.solve(&mut vm, 0, VPOS, 1).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(ranks(&mut vm, &got[0].cols), [0, 1, 2, 3]);
    assert_eq!(
        diagnosed(&mut vm),
        "(\"cannot find line breaking that satisfies constraints\")"
    );
    // Its details: the line's filled in, its force infinite.
    let details = cb.line_details(&mut vm, 0, VPOS, 1).unwrap();
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].force, f64::INFINITY);
    assert_eq!(details[0].last_column, Some(grob_of(&mut vm, "(col 3)")));
    eval(&mut vm, "(put! (col 2) 'line-break-permission 'allow)");

    // Ragged right: a line's demerits are its force squared alone.
    let (mut vm2, ps2) = breaking_vm("'(ragged-right . #t)").unwrap();
    let mut cb = ConstrainedBreaking::new(&mut vm2, Some(ps2)).unwrap();
    cb.best_solution(&mut vm2, 0, VPOS).unwrap();
    let (f01, f12) = (cb.lines.at(1, 0).force, cb.lines.at(2, 1).force);
    assert_eq!(cb.state[0].at(2, 1).demerits, f01 * f01 + f12 * f12);

    // Starting at a later column: the breaks from its break on, and
    // ending at a later starting column.
    let mut cb = ConstrainedBreaking::with_starts(&mut vm, Some(ps), &[0, 2]).unwrap();
    assert_eq!(cb.starting_breakpoints, [0, 1]);
    assert_eq!(cb.max_system_count(0, 1), 1);
    assert_eq!(cb.max_system_count(1, VPOS), 1);
    let got = cb.solve(&mut vm, 1, VPOS, 1).unwrap();
    assert_eq!(ranks(&mut vm, &got[0].cols), [2, 3]);
    let got = cb.best_solution(&mut vm, 0, 1).unwrap();
    assert_eq!(ranks(&mut vm, &got[0].cols), [0, 1, 2]);

    // No Paper_score: nothing.
    let mut none = ConstrainedBreaking::new(&mut vm, None).unwrap();
    assert!(none.solve(&mut vm, 0, VPOS, 1).unwrap().is_empty());
    assert!(none.best_solution(&mut vm, 0, VPOS).unwrap().is_empty());
    assert!(none.line_details(&mut vm, 0, VPOS, 1).unwrap().is_empty());
    assert_eq!(none.min_system_count(vm.heap_mut(), 0, VPOS), 1);
    assert_eq!(none.max_system_count(0, VPOS), 0);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_line_s_details_are_its_column_s_and_the_system_s_heights() {
    let Some((mut vm, ps)) = breaking_vm(
        "'(system-system-spacing . ((basic-distance . 12) (padding . 1) (minimum-distance . 8)))
         '(score-system-spacing . ((padding . 2) (minimum-distance . 9)))
         '(score-markup-spacing . ((basic-distance . 5) (padding . 3) (minimum-distance . 4)))",
    ) else {
        return;
    };
    // The System's heights by part of the line, its alignment's
    // translations and its footnotes, stood in for: each recorded.
    eval(
        &mut vm,
        "(define asked '())
         (define (%system-part-of-line-pure-height me start end begin)
           (set! asked (cons (list 'height start end begin) asked))
           (if begin (cons (- start 4) 1) (if (= end 3) '(+nan.0 . 2) '(+inf.0 . -inf.0))))
         (define al (align 0))
         (ly:grob-set-object! sys 'vertical-alignment al)
         (ly:grob-set-object! al 'elements (arr (staff 0) (staff 1) (group 0)))
         (put! (staff 0) 'staff-affinity 1)
         (define (%align-pure-minimum-translations me all a start end)
           (set! asked (cons (list 'translations (length all) a start end) asked))
           '(0 -6 -15))
         (define (%system-footnote-grobs-in-range sys start end)
           (list (thing 0) (thing 1) (thing 2)))
         (put! (thing 0) 'footnote #t)
         (put! (thing 0) 'footnote-text \"abc\")
         (put! (thing 1) 'footnote-text \"ab\")
         (put! (thing 2) 'footnote #t)
         (put! (thing 2) 'footnote-text 7)
         (define (%interpret-markup layout props m)
           (ly:make-stencil '() '(0 . 1) (cons 1 (+ 1 (string-length m)))))
         (put! (col 2) 'page-break-penalty 3)
         (put! (col 2) 'page-turn-penalty 'x)
         (put! (col 2) 'page-break-permission 'allow)
         (put! (col 2) 'page-turn-permission 'other)",
    );
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    // Not last: the system spacing; its begin from rank 0, its rest
    // empty, so (0, 0); the refpoints of staves 1 and 2, the spaceable
    // ones; the footnotes' and in-notes' heights.
    let line = cb.lines.at(1, 0);
    assert_eq!(line.shape.begin, Interval::new(-4.0, 1.0));
    assert_eq!(line.shape.rest, Interval::new(0.0, 0.0));
    assert_eq!(line.refpoint_extent, Interval::new(-15.0, -6.0));
    assert_eq!(
        (line.padding, line.min_distance, line.space),
        (1.0, 8.0, 12.0)
    );
    assert_eq!(
        (
            line.title_padding,
            line.title_min_distance,
            line.title_space
        ),
        (3.0, 4.0, 5.0)
    );
    assert_eq!(line.inverse_hooke, 5.0 + 12.0);
    assert_eq!(
        (line.footnote_heights.clone(), line.in_note_heights.clone()),
        (vec![3.0], vec![2.0])
    );
    // Its column's penalties, a number's or 0, and permissions, the
    // page's no looser than the line's (allow), the turn's no looser than
    // the page's.
    assert_eq!(
        (line.break_penalty, line.page_penalty, line.turn_penalty),
        (0.0, 3.0, 0.0)
    );
    let [allow, other] = ["allow", "other"].map(|s| vm.heap_mut().intern(s));
    assert_eq!((line.page_permission, line.turn_permission), (allow, other));
    // Last: the score's spacing, and a NaN end makes its rest (0, 0).
    let line = cb.lines.at(2, 1);
    assert_eq!((line.padding, line.min_distance), (2.0, 9.0));
    assert_eq!(line.shape.begin, Interval::new(-2.0, 1.0));
    assert_eq!(line.shape.rest, Interval::new(0.0, 0.0));
    assert_eq!(evaluated(&mut vm, "(length asked)"), "9");
    assert_eq!(
        evaluated(&mut vm, "(list-tail (reverse asked) 6)"),
        "((height 2 3 #t) (height 2 3 #f) (translations 3 1 2 3))"
    );
    // No spaceable staff: refpoints empty, so (0, 0).
    eval(
        &mut vm,
        "(put! (staff 1) 'staff-affinity 1) (put! (group 0) 'staff-affinity 1)",
    );
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    assert_eq!(cb.lines.at(1, 0).refpoint_extent, Interval::new(0.0, 0.0));
    assert_eq!(diagnosed(&mut vm), "()");

    // min_permission: force gives way to anything, allow to all but
    // force, anything else to nothing.
    let heap = vm.heap_mut();
    let [force, allow, other] = ["force", "allow", "other"].map(|s| heap.intern(s));
    for (a, b, want) in [
        (force, other, other),
        (allow, other, other),
        (allow, force, Value::NULL),
        (other, allow, Value::NULL),
        (Value::NULL, force, Value::NULL),
    ] {
        assert_eq!(min_permission(heap, a, b), want);
    }
}

/// A `Paper_book` printing [`breaking_vm`]'s Paper_score and a markup list
/// of one line, on a paper with `paper` set; the System's heights stood
/// in for, -3 to 2 in either part of any line.
fn breaking_book(layout: &str, paper: &str) -> Option<(Vm, Value, Value)> {
    let (mut vm, ps) = breaking_vm(layout)?;
    eval(
        &mut vm,
        &format!(
            "(define-public (layout-extract-page-properties layout) '())
             (define-public (markup-list? x) (and (pair? x) (eq? (car x) 'lines)))
             (define-public (interpret-markup-list paper props ml)
               (map (lambda (h) (ly:make-stencil '() '(0 . 10) (cons 0 h))) (cdr ml)))
             (define (%system-part-of-line-pure-height me start end begin) '(-3 . 2))
             (define book-paper (ly:make-output-def))
             (ly:output-def-set-variable! book-paper 'line-width 10)
             (for-each (lambda (v) (ly:output-def-set-variable! book-paper (car v) (cdr v)))
                       (list {paper}))
             (for-each (lambda (c) (put! (col c) 'page-break-permission 'allow)) '(2 3))"
        ),
    );
    let paper = eval(&mut vm, "book-paper");
    let lines = eval(&mut vm, "'(lines 4)");
    let book = vm.heap_mut().wrap(PaperBook {
        paper,
        parent: Value::FALSE,
        header: Value::NULL,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: vec![ps, lines],
        print_bookparts: false,
        performances: Vec::new(),
    });
    vm.heap_mut().root(book);
    Some((vm, ps, book))
}

#[test]
fn page_breaking_chunks_a_book_and_configures_its_lines() {
    let Some((mut vm, ps, book)) = breaking_book(
        "'(system-system-spacing . ((padding . 1) (minimum-distance . 8)))
         '(score-system-spacing . ((padding . 2) (minimum-distance . 9)))",
        "",
    ) else {
        return;
    };
    let mut pb = PageBreaking::new(&mut vm, book, None, None).unwrap();
    // A score, then the markup's line; the paper's defaults.
    assert_eq!(pb.system_specs.len(), 2);
    assert_eq!(pb.system_specs[0].pscore, Some(ps));
    assert!(pb.system_specs[1].prob.is_some());
    assert_eq!(
        (
            pb.paper_height,
            pb.orphan_penalty,
            pb.ragged,
            pb.ragged_last
        ),
        (1.0, 100000, false, false)
    );
    assert_eq!(pb.footnote_separator_stencil_height, 0.0);
    // Chunks: the start, the score's end, the markup; breaks: the start
    // and the book's end.
    let c3 = grob_of(&mut vm, "(col 3)");
    let at = |p: &BreakPosition| (p.system_spec_index, p.score_break, p.score_ender);
    assert_eq!(
        pb.chunks.iter().map(at).collect::<Vec<_>>(),
        [(VPOS, VPOS, false), (0, 1, true), (1, VPOS, false)]
    );
    assert_eq!(pb.chunks[1].col, Some(c3));
    assert_eq!(
        pb.breaks.iter().map(at).collect::<Vec<_>>(),
        [(VPOS, VPOS, false), (1, VPOS, false)]
    );
    assert_eq!(pb.last_break_position(), 1);
    assert_eq!(pb.line_breaking.len(), 2);
    assert_eq!(pb.line_breaking[0].start, [0]);
    assert!(pb.line_breaking[1].pscore.is_none());
    assert_eq!(pb.chunk_list(0, 1).len(), 3);
    assert_eq!(pb.next_system(&pb.chunks[0]), 0);
    assert_eq!(pb.next_system(&pb.chunks[1]), 1);
    assert_eq!(
        pb.breakpoint_property(&mut vm, 0, "is-title").unwrap(),
        Value::NULL
    );
    assert_eq!(
        pb.breakpoint_property(&mut vm, 1, "is-title").unwrap(),
        Value::TRUE
    );

    // The ideal configuration: the score's best, one line; the markup's
    // none.
    pb.set_to_ideal_line_configuration(&mut vm, 0, 1).unwrap();
    assert_eq!(pb.current_configurations, [vec![1, 0]]);
    assert_eq!(pb.system_count, 1);
    assert!(pb.is_last());
    assert!(!pb.ends_score());
    assert_eq!(pb.min_system_count(&mut vm, 0, 1).unwrap(), 1);
    assert_eq!(pb.max_system_count(&mut vm, 0, 1).unwrap(), 2);

    // Two systems: the score's two lines, then the markup's default
    // details. Each hangs below the one before: the first 2 above its
    // refpoint and 3 below, 5; the second's refpoint at least the first's
    // padding (1) below the first's bottom, and its minimum distance (8)
    // below the first's refpoint, 10, so 13 down, 8 below the first.
    pb.set_current_breakpoints(&mut vm, 0, 1, 2, vec![], vec![])
        .unwrap();
    assert_eq!(pb.current_configurations, [vec![2, 0]]);
    pb.cache_line_details(&mut vm, 0).unwrap();
    assert_eq!(pb.cached_line_details.len(), 3);
    let tallness: Vec<f64> = pb.cached_line_details[..2]
        .iter()
        .map(|l| l.tallness)
        .collect();
    assert_eq!(tallness, [5.0, 8.0]);
    assert_eq!(pb.cached_line_details[1].padding, 2.0);
    // Cached: asked again, the same details.
    let cached = pb.cached_line_details.clone();
    pb.cache_line_details(&mut vm, 0).unwrap();
    assert_eq!(pb.cached_line_details, cached);

    // Broken into those lines: the score's two systems and the markup.
    let div = pb.current_configuration(0);
    pb.break_into_pieces(&mut vm, 0, 1, &div).unwrap();
    let systems = pb.systems(&mut vm).unwrap();
    vm.define("%systems", systems);
    assert_eq!(
        evaluated(
            &mut vm,
            "(map (lambda (s) (if (ly:grob? s) (ly:grob-spanned-column-rank-interval s) 'prob)) %systems)"
        ),
        "((0 . 2) (2 . 3) prob)"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn page_breaking_splits_chunks_at_forced_page_breaks_and_fixed_counts() {
    // A forced page break at column 2: a chunk ends there, and the score's
    // line breaker starts there too.
    let Some((mut vm, _, book)) = breaking_book("", "") else {
        return;
    };
    eval(&mut vm, "(put! (col 2) 'page-break-permission 'force)");
    let mut pb = PageBreaking::new(&mut vm, book, None, None).unwrap();
    assert_eq!(pb.chunks.len(), 4);
    assert_eq!(pb.line_breaking[0].start, [0, 2]);
    pb.set_to_ideal_line_configuration(&mut vm, 0, 1).unwrap();
    assert_eq!(pb.current_configurations, [vec![1, 1, 0]]);
    assert_eq!(diagnosed(&mut vm), "()");

    // A break predicate: a break at each column it takes but the first.
    let Some((mut vm, _, book)) = breaking_book("", "") else {
        return;
    };
    fn every(_: &mut dyn Host, _: Value) -> Result<bool, Exception> {
        Ok(true)
    }
    let pb = PageBreaking::new(&mut vm, book, Some(every), None).unwrap();
    // Columns 1, 2 and 3 of the score, then the book's end; the line
    // breaker starting at each but the last, column 1, no line break, at
    // the next, column 2.
    assert_eq!(pb.breaks.len(), 5);
    assert_eq!(pb.line_breaking[0].start, [0, 2, 2]);
    assert_eq!(pb.line_breaking[0].starting_breakpoints, [0, 1, 1]);

    // The layout's system-count: the score's lines fixed, counted at each
    // break and chunk end since its chunk's start.
    let Some((mut vm, _, book)) = breaking_book("'(system-count . 2)", "") else {
        return;
    };
    let mut pb = PageBreaking::new(&mut vm, book, None, None).unwrap();
    assert_eq!(pb.chunks[1].forced_line_count, 2);
    pb.set_to_ideal_line_configuration(&mut vm, 0, 1).unwrap();
    assert_eq!(pb.current_configurations, [vec![2, 0]]);
    assert_eq!(pb.min_system_count(&mut vm, 0, 1).unwrap(), 2);
    assert_eq!(pb.max_system_count(&mut vm, 0, 1).unwrap(), 2);

    // A division that is none of these chunks': the best lines, with
    // LilyPond's programming error.
    pb.break_into_pieces(&mut vm, 0, 1, &[2]).unwrap();
    assert_eq!(
        diagnosed(&mut vm),
        "(\"did not find a valid page breaking configuration\")"
    );

    // The paper's settings: systems-per-page beside a bound drops both,
    // and a minimum above the maximum drops both, each warned of.
    let Some((mut vm, _, book)) = breaking_book(
        "",
        "'(systems-per-page . 2) '(max-systems-per-page . 3) '(orphan-penalty . 7)
         '(paper-height . 50) '(ragged-bottom . #t) '(footnote-padding . 1.5)",
    ) else {
        return;
    };
    let pb = PageBreaking::new(&mut vm, book, None, None).unwrap();
    assert_eq!((pb.max_systems_per_page, pb.min_systems_per_page), (0, 0));
    assert_eq!(
        (pb.max_systems_per_page(), pb.min_systems_per_page()),
        (2, 2)
    );
    assert_eq!(
        (pb.orphan_penalty, pb.paper_height, pb.ragged),
        (7, 50.0, true)
    );
    assert_eq!(pb.footnote_padding, 1.5);
    let Some((mut vm2, _, book2)) = breaking_book(
        "",
        "'(min-systems-per-page . 4) '(max-systems-per-page . 3)",
    ) else {
        return;
    };
    let pb2 = PageBreaking::new(&mut vm2, book2, None, None).unwrap();
    assert_eq!(
        (pb2.max_systems_per_page(), pb2.min_systems_per_page()),
        (0, 0)
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"ignoring min-systems-per-page and max-systems-per-page because systems-per-page was set\")"
    );
    assert_eq!(
        diagnosed(&mut vm2),
        "(\"min-systems-per-page is larger than max-systems-per-page, ignoring both values\")"
    );
}

#[test]
fn lines_without_a_page_break_between_are_compressed() {
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    let iv = Interval::new;
    let line = |vm: &mut Vm, perm: Value, shape: (f64, f64), refpoints: (f64, f64)| {
        let mut l = LineDetails::new(vm.heap_mut());
        l.page_permission = perm;
        l.shape = LineShape::new(iv(shape.0, shape.1), iv(shape.0, shape.1));
        l.refpoint_extent = iv(refpoints.0, refpoints.1);
        l.padding = 1.0;
        l.title_padding = 4.0;
        l.space = 10.0;
        l.inverse_hooke = 2.0;
        l.footnote_heights = vec![shape.1];
        l
    };
    let allow = vm.heap_mut().intern("allow");
    let mut lines = vec![
        line(&mut vm, Value::NULL, (-3.0, 2.0), (-5.0, 0.0)),
        line(&mut vm, allow, (-1.0, 1.0), (-2.0, 0.0)),
        line(&mut vm, allow, (-2.0, 2.0), (-1.0, 0.0)),
    ];
    lines[1].title = true;
    let compressed = compress_lines(&lines);
    assert_eq!(compressed.len(), 2);
    let c = &compressed[0];
    // The second line stacked on the first with the title padding: the
    // elevation 2 + 1, then 4.
    assert_eq!(c.shape.begin, iv(-3.0, 8.0));
    assert_eq!(c.refpoint_extent, iv(-2.0 + 6.0, 0.0));
    assert_eq!((c.space, c.inverse_hooke), (20.0, 4.0));
    assert_eq!(
        (c.compressed_lines_count, c.compressed_nontitle_lines_count),
        (2, 1)
    );
    assert!(!c.title);
    assert_eq!(c.footnote_heights, [2.0, 1.0]);
    assert_eq!(compressed[1], lines[2]);
    // Tightly spaced: no padding.
    lines[1].tight_spacing = true;
    assert_eq!(compress_lines(&lines)[0].shape.begin, iv(-3.0, 4.0));
    // Systems per page counted in the lines before compressing.
    assert_eq!(uncompress_solution(&[1, 1], &compressed), [2, 1]);
    assert_eq!(uncompress_solution(&[2], &compressed), [3]);
}

#[test]
fn line_shapes_stack_and_lines_hang_by_their_springs() {
    let iv = Interval::new;
    // piggyback: the mount raised until neither part overlaps, then by
    // the padding.
    let base = LineShape::new(iv(-2.0, 3.0), iv(-1.0, 1.0));
    let mount = LineShape::new(iv(-1.0, 2.0), iv(-4.0, 0.5));
    let stacked = base.piggyback(mount, 0.5);
    // Elevation max(3 + 1, 1 + 4) = 5.
    assert_eq!(stacked, LineShape::new(iv(-2.0, 7.5), iv(-1.0, 6.0)));
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    let mut a = LineDetails::new(vm.heap_mut());
    a.shape = LineShape::new(iv(-2.0, 3.0), iv(-1.0, 4.0));
    assert_eq!(a.full_height(), 6.0);
    a.tallness = 10.0;
    assert_eq!(a.tallness(), 10.0);
    a.refpoint_extent = iv(-3.0, 0.0);
    a.space = 12.0;
    a.title_space = 2.0;
    let mut b = LineDetails::new(vm.heap_mut());
    b.refpoint_extent = iv(-1.0, 1.0);
    // From a's bottom refpoint to b's top: 10 - 3 - 1 = 6.
    assert_eq!(a.spring_length(&b), 6.0);
    b.title = true;
    assert_eq!(a.spring_length(&b), 0.0);
    assert_eq!(LineDetails::new(vm.heap_mut()).full_height(), 0.0);
}

#[test]
fn calc_breaking_breaks_at_the_breaks_given_until_it_chooses_them() {
    // Columns 0, 2 and 3 breaks, at 0, 1/2 and 3/4; column 1 at 1/4.
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    eval(
        &mut vm,
        "(for-each (lambda (i) (put! (col i) 'when (ly:make-moment (/ i 4)))) '(0 1 2 3))",
    );
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    let want = [(0, 1), (1, 2)].map(|(i, j)| cb.space_line(&mut vm, i, j).unwrap());
    let breaking = |vm: &mut Vm, whens: &[&str]| {
        let whens: Vec<String> = whens.iter().map(|w| w.to_string()).collect();
        paper_score::give_breaks(vm, &whens);
        paper_score::calc_breaking(vm, ps).unwrap()
    };
    // At column 2's 1/2: two lines, each spaced as `solve` spaces it.
    let got = breaking(&mut vm, &["1/2 0"]);
    assert_eq!(got.len(), 2);
    for (got, want) in got.iter().zip(&want) {
        assert_eq!(ranks(&mut vm, &got.cols), ranks(&mut vm, &want.cols));
        assert_eq!((&got.config, got.force), (&want.config, want.force));
    }
    assert_eq!(ranks(&mut vm, &got[1].cols), [2, 3]);
    // A moment no break has (column 1's), and the first's and the last's,
    // which every line breaking has: the one line.
    for whens in [&["1/4 0"][..], &["0 0", "3/4 0"], &["1/2 1/8"]] {
        let got = breaking(&mut vm, whens);
        assert_eq!(got.len(), 1, "{whens:?}");
        assert_eq!(ranks(&mut vm, &got[0].cols), [0, 1, 2, 3]);
    }
    // None given again: the one line.
    assert_eq!(breaking(&mut vm, &[]).len(), 1);
    // given_lines itself: a break given twice or out of order is passed.
    assert_eq!(cb.given_lines(&mut vm, &[1, 1]).unwrap().len(), 2);
    assert_eq!(cb.given_lines(&mut vm, &[2, 1]).unwrap().len(), 1);
    assert_eq!(cb.given_lines(&mut vm, &[]).unwrap().len(), 1);
    let none = ConstrainedBreaking::new(&mut vm, None).unwrap();
    assert!(none.given_lines(&mut vm, &[1]).unwrap().is_empty());
    assert_eq!(diagnosed(&mut vm), "()");
}

/// What `text` evaluates to, shown.
fn evaluated(vm: &mut Vm, text: &str) -> String {
    let v = eval(vm, text);
    shown(vm, v)
}

#[test]
fn a_line_becomes_a_broken_system_bounded_by_its_columns_with_their_labels() {
    // place_line's line (columns 0, 1, 3 and 4, column 2 loose), labels on
    // columns 1, 2 and 4, and a pure height of the ranks less and more 1.
    let Some(mut vm) = line_vm(
        5,
        "(put! (col 1) 'X-extent '(0 . 1))
         (put! (col 2) 'X-extent '(0 . 0.25))
         (ly:grob-set-object! (col 2) 'between-cols (cons (col 1) (col 3)))
         (ly:grob-set-object! (col 2) 'spacing spacer)
         (put! (col 1) 'labels '(a))
         (put! (col 2) 'labels '(l))
         (put! (col 4) 'labels '(b c))
         (put! sys 'Y-extent
               (ly:make-unpure-pure-container
                 (lambda (g) '(0 . 0))
                 (lambda (g s e) (cons (- s 1) (+ e 1)))))",
    ) else {
        return;
    };
    let sys = grob_of(&mut vm, "sys");
    let ps = paper_score_of(&mut vm, sys);
    let cols: Vec<Value> = [0, 1, 3, 4]
        .map(|i| grob_of(&mut vm, &format!("(col {i})")))
        .into();
    let loose = grob_of(&mut vm, "(col 2)");
    let posns = ColumnXPositions {
        cols: cols.clone(),
        loose_cols: vec![loose],
        config: vec![0.0, 2.0, 10.0, 14.0],
        ..ColumnXPositions::default()
    };
    system::break_into_pieces(&mut vm, sys, &[posns]).unwrap();

    // One broken system, a copy of the root, the first, typeset in its
    // Paper_score, which keeps the root as its system.
    let line = eval(
        &mut vm,
        "(define line (car (ly:spanner-broken-into sys))) line",
    );
    assert_eq!(
        evaluated(&mut vm, "(length (ly:spanner-broken-into sys))"),
        "1"
    );
    assert_eq!(item::class(vm.heap_mut(), line), Some(item::Class::System));
    assert_eq!(
        grob::with_grob(vm.heap(), line, |g| g.original),
        Some(Some(sys))
    );
    assert_eq!(system::get_rank(vm.heap(), line), 0);
    assert_eq!(
        grob::with_grob(vm.heap(), line, |g| g.pscore),
        Some(Some(ps))
    );
    assert_eq!(paper_score::root_system(vm.heap(), ps), Some(sys));
    assert!(eval(&mut vm, "(eq? (ly:grob-layout line) (ly:grob-layout sys))") == Value::TRUE);
    // Elements of its own, none yet.
    let n = eval(
        &mut vm,
        "(ly:grob-array-length (ly:grob-object line 'all-elements))",
    );
    assert_eq!(shown(&vm, n), "0");
    // Bounded by the line's ends, its pure height the root's from rank 0
    // to rank 4, its labels the columns' last first, the loose one's last.
    for (text, want) in [
        ("(eq? (ly:spanner-bound line -1) (col 0))", "#t"),
        ("(eq? (ly:spanner-bound line 1) (col 4))", "#t"),
        (
            "(ly:grob-property-data line 'pure-Y-extent)",
            "(-1.0 . 5.0)",
        ),
        ("(ly:grob-property-data line 'labels)", "(l b c a)"),
        ("(ly:grob-property-data sys 'labels)", "()"),
    ] {
        assert_eq!(evaluated(&mut vm, text), want, "{text}");
    }
    // The columns placed as place_line places them, on the broken system.
    let at = |vm: &mut Vm, i: usize| {
        let v = eval(
            vm,
            &format!("(ly:grob-relative-coordinate (col {i}) sys 0)"),
        );
        vm.heap().to_f64(v).unwrap()
    };
    assert_eq!(
        (0..5).map(|i| at(&mut vm, i)).collect::<Vec<_>>(),
        [0.0, 2.0, 9.5, 10.0, 14.0]
    );
    for c in cols.into_iter().chain([loose]) {
        assert_eq!(paper_column::system(vm.heap(), c), Some(line));
    }
    // Its bounds on it, it is its own system: its rank twice.
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), line),
        (0, 0)
    );
    assert_eq!(diagnosed(&mut vm), "()");

    // A line with no columns makes no system.
    system::break_into_pieces(&mut vm, sys, &[ColumnXPositions::default()]).unwrap();
    assert_eq!(
        evaluated(&mut vm, "(length (ly:spanner-broken-into sys))"),
        "1"
    );
}

#[test]
fn each_line_s_broken_system_is_ranked_after_the_ones_before() {
    // breaking_vm's lines from break 0 to break 1 and from 1 to 2, the
    // root's pure height its ranks.
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    eval(
        &mut vm,
        "(put! sys 'Y-extent
               (ly:make-unpure-pure-container (lambda (g) '(0 . 0)) (lambda (g s e) (cons s e))))",
    );
    let sys = grob_of(&mut vm, "sys");
    let cb = ConstrainedBreaking::new(&mut vm, Some(ps)).unwrap();
    let lines = [
        cb.space_line(&mut vm, 0, 1).unwrap(),
        cb.space_line(&mut vm, 1, 2).unwrap(),
    ];
    system::break_into_pieces(&mut vm, sys, &lines).unwrap();
    let pieces: Vec<Value> = (0..2)
        .map(|i| {
            grob_of(
                &mut vm,
                &format!("(list-ref (ly:spanner-broken-into sys) {i})"),
            )
        })
        .collect();
    for (i, (left, right, height)) in [(0, 2, "(0.0 . 2.0)"), (2, 3, "(2.0 . 3.0)")]
        .into_iter()
        .enumerate()
    {
        assert_eq!(system::get_rank(vm.heap(), pieces[i]), i as u64);
        let bounds = [-1, 1].map(|d| spanner::get_bound(&mut vm, pieces[i], d).unwrap().unwrap());
        assert_eq!(ranks(&mut vm, &bounds), [left, right]);
        let h = eval(
            &mut vm,
            &format!(
                "(ly:grob-property-data (list-ref (ly:spanner-broken-into sys) {i}) 'pure-Y-extent)"
            ),
        );
        assert_eq!(shown(&vm, h), height);
    }
    // Column 1 is on the first, column 3 on the second.
    let [c1, c3] = ["(col 1)", "(col 3)"].map(|t| grob_of(&mut vm, t));
    assert_eq!(paper_column::system(vm.heap(), c1), Some(pieces[0]));
    assert_eq!(paper_column::system(vm.heap(), c3), Some(pieces[1]));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_broken_col_range_is_the_breakable_columns_between_on_no_system() {
    // Columns 0 to 4 breakable but 3; column 2 on a system already.
    let Some(mut vm) = line_vm(
        6,
        "(for-each (lambda (i) (put! (col i) 'line-break-permission 'allow)) '(0 1 2 4))
         (place! (thing 1) (col 1) (thing 5))",
    ) else {
        return;
    };
    let [sys, c0, c2, c4, c5, t1] = [
        "sys",
        "(col 0)",
        "(col 2)",
        "(col 4)",
        "(col 5)",
        "(thing 1)",
    ]
    .map(|t| grob_of(&mut vm, t));
    paper_column::set_system(vm.heap(), c2, sys);
    // Strictly between the ends' columns, by rank: not 0, 2 or 5.
    let cols = system::broken_col_range(&mut vm, sys, c0, c5).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [1, 4]);
    let cols = system::broken_col_range(&mut vm, sys, c0, c4).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [1]);
    // An item's column: (thing 1) is on column 1.
    let cols = system::broken_col_range(&mut vm, sys, t1, c5).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [4]);
    // Not past the system's columns.
    paper_column::set_rank(vm.heap(), c5, 100);
    let cols = system::broken_col_range(&mut vm, sys, c2, c5).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [4]);
    // A left rank of VPOS wraps to 0, as vsize does: column 0 too.
    let fresh = grob_of(&mut vm, "(column 0)");
    let cols = system::broken_col_range(&mut vm, sys, fresh, c4).unwrap();
    assert_eq!(ranks(&mut vm, &cols), [0, 1]);
    // An item without a column, which LilyPond reads through: none.
    let t2 = grob_of(&mut vm, "(thing 2)");
    assert!(
        system::broken_col_range(&mut vm, sys, t2, c5)
            .unwrap()
            .is_empty()
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_spanned_system_ranks_are_the_system_s_or_the_pieces_systems() {
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    // Systems 0, 1 and 2 of ranks 3, 1 and 2; column 0 on system 1,
    // column 2 on system 0, column 3 on system 2, column 1 on none.
    let g: Vec<Value> = (0..6)
        .map(|i| grob_of(&mut vm, &format!("(group {i})")))
        .collect();
    for (i, rank) in [3, 1, 2].into_iter().enumerate() {
        item::set_class(vm.heap_mut(), g[i], item::Class::System);
        system::set_rank(vm.heap(), g[i], rank);
    }
    for (c, s) in [(0, 1), (2, 0), (3, 2)] {
        let col = grob_of(&mut vm, &format!("(column {c})"));
        paper_column::set_system(vm.heap(), col, g[s]);
    }
    eval(
        &mut vm,
        "(for-each (lambda (t c) (ly:grob-set-parent! (thing t) 0 (column c)))
                   '(0 1 2 3 4) '(0 1 2 3 1))",
    );
    let t: Vec<Value> = (0..5)
        .map(|i| grob_of(&mut vm, &format!("(thing {i})")))
        .collect();
    assert_eq!(system::get_rank(vm.heap(), g[0]), 3);
    // An item on a system: its rank twice. Not on one: its pieces'
    // systems' ranks, (thing 2) on system 0 and (thing 3) on system 2;
    // with no piece, the empty interval.
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), t[0]),
        (1, 1)
    );
    grob::with_grob_mut(vm.heap(), t[1], |i| i.broken = [Some(t[2]), Some(t[3])]);
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), t[1]),
        (2, 3)
    );
    grob::with_grob_mut(vm.heap(), t[1], |i| i.broken = [None, Some(t[3])]);
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), t[1]),
        (2, 2)
    );
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), t[4]),
        (item::VPOS, 0)
    );

    // A spanner on one system, group 3 on system 1 and group 5 on system
    // 2: its rank twice. Group 4, between columns of two systems, is on
    // none: the empty interval, then its pieces' first and last.
    let bounds = [(3, 0, 0), (5, 3, 3), (4, 0, 2)];
    for (sp, l, r) in bounds {
        let [l, r] = [l, r].map(|c| grob_of(&mut vm, &format!("(column {c})")));
        spanner::set_bound(&mut vm, g[sp], -1, l).unwrap();
        spanner::set_bound(&mut vm, g[sp], 1, r).unwrap();
    }
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), g[3]),
        (1, 1)
    );
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), g[5]),
        (2, 2)
    );
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), g[4]),
        (item::VPOS, 0)
    );
    grob::with_grob_mut(vm.heap(), g[4], |s| s.broken_intos = vec![g[3], g[5]]);
    assert_eq!(
        item::spanned_system_rank_interval(vm.heap_mut(), g[4]),
        (1, 2)
    );
    // A piece is found by its system's rank from the first piece's.
    assert_eq!(
        spanner::find_broken_piece(vm.heap_mut(), g[4], g[2]),
        Some(g[5])
    );
    assert_eq!(
        spanner::find_broken_piece(vm.heap_mut(), g[4], g[1]),
        Some(g[3])
    );
    assert_eq!(spanner::find_broken_piece(vm.heap_mut(), g[4], g[0]), None);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_spanner_s_bounds_on_no_system_become_their_pieces_facing_into_it() {
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    // Column 1 on a system, column 0 on none; things 0 and 3 on column 0,
    // broken into things 1 and 2 and things 4 and 5.
    let sys = grob_of(&mut vm, "(group 5)");
    item::set_class(vm.heap_mut(), sys, item::Class::System);
    let c1 = grob_of(&mut vm, "(column 1)");
    paper_column::set_system(vm.heap(), c1, sys);
    eval(
        &mut vm,
        "(for-each (lambda (t) (ly:grob-set-parent! (thing t) 0 (column 0))) '(0 3))",
    );
    let t: Vec<Value> = (0..6)
        .map(|i| grob_of(&mut vm, &format!("(thing {i})")))
        .collect();
    grob::with_grob_mut(vm.heap(), t[0], |i| i.broken = [Some(t[1]), Some(t[2])]);
    grob::with_grob_mut(vm.heap(), t[3], |i| i.broken = [Some(t[4]), Some(t[5])]);
    let [g3, g4] = ["(group 3)", "(group 4)"].map(|t| grob_of(&mut vm, t));
    let bound = |vm: &mut Vm, sp: Value, d| spanner::get_bound(vm, sp, d).unwrap();

    // The left bound becomes its piece beginning a line, the right one
    // its piece ending one; the left piece is the spanner's X parent.
    spanner::set_bound(&mut vm, g3, -1, t[0]).unwrap();
    spanner::set_bound(&mut vm, g3, 1, t[3]).unwrap();
    spanner::set_my_columns(&mut vm, g3).unwrap();
    assert_eq!(bound(&mut vm, g3, -1), Some(t[2]));
    assert_eq!(bound(&mut vm, g3, 1), Some(t[4]));
    assert_eq!(grob::get_parent(vm.heap(), g3, 0), Some(t[2]));

    // A bound on a system stays; one on none without that piece too.
    spanner::set_bound(&mut vm, g4, -1, c1).unwrap();
    spanner::set_bound(&mut vm, g4, 1, t[1]).unwrap();
    spanner::set_my_columns(&mut vm, g4).unwrap();
    assert_eq!(bound(&mut vm, g4, -1), Some(c1));
    assert_eq!(bound(&mut vm, g4, 1), Some(t[1]));
    assert_eq!(diagnosed(&mut vm), "()");
}

/// [`grobs_vm`] broken into two systems, `(group 0)` of rank 0 and
/// `(group 1)` of rank 1, each the parent on both axes of a column on it,
/// `(column 0)` and `(column 1)`. On column 0, under group 0 on Y too:
/// `(thing 0)` and `(thing 4)`; on column 1, under group 1: `(thing 1)`
/// and `(thing 5)`. `(thing 2)` is on column 0 with no Y parent;
/// `(thing 3)`, on no column, is broken into things 4 and 5; `(bit 0)` is
/// on none, with no pieces. `(group 2)`, from column 0 to column 1, is
/// broken into `(group 3)` on column 0 and `(group 4)` on column 1, each
/// under its system on Y. Gives the systems, things and groups.
fn substitution_vm() -> Option<(Vm, Vec<Value>, Vec<Value>)> {
    let mut vm = grobs_vm()?;
    eval(
        &mut vm,
        "(for-each (lambda (g a p) (ly:grob-set-parent! g a p))
           (list (column 0) (column 0) (column 1) (column 1)
                 (thing 0) (thing 0) (thing 4) (thing 4) (thing 1) (thing 1) (thing 5) (thing 5)
                 (thing 2) (group 3) (group 4))
           '(0 1 0 1 0 1 0 1 0 1 0 1 0 1 1)
           (list (group 0) (group 0) (group 1) (group 1)
                 (column 0) (group 0) (column 0) (group 0) (column 1) (group 1) (column 1) (group 1)
                 (column 0) (group 0) (group 1)))",
    );
    let g: Vec<Value> = (0..5)
        .map(|i| grob_of(&mut vm, &format!("(group {i})")))
        .collect();
    let t: Vec<Value> = (0..6)
        .map(|i| grob_of(&mut vm, &format!("(thing {i})")))
        .collect();
    for (i, &sys) in g[..2].iter().enumerate() {
        item::set_class(vm.heap_mut(), sys, item::Class::System);
        system::set_rank(vm.heap(), sys, i as u64);
        let col = grob_of(&mut vm, &format!("(column {i})"));
        paper_column::set_system(vm.heap(), col, sys);
    }
    grob::with_grob_mut(vm.heap(), t[3], |i| i.broken = [Some(t[4]), Some(t[5])]);
    let [c0, c1] = ["(column 0)", "(column 1)"].map(|t| grob_of(&mut vm, t));
    for (sp, l, r) in [(2, c0, c1), (3, c0, c0), (4, c1, c1)] {
        spanner::set_bound(&mut vm, g[sp], -1, l).unwrap();
        spanner::set_bound(&mut vm, g[sp], 1, r).unwrap();
    }
    grob::with_grob_mut(vm.heap(), g[2], |s| s.broken_intos = vec![g[3], g[4]]);
    Some((vm, g, t))
}

/// The items of the list `v`, and its tail.
fn list_items(vm: &Vm, mut v: Value) -> (Vec<Value>, Value) {
    let mut items = Vec::new();
    while let (Some(car), Some(cdr)) = (vm.heap().car(v), vm.heap().cdr(v)) {
        items.push(car);
        v = cdr;
    }
    (items, v)
}

/// The grobs of the grob array `v` and whether it is ordered.
fn array_items(vm: &Vm, v: Value) -> (Vec<Value>, bool) {
    let ga = vm.heap().foreign::<GrobArray>(v).expect("a grob array");
    (ga.grobs.clone(), ga.ordered)
}

#[test]
fn each_piece_of_a_spanner_links_to_the_grobs_on_its_system() {
    let Some((mut vm, g, t)) = substitution_vm() else {
        return;
    };
    let link = vm.heap_mut().intern("link");
    let object =
        |vm: &mut Vm, grob: Value| grob::with_grob(vm.heap(), grob, |o| o.objects).unwrap();
    let get = |vm: &mut Vm, grob: Value| {
        let alist = object(vm, grob);
        let entry = vm.heap().car(alist).unwrap();
        (vm.heap().car(entry).unwrap(), vm.heap().cdr(entry).unwrap())
    };

    // A list: each grob on the piece's system, or its piece there, kept
    // if the system is its ancestor on both axes; *unspecified* for
    // another. (thing 2) has no Y parent; (group 2) is replaced by the
    // piece.
    let val = eval(
        &mut vm,
        "(list (thing 0) (thing 1) (thing 3) (group 2) (thing 2))",
    );
    break_substitution::substitute_one_mutable_property(vm.heap_mut(), g[2], link, val);
    let u = Value::UNSPECIFIED;
    for (piece, want) in [
        (g[3], [t[0], u, t[4], g[3], u]),
        (g[4], [u, t[1], t[5], g[4], u]),
    ] {
        let (sym, val) = get(&mut vm, piece);
        assert_eq!(sym, link);
        assert_eq!(list_items(&vm, val), (want.to_vec(), Value::NULL));
    }
    // The original is left as it is.
    assert_eq!(object(&mut vm, g[2]), Value::NULL);

    // A vector, the car and cdr of a pair.
    let val = eval(&mut vm, "(vector (thing 1) (cons (group 2) (thing 0)))");
    break_substitution::substitute_one_mutable_property(vm.heap_mut(), g[2], link, val);
    let (_, val) = get(&mut vm, g[4]);
    let items = vm.heap().vector_items(val).unwrap().to_vec();
    assert_eq!(items[0], t[1]);
    assert_eq!(list_items(&vm, items[1]), (vec![g[4]], u));

    // A short or ordered grob array: a new one as ordered, without the
    // grobs that have no substitute.
    for ordered in [true, false] {
        let grobs = vec![t[1], t[0], t[3], t[2]];
        let val = vm.heap_mut().wrap(GrobArray { grobs, ordered });
        break_substitution::substitute_one_mutable_property(vm.heap_mut(), g[2], link, val);
        let (_, a3) = get(&mut vm, g[3]);
        let (_, a4) = get(&mut vm, g[4]);
        assert_eq!(array_items(&vm, a3), (vec![t[0], t[4]], ordered));
        assert_eq!(array_items(&vm, a4), (vec![t[1], t[5]], ordered));
    }

    // A grob's own links: an entry without a substitute is left out.
    let orig = eval(
        &mut vm,
        "(list (cons 'a (thing 1)) (cons 'b (thing 3)) (cons 'c 7))",
    );
    break_substitution::substitute_object_links(
        vm.heap_mut(),
        t[0],
        break_substitution::Criterion::System(g[0]),
        orig,
    );
    let objects = object(&mut vm, t[0]);
    assert_eq!(shown(&vm, objects).matches('(').count(), 3);
    assert_eq!(evaluated(&mut vm, "(ly:grob-object (thing 0) 'a)"), "()");
    assert_eq!(
        evaluated(&mut vm, "(eq? (ly:grob-object (thing 0) 'b) (thing 4))"),
        "#t"
    );
    assert_eq!(evaluated(&mut vm, "(ly:grob-object (thing 0) 'c)"), "7");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_long_unordered_grob_array_is_substituted_by_the_systems_its_items_are_on() {
    let Some((mut vm, g, t)) = substitution_vm() else {
        return;
    };
    let elts = vm.heap_mut().intern("elts");
    let [c0, bit] = ["(column 0)", "(bit 0)"].map(|s| grob_of(&mut vm, s));
    // Items on system 0 (things 0 and 2), on 1 (things 1 and 5), on both
    // (thing 3), on neither ((bit 0)); and two spanners.
    let grobs = vec![
        t[1], t[0], g[2], t[3], t[2], t[1], t[0], t[5], g[4], t[0], t[1], t[0], t[1], t[0], t[1],
        t[0], bit,
    ];
    let val = vm.heap_mut().wrap(GrobArray {
        grobs: grobs.clone(),
        ordered: false,
    });
    grob::set_object(vm.heap_mut(), g[2], elts, val);
    // (group 3) has an array there already, added to.
    let had = vm.heap_mut().wrap(GrobArray {
        grobs: vec![c0],
        ordered: false,
    });
    grob::set_object(vm.heap_mut(), g[3], elts, had);
    break_substitution::substitute_one_mutable_property(vm.heap_mut(), g[2], elts, val);

    // The items by the first system they are on, stably: system 0's
    // first, then thing 3 among them, system 1's; each piece takes those
    // from the first to the last on its system, those between too, then
    // the spanners. (thing 2) has no Y parent; system 0's things are
    // between thing 3 and system 1's, and have no piece on system 1.
    let a3 = eval(&mut vm, "(ly:grob-object (group 3) (quote elts))");
    assert_eq!(a3, had);
    assert_eq!(
        array_items(&vm, a3),
        (
            vec![c0, t[0], t[4], t[0], t[0], t[0], t[0], t[0], g[3]],
            false
        )
    );
    let a4 = eval(&mut vm, "(ly:grob-object (group 4) (quote elts))");
    assert_eq!(
        array_items(&vm, a4),
        (
            vec![t[5], t[1], t[1], t[5], t[1], t[1], t[1], g[4], g[4]],
            true
        )
    );

    // Ordered, the same grobs are substituted in their order.
    let val = vm.heap_mut().wrap(GrobArray {
        grobs,
        ordered: true,
    });
    break_substitution::substitute_one_mutable_property(vm.heap_mut(), g[2], elts, val);
    let a3 = eval(&mut vm, "(ly:grob-object (group 3) (quote elts))");
    assert_eq!(array_items(&vm, a3).0[..3], [t[0], g[3], t[4]]);
    assert_eq!(diagnosed(&mut vm), "()");
}

/// [`line_vm`]'s five columns, `(col 2)` breakable too and broken into two
/// pieces (`item::clone`), all under `sys` on both axes and typeset in it;
/// `sys` a system bounded by columns 0 and 4, typeset in a Paper_score.
/// After `setup`, `sys` broken into two lines: columns 0, 1 and the left
/// piece of 2, and the right piece of 2, 3 and 4. Gives the VM, the two
/// broken systems and the two pieces.
fn broken_vm(setup: &str) -> Option<(Vm, [Value; 2], [Value; 2])> {
    let mut vm = line_vm(
        5,
        "(put! (col 2) 'line-break-permission 'allow)
         (put! sys 'Y-extent
               (ly:make-unpure-pure-container (lambda (g) '(0 . 0)) (lambda (g s e) (cons s e))))
         (for-each (lambda (c) (ly:grob-set-parent! c 1 sys)) cols)",
    )?;
    let sys = grob_of(&mut vm, "sys");
    system::make(vm.heap_mut(), sys);
    paper_score_of(&mut vm, sys);
    let c: Vec<Value> = (0..5)
        .map(|i| grob_of(&mut vm, &format!("(col {i})")))
        .collect();
    spanner::set_bound(&mut vm, sys, -1, c[0]).unwrap();
    spanner::set_bound(&mut vm, sys, 1, c[4]).unwrap();
    let pieces = [(); 2].map(|()| {
        let p = item::clone(&mut vm, c[2]).unwrap();
        vm.heap_mut().root(p);
        p
    });
    grob::with_grob_mut(vm.heap(), c[2], |g| g.broken = pieces.map(Some));
    for &g in c.iter().chain(&pieces) {
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    eval(&mut vm, setup);
    let lines =
        [vec![c[0], c[1], pieces[0]], vec![pieces[1], c[3], c[4]]].map(|cols| ColumnXPositions {
            cols,
            config: vec![0.0, 2.0, 4.0],
            ..ColumnXPositions::default()
        });
    system::break_into_pieces(&mut vm, sys, &lines).unwrap();
    let broken = pieces_into(&mut vm, sys);
    Some((vm, [broken[0], broken[1]], pieces))
}

/// The broken pieces of the spanner `sp`, through `ly:spanner-broken-into`.
fn pieces_into(vm: &mut Vm, sp: Value) -> Vec<Value> {
    vm.heap_mut().root(sp);
    let f = eval(vm, "ly:spanner-broken-into");
    let list = Host::call(vm, f, &[sp]).unwrap();
    list_items(vm, list).0
}

/// The grobs of `g`'s grob array `name`.
fn object_array(vm: &mut Vm, g: Value, name: &str) -> Vec<Value> {
    vm.heap_mut().root(g);
    let f = eval(vm, "ly:grob-object");
    let sym = vm.heap_mut().intern(name);
    let v = Host::call(vm, f, &[g, sym]).unwrap();
    array_items(vm, v).0
}

#[test]
fn a_group_across_the_break_is_broken_into_a_piece_on_each_line() {
    // (group 0), a staff's group with nothing else on it, from column 0
    // to column 4, under sys on Y; (thing 1) on column 1 and (thing 2) on
    // column 3 under it on Y, its elements. (thing 1) records its
    // after-line-breaking.
    let Some((mut vm, lines, pieces)) = broken_vm(
        "(define seen '())
         (for-each (lambda (t c) (ly:grob-set-parent! t 0 c) (ly:grob-set-parent! t 1 (group 0)))
                   (list (thing 1) (thing 2)) (list (col 1) (col 3)))
         (ly:grob-set-parent! (group 0) 1 sys)
         (ly:grob-set-object! (group 0) 'elements (arr (thing 1) (thing 2)))
         (put! (thing 1) 'after-line-breaking (lambda (g) (set! seen (cons g seen)) #t))",
    ) else {
        return;
    };
    let [sys, g0, t1, t2, c0, c1, c2, c3, c4] = [
        "sys",
        "(group 0)",
        "(thing 1)",
        "(thing 2)",
        "(col 0)",
        "(col 1)",
        "(col 2)",
        "(col 3)",
        "(col 4)",
    ]
    .map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, g0, -1, c0).unwrap();
    spanner::set_bound(&mut vm, g0, 1, c4).unwrap();
    for g in [g0, t1, t2] {
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }

    system::do_break_substitution_and_fixup_refpoints(&mut vm, sys).unwrap();

    // A piece of the group on each line, bounded by the line's columns,
    // numbered by its line.
    let p = pieces_into(&mut vm, g0);
    assert_eq!(p.len(), 2);
    for (i, (piece, bounds)) in p.iter().zip([[c0, pieces[0]], [pieces[1], c4]]).enumerate() {
        assert_eq!(spanner::get_system(vm.heap_mut(), *piece), Some(lines[i]));
        assert_eq!(
            grob::with_grob(vm.heap(), *piece, |g| g.break_index),
            Some(i)
        );
        let got = [-1, 1].map(|d| spanner::get_bound(&mut vm, *piece, d).unwrap().unwrap());
        assert_eq!(got, bounds);
        // Under its line on Y, its parent moved from sys.
        assert_eq!(grob::get_parent(vm.heap(), *piece, 1), Some(lines[i]));
    }
    // Each thing under the group's piece on its line, and each piece's
    // elements the things on its line.
    assert_eq!(grob::get_parent(vm.heap(), t1, 1), Some(p[0]));
    assert_eq!(grob::get_parent(vm.heap(), t2, 1), Some(p[1]));
    assert_eq!(object_array(&mut vm, p[0], "elements"), [t1]);
    assert_eq!(object_array(&mut vm, p[1], "elements"), [t2]);
    // The columns under their lines on X.
    assert_eq!(grob::get_parent(vm.heap(), c1, 0), Some(lines[0]));
    assert_eq!(grob::get_parent(vm.heap(), c3, 0), Some(lines[1]));

    // The group, column 2 and sys are on no line, and dead; the rest live.
    for (g, want) in [
        (g0, false),
        (c2, false),
        (sys, false),
        (t1, true),
        (t2, true),
        (c1, true),
    ] {
        assert_eq!(grob::is_live_grob(vm.heap_mut(), g), want);
    }
    // Each line's elements sys's on it, column 2 by its piece there, each
    // once where it first is (`uniquify`, MS3 step 33): in sys's order.
    let want = [
        vec![c0, c1, pieces[0], p[0], t1],
        vec![pieces[1], c3, c4, p[1], t2],
    ];
    for (line, want) in lines.into_iter().zip(want) {
        assert_eq!(object_array(&mut vm, line, "all-elements"), want);
    }
    // after-line-breaking read once, on the line.
    assert_eq!(
        evaluated(
            &mut vm,
            "(and (= (length seen) 1) (eq? (car seen) (thing 1)))"
        ),
        "#t"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_spanner_breaks_on_its_column_s_pieces_and_inside_its_parent() {
    let Some((mut vm, lines, pieces)) = broken_vm("") else {
        return;
    };
    let [g0, g1, g2, g3, c0, c1, c2, c4] = [
        "(group 0)",
        "(group 1)",
        "(group 2)",
        "(group 3)",
        "(col 0)",
        "(col 1)",
        "(col 2)",
        "(col 4)",
    ]
    .map(|t| grob_of(&mut vm, t));

    // Spanning column 2 alone: a piece bounded twice by each of its
    // pieces, on its line.
    spanner::set_bound(&mut vm, g1, -1, c2).unwrap();
    spanner::set_bound(&mut vm, g1, 1, c2).unwrap();
    spanner::do_break_processing(&mut vm, g1).unwrap();
    let p = pieces_into(&mut vm, g1);
    assert_eq!(p.len(), 2);
    for (i, &piece) in p.iter().enumerate() {
        let got = [-1, 1].map(|d| spanner::get_bound(&mut vm, piece, d).unwrap().unwrap());
        assert_eq!(got, [pieces[i]; 2]);
        assert_eq!(spanner::get_system(vm.heap_mut(), piece), Some(lines[i]));
        assert!(object_array(&mut vm, lines[i], "all-elements").contains(&piece));
    }

    // Broken already, or on one line (group 3, columns 0 to 1): nothing.
    spanner::do_break_processing(&mut vm, g1).unwrap();
    assert_eq!(pieces_into(&mut vm, g1), p);
    spanner::set_bound(&mut vm, g3, -1, c0).unwrap();
    spanner::set_bound(&mut vm, g3, 1, c1).unwrap();
    spanner::do_break_processing(&mut vm, g3).unwrap();
    assert!(pieces_into(&mut vm, g3).is_empty());

    // Under group 3 on Y, which spans columns 0 to 1: from column 0 to 4,
    // both pieces reach column 2, outside it, and are left out.
    eval(
        &mut vm,
        "(ly:grob-set-parent! (group 3) 1 sys) (ly:grob-set-parent! (group 2) 1 (group 3))",
    );
    spanner::set_bound(&mut vm, g2, -1, c0).unwrap();
    spanner::set_bound(&mut vm, g2, 1, c4).unwrap();
    spanner::do_break_processing(&mut vm, g2).unwrap();
    assert!(pieces_into(&mut vm, g2).is_empty());
    let orphan =
        "Spanner `Group' is not fully contained in parent spanner.  Ignoring orphaned part";
    assert_eq!(diagnosed(&mut vm), format!("({orphan:?} {orphan:?})"));

    // Without a right bound: not broken, and then killed.
    spanner::set_bound(&mut vm, g0, -1, c0).unwrap();
    spanner::do_break_processing(&mut vm, g0).unwrap();
    assert!(pieces_into(&mut vm, g0).is_empty());
    grob::handle_broken_dependencies(&mut vm, g0).unwrap();
    assert!(!grob::is_live_grob(vm.heap_mut(), g0));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_broken_copy_keeps_neither_its_original_s_pure_height_nor_its_pure_cache() {
    // `Item::Item (Item const &)` leaves `cached_pure_height_valid_` false
    // and `Spanner`'s copy has no `pure_property_cache_` (item.cc:46–50,
    // spanner.hh:51): a piece computes its pure height and caches its pure
    // properties anew (OPEN B4).
    let Some((mut vm, lines, pieces)) = broken_vm("") else {
        return;
    };
    let [g1, c0, c2] = ["(group 1)", "(col 0)", "(col 2)"].map(|t| grob_of(&mut vm, t));
    let kept = |vm: &Vm, g: Value| grob::with_grob(vm.heap(), g, |g| g.pure_height).flatten();
    // The broken systems' pure heights are the System's between their
    // ends' ranks (`break_into_pieces`), each its own.
    let pure = |vm: &mut Vm, g: Value| {
        let f = eval(vm, "(lambda (g) (ly:grob-property g 'pure-Y-extent))");
        let v = Host::call(vm, f, &[g]).unwrap();
        shown(vm, v)
    };
    assert_eq!(pure(&mut vm, lines[0]), "(0.0 . 2.0)");
    assert_eq!(pure(&mut vm, lines[1]), "(2.0 . 4.0)");
    // An item's pure height, kept, is not its copies'.
    eval(
        &mut vm,
        "(put! (col 0) 'Y-extent (ly:make-unpure-pure-container (lambda (g) '(1 . 2)) (lambda (g s e) '(3 . 5))))",
    );
    lily_engine_layout::unpure::pure_y_extent(&mut vm, c0, c0, 0, 4).unwrap();
    assert_eq!(
        kept(&vm, c0),
        Some(Interval {
            left: 3.0,
            right: 5.0
        })
    );
    let copy = item::clone(&mut vm, c0).unwrap();
    vm.heap_mut().root(copy);
    assert_eq!(kept(&vm, copy), None);
    assert_eq!(kept(&vm, c2), None);
    assert!(pieces.iter().all(|&p| kept(&vm, p).is_none()));
    // A spanner's pure cache is not its pieces'; under sys on Y, from
    // column 0 to 4, it is broken at column 2.
    eval(
        &mut vm,
        "(put! (group 1) 'foo 1) (ly:grob-set-parent! (group 1) 1 sys)",
    );
    let foo = vm.heap_mut().intern("foo");
    spanner::cache_pure_property(vm.heap_mut(), g1, foo, 0, 4, Value::TRUE);
    spanner::set_bound(&mut vm, g1, -1, c0).unwrap();
    let c4 = grob_of(&mut vm, "(col 4)");
    spanner::set_bound(&mut vm, g1, 1, c4).unwrap();
    spanner::do_break_processing(&mut vm, g1).unwrap();
    let p = pieces_into(&mut vm, g1);
    assert_eq!(p.len(), 2);
    assert_eq!(
        spanner::get_cached_pure_property(vm.heap(), g1, foo, 0, 4),
        Some(Value::TRUE)
    );
    for piece in p {
        assert_eq!(
            spanner::get_cached_pure_property(vm.heap(), piece, foo, 0, 4),
            None
        );
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn pre_processing_reads_before_line_breaking_then_springs_and_rods_after_the_pieces() {
    let Some(mut vm) = line_vm(3, "") else { return };
    let [sys, t0, t1] = ["sys", "(thing 0)", "(thing 1)"].map(|t| grob_of(&mut vm, t));
    system::make(vm.heap_mut(), sys);
    for g in [t0, t1] {
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    // Each records its reads; (thing 0)'s before-line-breaking adds
    // (thing 2) to the system's grobs.
    eval(
        &mut vm,
        "(define seen '())
         (define (log! what)
           (lambda (g) (set! seen (cons (cons what g) seen)) #t))
         (for-each (lambda (g)
                     (put! g 'before-line-breaking (log! 'before))
                     (put! g 'springs-and-rods (log! 'springs)))
                   (list sys (thing 0) (thing 1) (thing 2)))
         (put! (thing 0) 'before-line-breaking
               (lambda (g)
                 (set! seen (cons (cons 'before g) seen))
                 (%grob-array-add! (ly:grob-object sys 'all-elements) (thing 2))
                 #t))",
    );
    system::pre_processing(&mut vm, sys).unwrap();
    // The system first, then its grobs as the array was when each scope
    // began: (thing 2) only in the second.
    assert_eq!(
        evaluated(
            &mut vm,
            "(map (lambda (e)
                    (list (car e)
                          (cond ((eq? (cdr e) sys) 'sys)
                                ((eq? (cdr e) (thing 0)) 0)
                                ((eq? (cdr e) (thing 1)) 1)
                                (else 2))))
                  (reverse seen))"
        ),
        "((before sys) (before 0) (before 1) (springs sys) (springs 0) (springs 1) (springs 2))"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn post_processing_moves_the_system_up_by_its_height_and_reads_each_stencil_once() {
    let Some(mut vm) = line_vm(2, "") else { return };
    let [sys, other, t0, t1, t2] =
        ["sys", "(group 3)", "(thing 0)", "(thing 1)", "(thing 2)"].map(|t| grob_of(&mut vm, t));
    for s in [sys, other] {
        system::make(vm.heap_mut(), s);
    }
    for g in [t1, t0, t2] {
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    // (thing 1) twice, (thing 2) dead; each records its stencil's read.
    eval(
        &mut vm,
        "(%grob-array-add! (ly:grob-object sys 'all-elements) (thing 1))
         (define seen '())
         (put! sys 'Y-extent '(-3 . 2))
         (put! (group 3) 'Y-extent '(+inf.0 . -inf.0))
         (for-each (lambda (g n)
                     (put! g 'stencil (lambda (g) (set! seen (cons n seen)) '())))
                   (list sys (group 3) (thing 0) (thing 1) (thing 2))
                   '(sys other 0 1 2))
         (ly:grob-suicide! (thing 2))",
    );
    let before = grob::relative_coordinate(&mut vm, sys, Value::NULL, 1).unwrap();
    system::post_processing(&mut vm, sys).unwrap();
    let after = grob::relative_coordinate(&mut vm, sys, Value::NULL, 1).unwrap();
    assert_eq!(after - before, -2.0);
    // The system's, then each live grob's once, in all-elements' order
    // (`uniquify` keeps the first), (thing 1) typeset first.
    assert_eq!(evaluated(&mut vm, "(reverse seen)"), "(sys 1 0)");
    assert_eq!(diagnosed(&mut vm), "()");

    // An empty extent: not moved, LilyPond's programming error.
    let before = grob::relative_coordinate(&mut vm, other, Value::NULL, 1).unwrap();
    system::post_processing(&mut vm, other).unwrap();
    let after = grob::relative_coordinate(&mut vm, other, Value::NULL, 1).unwrap();
    assert_eq!(after, before);
    assert_eq!(evaluated(&mut vm, "(reverse seen)"), "(sys 1 0 other)");
    assert_eq!(diagnosed(&mut vm), "(\"system with empty extent\")");
}

#[test]
fn a_broken_system_becomes_a_paper_system_of_its_grobs_print_stencils_by_layer() {
    // (thing 1) to (thing 4) on the first line, under it at X 1 to 4 and
    // Y 0, (thing 2) at -1; their layers 2, 0, none and 0, (thing 4) with
    // no stencil, (thing 2) 1 and 0.5 staff spaces of 2 off. The line's
    // alignment: (staff 0) at -4, (staff 1) a loose line, (group 0) dead.
    // Its left bound's details give a Y extent.
    let Some((mut vm, lines, pieces)) = broken_vm(
        "(for-each ly:register-stencil-expression '(sys one two three))
         (define (stil e x y) (ly:make-stencil (list e) x y))
         (define (on-line! s)
           (for-each (lambda (t x y)
                       (ly:grob-set-parent! t 0 s)
                       (ly:grob-set-parent! t 1 s)
                       (put! t 'X-offset x)
                       (put! t 'Y-offset y)
                       (%grob-array-add! (ly:grob-object s 'all-elements) t))
                     (list (thing 1) (thing 2) (thing 3) (thing 4))
                     '(1 2 3 4)
                     '(0 -1 0 0))
           (for-each (lambda (g y) (ly:grob-set-parent! g 1 s) (put! g 'Y-offset y))
                     (list (staff 0) (staff 1) (group 0))
                     '(-4 -6 7))
           (ly:grob-set-object! s 'vertical-alignment (align 0))
           (put! s 'X-extent '(0 . 10))
           (put! s 'Y-extent '(-3 . 2))
           (put! s 'stencil (stil 'sys '(0 . 10) '(-3 . 2)))
           (put! s 'vertical-skylines 'sky))
         (put! (thing 1) 'stencil (stil 'one '(0 . 1) '(0 . 1)))
         (put! (thing 1) 'layer 2)
         (put! (thing 2) 'stencil (stil 'two '(-1 . 0) '(5 . 6)))
         (put! (thing 2) 'layer 0)
         (put! (thing 2) 'extra-offset '(1 . 0.5))
         (ly:grob-set-object! (thing 2) 'staff-symbol (staff 0))
         (put! (staff 0) 'staff-space 2)
         (ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'staff-space 1.0)
         (put! (thing 3) 'stencil (stil 'three '(0 . 9) '(0 . 2)))
         (put! (thing 4) 'layer 0)
         (ly:grob-set-object! (align 0) 'elements (arr (staff 0) (staff 1) (group 0)))
         (put! (staff 1) 'staff-affinity 1)
         (ly:grob-suicide! (group 0))
         (put! (col 0) 'line-break-system-details '((Y-extent . (-1 . 1)) (a . 1)))
         (define (summary p)
           (let* ((st (ly:prob-property p 'stencil))
                  (e (ly:stencil-expr st)))
             (list (car e)
                   (map (lambda (x)
                          (if (eq? (car x) 'translate-stencil)
                              (list (cadr x) (car (caddr (caddr x))))
                              (car (caddr x))))
                        (cdr e))
                   (ly:stencil-extent st 0)
                   (ly:stencil-extent st 1)
                   (map (lambda (k) (ly:prob-property p k))
                        '(a vertical-skylines page-break-permission page-turn-permission
                          page-break-penalty page-turn-penalty last-in-score
                          staff-refpoint-extent)))))",
    ) else {
        return;
    };
    // The root system ends at (col 2), whose left piece ends the first
    // line, with its permissions and penalties.
    let [sys, c2] = ["sys", "(col 2)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, sys, 1, c2).unwrap();
    let allow = eval(&mut vm, "'allow");
    grob::set_property(vm.heap_mut(), pieces[0], "page-break-permission", allow);
    let penalty = Value::fixnum(3).unwrap();
    grob::set_property(vm.heap_mut(), pieces[0], "page-turn-penalty", penalty);
    let on_line = eval(&mut vm, "on-line!");
    Host::call(&mut vm, on_line, &[lines[0]]).unwrap();

    let summary = eval(&mut vm, "summary");
    let pl = system::get_paper_system(&mut vm, lines[0]).unwrap();
    vm.heap_mut().root(pl);
    let got = Host::call(&mut vm, summary, &[pl]).unwrap();
    // The system's own stencil first, then by layer: (thing 2) moved by
    // its extra offset in staff spaces, (thing 3), (thing 1). X: the
    // system's extent and the stencils'; Y: the details'.
    assert_eq!(
        shown(&vm, got),
        "(combine-stencil (sys ((4.0 . 0.0) two) ((3.0 . 0.0) three) ((1.0 . 0.0) one)) \
         (0.0 . 12.0) (-1.0 . 1.0) (1 sky allow () () 3 #t (-4.0 . -4.0)))"
    );
    let grob = eval(&mut vm, "(lambda (p) (ly:prob-property p 'system-grob))");
    assert_eq!(Host::call(&mut vm, grob, &[pl]).unwrap(), lines[0]);
    // Moved up by the top of its height, after which its stencils are
    // read.
    let y = grob::relative_coordinate(&mut vm, lines[0], Value::NULL, 1).unwrap();
    assert_eq!(y, -2.0);

    // The second line: no grobs, no stencil, no details, its right bound
    // (col 4) no piece of the root's; its Y extent the system's, and its
    // vertical skylines the axis group's.
    let pl = system::get_paper_system(&mut vm, lines[1]).unwrap();
    vm.heap_mut().root(pl);
    let got = Host::call(&mut vm, summary, &[pl]).unwrap();
    assert_eq!(
        shown(&vm, got),
        "(combine-stencil () (+inf.0 . -inf.0) (0.0 . 0.0) \
         (() (#<Skyline> . #<Skyline>) () () () () () (+inf.0 . -inf.0)))"
    );
    assert_eq!(Host::call(&mut vm, grob, &[pl]).unwrap(), lines[1]);

    // The broken systems, in order.
    let broken = system::get_broken_system_grobs(vm.heap_mut(), sys);
    assert_eq!(list_items(&vm, broken), (lines.to_vec(), Value::NULL));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_paper_score_s_paper_systems_are_made_once_by_breaking_its_system() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    eval(&mut vm, "(put! sys 'Y-extent '(-1 . 1))");
    let sys = grob_of(&mut vm, "sys");
    let f = eval(&mut vm, "ly:paper-score-paper-systems");
    let systems = Host::call(&mut vm, f, &[ps]).unwrap();
    vm.heap_mut().root(systems);
    // The one line: one broken system, drawn.
    let broken = pieces_into(&mut vm, sys);
    assert_eq!(broken.len(), 1);
    let check = eval(
        &mut vm,
        "(lambda (v b)
           (let ((p (vector-ref v 0)))
             (list (vector-length v) (ly:prob? p) (eq? (ly:prob-property p 'system-grob) b)
                   (ly:stencil-expr (ly:prob-property p 'stencil))
                   (ly:grob-relative-coordinate b b 1))))",
    );
    let got = Host::call(&mut vm, check, &[systems, broken[0]]).unwrap();
    assert_eq!(shown(&vm, got), "(1 #t #t (combine-stencil) 0.0)");
    let y = grob::relative_coordinate(&mut vm, broken[0], Value::NULL, 1).unwrap();
    assert_eq!(y, -1.0);
    // Kept: not broken again.
    assert_eq!(Host::call(&mut vm, f, &[ps]).unwrap(), systems);
    assert_eq!(pieces_into(&mut vm, sys).len(), 1);
    assert_eq!(diagnosed(&mut vm), "()");

    // No root system: none.
    let layout = eval(&mut vm, "(ly:grob-layout (staff 0))");
    let lone = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(lone);
    let got = Host::call(&mut vm, f, &[lone]).unwrap();
    assert_eq!(shown(&vm, got), "#()");
}

#[test]
fn a_compile_processes_the_paper_score_then_breaks_its_system_once() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    eval(
        &mut vm,
        "(define seen '())
         (put! sys 'Y-extent '(-1 . 1))
         (put! sys 'before-line-breaking (lambda (g) (set! seen (cons 'before seen)) #t))
         (put! sys 'springs-and-rods (lambda (g) (set! seen (cons 'springs seen)) #t))",
    );
    let sys = grob_of(&mut vm, "sys");
    // Processed: the system's pre_processing, not broken yet.
    paper_score::process(&mut vm, ps).unwrap();
    assert_eq!(evaluated(&mut vm, "(reverse seen)"), "(before springs)");
    assert!(pieces_into(&mut vm, sys).is_empty());
    // Broken into the one line, once: neither again nor by the paper
    // systems, which draw the broken system there is.
    paper_score::break_systems(&mut vm, ps).unwrap();
    let broken = pieces_into(&mut vm, sys);
    assert_eq!(broken.len(), 1);
    paper_score::break_systems(&mut vm, ps).unwrap();
    assert_eq!(pieces_into(&mut vm, sys), broken);
    let f = eval(&mut vm, "ly:paper-score-paper-systems");
    let systems = Host::call(&mut vm, f, &[ps]).unwrap();
    vm.heap_mut().root(systems);
    let drawn = eval(
        &mut vm,
        "(lambda (v) (ly:prob-property (vector-ref v 0) 'system-grob))",
    );
    assert_eq!(Host::call(&mut vm, drawn, &[systems]).unwrap(), broken[0]);
    assert_eq!(pieces_into(&mut vm, sys), broken);
    assert_eq!(diagnosed(&mut vm), "()");

    // No root system: nothing to process or break.
    let layout = eval(&mut vm, "(ly:grob-layout (staff 0))");
    let lone = vm.heap_mut().wrap(PaperScore::new(layout));
    vm.heap_mut().root(lone);
    paper_score::process(&mut vm, lone).unwrap();
    paper_score::break_systems(&mut vm, lone).unwrap();
    assert!(vm.heap().foreign::<PaperScore>(lone).unwrap().broken);
}

#[test]
fn a_score_in_a_markup_is_formatted_only_with_a_layout_and_no_errors() {
    let Some((mut vm, ps)) = breaking_vm("") else {
        return;
    };
    // `ly:music-output?`: a Paper_score, nothing else.
    let is = eval(&mut vm, "ly:music-output?");
    assert_eq!(Host::call(&mut vm, is, &[ps]).unwrap(), Value::TRUE);
    assert_eq!(caught(&mut vm, "(ly:music-output? 1)"), "#f");
    let f = eval(&mut vm, "ly:score-embedded-format");
    let midi = eval(
        &mut vm,
        "(define-public midi (ly:make-output-def))
         (ly:output-def-set-variable! midi 'output-def-kind 'midi)
         midi",
    );
    let score = |vm: &mut Vm, error: bool| {
        let mut s = Score::new(Value::FALSE);
        s.defs = vec![midi];
        s.error = error;
        let v = vm.heap_mut().wrap(s);
        vm.heap_mut().root(v);
        v
    };
    // No `\layout` among its definitions: #f; errors in its music: '(),
    // before one is looked for.
    let unlaid = score(&mut vm, false);
    assert_eq!(
        Host::call(&mut vm, f, &[unlaid, midi]).unwrap(),
        Value::FALSE
    );
    let failed = score(&mut vm, true);
    assert_eq!(
        Host::call(&mut vm, f, &[failed, midi]).unwrap(),
        Value::NULL
    );
    // LY_ASSERT_SMOB's type errors, the score's first.
    let one = eval(&mut vm, "1");
    for (args, at, what) in [([one, one], 1, "Score"), ([failed, one], 2, "Output_def")] {
        let e = Host::call(&mut vm, f, &args).unwrap_err().text(vm.heap());
        let want = format!("Wrong type argument in position {at} (expecting {what})");
        assert!(
            e.contains("ly:score-embedded-format") && e.contains(&want),
            "{e}"
        );
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_paper_system_is_a_prob_with_its_stencil_and_properties() {
    let Some(mut vm) = grobs_vm() else { return };
    // The details' Y extent replaces the stencil's.
    let init = eval(&mut vm, "'((Y-extent . (-1 . 1)) (a . 1))");
    let p = system::make_paper_system(vm.heap_mut(), init);
    vm.heap_mut().root(p);
    let s = eval(
        &mut vm,
        "(ly:register-stencil-expression 'x)
         (ly:make-stencil '(x) '(0 . 2) '(-3 . 3))",
    );
    let st = *vm.heap().foreign::<Stencil>(s).unwrap();
    system::paper_system_set_stencil(vm.heap_mut(), p, st);
    let f = eval(
        &mut vm,
        "(lambda (p)
           (let ((st (ly:prob-property p 'stencil)))
             (ly:prob-set-property! p 'a 5)
             (list (ly:prob? p) (ly:stencil-expr st) (ly:stencil-extent st 0)
                   (ly:stencil-extent st 1) (ly:prob-property p 'a)
                   (ly:prob-property p 'none 'fallback)
                   (ly:prob-immutable-properties p))))",
    );
    let got = Host::call(&mut vm, f, &[p]).unwrap();
    assert_eq!(
        shown(&vm, got),
        "(#t (x) (0.0 . 2.0) (-1.0 . 1.0) 5 fallback ((Y-extent -1 . 1) (a . 1)))"
    );
    // No number pair: the stencil's own.
    let init = eval(&mut vm, "'((Y-extent . wide))");
    let q = system::make_paper_system(vm.heap_mut(), init);
    vm.heap_mut().root(q);
    system::paper_system_set_stencil(vm.heap_mut(), q, st);
    let y = eval(
        &mut vm,
        "(lambda (p) (ly:stencil-extent (ly:prob-property p 'stencil) 1))",
    );
    let got = Host::call(&mut vm, y, &[q]).unwrap();
    assert_eq!(shown(&vm, got), "(-3.0 . 3.0)");

    // As LilyPond 2.26.0 prints and compares them (session 6's prob.ly):
    // a property set again changes in place, a new one goes first; equal?
    // compares the properties in order, not the types: p and r set theirs
    // to the same list, q in the other order.
    let made: Vec<Value> = ["'((a . 1))", "'((a . 1))", "'((a . 1))"]
        .iter()
        .map(|init| {
            let init = eval(&mut vm, init);
            let p = system::make_paper_system(vm.heap_mut(), init);
            vm.heap_mut().root(p);
            p
        })
        .collect();
    let f = eval(
        &mut vm,
        "(lambda (p q r)
           (for-each (lambda (k v) (ly:prob-set-property! p k v)) '(b c b) '(2 3 4))
           (for-each (lambda (k v) (ly:prob-set-property! q k v)) '(c b) '(3 4))
           (for-each (lambda (k v) (ly:prob-set-property! r k v)) '(b c) '(4 3))
           (list (format #f \"~a\" p) (equal? p q) (equal? p r)
                 (ly:prob-mutable-properties p)))",
    );
    let got = Host::call(&mut vm, f, &made).unwrap();
    assert_eq!(
        shown(&vm, got),
        "(#<Prob: paper-system C++: Prob((c . 3) (b . 4))((a . 1)) >\n #f #t \
         ((c . 3) (b . 4)))"
    );
}

/// The System's callbacks, each with the type its argument is checked
/// for and the C++ method that names the error (LilyPond 2.26.0's,
/// session 5's `errs.ly`), and its arity. `ly:system::calc-pure-height`
/// checks no system in LilyPond, which reads through a null.
const SYSTEM_CALLBACKS: [(&str, &str, &str, usize); 7] = [
    ("ly:system::height", "ly:height", "Grob", 1),
    (
        "ly:system::calc-pure-height",
        "ly:calc-pure-height",
        "System",
        3,
    ),
    (
        "ly:system::calc-pure-relevant-grobs",
        "ly:calc-pure-relevant-grobs",
        "Grob",
        1,
    ),
    (
        "ly:system::footnotes-before-line-breaking",
        "ly:footnotes-before-line-breaking",
        "Grob",
        1,
    ),
    (
        "ly:system::footnotes-after-line-breaking",
        "ly:footnotes-after-line-breaking",
        "System",
        1,
    ),
    (
        "ly:system::vertical-skyline-elements",
        "ly:vertical-skyline-elements",
        "System",
        1,
    ),
    (
        "ly:system::get-vertical-alignment",
        "ly:get-vertical-alignment",
        "Grob",
        1,
    ),
];

#[test]
fn the_system_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, method, kind, arity) in SYSTEM_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let rest = " 0".repeat(arity - 1);
        let got = caught(&mut vm, &format!("({name} 'x{rest})"));
        let want = format!("Wrong type argument in position 1 (expecting {kind}): x");
        assert!(got.contains(method) && got.contains(&want), "{name}: {got}");
        for args in ["", &" 'x".repeat(arity + 1)] {
            let got = caught(&mut vm, &format!("({name}{args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
    // The columns of the pure height are converted first, as LilyPond's.
    let got = caught(&mut vm, "(ly:system::calc-pure-height 'x 'a 0)");
    assert!(
        got.contains("Wrong type (expecting exact integer): a"),
        "{got}"
    );
}

#[test]
fn the_system_s_pure_height_is_its_staves_at_their_translations_and_its_own() {
    let Some(mut vm) = grobs_vm() else { return };
    let sys = grob_of(&mut vm, "(group 3)");
    system::make(vm.heap_mut(), sys);
    // Groups 0 and 1 in align 0, translated by 0 and -10; each, and the
    // system, with its heights by measure between the ranks 0, 4, 8, 12.
    eval(
        &mut vm,
        "(define sys (group 3))
         (define (%break-ranks me) #(0 4 8 12))
         (define (%align-pure-minimum-translations me all-grobs a start end)
           (set! translated (list (eq? me (align 0)) all-grobs a start end))
           '(0.0 -10.0))
         (define translated #f)
         (for-each (lambda (g)
                     (ly:grob-set-parent! g 1 (align 0))
                     (ly:grob-set-object! g 'pure-Y-common g)
                     (put! g 'adjacent-pure-heights
                           (cons #((0 . 1) (2 . 3) (-100 . 100)) #((5 . 6) (7 . 8) (-4 . -3)))))
                   (list (group 0) (group 1)))
         (ly:grob-set-object! (align 0) 'elements (arr (group 0) (group 1)))
         (ly:grob-set-object! sys 'pure-Y-common sys)
         (put! sys 'adjacent-pure-heights
               (cons #((0 . 0) (10 . 11) (0 . 0)) #((0 . 0) (-20 . -19) (0 . 0))))",
    );
    // No vertical alignment: empty, the system's own heights left out.
    assert_eq!(
        caught(&mut vm, "(ly:system::calc-pure-height sys 4 12)"),
        "(+inf.0 . -inf.0)"
    );
    // At the beginning of the line from 4 to 5, (2 . 3), (-8 . -7) and
    // the system's (10 . 11); in the rest from 4 to 12, (-4 . 8),
    // (-14 . -2) and the system's (-20 . 0).
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'vertical-alignment (align 0))",
    );
    assert_eq!(
        caught(&mut vm, "(ly:system::calc-pure-height sys 4 12)"),
        "(-20.0 . 11.0)"
    );
    assert_eq!(
        evaluated(&mut vm, "translated"),
        format!(
            "(#t ({} {}) 1 4 12)",
            evaluated(&mut vm, "(group 0)"),
            evaluated(&mut vm, "(group 1)")
        )
    );
    // Fewer translations than staves: the rest not moved.
    eval(
        &mut vm,
        "(define (%align-pure-minimum-translations me all-grobs a start end) '(1.0))",
    );
    assert_eq!(
        caught(&mut vm, "(ly:system::calc-pure-height sys 4 8)"),
        "(-20.0 . 11.0)"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_system_s_relevant_grobs_alignment_skyline_elements_and_staves() {
    let Some(mut vm) = grobs_vm() else { return };
    let [sys, t1] = ["(group 3)", "(thing 1)"].map(|t| grob_of(&mut vm, t));
    system::make(vm.heap_mut(), sys);
    let piece = item::clone(&mut vm, t1).unwrap();
    let sym = vm.heap_mut().intern("piece");
    grob::set_object(vm.heap_mut(), sys, sym, piece);
    eval(
        &mut vm,
        "(define sys (group 3))
         (define piece (ly:grob-object sys 'piece))
         (ly:grob-set-object! sys 'elements
                              (arr (group 0) (thing 0) piece (delim 0) (staff 0)))
         (ly:grob-set-object! (align 0) 'elements (arr (staff 1) (group 1) (group 2)))
         (define (names v)
           (map (lambda (g)
                  (cond ((eq? g piece) 'piece)
                        (else (assq-ref (ly:grob-property g 'meta) 'name))))
                (if (ly:grob-array? v) (ly:grob-array->list v) v)))",
    );
    // No axis group and no broken piece of an item.
    assert_eq!(
        evaluated(&mut vm, "(names (ly:system::calc-pure-relevant-grobs sys))"),
        "(Thing Delim Staff)"
    );
    // No alignment among its elements: '(), LilyPond's programming error.
    assert_eq!(
        evaluated(&mut vm, "(ly:system::get-vertical-alignment sys)"),
        "()"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"didn't find a vertical alignment in this system\")"
    );
    // Its delimiters, alone without its vertical-alignment.
    assert_eq!(
        evaluated(
            &mut vm,
            "(names (ly:system::vertical-skyline-elements sys))"
        ),
        "(Delim)"
    );
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'elements
                              (arr (group 0) (align 0) (thing 0) piece (delim 0)))",
    );
    assert_eq!(
        evaluated(
            &mut vm,
            "(eq? (ly:system::get-vertical-alignment sys) (align 0))"
        ),
        "#t"
    );
    // Then with the alignment's hara-kiri groups.
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'vertical-alignment (align 0))",
    );
    assert_eq!(
        evaluated(
            &mut vm,
            "(names (ly:system::vertical-skyline-elements sys))"
        ),
        "(Delim Group Group)"
    );
    // The staves of `vertical_alignment`, which LilyPond 2.26.0 reads and
    // no grob has: none.
    for f in [
        "get-staves",
        "get-spaceable-staves",
        "get-nonspaceable-staves",
    ] {
        assert_eq!(evaluated(&mut vm, &format!("(ly:system::{f} sys)")), "()");
    }
    // Under that name, the live ones, spaceable or not by their
    // staff-affinity.
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'vertical_alignment (align 0))
         (put! (group 1) 'staff-affinity 1)
         (ly:grob-suicide! (group 2))",
    );
    assert_eq!(
        evaluated(&mut vm, "(names (ly:system::get-staves sys))"),
        "(Staff Group)"
    );
    assert_eq!(
        evaluated(&mut vm, "(names (ly:system::get-spaceable-staves sys))"),
        "(Staff)"
    );
    assert_eq!(
        evaluated(&mut vm, "(names (ly:system::get-nonspaceable-staves sys))"),
        "(Group)"
    );
    assert!(caught(&mut vm, "(ly:system::get-staves (thing 0))").contains("expecting System"));
    // The rank page layout reads.
    assert_eq!(evaluated(&mut vm, "(%system-rank sys)"), "0");
    system::set_rank(vm.heap(), sys, 3);
    assert_eq!(evaluated(&mut vm, "(%system-rank sys)"), "3");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_footnotes_after_line_breaking_are_those_inside_the_system_by_column() {
    let Some(mut vm) = line_vm(5, "") else { return };
    let sys = grob_of(&mut vm, "sys");
    system::make(vm.heap_mut(), sys);
    let [c0, c4] = ["(col 0)", "(col 4)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, sys, -1, c0).unwrap();
    spanner::set_bound(&mut vm, sys, 1, c4).unwrap();
    // Footnotes on columns 3, 1, 2, 0 and 4, and a thing on column 2;
    // (span 0), a footnote from column 1 to 4 placed on its left and
    // moved right, after them.
    for t in [
        "(foot 0)",
        "(foot 1)",
        "(foot 2)",
        "(foot 3)",
        "(thing 0)",
        "(span 0)",
    ] {
        let g = grob_of(&mut vm, t);
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    let [span, c1] = ["(span 0)", "(col 1)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, span, -1, c1).unwrap();
    spanner::set_bound(&mut vm, span, 1, c4).unwrap();
    eval(
        &mut vm,
        "(for-each (lambda (g c) (ly:grob-set-parent! g 0 (col c)))
                   (list (foot 0) (foot 1) (foot 2) (foot 3) (thing 0)) '(3 1 2 0 2))
         (put! (span 0) 'X-offset 1)
         (define (index g)
           (let loop ((i 0)) (cond ((eq? g (span 0)) 's) ((eq? (foot i) g) i) (else (loop (1+ i))))))",
    );
    assert_eq!(
        evaluated(
            &mut vm,
            "(map index (ly:grob-array->list (ly:system::footnotes-before-line-breaking sys)))"
        ),
        "(0 1 2 3 s)"
    );
    // Those at the ends, which are not at the end of a line, left out; the
    // rest by their columns.
    eval(
        &mut vm,
        "(ly:grob-set-object! sys 'footnotes-before-line-breaking
                              (ly:system::footnotes-before-line-breaking sys))",
    );
    let after = "(map index (ly:grob-array->list (ly:system::footnotes-after-line-breaking sys)))";
    assert_eq!(evaluated(&mut vm, after), "(1 2 0 s)");
    // Placed on its right, at the system's end: still in, at the end of
    // the line.
    eval(&mut vm, "(put! (span 0) 'spanner-placement 1)");
    assert_eq!(evaluated(&mut vm, after), "(1 2 0 s)");
    // Not visible, or dead: left out.
    eval(
        &mut vm,
        "(put! (foot 2) 'break-visibility #(#t #f #t))
         (ly:grob-suicide! (foot 1))",
    );
    assert_eq!(evaluated(&mut vm, after), "(0 s)");
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The rods kept on `from` (`minimum-distances`), each as the index in
/// `names` of the column it goes to and its distance, in the order kept.
fn rods_of(vm: &mut Vm, from: &str, names: &[&str]) -> Vec<(usize, f64)> {
    let list = eval(
        vm,
        &format!("(reverse (ly:grob-object {from} 'minimum-distances '()))"),
    );
    let grobs: Vec<Value> = names.iter().map(|t| grob_of(vm, t)).collect();
    let mut out = Vec::new();
    let mut p = list;
    while let (Some(entry), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
        let to = vm.heap().car(entry).unwrap();
        let d = vm.heap().cdr(entry).and_then(|d| vm.heap().to_f64(d));
        out.push((grobs.iter().position(|&g| g == to).unwrap(), d.unwrap()));
        p = rest;
    }
    out
}

/// A [`line_vm`] of columns 0 to 4 and their pieces: column 1's piece
/// ending a line (column 5), column 2's beginning one (column 6) and
/// column 4's ending one (column 7), each of its original's rank. Columns
/// 1 to 3 breakable, 3 on the system already; the spacer's left bound
/// (thing 1) 0.5 left of column 0, its right column 4.
fn rods_vm(setup: &str) -> Option<Vm> {
    let mut vm = line_vm(
        8,
        "(for-each (lambda (i) (put! (col i) 'line-break-permission 'allow)) '(1 2 3))
         (place! (thing 1) (col 0) (thing 5))
         (put! (thing 1) 'X-offset -0.5)",
    )?;
    let c: Vec<Value> = (0..8)
        .map(|i| grob_of(&mut vm, &format!("(col {i})")))
        .collect();
    for (orig, piece, d) in [(1, 5, 0), (2, 6, 1), (4, 7, 0)] {
        let rank = paper_column::rank(vm.heap(), c[orig]);
        paper_column::set_rank(vm.heap(), c[piece], rank);
        grob::with_grob_mut(vm.heap(), c[orig], |g| g.broken[d] = Some(c[piece]));
    }
    let sys = grob_of(&mut vm, "sys");
    paper_column::set_system(vm.heap(), c[3], sys);
    let [spacer, t1] = ["spacer", "(thing 1)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, spacer, -1, t1).unwrap();
    spanner::set_bound(&mut vm, spacer, 1, c[4]).unwrap();
    eval(&mut vm, setup);
    Some(vm)
}

#[test]
fn a_spanner_s_minimum_length_is_rods_past_its_broken_columns_and_between_its_bounds() {
    const NAMES: [&str; 8] = [
        "(col 0)", "(col 1)", "(col 2)", "(col 3)", "(col 4)", "(col 5)", "(col 6)", "(col 7)",
    ];
    let Some(mut vm) = rods_vm(
        "(put! spacer 'minimum-length 2)
         (put! spacer 'minimum-length-after-break 3)",
    ) else {
        return;
    };
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    // From the left bound, 1.5 from column 0, to column 1's piece ending a
    // line; from column 2's piece beginning one to column 4, 1.5 and the
    // difference after a break, 1; between the bounds, and to column 4's
    // piece ending a line, 1.5.
    assert_eq!(
        rods_of(&mut vm, "(col 0)", &NAMES),
        [(5, 1.5), (4, 1.5), (7, 1.5)]
    );
    assert_eq!(rods_of(&mut vm, "(col 6)", &NAMES), [(4, 2.5)]);
    for i in [1, 2, 3, 4, 5, 7] {
        assert!(rods_of(&mut vm, NAMES[i], &NAMES).is_empty(), "{i}");
    }
    assert_eq!(diagnosed(&mut vm), "()");

    // Without minimum-length-after-break, minimum-length after a break
    // too; without minimum-length, 0 but after a break.
    let mut vm = rods_vm("(put! spacer 'minimum-length 2)").unwrap();
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    assert_eq!(
        rods_of(&mut vm, "(col 0)", &NAMES),
        [(5, 1.5), (4, 1.5), (7, 1.5)]
    );
    assert_eq!(rods_of(&mut vm, "(col 6)", &NAMES), [(4, 1.5)]);
    let mut vm = rods_vm("(put! spacer 'minimum-length-after-break 3)").unwrap();
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    // The rods of -0.5 are not kept.
    assert!(rods_of(&mut vm, "(col 0)", &NAMES).is_empty());
    assert_eq!(rods_of(&mut vm, "(col 6)", &NAMES), [(4, 2.5)]);

    // Neither a number: no rods; nor with a bound missing.
    let mut vm = rods_vm("(put! spacer 'minimum-length 'x)").unwrap();
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    for name in NAMES {
        assert!(rods_of(&mut vm, name, &NAMES).is_empty(), "{name}");
    }
    let mut vm = rods_vm("(put! spacer 'minimum-length 2)").unwrap();
    let spacer = grob_of(&mut vm, "spacer");
    grob::with_grob_mut(vm.heap(), spacer, |g| g.bounds[1] = None);
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    for name in NAMES {
        assert!(rods_of(&mut vm, name, &NAMES).is_empty(), "{name}");
    }

    // No root system, where LilyPond reads through a null: no broken
    // columns, the rods between the bounds only.
    let mut vm = rods_vm(
        "(put! spacer 'minimum-length 2)
         (ly:grob-set-parent! spacer 1 (thing 5))",
    )
    .unwrap();
    eval(&mut vm, "(ly:spanner::set-spacing-rods spacer)");
    assert_eq!(rods_of(&mut vm, "(col 0)", &NAMES), [(4, 1.5), (7, 1.5)]);
    assert!(rods_of(&mut vm, "(col 6)", &NAMES).is_empty());
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_clique_of_loose_columns_is_placed_from_its_right_end() {
    // Columns 2 and 3 loose between 1 and 4: 3 at 0.5 left of 4, 2 at
    // 0.5 left of 3; then 3 again, between 2 and 4, where it is.
    let Some(mut vm) = loose_vm(
        "(put! (col 3) 'X-extent '(0 . 0.25))
         (ly:grob-set-object! (col 3) 'between-cols (cons (col 2) (col 4)))
         (ly:grob-set-object! (col 3) 'spacing spacer)",
    ) else {
        return;
    };
    let c3 = grob_of(&mut vm, "(col 3)");
    // Loose, so on no system until placed.
    grob::with_grob_mut(vm.heap(), c3, |g| g.column.system = None);
    assert_eq!(place_loose(&mut vm, &["(col 2)", "(col 3)"]), [13.0, 13.5]);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_loose_column_off_the_line_hangs_from_the_system_s_end() {
    // Hanging between 1 and 4, which is on no system: to the system's
    // right bound, column 3, which ranks before 4.
    let between = "(ly:grob-set-object! (col 2) 'between-cols (cons (col 1) (col 4)))";
    let Some(mut vm) = loose_vm(between) else {
        return;
    };
    let [sys, c3, c4] = ["sys", "(col 3)", "(col 4)"].map(|t| grob_of(&mut vm, t));
    grob::with_grob_mut(vm.heap(), c4, |g| g.column.system = None);
    spanner::set_bound(&mut vm, sys, 1, c3).unwrap();
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [9.5]);
    assert_eq!(diagnosed(&mut vm), "()");

    // Bounded by 4 itself: LilyPond's programming error, and 4 kept
    // without an original system to search.
    let mut vm = loose_vm(between).unwrap();
    let [sys, c4] = ["sys", "(col 4)"].map(|t| grob_of(&mut vm, t));
    grob::with_grob_mut(vm.heap(), c4, |g| g.column.system = None);
    spanner::set_bound(&mut vm, sys, 1, c4).unwrap();
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [13.5]);
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Loose column does not have right side to attach to.\")"
    );
    // With one, its last column on the system before the bound: 3.
    let mut vm = loose_vm(between).unwrap();
    let [sys, c4] = ["sys", "(col 4)"].map(|t| grob_of(&mut vm, t));
    grob::with_grob_mut(vm.heap(), c4, |g| g.column.system = None);
    spanner::set_bound(&mut vm, sys, 1, c4).unwrap();
    grob::with_grob_mut(vm.heap(), sys, |g| g.original = Some(sys));
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [9.5]);
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Loose column does not have right side to attach to.\")"
    );

    // Between what are no columns: LilyPond's programming error, and
    // not placed.
    let mut vm = loose_vm("(ly:grob-set-object! (col 2) 'between-cols '(1 . 2))").unwrap();
    assert_eq!(place_loose(&mut vm, &["(col 2)"]), [0.0]);
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Can't attach loose column sensibly.  Attaching to end of system.\")"
    );
}

/// `(ly:spacing-spanner::calc-common-shortest-duration spacer)` over
/// [`line_vm`]'s `n` columns after `setup`, as `display` writes it.
fn common_shortest(n: usize, setup: &str) -> Option<String> {
    let mut vm = line_vm(n, setup)?;
    let v = eval(
        &mut vm,
        "(ly:spacing-spanner::calc-common-shortest-duration spacer)",
    );
    let m = vm.heap().foreign::<Moment>(v).copied().expect("a moment");
    assert!(m.grace.is_zero());
    let mut out = String::new();
    m.main.write(&mut out);
    Some(out)
}

#[test]
fn the_common_shortest_duration_is_the_one_most_measures_start() {
    // Bars at 0, 3, 5 and 8; measures of shortest 1/16, 1/8 and 1/4 (the
    // grace column left out).
    let measures = |third: &str| {
        format!(
            "(for-each (lambda (i) (put! (col i) 'line-break-permission 'allow)) '(0 3 5 8))
             (put! (col 1) 'shortest-starter-duration 1/8)
             (put! (col 2) 'shortest-starter-duration 1/16)
             (put! (col 4) 'shortest-starter-duration 1/8)
             (put! (col 6) 'shortest-starter-duration 1/32)
             (put! (col 6) 'when (ly:make-moment 1 -1/32))
             (put! (col 7) 'shortest-starter-duration {third})"
        )
    };
    // A tie: the shortest, under the base's 1/8.
    let Some(got) = common_shortest(9, &measures("1/4")) else {
        return;
    };
    assert_eq!(got, "1/16");
    // Two of 1/8: 1/8, and so the base's.
    assert_eq!(common_shortest(9, &measures("1/8")).unwrap(), "1/8");
    assert_eq!(
        common_shortest(
            9,
            &format!(
                "{} (put! spacer 'base-shortest-duration (ly:make-moment 1/32))",
                measures("1/8")
            )
        )
        .unwrap(),
        "1/32"
    );
    // A longer base, and its main part only.
    assert_eq!(
        common_shortest(
            9,
            &format!(
                "{} (put! spacer 'base-shortest-duration (ly:make-moment 1 2 1 64))",
                measures("1/8")
            )
        )
        .unwrap(),
        "1/8"
    );
    // A base that is no moment: 1/8.
    assert_eq!(
        common_shortest(
            9,
            &format!(
                "{} (put! spacer 'base-shortest-duration 1/2)",
                measures("1/4")
            )
        )
        .unwrap(),
        "1/16"
    );
    // The measure of 1/4 four times over the 1/16 and 1/8: the base's.
    assert_eq!(
        common_shortest(
            9,
            "(for-each (lambda (i) (put! (col i) 'line-break-permission 'allow)) '(0 2 4 6 8))
             (for-each (lambda (i) (put! (col i) 'shortest-starter-duration 1/4)) '(1 3 5 7))"
        )
        .unwrap(),
        "1/8"
    );
    // No measure ends after the music: none counted.
    assert_eq!(
        common_shortest(
            4,
            "(put! (col 0) 'line-break-permission 'allow)
             (put! (col 1) 'shortest-starter-duration 1/32)
             (put! (col 2) 'line-break-permission 'allow)
             (put! (col 3) 'shortest-starter-duration 1/64)"
        )
        .unwrap(),
        "1/32"
    );

    // The callbacks check their spanner.
    let mut vm = line_vm(2, "").unwrap();
    for name in ["set-springs", "calc-common-shortest-duration"] {
        let e = vm
            .eval_text(&format!("(ly:spacing-spanner::{name} (col 0))"), None)
            .unwrap_err();
        assert_eq!(
            e.text(vm.heap()),
            format!(
                "In procedure ly:spacing-spanner::{name}: Wrong type argument in \
                 position 1 (expecting Spanner): #<Grob Piece >"
            )
        );
    }
}

/// The springs kept on the grobs `from` (`ideal-distances`), each as
/// `(from, to, spring)` by the names given, in the order kept.
fn kept_springs(vm: &mut Vm, names: &[(&str, &str)]) -> Vec<(String, String, Spring)> {
    let grobs: Vec<(Value, String)> = names
        .iter()
        .map(|(t, n)| (grob_of(vm, t), n.to_string()))
        .collect();
    let name = |v: Value| {
        grobs
            .iter()
            .find(|(g, _)| *g == v)
            .map_or("?".into(), |(_, n)| n.clone())
    };
    let mut out = Vec::new();
    for (g, n) in &grobs {
        let list = eval(
            vm,
            &format!(
                "(reverse (ly:grob-object {} 'ideal-distances '()))",
                names.iter().find(|(_, m)| m == n).unwrap().0
            ),
        );
        let _ = g;
        let mut p = list;
        while let (Some(entry), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            let to = vm.heap().cdr(entry).unwrap();
            let sp = spaceable::get_spring(vm, *g, to).unwrap();
            out.push((n.clone(), name(to), sp));
            p = rest;
        }
    }
    out
}

/// The rods kept on the columns of [`line_vm`] (`minimum-distances`),
/// each as `(from, to, distance)` by rank, in the order kept.
fn kept_rods(vm: &mut Vm, n: usize) -> Vec<(u64, u64, f64)> {
    let mut out = Vec::new();
    for i in 0..n {
        let c = grob_of(vm, &format!("(col {i})"));
        let list = eval(
            vm,
            &format!("(reverse (ly:grob-object (col {i}) 'minimum-distances '()))"),
        );
        let mut p = list;
        while let (Some(entry), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            let to = vm.heap().car(entry).unwrap();
            let d = vm
                .heap()
                .cdr(entry)
                .and_then(|d| vm.heap().to_f64(d))
                .unwrap();
            out.push((
                paper_column::rank(vm.heap(), c),
                paper_column::rank(vm.heap(), to),
                d,
            ));
            p = rest;
        }
    }
    out
}

/// `Spring (ideal, min)` with its inverse stretch strength `stretch`,
/// through the setters.
fn set_spring(ideal: f64, min: f64, stretch: Option<f64>) -> Spring {
    let mut errors = Vec::new();
    let mut s = Spring::new(ideal, min, &mut errors);
    if let Some(stretch) = stretch {
        s.set_inverse_stretch_strength(stretch, &mut errors);
    }
    assert!(errors.is_empty());
    s
}

/// A bar (0), two quarters (1, 2) and a bar (3) at 1/2, spaced with an
/// increment of 1.2, a shortest duration space of 2 and the common
/// shortest duration from its callback, after `setup`.
const QUARTERS: &str = "(put! (col 0) 'line-break-permission 'allow)
     (put! (col 3) 'line-break-permission 'allow)
     (put! (col 1) 'shortest-starter-duration 1/4)
     (put! (col 1) 'shortest-playing-duration 1/4)
     (put! (col 2) 'shortest-starter-duration 1/4)
     (put! (col 2) 'shortest-playing-duration 1/4)
     (put! (col 2) 'when (ly:make-moment 1/4))
     (put! (col 3) 'when (ly:make-moment 1/2))
     (put! spacer 'spacing-increment 1.2)
     (put! spacer 'shortest-duration-space 2.0)
     (put! spacer 'common-shortest-duration
           ly:spacing-spanner::calc-common-shortest-duration)";

/// The names of [`QUARTERS`]' columns.
const FOUR: [(&str, &str); 4] = [
    ("(col 0)", "0"),
    ("(col 1)", "1"),
    ("(col 2)", "2"),
    ("(col 3)", "3"),
];

/// [`line_vm`]'s 4 columns of [`QUARTERS`] after `setup`, their springs
/// set by the callback.
fn quarters(setup: &str) -> Option<Vm> {
    let mut vm = line_vm(4, &format!("{QUARTERS} {setup}"))?;
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    Some(vm)
}

/// `(from, to, spring)` with the names as strings.
fn kept(from: &str, to: &str, s: Spring) -> (String, String, Spring) {
    (from.into(), to.into(), s)
}

#[test]
fn the_springs_are_set_between_each_pair_of_columns_by_their_durations() {
    let Some(mut vm) = quarters("") else {
        return;
    };
    // The bar to the first quarter, at the same moment: 0.5 past their
    // minimum distance of 0. Each quarter (2 + log₂ 2) × 1.2 of the
    // common shortest 1/8 (the base's, under the measures' 1/4), at
    // least the increment, stretching as the difference; no minimum
    // before a musical column without spacing wishes.
    let len = (2.0 + 1.0) * 1.2;
    let quarter = set_spring(len, 1.2, Some(len - 1.2));
    let mut to_musical = quarter;
    to_musical.set_min_distance(0.0, &mut Vec::new());
    assert_eq!(
        kept_springs(&mut vm, &FOUR),
        [
            kept("0", "1", set_spring(0.5, 0.0, None)),
            kept("1", "2", to_musical),
            kept("2", "3", quarter),
        ]
    );
    // The breakable and musical columns' neighbours, in order.
    let v = eval(
        &mut vm,
        "(define (index-of g)
           (let loop ((cs cols) (i 0))
             (if (eq? (car cs) g) i (loop (cdr cs) (1+ i)))))
         (map (lambda (c)
                (map (lambda (n)
                       (let ((g (ly:grob-object c n #f)))
                         (if g (index-of g) g)))
                     '(left-neighbor right-neighbor)))
              cols)",
    );
    assert_eq!(shown(&vm, v), "((#f 1) (0 2) (1 3) (2 #f))");
    // No skylines, no rods.
    assert_eq!(kept_rods(&mut vm, 4), []);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_rods_reach_over_the_columns_until_none_can_be_reached() {
    // Column 0 sticks out to 6.5, column 1 from -1 to 1, column 2 from
    // -1.5 to 1.5, each widened by 0.5 and at the same height.
    let three = |y1: &str| {
        format!(
            "(sky! (col 0) (box! (bit 0) (col 0) '(0 . 6) '(0 . 1)))
             (sky! (col 1) (box! (bit 1) (col 1) '(-0.5 . 0.5) '{y1}))
             (sky! (col 2) (box! (bit 2) (col 2) '(-1 . 1) '(0 . 1)))
             (put! (col 0) 'line-break-permission 'allow)
             (put! (col 2) 'line-break-permission 'allow)"
        )
    };
    let Some(mut vm) = line_vm(3, &three("(0 . 1)")) else {
        return;
    };
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    // 6.5 + 1 + 0.1 and 1 + 1.5 + 0.1; column 0 cannot reach column 2
    // past 7.6 + 2.6 less column 2's stickout of 1.5.
    assert_eq!(kept_rods(&mut vm, 3), [(0, 1, 7.6), (1, 2, 2.6)]);

    // Column 1 above the others: column 0 reaches over it, 6.5 + 1.5 +
    // 0.1 from column 2.
    let mut vm = line_vm(3, &three("(5 . 6)")).unwrap();
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    assert_eq!(kept_rods(&mut vm, 3), [(0, 2, 6.5 + 1.5 + 0.1)]);

    // The last column's padding.
    let mut vm = line_vm(
        3,
        &format!("{} (put! (col 2) 'padding 1)", three("(0 . 1)")),
    )
    .unwrap();
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    assert_eq!(kept_rods(&mut vm, 3), [(0, 1, 8.5), (1, 2, 3.5)]);

    // Column 1 above, and column 2 sticking out 1.5 right of its
    // column: only column 0's overhang carried past column 1 (6.5, not
    // column 1's 1) reaches it, 6.5 - 1.5 + 0.1 apart.
    let mut vm = line_vm(
        3,
        "(sky! (col 0) (box! (bit 0) (col 0) '(0 . 6) '(0 . 1)))
         (sky! (col 1) (box! (bit 1) (col 1) '(-0.5 . 0.5) '(5 . 6)))
         (sky! (col 2) (box! (bit 2) (col 2) '(2 . 3) '(0 . 1)))
         (put! (col 0) 'line-break-permission 'allow)
         (put! (col 2) 'line-break-permission 'allow)",
    )
    .unwrap();
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    assert_eq!(kept_rods(&mut vm, 3), [(0, 2, 6.5 - 1.5 + 0.1)]);

    // An empty column is no rod's.
    let mut vm = line_vm(3, &format!("{} (sky! (col 1))", three("(0 . 1)"))).unwrap();
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    assert_eq!(kept_rods(&mut vm, 3), [(0, 2, 6.5 + 1.5 + 0.1)]);
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_loose_column_is_pruned_between_its_neighbours() {
    // Column 2, a clef change between the quarters at 1 and 3, whose
    // spacing wishes, bits 0 to 2, make columns 1 and 3 its neighbours.
    let line = |setup: &str| {
        format!(
            "(put! (col 0) 'line-break-permission 'allow)
             (put! (col 4) 'line-break-permission 'allow)
             (put! (col 1) 'shortest-starter-duration 1/4)
             (put! (col 1) 'shortest-playing-duration 1/4)
             (put! (col 3) 'shortest-starter-duration 1/4)
             (put! (col 3) 'shortest-playing-duration 1/4)
             (put! (col 3) 'when (ly:make-moment 1/4))
             (put! (col 4) 'when (ly:make-moment 1/2))
             (put! (col 2) 'when (ly:make-moment 1/4))
             (put! (col 2) 'allow-loose-spacing #t)
             (put! spacer 'spacing-increment 1.2)
             (put! spacer 'shortest-duration-space 2.0)
             (put! spacer 'common-shortest-duration (ly:make-moment 1/8))
             (define (wish! b from to)
               (ly:grob-set-parent! b 0 from)
               (ly:grob-set-object! b 'right-items (arr to))
               (ly:grob-set-object! from 'spacing-wishes
                 (apply arr (cons b (ly:grob-array->list
                                     (ly:grob-object from 'spacing-wishes (arr)))))))
             (ly:grob-set-parent! (bit 5) 0 (col 2))
             (ly:grob-set-parent! (bit 6) 0 (col 3))
             (wish! (bit 0) (col 1) (bit 6))
             (wish! (bit 1) (col 1) (bit 5))
             (wish! (bit 2) (col 2) (bit 6))
             {setup}"
        )
    };
    let names = [
        ("(col 0)", "0"),
        ("(col 1)", "1"),
        ("(col 2)", "2"),
        ("(col 3)", "3"),
        ("(col 4)", "4"),
    ];
    let Some(mut vm) = line_vm(5, &line("")) else {
        return;
    };
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    let len = (2.0 + 1.0) * 1.2;
    let quarter = set_spring(len, 1.2, Some(len - 1.2));
    let mut to_musical = quarter;
    to_musical.set_min_distance(0.0, &mut Vec::new());
    // No spring to or from column 2, between columns 1 and 3, and a rod
    // between them of what its wishes, neither note nor staff spacing,
    // give: 0.
    assert_eq!(
        kept_springs(&mut vm, &names),
        [
            kept("0", "1", set_spring(0.5, 0.0, None)),
            kept("1", "3", to_musical),
            kept("3", "4", quarter),
        ]
    );
    let v = eval(
        &mut vm,
        "(let ((b (ly:grob-object (col 2) 'between-cols)))
           (list (eq? (car b) (col 1)) (eq? (cdr b) (col 3))
                 (ly:grob-property (col 2) 'maybe-loose)))",
    );
    assert_eq!(shown(&vm, v), "(#t #t ())");
    assert_eq!(kept_rods(&mut vm, 5), [(1, 3, 0.0)]);
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Subversive spacing wish\" \"Subversive spacing wish\")"
    );

    // Not loose: not allowed, or of some width in series with its
    // neighbours.
    for setup in [
        "(put! (col 2) 'allow-loose-spacing 'yes)",
        "(put! (col 2) 'X-extent '(0 . 1))",
    ] {
        let mut vm = line_vm(5, &line(setup)).unwrap();
        eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
        let got: Vec<_> = kept_springs(&mut vm, &names)
            .into_iter()
            .map(|(f, t, _)| format!("{f}{t}"))
            .collect();
        assert_eq!(got, ["01", "12", "23", "34"], "{setup}");
        assert_eq!(diagnosed(&mut vm), "()", "{setup}");
    }

    // Breakable, it stays, maybe loose, and the quarter spans it.
    let mut vm = line_vm(5, &line("(put! (col 2) 'line-break-permission 'allow)")).unwrap();
    eval(&mut vm, "(ly:spacing-spanner::set-springs spacer)");
    let got: Vec<_> = kept_springs(&mut vm, &names)
        .into_iter()
        .map(|(f, t, _)| format!("{f}{t}"))
        .collect();
    assert_eq!(got, ["01", "13", "23", "34"]);
    let v = eval(
        &mut vm,
        "(let ((b (ly:grob-object (col 2) 'between-cols)))
           (list (eq? (car b) (col 1)) (eq? (cdr b) (col 3))
                 (ly:grob-property (col 2) 'maybe-loose)))",
    );
    assert_eq!(shown(&vm, v), "(#t #t #t)");
}

#[test]
fn uniform_packed_and_grace_spacing_change_the_springs() {
    let len = (2.0 + 1.0) * 1.2;
    // Stretching uniformly: no minimum, the default strengths.
    let Some(mut vm) = quarters("(put! spacer 'uniform-stretching #t)") else {
        return;
    };
    let uniform = |ideal: f64, min: f64| {
        let mut s = set_spring(ideal, min, Some(len - 1.2));
        s.set_min_distance(0.0, &mut Vec::new());
        s.set_default_strength();
        s
    };
    assert_eq!(
        kept_springs(&mut vm, &FOUR),
        [
            kept("0", "1", uniform(0.5, 0.0)),
            kept("1", "2", uniform(len, 1.2)),
            kept("2", "3", uniform(len, 1.2)),
        ]
    );

    // Between the bar and the first quarter 4 apart (their skylines), 0.5
    // further: stretching uniformly compresses down to 0 too.
    let mut vm = quarters(
        "(put! spacer 'uniform-stretching #t)
         (sky! (col 0) (box! (bit 0) (col 0) '(0 . 2) '(0 . 1)))
         (sky! (col 1) (box! (bit 1) (col 1) '(-1 . 1) '(0 . 1)))",
    )
    .unwrap();
    assert_eq!(
        kept_springs(&mut vm, &FOUR)[0],
        kept("0", "1", spring(4.5, 0.0, 4.5, 4.5, -1.0))
    );

    // Packed: as long as the left column is wide, stretching as 1.
    let mut vm = quarters(
        "(put! spacer 'packed-spacing #t)
         (put! (col 1) 'X-extent '(-1 . 3))",
    )
    .unwrap();
    let packed = |ideal: f64, no_minimum: bool| {
        let mut s = set_spring(len, 1.2, Some(len - 1.2));
        if no_minimum {
            s.set_min_distance(0.0, &mut Vec::new());
        }
        s.set_ideal_distance(ideal, &mut Vec::new());
        s.set_inverse_stretch_strength(1.0, &mut Vec::new());
        s
    };
    assert_eq!(
        kept_springs(&mut vm, &FOUR),
        [
            kept("0", "1", set_spring(0.5, 0.0, None)),
            kept("1", "2", packed(3.0, true)),
            // Column 2 has no extent: its minimum.
            kept("2", "3", packed(1.2, false)),
        ]
    );

    // Into a grace note from a main note, 0.8 as long.
    let mut vm = quarters("(put! (col 2) 'when (ly:make-moment 1/4 -1/16))").unwrap();
    let mut grace = set_spring(len, 1.2, Some(len - 1.2));
    grace.set_min_distance(0.0, &mut Vec::new());
    grace *= 0.8;
    let springs = kept_springs(&mut vm, &FOUR);
    assert_eq!(springs[1], kept("1", "2", grace));
    // From it, without a grace spacing, the old grace spacing: half the
    // common shortest duration's (2 + 0) × 1.2 and half the increment.
    let half = (2.0 + 0.0) * 1.2 / 2.0;
    assert_eq!(
        springs[2],
        kept("2", "3", set_spring(half, 1.2 / 2.0, None))
    );
    // And a bar before a grace note, a grace sixteenth later: the space
    // of no main part, (2 + 0 - 1) × 1.2, 0.8 as long.
    let mut vm = quarters("(put! (col 1) 'when (ly:make-moment 0 -1/16))").unwrap();
    let mut bar = set_spring((2.0 + 0.0 - 1.0) * 1.2, 0.0, None);
    bar *= 0.8;
    assert_eq!(kept_springs(&mut vm, &FOUR)[0], kept("0", "1", bar));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_breakable_column_s_pieces_get_springs_too() {
    // Column 0 broken into pieces 10 (left, the end of a line) and 11
    // (right, the beginning of one); column 3 into 12 and 13.
    let Some(mut vm) = line_vm(4, QUARTERS) else {
        return;
    };
    for (c, l, r) in [(0, 10, 11), (3, 12, 13)] {
        let [col, lp, rp] = [c, l, r].map(|n| grob_of(&mut vm, &format!("(piece {n})")));
        for p in [lp, rp] {
            paper_column::make(vm.heap_mut(), p);
            paper_column::copied(vm.heap(), p, col);
            grob::with_grob_mut(vm.heap(), p, |g| g.original = Some(col));
        }
        grob::with_grob_mut(vm.heap(), col, |g| g.broken = [Some(lp), Some(rp)]);
    }
    eval(
        &mut vm,
        "(for-each (lambda (n) (put! (piece n) 'line-break-permission 'allow))
                   '(10 11 12 13))
         (put! (piece 12) 'when (ly:make-moment 1/2))
         (put! (piece 13) 'when (ly:make-moment 1/2))
         (ly:spacing-spanner::set-springs spacer)",
    );
    let names = [
        ("(col 0)", "0"),
        ("(col 1)", "1"),
        ("(col 2)", "2"),
        ("(col 3)", "3"),
        ("(piece 10)", "0<"),
        ("(piece 11)", "0>"),
        ("(piece 12)", "3<"),
    ];
    let len = (2.0 + 1.0) * 1.2;
    let quarter = set_spring(len, 1.2, Some(len - 1.2));
    let mut to_musical = quarter;
    to_musical.set_min_distance(0.0, &mut Vec::new());
    let bar = set_spring(0.5, 0.0, None);
    // After the bar, from it and from its piece beginning a line; before
    // the bar, to it and to its piece ending one.
    assert_eq!(
        kept_springs(&mut vm, &names),
        [
            kept("0", "1", bar),
            kept("1", "2", to_musical),
            kept("2", "3", quarter),
            kept("2", "3<", quarter),
            kept("0>", "1", bar),
        ]
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

/// `(ly:item-break-dir g)`, `(ly:grob-original g)` and whether `g` is
/// live, for each of `texts`.
fn pieces_of(vm: &mut Vm, texts: &[&str]) -> Vec<String> {
    texts
        .iter()
        .map(|t| {
            let v = eval(
                vm,
                &format!("(list (ly:item-break-dir {t}) (eq? (ly:grob-original {t}) {t}) (%grob-live? {t}))"),
            );
            shown(vm, v)
        })
        .collect()
}

#[test]
fn the_non_musical_items_break_into_pieces_that_take_their_original_s_objects() {
    // A non-musical column 0 with thing 0 and thing 1 (begin-of-line-
    // visible) on it, a musical column 1 with thing 2, and a spanner,
    // typeset in that order; thing 0's objects name both columns' things.
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    let sys = grob_of(&mut vm, "(group 5)");
    system::make(vm.heap_mut(), sys);
    eval(
        &mut vm,
        "(define sys (group 5))
         (put! (column 0) 'non-musical #t)
         (put! (column 1) 'non-musical #f)
         (for-each (lambda (t c) (ly:grob-set-parent! (thing t) 0 (column c))) '(0 1 2) '(0 0 1))
         (put! (thing 0) 'foo 1)
         (put! (thing 1) 'break-visibility #(#f #f #t))
         (ly:grob-set-object! (column 0) 'elements (arr (thing 0) (thing 1)))
         (ly:grob-set-object! (thing 0) 'side-support-elements (arr (thing 1) (thing 2)))
         (ly:grob-set-object! (thing 0) 'friend (thing 1))
         (ly:grob-set-object! (thing 0) 'stranger (thing 2))
         (ly:grob-set-object! (thing 0) 'mixed (list (thing 1) 'x (vector (thing 0) (thing 2))))",
    );
    let order = [
        "(column 0)",
        "(thing 0)",
        "(thing 1)",
        "(column 1)",
        "(thing 2)",
        "(group 0)",
    ];
    for t in order {
        let g = grob_of(&mut vm, t);
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    let column = grob_of(&mut vm, "(column 0)");
    paper_column::set_rank(vm.heap(), column, 3);
    system::make_prebroken_pieces(&mut vm, sys).unwrap();
    eval(
        &mut vm,
        "(define (all) (ly:grob-array->list (ly:grob-object sys 'all-elements)))
         (define (piece-of g d)
           (list-ref (all) (+ 6 (cond ((eq? g (column 0)) 0) ((eq? g (thing 0)) 2) (else 4)) (if (= d -1) 0 1))))",
    );
    // Each non-musical item's two pieces after the originals, left then
    // right, in the originals' order; the musical ones and the spanner
    // have none.
    let all = eval(&mut vm, "(length (all))");
    assert_eq!(shown(&vm, all), "12");
    let names = [
        "(piece-of (column 0) -1)",
        "(piece-of (column 0) 1)",
        "(piece-of (thing 0) -1)",
        "(piece-of (thing 0) 1)",
        "(piece-of (thing 1) -1)",
        "(piece-of (thing 1) 1)",
    ];
    let originals = eval(
        &mut vm,
        "(map (lambda (p o) (eq? (ly:grob-original p) o))
              (list-tail (all) 6)
              (list (column 0) (column 0) (thing 0) (thing 0) (thing 1) (thing 1)))",
    );
    assert_eq!(shown(&vm, originals), "(#t #t #t #t #t #t)");
    // Thing 1 is visible only beginning a line: its original and its
    // piece ending one die.
    assert_eq!(
        pieces_of(&mut vm, &names),
        [
            "(-1 #f #t)",
            "(1 #f #t)",
            "(-1 #f #t)",
            "(1 #f #t)",
            "(-1 #f #f)",
            "(1 #f #t)"
        ]
    );
    assert_eq!(
        pieces_of(&mut vm, &["(thing 1)", "(column 1)", "(thing 2)"]),
        ["(0 #t #f)", "(0 #t #t)", "(0 #t #t)"]
    );
    for t in ["(column 1)", "(thing 2)", "(group 0)"] {
        let g = grob_of(&mut vm, t);
        assert!(!item::is_broken(vm.heap(), g), "{t}");
    }
    // A column's pieces keep its rank and have no system.
    let piece = grob_of(&mut vm, "(piece-of (column 0) 1)");
    assert_eq!(paper_column::rank(vm.heap(), piece), 3);
    assert_eq!(paper_column::system(vm.heap(), piece), None);
    // The pieces' objects are their original's, each item its piece on
    // the same side, an item with none (thing 2, musical) left out of an
    // array, unspecified in a list's vector, its entry dropped alone.
    let objects = eval(
        &mut vm,
        "(let ((p (piece-of (thing 0) 1)) (r (lambda (g) (piece-of g 1))))
           (list (equal? (ly:grob-array->list (ly:grob-object p 'side-support-elements)) (list (r (thing 1))))
                 (eq? (ly:grob-object p 'friend) (r (thing 1)))
                 (ly:grob-object p 'stranger 'none)
                 (let ((m (ly:grob-object p 'mixed)))
                   (list (eq? (car m) (r (thing 1))) (cadr m)
                         (eq? (vector-ref (caddr m) 0) p)
                         (unspecified? (vector-ref (caddr m) 1))))
                 (equal? (ly:grob-array->list (ly:grob-object (piece-of (column 0) -1) 'elements))
                         (list (piece-of (thing 0) -1) (piece-of (thing 1) -1)))
                 (eq? (ly:grob-object (thing 0) 'friend) (thing 1))))",
    );
    assert_eq!(shown(&vm, objects), "(#t #t none (#t x #t #t) #t #t)");
    // The properties copied, not shared; the parents fixed up to the
    // parent's piece on the same side.
    let props = eval(
        &mut vm,
        "(let ((p (piece-of (thing 0) -1)))
           (%grob-set-property-data! p 'foo 2)
           (list (ly:grob-property (thing 0) 'foo) (ly:grob-property p 'foo)
                 (eq? (ly:grob-parent p 0) (piece-of (column 0) -1))
                 (eq? (ly:grob-parent (thing 0) 0) (column 0))
                 (eq? (ly:grob-parent (piece-of (column 0) 1) 0) (ly:grob-parent (column 0) 0))))",
    );
    assert_eq!(shown(&vm, props), "(1 2 #t #t #t)");
    assert_eq!(diagnosed(&mut vm), "()");
    // Breaking again: an item broken already and a piece refuse, with
    // LilyPond's programming errors.
    for t in ["(column 0)", "(piece-of (thing 0) 1)"] {
        let g = grob_of(&mut vm, t);
        item::break_breakable_item(&mut vm, g, sys).unwrap();
    }
    assert_eq!(
        diagnosed(&mut vm),
        "(\"item is already broken\" \"item is a clone; refusing to break\")"
    );
}

#[test]
fn an_item_is_visible_at_a_break_only_where_its_break_visibility_says_t() {
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    let [t0, t1] = ["(thing 0)", "(thing 1)"].map(|t| grob_of(&mut vm, t));
    let visible = |vm: &mut Vm, v: &str| {
        eval(vm, &format!("(put! (thing 0) 'break-visibility {v})"));
        item::break_visible(vm, t0).unwrap()
    };
    // Unbroken, the middle entry: `#t` alone; no vector, visible; a
    // vector too short, not.
    assert!(visible(&mut vm, "#(#f #t #f)"));
    assert!(!visible(&mut vm, "#(#t 1 #t)"));
    assert!(visible(&mut vm, "'()"));
    assert!(!visible(&mut vm, "#(#t)"));
    // A piece beginning a line, the third.
    grob::with_grob_mut(vm.heap(), t0, |g| g.broken = [None, Some(t1)]);
    grob::with_grob_mut(vm.heap(), t1, |g| g.original = Some(t0));
    eval(&mut vm, "(put! (thing 1) 'break-visibility #(#f #f #t))");
    assert!(item::break_visible(&mut vm, t1).unwrap());
    eval(&mut vm, "(put! (thing 1) 'break-visibility #(#t #t #f))");
    assert!(!item::break_visible(&mut vm, t1).unwrap());
}

/// `columns.tsv`'s entry of a spring to `to`.
fn spring_entry(to: &str, s: Spring) -> String {
    format!(
        "{to} {:.4} {:.4} {:.4} {:.4}",
        s.ideal_distance, s.min_distance, s.inverse_stretch_strength, s.inverse_compress_strength
    )
}

#[test]
fn columns_tsv_has_each_column_and_piece_with_its_wishes_springs_and_rods() {
    // a_breakable_column_s_pieces_get_springs_too's line, its columns
    // and the spacer heard (pieces are not), a staff spacing wished for
    // on column 0 and a rod from column 1 to column 2 by hand.
    let Some(mut vm) = line_vm(4, QUARTERS) else {
        return;
    };
    eval(&mut vm, THROWS);
    for (c, l, r) in [(0, 10, 11), (3, 12, 13)] {
        let [col, lp, rp] = [c, l, r].map(|n| grob_of(&mut vm, &format!("(piece {n})")));
        for p in [lp, rp] {
            paper_column::make(vm.heap_mut(), p);
            paper_column::copied(vm.heap(), p, col);
            grob::with_grob_mut(vm.heap(), p, |g| g.original = Some(col));
        }
        grob::with_grob_mut(vm.heap(), col, |g| g.broken = [Some(lp), Some(rp)]);
    }
    eval(
        &mut vm,
        "(for-each (lambda (n) (put! (piece n) 'line-break-permission 'allow))
                   '(10 11 12 13))
         (put! (piece 12) 'when (ly:make-moment 1/2))
         (put! (piece 13) 'when (ly:make-moment 1/2))
         (put! spacer 'springs-and-rods ly:spacing-spanner::set-springs)",
    );
    let heard = [
        (
            "(col 0)",
            "NonMusicalPaperColumn\t-\t-\t0 0\tScore#0\tPaper_column_engraver",
        ),
        (
            "(col 1)",
            "PaperColumn\t-\t-\t0 0\tScore#0\tPaper_column_engraver",
        ),
        (
            "(col 2)",
            "PaperColumn\t-\t-\t1/4 0\tScore#0\tPaper_column_engraver",
        ),
        (
            "(col 3)",
            "NonMusicalPaperColumn\t-\t-\t1/2 0\tScore#0\tPaper_column_engraver",
        ),
        (
            "(thing 0)",
            "StaffSpacing\t-\t-\t0 0\tStaff#0\tSeparating_line_group_engraver",
        ),
        (
            "spacer",
            "SpacingSpanner\t-\t-\t0 0;1/2 0\tScore#0\tSpacing_engraver",
        ),
    ];
    let elements: Vec<(Value, &str)> = heard
        .iter()
        .map(|&(t, l)| (grob_of(&mut vm, t), l))
        .collect();
    let [sys, all] = ["sys", "(arr)"].map(|t| eval(&mut vm, t));
    let columns =
        |vm: &mut Vm| system_columns(vm, sys, all, &elements).unwrap_or_else(|e| panic!("{e}"));
    let len = (2.0 + 1.0) * 1.2;
    let quarter = set_spring(len, 1.2, Some(len - 1.2));
    let mut to_musical = quarter;
    to_musical.set_min_distance(0.0, &mut Vec::new());
    let bar = set_spring(0.5, 0.0, None);
    let [n0, n0l, n0r] = ["0", "-1", "1"].map(|d| format!("NonMusicalPaperColumn@0 0@{d}"));
    let [m1, m2] = ["0 0", "1/4 0"].map(|w| format!("PaperColumn@{w}@0"));
    let [n3, n3l, n3r] = ["0", "-1", "1"].map(|d| format!("NonMusicalPaperColumn@1/2 0@{d}"));
    // The springs-and-rods read first, then the wish and the rod, which
    // the spacing would have read otherwise.
    let first = columns(&mut vm);
    eval(
        &mut vm,
        "(ly:grob-set-object! (col 0) 'spacing-wishes (arr (thing 0) (thing 1)))
         (ly:grob-set-object! (col 1) 'minimum-distances (list (cons (col 2) 0.25) (cons (piece 12) 1/3)))",
    );
    let lines = columns(&mut vm);
    // Each column's springs newest first, as ideal-distances holds them:
    // column 2's to the piece of column 3 ending a line after its own.
    let want = [
        format!(
            "{n0}\tStaffSpacing@0 0@Staff#0|-@-@-\t{}\t-",
            spring_entry(&m1, bar)
        ),
        format!("{n0l}\t-\t-\t-"),
        format!("{n0r}\t-\t{}\t-", spring_entry(&m1, bar)),
        format!(
            "{m1}\t-\t{}\t{m2} 0.2500|{n3l} 0.3333",
            spring_entry(&m2, to_musical)
        ),
        format!(
            "{m2}\t-\t{}|{}\t-",
            spring_entry(&n3l, quarter),
            spring_entry(&n3, quarter)
        ),
        format!("{n3}\t-\t-\t-"),
        format!("{n3l}\t-\t-\t-"),
        format!("{n3r}\t-\t-\t-"),
    ];
    assert_eq!(lines.lines().collect::<Vec<_>>(), want);
    // Before the wish and the rod: the same but for them.
    assert_eq!(
        first.lines().next().unwrap(),
        format!("{n0}\t-\t{}\t-", spring_entry(&m1, bar))
    );
    assert_eq!(
        first.lines().nth(3).unwrap(),
        format!("{m1}\t-\t{}\t-", spring_entry(&m2, to_musical))
    );
    // A spacer whose springs-and-rods throws: its springs and rods
    // unreached, its wishes written, the property as it was.
    eval(&mut vm, "(put! spacer 'springs-and-rods %throws)");
    let unreached = columns(&mut vm);
    assert_eq!(
        unreached.lines().next().unwrap(),
        format!("{n0}\tStaffSpacing@0 0@Staff#0|-@-@-\t?\t?")
    );
    assert!(
        unreached.lines().all(|l| l.ends_with("\t?\t?")),
        "{unreached}"
    );
    let back = eval(
        &mut vm,
        "(eq? (ly:grob-property-data spacer 'springs-and-rods) %throws)",
    );
    assert_eq!(shown(&vm, back), "#t");
    // Without a SpacingSpanner heard, no line.
    assert_eq!(
        system_columns(&mut vm, sys, all, &elements[..5]).unwrap(),
        ""
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_column_fills_its_measure_if_the_next_bar_is_over_half_a_measure_later() {
    // A bar (0) of a 1/2 measure, a rest (1) and a broken bar (2) at
    // `when`.
    let filled = |when: &str, setup: &str| -> Option<bool> {
        let mut vm = line_vm(
            3,
            &format!(
                "(put! (col 0) 'line-break-permission 'allow)
                 (put! (col 0) 'measure-length (ly:make-moment 1/2))
                 (put! (col 1) 'shortest-starter-duration 1/2)
                 (put! (col 2) 'line-break-permission 'allow)
                 (put! (col 2) 'when (ly:make-moment {when}))
                 {setup}"
            ),
        )?;
        let [me, c0, c1, c2, p] =
            ["spacer", "(col 0)", "(col 1)", "(col 2)", "(piece 10)"].map(|t| grob_of(&mut vm, t));
        if !setup.contains("unbroken") {
            grob::with_grob_mut(vm.heap(), c2, |g| g.broken = [Some(p), None]);
        }
        let got =
            fills_measure(&mut vm, me, c0, c1).unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
        Some(got)
    };
    let Some(got) = filled("1/2", "") else {
        return;
    };
    assert!(got);
    // Pickups shorten measures: over half is enough.
    assert!(filled("5/16", "").unwrap());
    assert!(!filled("1/4", "").unwrap());
    // Not if the next bar is not broken, is musical or unused, the left
    // column musical, the column not, or without a measure length.
    for setup in [
        "'unbroken",
        "(put! (col 2) 'shortest-starter-duration 1/4)",
        "(put! (col 2) 'used '()) (put! (col 2) 'line-break-permission '())
         (ly:grob-set-object! (col 2) 'bounded-by-me '())",
        "(put! (col 0) 'shortest-starter-duration 1/4)",
        "(put! (col 1) 'shortest-starter-duration '())",
        "(put! (col 0) 'measure-length 1/2)",
    ] {
        assert!(!filled("1/2", setup).unwrap(), "{setup}");
    }
}

/// Binds `name` to a title, a `Prob` of the properties `alist`.
fn title(vm: &mut Vm, name: &str, alist: &str) {
    let (type_, init) = (eval(vm, "'paper-system"), eval(vm, alist));
    let prob = lily_engine_music::make_prob(vm.heap_mut(), type_, init);
    vm.heap_mut().root(prob);
    eval(vm, &format!("(define {name} #f)"));
    let set = eval(vm, &format!("(lambda (p) (set! {name} p))"));
    Host::call(vm, set, &[prob]).unwrap();
}

#[test]
fn titles_between_systems_are_spaced_by_their_skylines_or_stencils() {
    // From page-layout-problem.cc:733–770, 826–830 and find_system_offsets'
    // titles on `aligned_system`'s system, its first staff's top 2.5 above
    // it, the staves 9 apart (ideal), the last's bottom 3 under it. `t`'s
    // stencil is 3 over and 10 under it; `t2`'s 2 and 1; `ts` has
    // skylines 1 over and 4 under, `tl` from 8 to 10 under; `tt` is
    // tightly spaced; `tb` asks for a bottom padding of 90.
    let stencil = |y: &str| format!("(cons 'stencil (ly:make-stencil '() '(0 . 1) '{y}))");
    let skylines = |y: &str| format!("(cons 'vertical-skylines (%box-skylines '(0 . 20) '{y} 0))");
    let titles = [
        ("t", format!("(list {})", stencil("(-10 . 3)"))),
        ("t2", format!("(list {})", stencil("(-1 . 2)"))),
        (
            "ts",
            format!("(list {} {})", skylines("(-4 . 1)"), stencil("(-10 . 3)")),
        ),
        (
            "tl",
            format!(
                "(list {} {})",
                skylines("(-10 . -8)"),
                stencil("(-10 . -8)")
            ),
        ),
        (
            "tt",
            format!("(list '(tight-spacing . #t) {})", stencil("(-10 . 3)")),
        ),
        (
            "tb",
            format!("(list '(bottom-padding . 90) {})", stencil("(-10 . 3)")),
        ),
    ];
    let loose_top = "(put! (group 0) 'staff-affinity -1) (put! (group 1) 'staff-affinity '())";
    let loose_rising = &format!(
        "{loose_top} (put! (group 0) 'nonstaff-relatedstaff-spacing '((basic-distance . 10)))"
    );
    let loose_bottom = "(put! (group 2) 'staff-affinity 1)";
    let page = |height: f64| format!("(list '(paper-height . {height}))");
    let empty = paper_with("'()");
    let over = |by: &str| format!("(\"compressing over-full page by {by} staff-spaces\")");
    for (paper, height, systems, setup, ragged, got, warned) in [
        // A title 3 under the top, its stencil's top; the first staff 2.5
        // + 10 under it. Ragged, the force is the one that fills the page.
        (&empty, 100.0, "(list t (group 5))", "", true, "((3.0 13.000000000000004) . 8.055555555555555)", "()"),
        (&empty, 100.0, "(list t t2 (group 5))", "", true, "((3.0 15.0 16.000000000000004) . 7.722222222222222)", "()"),
        // Under the system: the last staff's bottom 3 over the title's top
        // 3, or with skylines its top 1; one at the top, 1 under the top.
        (&empty, 100.0, "(list (group 5) t)", "", true, "((3.552713678800501e-15 17.500000000000004) . 8.055555555555555)", "()"),
        (&empty, 100.0, "(list (group 5) ts)", "", true, "((3.552713678800501e-15 15.500000000000004) . 8.944444444444445)", "()"),
        (&empty, 100.0, "(list ts (group 5))", "", true, "((1.0 5.0) . 8.944444444444445)", "()"),
        // The specs: top-markup over top-system, then markup-markup,
        // markup-system and score-markup.
        (
            &paper_with(
                "'((top-system-spacing (minimum-distance . 50)) (top-markup-spacing (minimum-distance . 4))
                   (markup-markup-spacing (minimum-distance . 20)) (markup-system-spacing (minimum-distance . 40))
                   (score-markup-spacing (minimum-distance . 30)))",
            ),
            200.0,
            "(list t t2 (group 5) t)",
            "",
            true,
            "((4.0 24.0 61.5 103.0) . 9.666666666666666)",
            "()",
        ),
        // Its padding (1) under the system, but not when tightly spaced.
        (&paper_with("'((score-markup-spacing (padding . 1)))"), 100.0, "(list (group 5) t)", "", true, "((3.552713678800501e-15 18.500000000000004) . 7.944444444444445)", "()"),
        (&paper_with("'((score-markup-spacing (padding . 1)))"), 100.0, "(list (group 5) tt)", "", true, "((3.552713678800501e-15 17.500000000000004) . 8.055555555555555)", "()"),
        // The last title's bottom padding off the height; compressed, the
        // force is -inf.
        (&empty, 100.0, "(list (group 5) tb)", "", false, "((3.552713678800501e-15 5.500000000000002) . -inf.0)", &over("16.5")),
        // Nothing else is spaced; its footnotes are read first, with the
        // paper and the page.
        (&empty, 100.0, "(list (group 5) 1)", "", false, "((3.552713678800501e-15) . 9.5)", "(\"Systems on a page must be a prob or grob.\" \"got a system that was neither a Grob nor a Prob\")"),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        eval(&mut vm, setup);
        for (name, alist) in &titles {
            title(&mut vm, name, alist);
        }
        let solved = page_layout(&mut vm, "%page-layout-problem", paper, &page(height), systems, ragged);
        assert_eq!(solved, format!("{got} {warned}"), "{paper} {systems} {ragged}");
    }

    // The loose lines: one at the top of a system under a title, at least
    // its top (2.5) plus the title's bottom (10) and the system's padding
    // under the title; at the top of the page, the head's padding and its
    // top. One at the bottom of a system over a title, its bottom (3),
    // the title's top (3) and the title's padding over it; at the foot,
    // its bottom and the foot's padding. Skylines under the system's
    // loose lines keep the title at least as low as they are.
    for (paper, height, systems, setup, ragged, got) in [
        // Group 0 at 16 - 0.5 = 3 + 12.5, then 3 + 13.5.
        (
            &empty,
            100.0,
            "(list t (group 5))",
            loose_top,
            true,
            "(3.0 16.000000000000004) () ((0.5000000000000036 . -16.000000000000004) \
             (-2.5 . -16.000000000000004) (-11.5 . -16.000000000000004))",
        ),
        (
            &paper_with("'((markup-system-spacing (padding . 1)))"),
            100.0,
            "(list t (group 5))",
            loose_top,
            true,
            "(3.0 17.000000000000004) () ((0.5000000000000036 . -17.000000000000004) \
             (-2.5 . -17.000000000000004) (-11.5 . -17.000000000000004))",
        ),
        // Group 0, 10 over its staff if it could be, at 6.5 - 2 = 2 + 2.5.
        (
            &paper_with("'((top-system-spacing (minimum-distance . 9) (padding . 2)))"),
            100.0,
            "(list (group 5))",
            loose_rising,
            true,
            "(6.5) () ((2.0 . -6.5) (-2.5 . -6.5) (-11.5 . -6.5))",
        ),
        // Group 2 at 8.8 = 15.8 - (3 + 3 + 1).
        (
            &paper_with("'((score-markup-spacing (padding . 1)))"),
            100.0,
            "(list (group 5) t)",
            loose_bottom,
            true,
            "(3.552713678800501e-15 15.800000000000004) () ((-2.5 . -3.552713678800501e-15) \
             (-6.0 . -3.552713678800501e-15) (-8.8 . -3.552713678800501e-15))",
        ),
        // Group 2 at 13.9 - (3 + 2), not where its springs would put it.
        (
            &paper_with("'((last-bottom-spacing (padding . 2)))"),
            13.9,
            "(list (group 5))",
            loose_bottom,
            false,
            "(3.552713678800501e-15) () ((-2.5 . -3.552713678800501e-15) \
             (-6.099999999999996 . -3.552713678800501e-15) (-8.899999999999997 . -3.552713678800501e-15))",
        ),
        // The skylines' distance (1.3) less than the loose lines' (6.3).
        (
            &empty,
            100.0,
            "(list (group 5) tl)",
            loose_bottom,
            true,
            "(3.552713678800501e-15 8.800000000000004) () ((-2.5 . -3.552713678800501e-15) \
             (-6.0 . -3.552713678800501e-15) (-8.8 . -3.552713678800501e-15))",
        ),
    ] {
        let Some(mut vm) = grobs_vm() else { return };
        aligned_system(&mut vm, 2.5, "()");
        eval(&mut vm, setup);
        for (name, alist) in &titles {
            title(&mut vm, name, alist);
        }
        let solved = page_solution(&mut vm, paper, &page(height), systems, ragged);
        let solved = format!("{solved} {}", aligned(&mut vm));
        assert_eq!(solved, got, "{paper} {systems}");
    }
}

#[test]
fn a_page_s_lines_give_their_footnotes_and_footnote_stencils() {
    // From page-layout-problem.cc:42–117 on the system of
    // `the_footnotes_after_line_breaking_are_those_inside_the_system_by_column`,
    // whose footnotes after line breaking are (1 2 0 s); `tn` a title with
    // two footnotes, `t0` one with none, `tf` one whose footnote stencil
    // is #f.
    let Some(mut vm) = line_vm(5, "") else { return };
    let sys = grob_of(&mut vm, "sys");
    system::make(vm.heap_mut(), sys);
    let [c0, c4] = ["(col 0)", "(col 4)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, sys, -1, c0).unwrap();
    spanner::set_bound(&mut vm, sys, 1, c4).unwrap();
    for t in [
        "(foot 0)",
        "(foot 1)",
        "(foot 2)",
        "(foot 3)",
        "(thing 0)",
        "(span 0)",
    ] {
        let g = grob_of(&mut vm, t);
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    let [span, c1] = ["(span 0)", "(col 1)"].map(|t| grob_of(&mut vm, t));
    spanner::set_bound(&mut vm, span, -1, c1).unwrap();
    spanner::set_bound(&mut vm, span, 1, c4).unwrap();
    title(&mut vm, "tn", "'((footnotes \"a\" \"b\"))");
    title(&mut vm, "t0", "'((footnotes))");
    title(&mut vm, "tf", "'((footnote-stencil . #f))");
    eval(
        &mut vm,
        "(for-each (lambda (g c) (ly:grob-set-parent! g 0 (col c)))
                   (list (foot 0) (foot 1) (foot 2) (foot 3) (thing 0)) '(3 1 2 0 2))
         (put! (span 0) 'X-offset 1)
         (ly:grob-set-object! sys 'footnotes-before-line-breaking
                              (ly:system::footnotes-before-line-breaking sys))
         (ly:grob-set-object! sys 'footnotes-after-line-breaking
                              ly:system::footnotes-after-line-breaking)
         (define (index g)
           (let loop ((i 0))
             (cond ((not g) 'p) ((eq? g (span 0)) 's) ((eq? (foot i) g) i) (else (loop (1+ i))))))
         (define st1 (ly:make-stencil '() '(0 . 1) '(0 . 1)))
         (define st2 (ly:make-stencil '() '(0 . 1) '(0 . 2)))
         (define (named s) (cond ((eq? s st1) 'a) ((eq? s st2) 'b) (else s)))",
    );
    let mut got = |expr: &str| {
        let v = evaluated(&mut vm, expr);
        format!("{v} {}", diagnosed(&mut vm))
    };
    // A system's footnotes after line breaking, a null for each of a
    // title's; a grob that is no system is an error, anything else skipped.
    assert_eq!(
        got("(map index (%footnote-grobs (list sys tn t0 (thing 0) 1 sys)))"),
        "(1 2 0 s p p 1 2 0 s) (\"got a grob for footnotes that wasn't a System\")"
    );
    assert_eq!(got("(%footnote-count (list sys tn))"), "6 ()");
    assert_eq!(got("(%footnote-count (list t0))"), "0 ()");
    assert_eq!(got("(%footnote-count '())"), "0 ()");
    // No lines: none. The first neither a grob nor a Prob, or without a
    // footnote stencil yet: none, and an error.
    assert_eq!(got("(%footnotes-from-lines '())"), "() ()");
    assert_eq!(
        got("(%footnotes-from-lines (list 1 sys))"),
        "() (\"Systems on a page must be a prob or grob.\")"
    );
    for lines in ["(list sys tn)", "(list tn sys)"] {
        assert_eq!(
            got(&format!("(%footnotes-from-lines {lines})")),
            "() (\"Footnotes must be added to lines before they are retrieved.\")",
            "{lines}"
        );
    }
    // Each line's stencil, '() for one without; one neither a grob nor a
    // Prob left out, with the error. A stencil of #f is one.
    eval(
        &mut vm,
        "(put! sys 'footnote-stencil st1)
         (ly:prob-set-property! tn 'footnote-stencil st2)",
    );
    let mut got = |expr: &str| {
        let v = evaluated(&mut vm, expr);
        format!("{v} {}", diagnosed(&mut vm))
    };
    assert_eq!(
        got("(map named (%footnotes-from-lines (list sys tn 1 t0 (thing 0))))"),
        "(a b () ()) (\"Systems on a page must be a prob or grob.\")"
    );
    assert_eq!(
        got("(map named (%footnotes-from-lines (list tn sys)))"),
        "(b a) ()"
    );
    assert_eq!(
        got("(map named (%footnotes-from-lines (list tf sys)))"),
        "(#f a) ()"
    );
}

#[test]
fn footnotes_are_numbered_and_stacked_onto_their_lines() {
    // From page-layout-problem.cc:127–352. Number n's markup is a stencil
    // n wide (0 to −1 for 3) and 1 high; a footnote's text 10 wide and its
    // height high, its expression none for `#f`. The system's footnotes:
    // (foot 0) numbered, (foot 1) and (foot 3) in-notes, (span 0) broken
    // into (thing 2), whose text it takes, and (thing 3), which numbers
    // it, and (foot 2) empty. `tn` a title with footnotes k1 (numbered),
    // k2, k3 (empty) and k4 (numbered); `t0` one with none.
    let Some(mut vm) = line_vm(5, "") else { return };
    let sys = grob_of(&mut vm, "sys");
    system::make(vm.heap_mut(), sys);
    let [span, t2, t3] = ["(span 0)", "(thing 2)", "(thing 3)"].map(|t| grob_of(&mut vm, t));
    grob::with_grob_mut(vm.heap(), span, |s| s.broken_intos = vec![t2, t3]);
    eval(
        &mut vm,
        "(for-each ly:register-stencil-expression '(num txt p))
         (define-public (markup-command-signature p) (procedure-property p 'markup-signature))
         (define-public (markup-list-function? p) #f)
         (define seen #f)
         (define-public (num-markup layout props n)
           (ly:make-stencil (list 'num n) (if (= n 3) '(0.0 . -1.0) (cons 0.0 (* 1.0 n))) '(0.0 . 1.0)))
         (define-public (txt-markup layout props key h)
           (set! seen props)
           (ly:make-stencil (if key (list 'txt key) '()) '(0.0 . 10.0) (cons 0.0 h)))
         (set-procedure-property! num-markup 'markup-signature (list number?))
         (set-procedure-property! txt-markup 'markup-signature (list symbol? number?))
         (define asserted '())
         (define (assertion n) (set! asserted (cons n asserted)))
         ;; A callback, as any procedure a grob property holds: its value
         ;; is the function.
         (define (asserting g) assertion)
         (define (note! g key h numbered footnote assert)
           (put! g 'footnote-text (list txt-markup key h))
           (put! g 'automatically-numbered numbered)
           (put! g 'footnote footnote)
           (put! g 'numbering-assertion-function assert))
         (note! (foot 0) 'a 2.0 #t #t asserting)
         (note! (foot 1) 'b 1.0 #f #f 'x)
         (note! (span 0) 'own 9.0 '() #t '())
         (put! (thing 2) 'footnote-text (list txt-markup 'piece 3.0))
         (put! (thing 3) 'automatically-numbered #t)
         (note! (foot 2) #f 1.0 #f #t '())
         (note! (foot 3) 'c 2.0 #f '() asserting)
         (ly:grob-set-object! sys 'footnotes-after-line-breaking
                              (arr (foot 0) (foot 1) (span 0) (foot 2) (foot 3)))
         (define p1 (ly:make-stencil '(p 1) '(0.0 . 10.0) '(0.0 . 2.0)))
         (define p2 (ly:make-stencil '(p 2) '(0.0 . 10.0) '(0.0 . 1.0)))
         (define (shape s) (list (ly:stencil-extent s 0) (ly:stencil-extent s 1)))
         (define (table-of paper)
           (map (lambda (e) (if (ly:stencil? (cdr e)) (cons (car e) (shape (cdr e))) e))
                (ly:output-def-lookup paper 'number-footnote-table)))
         (define (paper-of . vars)
           (let ((paper (ly:make-output-def)))
             (for-each (lambda (v) (ly:output-def-set-variable! paper (car v) (cdr v))) vars)
             paper))
         (define paper
           (paper-of '(line-width . 40) '(property-defaults (a . 1))
                     (cons 'footnote-numbering-function (lambda (n) (list num-markup n)))
                     '(footnote-padding . 0.5) '(in-note-padding . 0.25)
                     '(footnote-number-raise . 0.5)))
         (define top (paper-of '(number-footnote-table (old . x))))",
    );
    title(
        &mut vm,
        "tn",
        "(list (cons 'footnotes (list (list 'k1 #t p1) (list 'k2 #f p2)
                                      (list 'k3 #f (ly:make-stencil '())) (list 'k4 #t p1))))",
    );
    title(&mut vm, "t0", "'((footnotes))");
    let mut got = |expr: &str| {
        let v = evaluated(&mut vm, expr);
        format!("{v} {}", diagnosed(&mut vm))
    };
    // No paper: an error, and nothing done.
    assert_eq!(
        got("(%add-footnotes-to-lines! (list sys) 0 #f top)
             (list (ly:grob-property-data sys 'footnote-stencil) (table-of top))"),
        "(() ((old . x))) (\"Cannot get footnotes because there is no valid paper block.\")"
    );
    // Numbers 2 to 10 for the nine footnotes, each grob's assertion
    // called with its own; the widest ends at 10. A grob that is no system
    // an error (in %footnote-grobs and here), anything else skipped.
    let wrong = "\"got a grob for footnotes that wasn't a System\"";
    assert_eq!(
        got(
            "(%add-footnotes-to-lines! (list sys tn (thing 0) 7 t0) 2 paper top)
             (reverse asserted)"
        ),
        format!("(2 6) ({wrong} {wrong})")
    );
    // The page's properties, as titling.scm makes them.
    assert_eq!(
        got("seen"),
        "(((line-width . 40.0) (a . 1) (font-encoding . latin1))) ()"
    );
    // (foot 0) takes number 2's stencil, moved 8 right to end at 10, raised
    // 2 + 0.5 − 1 = 1.5 to its top and put at its left (−10): X −2 to 10,
    // Y 0 to 2.5. (span 0) takes number 3's, 0 to −1 on X: an empty
    // interval, so not moved right, but not the empty box's, so put at
    // the left by its right end (0 − (−1) = 1); raised 3 + 0.5 − 1 = 2.5
    // over (thing 2)'s 3: Y 0 to 3.5, stacked 0.5 under (foot 0)'s: −4 to
    // −0.5. (foot 2) is empty, not stacked; (foot 1) and (foot 3) the
    // in-notes, 0.25 apart.
    assert_eq!(
        got("(list (shape (ly:grob-property-data sys 'footnote-stencil))
                   (shape (ly:grob-property-data sys 'in-note-stencil)))"),
        "(((-2.0 . 10.0) (-4.0 . 2.5)) ((0.0 . 10.0) (-2.25 . 1.0))) ()"
    );
    assert_eq!(
        got("(ly:stencil-expr (ly:grob-property-data sys 'footnote-stencil))"),
        "(combine-stencil (translate-stencil (0.0 . -4.0) (combine-stencil \
         (translate-stencil (1.0 . 0.0) (translate-stencil (0.0 . 2.5) (num 3))) (txt piece))) \
         (translate-stencil (-10.0 . 0.0) (translate-stencil (0.0 . 1.5) (translate-stencil (8.0 . 0.0) (num 2)))) \
         (txt a)) ()"
    );
    assert_eq!(
        got("(map (lambda (g) (let ((t (ly:grob-property-data g 'text))) (if (pair? t) (cadr t) t)))
                  (list (foot 0) (foot 1) (span 0) (thing 2) (thing 3) (foot 2) (foot 3)))"),
        "(2 () 3 3 3 () ()) ()"
    );
    // tn: k1 takes number 4's, moved 6 right and raised 1.5 to p1's top,
    // at its left (−10): X −4 to 10, Y 0 to 2.5; p2 0.5 under it (−1.5 to
    // −0.5), k3 empty; k4 number 5's, X −5 to 10, 0.5 under p2: −4.5 to
    // −2. The table gives each key its number's stencil, unmoved, the
    // empty stencil if none, before what it had. t0's footnote stencil
    // is empty.
    let empty = "((+inf.0 . -inf.0) (+inf.0 . -inf.0))";
    assert_eq!(
        got("(list (shape (ly:prob-property tn 'footnote-stencil))
                   (shape (ly:prob-property t0 'footnote-stencil))
                   (table-of top))"),
        format!(
            "(((-5.0 . 10.0) (-4.5 . 2.5)) {empty} \
             ((k4 (0.0 . 5.0) (0.0 . 1.0)) (k3 {}) (k2 {}) (k1 (0.0 . 4.0) (0.0 . 1.0)) (old . x))) ()",
            &empty[1..empty.len() - 1],
            &empty[1..empty.len() - 1]
        )
    );
    // A paper without padding, raise or a table: 0 and none. From 0 on,
    // the widest number ends at 3: k1's number 0, moved 3 right, raised
    // 2 − 1 = 1 and put at the left (−3); p2 right under it (−1 to 0);
    // k4's number 1, X −1 to 10, right under p2: −3 to −1.
    assert_eq!(
        got("(define paper2 (paper-of '(line-width . 40)
                                      (cons 'footnote-numbering-function (lambda (n) (list num-markup n)))))
             (define top2 (paper-of))
             (%add-footnotes-to-lines! (list tn) 0 paper2 top2)
             (list (shape (ly:prob-property tn 'footnote-stencil)) (table-of top2))"),
        format!(
            "(((-1.0 . 10.0) (-3.0 . 2.0)) ((k4 (0.0 . 1.0) (0.0 . 1.0)) (k3 {}) (k2 {}) (k1 (0.0 . 0.0) (0.0 . 1.0)))) ()",
            &empty[1..empty.len() - 1],
            &empty[1..empty.len() - 1]
        )
    );
}

#[test]
fn footnotes_are_stacked_into_the_page_s_foot_under_the_separator() {
    // From page-layout-problem.cc:354–404 and the constructor's foot
    // (:419–440), each footnote put over the foot at
    // add_at_edge's offset, the foot's top less its bottom plus the
    // padding. The foot is 2 high, the footnotes f1, f2 and f3 1, 3 and 2
    // (f3 from −1), `e` empty (no expression) and `#f` no stencil; the
    // separator `h` high, for 0 empty (an expression, but no extent).
    let Some(mut vm) = grobs_vm() else { return };
    aligned_system(&mut vm, 2.5, "()");
    eval(
        &mut vm,
        "(for-each ly:register-stencil-expression '(foot f1 f2 f3 fn sep))
         (define-public (markup-command-signature p) (procedure-property p 'markup-signature))
         (define-public (markup-list-function? p) #f)
         (define seen #f)
         (define-public (sep-markup layout props h)
           (set! seen props)
           (if (= h 0)
               (ly:make-stencil '(sep))
               (ly:make-stencil '(sep) '(0.0 . 10.0) (cons 0.0 h))))
         (set-procedure-property! sep-markup 'markup-signature (list number?))
         (define (stil expr y) (ly:make-stencil expr '(0.0 . 10.0) y))
         (define foot (ly:make-stencil '(foot) '(0.0 . 1.0) '(0.0 . 2.0)))
         (define notes (list (stil '(f1) '(0.0 . 1.0)) (stil '(f2) '(0.0 . 3.0)) #f
                             (stil '() '(0.0 . 1.0)) (stil '(f3) '(-1.0 . 1.0))))
         (define (shape s) (list (ly:stencil-extent s 0) (ly:stencil-extent s 1)))
         (define (paper-of . vars)
           (let ((paper (ly:make-output-def)))
             (for-each (lambda (v) (ly:output-def-set-variable! paper (car v) (cdr v))) vars)
             paper))
         (define paper
           (paper-of '(line-width . 40) '(property-defaults (a . 1))
                     '(footnote-padding . 0.5) '(footnote-footer-padding . 1)
                     (cons 'footnote-separator-markup (list sep-markup 0.25))))",
    );
    let mut got = |expr: &str| {
        let v = evaluated(&mut vm, expr);
        format!("{v} {}", diagnosed(&mut vm))
    };
    // The last first: f3 1 over the foot's top (2 + 1 + 1, at 3 to 5),
    // then f2 0.5 over it (5 + 0.5, 5.5 to 8.5), f1 (8.5 + 0.5, 9 to 10);
    // `e` and `#f` left out. The separator 0.5 over f1: 10.5 to 10.75,
    // interpreted with the page's properties.
    assert_eq!(
        got(
            "(define stacked (%add-footnotes-to-footer notes foot paper))
             (list (shape stacked) seen)"
        ),
        "(((0.0 . 10.0) (0.0 . 10.75)) (((line-width . 40.0) (a . 1) (font-encoding . latin1)))) ()"
    );
    assert_eq!(
        got("(ly:stencil-expr stacked)"),
        "(combine-stencil (translate-stencil (0.0 . 10.5) (sep)) \
         (translate-stencil (0.0 . 9.0) (f1)) (translate-stencil (0.0 . 5.5) (f2)) \
         (translate-stencil (0.0 . 4.0) (f3)) (foot)) ()"
    );
    // No footnote stacked: the foot as it was, and no separator made.
    assert_eq!(
        got("(set! seen #f)
             (let ((s (%add-footnotes-to-footer (list #f (stil '() '(0.0 . 1.0))) foot paper)))
               (list (shape s) (ly:stencil-expr s) seen))"),
        "(((0.0 . 1.0) (0.0 . 2.0)) (foot) #f) ()"
    );
    // A separator that is empty, or no markup, is left out; without
    // paddings, f3 at 2 to 4, f2 at 4 to 7, f1 at 7 to 8. (The page's
    // properties need the paper's line width: LilyPond's
    // `ly:paper-get-number` of none is a type error.)
    for separator in ["(list sep-markup 0)", "'none"] {
        assert_eq!(
            got(&format!(
                "(let ((s (%add-footnotes-to-footer notes foot
                                                    (paper-of '(line-width . 40)
                                                              (cons 'footnote-separator-markup {separator})))))
                   (list (shape s) (ly:stencil-expr s)))"
            )),
            "(((0.0 . 10.0) (0.0 . 8.0)) (combine-stencil (translate-stencil (0.0 . 7.0) (f1)) \
             (translate-stencil (0.0 . 4.0) (f2)) (translate-stencil (0.0 . 3.0) (f3)) (foot))) ()",
            "{separator}"
        );
    }

    // On a page: `aligned_system`'s system, whose springs' minimum
    // distances add to 13.5, has a footnote 3 high. Over the page's foot
    // (2 high) by the footer padding (1), the footer is 6 high, and a page
    // 10 high is over-full by 13.5 + 6 − 10 = 9.5; with no foot, by the
    // footnote alone (3), 6.5; without a paper, by the foot's 2 alone,
    // 5.5, with the warning.
    eval(
        &mut vm,
        "(%grob-set-property-data! (group 5) 'footnote-stencil (stil '(fn) '(0.0 . 3.0)))",
    );
    let over = |by: &str| format!("\"compressing over-full page by {by} staff-spaces\"");
    let footed = "(list '(paper-height . 10) (cons 'foot-stencil foot))";
    let footer = paper_with("'((line-width . 40) (footnote-footer-padding . 1))");
    for (paper, page, warned) in [
        (footer.as_str(), footed, format!("({})", over("9.5"))),
        (
            &footer,
            "(list '(paper-height . 10))",
            format!("({})", over("6.5")),
        ),
        (
            "#f",
            footed,
            format!(
                "(\"A page layout problem has been initiated that cannot accommodate footnotes.\" {})",
                over("5.5")
            ),
        ),
    ] {
        let solved = page_solution(&mut vm, paper, page, "(list (group 5))", false);
        assert!(
            solved.ends_with(&format!(" {warned}")),
            "{paper} {page}: {solved}"
        );
    }
}

/// The note head's callbacks (MS3 step 37), each of the grob alone.
const NOTE_HEAD_CALLBACKS: [&str; 5] = [
    "ly:note-head::print",
    "ly:note-head::stem-x-shift",
    "ly:note-head::include-ledger-line-height",
    "ly:note-head::calc-stem-attachment",
    "ly:note-head::calc-tab-stem-attachment",
];

#[test]
fn the_note_head_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in NOTE_HEAD_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
    let name = "ly:note-head::stem-attachment";
    let got = caught(&mut vm, &format!("({name} 'x \"a\")"));
    assert!(
        got.contains(name)
            && got.contains("Wrong type argument in position 1 (expecting Font_metric): x"),
        "{got}"
    );
    for args in ["'x", "'x \"a\" 1 2"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
    }
}

#[test]
fn the_ledger_line_height_reaches_from_the_head_to_its_first_ledger() {
    let Some(mut vm) = grobs_vm() else { return };
    set_layout(&mut vm);
    // Heads 0 to 3 and things 0 to 2 on (staff 0), five lines a staff
    // space apart, all under (thing 5); each a staff space high about its
    // offset, read once.
    eval(
        &mut vm,
        "(ly:grob-set-parent! (staff 0) 1 (thing 5))
         (put! (staff 0) 'line-positions '(-4 -2 0 2 4))
         (for-each (lambda (g)
                     (ly:grob-set-parent! g 1 (thing 5))
                     (ly:grob-set-object! g 'staff-symbol (staff 0))
                     (put! g 'Y-extent '(-0.5 . 0.5)))
                   (list (head 0) (head 1) (head 2) (head 3) (thing 0) (thing 1)))
         (put! (thing 2) 'Y-extent '(-0.5 . 0.5))",
    );
    let height = |vm: &mut Vm, g: &str, y: &str| {
        eval(vm, &format!("(put! {g} 'Y-offset {y})"));
        caught(
            vm,
            &format!("(ly:note-head::include-ledger-line-height {g})"),
        )
    };
    // Within the staff, none; above it, down to the first ledger below
    // it, 1 a staff space from the top line; below it, up to its first.
    assert_eq!(height(&mut vm, "(head 0)", "0.0"), "(0.0 . 0.0)");
    assert_eq!(height(&mut vm, "(head 1)", "4.5"), "(-1.0 . 0.0)");
    assert_eq!(height(&mut vm, "(head 2)", "-4.5"), "(0.0 . 1.0)");
    assert_eq!(height(&mut vm, "(head 3)", "3.5"), "(0.0 . 0.0)");
    // A staff space of 2: the lines and the position twice as far.
    eval(&mut vm, "(put! (staff 0) 'staff-space 2)");
    assert_eq!(height(&mut vm, "(thing 0)", "6.5"), "(-1.0 . 0.0)");
    // No lines: line_span's empty (1 . -1), not scaled.
    eval(&mut vm, "(put! (staff 0) 'line-positions '())");
    assert_eq!(height(&mut vm, "(thing 1)", "3.0"), "(-2.5 . 0.0)");
    // Without a staff symbol, none.
    assert_eq!(height(&mut vm, "(thing 2)", "3.0"), "(0.0 . 0.0)");
}

#[test]
fn the_stem_x_shift_reads_the_stem_s_positioning_and_is_0() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(define positioned 0)
         (ly:grob-set-object! (head 0) 'stem (stem 0))
         (put! (stem 0) 'positioning-done (lambda (g) (set! positioned (1+ positioned)) #t))",
    );
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:note-head::stem-x-shift (head 0)) (ly:note-head::stem-x-shift (head 0))
                   (ly:note-head::stem-x-shift (head 1)) positioned)"
        ),
        "(0 0 0 1)"
    );
}

#[test]
fn the_tab_stem_attachment_is_1_35_towards_the_stem() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(&mut vm, "(ly:grob-set-object! (head 0) 'stem (stem 0))");
    let attachment = |vm: &mut Vm, d: &str| {
        eval(vm, &format!("(put! (stem 0) 'direction {d})"));
        caught(vm, "(ly:note-head::calc-tab-stem-attachment (head 0))")
    };
    assert_eq!(attachment(&mut vm, "-1"), "(0.0 . -1.35)");
    assert_eq!(attachment(&mut vm, "1"), "(0.0 . 1.35)");
    // No direction, or no stem: up.
    assert_eq!(attachment(&mut vm, "'()"), "(0.0 . 1.35)");
    assert_eq!(
        caught(&mut vm, "(ly:note-head::calc-tab-stem-attachment (head 1))"),
        "(0.0 . 1.35)"
    );
}

/// The cases of [`HEAD_PROBE`]: each head's `duration-log`, `style`,
/// `glyph-name`, `direction` and `staff-position`.
const HEAD_CASES: &str = r#"(list
  '(2 default "" 1 0) '(0 default "" 1 0) '(1 default "" 1 0) '(5 default "" -1 0)
  '(-1 default "" 1 0) '(-2 default "" 1 0) '(-2 default "" -1 0) '(-3 default "" -1 0)
  '(2 cross "2cross" 1 0) '(2 triangle "2triangle" 1 0) '(2 triangle "2triangle" -1 0)
  '(2 triangle "2triangle" 0 0) '(2 xyz "nonsense" 1 0) '(2 default "2cross" 1 0)
  '(0 mensural "0mensural" 1 0) '(0 harmonic "0harmonic" 1 0) '(1 slash "1slash" 1 0)
  '(-2 default "" 0 0))"#;

/// The glyphs whose stem attachments [`HEAD_PROBE`] writes, up, down and
/// centred, of the note head's font and of `emmentaler-20` unscaled.
const HEAD_GLYPHS: &str = r#"'("noteheads.s2" "noteheads.s0" "noteheads.s-1" "noteheads.sM1" "noteheads.uM2"
  "noteheads.dM2" "noteheads.u2triangle" "noteheads.d2triangle" "noteheads.s2cross"
  "noteheads.s0harmonic" "noteheads.s1slash" "rests.2" "nonexistent")"#;

/// A score whose first note heads are given [`HEAD_CASES`]' properties and
/// printed by `ly:note-head::print`, each written as a line: its stencil
/// as `tools/compare/stencils.ly`'s datum `(x y expr)`, and its
/// `ly:note-head::calc-stem-attachment` with its stem up, down and of no
/// direction; before the first, a line of [`HEAD_GLYPHS`]' attachments.
const HEAD_PROBE: &str = r#"\version "2.26.0"
#(define cases CASES)
#(define glyphs GLYPHS)
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define n 0)
#(define (probe g)
  (if (= n 0)
      (begin
        (write (map (lambda (f)
                      (map (lambda (gl)
                             (map (lambda (d) (ly:note-head::stem-attachment f gl d)) '(1 -1 0)))
                           glyphs))
                    (list (ly:grob-default-font g) (ly:system-font-load "emmentaler-20"))))
        (newline)))
  (if (< n (length cases))
      (let ((c (list-ref cases n)) (stem (ly:grob-object g 'stem)))
        (set! n (1+ n))
        (for-each (lambda (sym v) (ly:grob-set-property! g sym v))
                  '(duration-log style glyph-name direction staff-position) c)
        (let ((s (ly:note-head::print g)) (orig (ly:grob-property stem 'direction)))
          (write (list (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
                       (map (lambda (d)
                              (ly:grob-set-property! stem 'direction d)
                              (ly:note-head::calc-stem-attachment g))
                            '(1 -1 0))))
          (newline)
          (ly:grob-set-property! stem 'direction orig)
          s))
      (ly:note-head::print g)))
\layout { \context { \Voice \override NoteHead.stencil = #probe } }
{ \repeat unfold 20 c'4 }
"#;

/// [`print_vm`] with `(head 0)` a note head of the fonts of `(column 0)`
/// at the font size of none, its stem `(stem 0)` and its staff symbol
/// `(staff 0)`, of five lines, and `direction` a property of
/// `define-grob-properties.scm`'s type, which `get_strict_grob_direction`
/// sets; and `(head-case n)`, [`HEAD_PROBE`]'s line
/// of the case `n`, and `head-glyphs`, its first line.
fn head_vm(packs: &PathBuf) -> Option<Vm> {
    let mut vm = print_vm(packs)?;
    eval(
        &mut vm,
        &format!(
            "(set-object-property! 'direction 'backend-type? ly:dir?)
             (put! (head 0) 'fonts (ly:grob-property (column 0) 'fonts))
             (ly:grob-set-object! (head 0) 'stem (stem 0))
             (ly:grob-set-object! (head 0) 'staff-symbol (staff 0))
             (put! (staff 0) 'line-positions '(-4 -2 0 2 4))
             (define head-cases {HEAD_CASES})
             (define (head-case n)
               (let ((c (list-ref head-cases n)) (g (head 0)) (stem (stem 0)))
                 (for-each (lambda (sym v) (put! g sym v))
                           '(duration-log style glyph-name direction staff-position) c)
                 (let ((s (ly:note-head::print g)))
                   (list (%stencil->dump s)
                         (map (lambda (d)
                                (put! stem 'direction d)
                                (ly:note-head::calc-stem-attachment g))
                              '(1 -1 0))))))
             (define (head-glyphs)
               (map (lambda (f)
                      (map (lambda (gl)
                             (map (lambda (d) (ly:note-head::stem-attachment f gl d)) '(1 -1 0)))
                           {HEAD_GLYPHS}))
                    (list (ly:grob-default-font (head 0)) (ly:system-font-load \"emmentaler-20\"))))"
        ),
    );
    Some(vm)
}

/// The warnings the head cases give, each as often as the engine gives
/// it: the print and the three stem attachments of `xyz`'s and of the
/// maxima's head, the longa's of no direction once (then up), and
/// `rests.2`'s attachment up and down of each font. LilyPond's layout
/// reads the stem attachment of the probed heads again after the probe,
/// so that it may give more.
const HEAD_WARNINGS: [(&str, usize); 4] = [
    ("none of note heads `noteheads.s' or `noteheads.u' found", 4),
    ("none of note heads `noteheads.s' or `noteheads.d' found", 4),
    (" must be UP or DOWN; using UP", 1),
    ("no stem attachment found in font for glyph rests.2", 4),
];

#[test]
fn a_note_head_s_print_and_stem_attachment_are_lilypond_s() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is unverified against it");
        return;
    };
    let Some(mut vm) = head_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-head-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("head.ly"),
        HEAD_PROBE
            .replace("CASES", HEAD_CASES)
            .replace("GLYPHS", HEAD_GLYPHS),
    )
    .unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "head.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 19, "{lilypond}");
    assert_eq!(caught(&mut vm, "(head-glyphs)"), lines[0]);
    for (n, line) in lines[1..].iter().enumerate() {
        let got = caught(&mut vm, &format!("(head-case {n})"));
        assert_eq!(otf_names(&got), otf_names(line), "case {n}");
    }
    let diagnostics = diagnosed(&mut vm);
    for (warning, count) in HEAD_WARNINGS {
        assert!(
            stderr.matches(warning).count() >= count,
            "{warning}: {stderr}"
        );
        assert_eq!(
            diagnostics.matches(warning).count(),
            count,
            "{warning}: {diagnostics}"
        );
    }
    assert!(diagnostics.contains("direction of grob Head must be UP or DOWN"));
}

#[test]
fn a_mensural_head_between_the_lines_takes_its_r_glyph() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = head_vm(packs) else {
        return;
    };
    let print = |vm: &mut Vm, pos: &str| {
        caught(
            vm,
            &format!(
                "(put! (head 0) 'style 'mensural)
                 (put! (head 0) 'glyph-name \"M1mensural\")
                 (put! (head 0) 'staff-position {pos})
                 (caddr (ly:stencil-expr (ly:note-head::print (head 0))))"
            ),
        )
    };
    // On a staff line or a ledger line, the breve's glyph; between them,
    // or without a staff symbol, its `r` glyph, as LilyPond 2.26.0 prints
    // `\breve`s at -14 and -13 (`on_line` with the ledgers).
    for (pos, glyph) in [
        ("2", "sM1mensural"),
        ("1", "srM1mensural"),
        ("6", "sM1mensural"),
        ("7", "srM1mensural"),
        ("-14", "sM1mensural"),
        ("-13", "srM1mensural"),
    ] {
        assert_eq!(
            print(&mut vm, pos),
            format!("\"noteheads.{glyph}\""),
            "{pos}"
        );
    }
    eval(&mut vm, "(ly:grob-set-object! (head 0) 'staff-symbol '())");
    assert_eq!(print(&mut vm, "2"), "\"noteheads.srM1mensural\"");
}

/// The glyph pages.scm (stencils.ly) draws for the NoteHead of
/// layout.tsv's link `link`, and its duration log.
fn head_glyph<'a>(pages: &'a str, link: &str) -> (&'a str, i32) {
    // layout.tsv's link file:line:char:column, pages.scm's textedit link
    // ends at the column + 1, its file escaped as a URI's path.
    let (at, column) = link.rsplit_once(':').unwrap();
    let at = at
        .replace('<', "%3c")
        .replace('>', "%3e")
        .replace(' ', "%20");
    let column: u32 = column.parse().unwrap();
    let link = format!("/{at}:{}\" (named-glyph ", column + 1);
    // T19's repeat draws a head twice from one note.
    let found: Vec<&str> = pages
        .match_indices(&link)
        .map(|(i, _)| form(pages, i + link.len() - "(named-glyph ".len()))
        .collect();
    assert!(!found.is_empty(), "{link}");
    assert!(found.iter().all(|g| *g == found[0]), "{link}");
    let glyph = found[0];
    let name = glyph.rsplit('"').nth(1).unwrap();
    let log: i32 = name.strip_prefix("noteheads.s").unwrap().parse().unwrap();
    (glyph, log)
}

#[test]
fn the_note_heads_print_is_lilypond_s_on_y1_s_five() {
    // Each NoteHead of the five's layout.tsv, printed with the duration
    // log of the glyph pages.scm (stencils.ly) draws for its link: the
    // expression pages.scm's, the extents layout.tsv's, within layout.mjs'
    // TOLERANCE.
    let refs = corpus().join("ref");
    if FIVE.iter().any(|id| {
        !refs.join(id).join("layout.tsv").exists() || !refs.join(id).join("pages.scm").exists()
    }) {
        eprintln!("skipped: no layout.tsv or pages.scm in {}", refs.display());
        return;
    }
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is unverified");
        return;
    };
    let Some(mut vm) = head_vm(packs) else {
        return;
    };
    let extent = |e: &str| match e.split_once(' ') {
        Some((l, r)) => format!("({l} . {r})"),
        None => "(+inf.0 . -inf.0)".into(),
    };
    let mut heads = 0;
    for id in FIVE {
        let pages = std::fs::read_to_string(refs.join(id).join("pages.scm")).unwrap();
        let tsv = std::fs::read_to_string(refs.join(id).join("layout.tsv")).unwrap();
        for line in tsv.lines().filter(|l| l.starts_with("NoteHead\t")) {
            let l: Vec<&str> = line.split('\t').collect();
            let (glyph, log) = head_glyph(&pages, l[2]);
            let got = caught(
                &mut vm,
                &format!(
                    "(put! (head 0) 'duration-log {log})
                     (%stencil->dump (ly:note-head::print (head 0)))"
                ),
            );
            let expected = format!("({} {} {glyph})", extent(l[10]), extent(l[11]));
            assert!(
                same_within(&otf_names(&got), &expected),
                "{id} {line}:\n{got}\n{expected}"
            );
            heads += 1;
        }
    }
    assert_eq!(heads, 23);
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The rest's callbacks (MS3 step 37), each of the grob alone, and
/// `ly:rest::pure-height`, of the grob, a start and an end.
const REST_CALLBACKS: [&str; 7] = [
    "ly:rest::print",
    "ly:rest::width",
    "ly:rest::height",
    "ly:rest::y-offset-callback",
    "ly:rest::calc-cross-staff",
    "ly:note-column::calc-main-extent",
    "ly:rest::pure-height",
];

#[test]
fn the_rest_s_and_the_note_column_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in REST_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let (args, wrong) = if name.ends_with("pure-height") {
            ("'x 0 0", ["'x", "'x 0 0 0"])
        } else {
            ("'x", ["", "'x 'x"])
        };
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in wrong {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn a_rest_without_a_staff_is_at_its_voiced_position() {
    let Some(mut vm) = grobs_vm() else { return };
    let y = |vm: &mut Vm, props: &str| {
        caught(
            vm,
            &format!(
                "(for-each (lambda (p) (put! (head 0) (car p) (cdr p))) '({props}))
                      (ly:rest::y-offset-callback (head 0))"
            ),
        )
    };
    // No staff symbol: a staff space of 1 and no lines to align with.
    assert_eq!(
        y(
            &mut vm,
            "(duration-log . 0) (direction . 1) (voiced-position . 4)"
        ),
        "2.0"
    );
    assert_eq!(y(&mut vm, "(direction . -1) (voiced-position . 3)"), "-1.5");
    // voiced-position as from_scm<int> (…, 0) reads it: no int, none.
    assert_eq!(y(&mut vm, "(voiced-position . 3.0)"), "0.0");
    assert_eq!(y(&mut vm, "(voiced-position . ()) (direction . 0)"), "0.0");
    // A staff-position overrides, a whole rest's a line higher.
    assert_eq!(y(&mut vm, "(staff-position . 1)"), "1.5");
    assert_eq!(
        y(&mut vm, "(duration-log . 2) (staff-position . -3)"),
        "-1.5"
    );
    // duration-log as scm_to_int converts it.
    let got = y(&mut vm, "(duration-log . 2.0)");
    assert!(
        got.contains("Wrong type") && got.contains("exact integer"),
        "{got}"
    );
    let got = y(&mut vm, "(duration-log . ())");
    assert!(got.contains("Wrong type"), "{got}");
    // No duration log: no stencil, of empty extents.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:stencil-expr (ly:rest::print (head 0))) (ly:rest::height (head 0))
                   (ly:rest::pure-height (head 0) 0 0) (ly:rest::width (head 0)))"
        ),
        "(() (+inf.0 . -inf.0) (+inf.0 . -inf.0) (+inf.0 . -inf.0))"
    );
}

#[test]
fn a_rest_is_cross_staff_with_its_stem() {
    let Some(mut vm) = grobs_vm() else { return };
    let cross = |vm: &mut Vm| caught(vm, "(ly:rest::calc-cross-staff (head 0))");
    assert_eq!(cross(&mut vm), "#f");
    eval(&mut vm, "(ly:grob-set-object! (head 0) 'stem (stem 0))");
    assert_eq!(cross(&mut vm), "()");
    eval(&mut vm, "(put! (stem 0) 'cross-staff #t)");
    assert_eq!(cross(&mut vm), "#t");
}

/// The cases of [`REST_PROBE`]: each rest's `duration-log`, `style`,
/// `direction`, `voiced-position`, `staff-position` and `font-size` (none
/// for `()`), the staff position it is moved to, and its staff's
/// `line-positions` (`default`, five lines, or a list).
const REST_CASES: &str = r#"(list
 '(2 default 0 4 () () 0 default) '(2 default 1 4 () () 0 default) '(2 default -1 4 () () 0 default)
 '(1 default 0 4 () () 0 default) '(1 default 1 4 () () 6 default) '(1 default -1 4 () () -6 default)
 '(1 default 1 3 () () 5 default) '(1 default 0 4 () () -2.5 default) '(1 default 0 4 () () 2.5 default)
 '(0 default 0 4 () () 2 default) '(0 default -1 4 () () -3 default) '(0 default 1 4 () () 7 default)
 '(0 default 0 4 () () -5 default) '(-1 default 0 4 () () 2 default) '(-1 default 0 4 () () 3 default)
 '(-1 default 0 4 () () -6 default) '(-1 default 1 4 () () 5 default) '(-1 default 0 4 () 3 2 default)
 '(-2 default 0 4 () () 0 default) '(-2 default -1 4 () () 0 default) '(2 default 0 4 3 () 0 default)
 '(0 default 0 4 3 () 0 default) '(0 default 1 4 -1.5 () 0 default) '(1 default -1 4 -3 () 0 default)
 '(3 default 1 4 () () 0 default) '(7 default 0 4 () () 0 default) '(11 default 0 4 () () 0 default)
 '(2 classical 0 4 () () 0 default) '(1 classical 0 4 () () 6 default) '(2 z 0 4 () () 0 default)
 '(1 mensural 0 4 () () 6 default) '(5 mensural 0 4 () () 0 default) '(2 neomensural 0 4 () () 0 default)
 '(2 xyz 0 4 () () 0 default) '(-1 default 0 4 () () 0 (0)) '(-1 default 1 4 () () 0 (0))
 '(0 default 0 4 () () 0 (0)) '(1 default 1 4 () () 0 (-3 -1 1 3)) '(1 default -1 4 () () 1 (-3 -1 1 3))
 '(0 default 0 4 () () 0 ()) '(0 default -1 4 () () 0 (2 -2 0)))"#;

/// A score whose first rests are given [`REST_CASES`]' properties and moved
/// to their staff positions, each written as a line: its
/// `ly:rest::print` as `tools/compare/stencils.ly`'s datum `(x y expr)`,
/// its `::height`, `::pure-height`, `::width`, `::y-offset-callback` and
/// `::calc-cross-staff`.
const REST_PROBE: &str = r#"\version "2.26.0"
#(define cases CASES)
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (dump s) (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s))))
#(define n 0)
#(define (probe g)
  (if (< n (length cases))
      (let* ((c (list-ref cases n))
             (staff (ly:grob-object g 'staff-symbol))
             (orig (ly:grob-property staff 'line-positions))
             (lines (list-ref c 7)))
        (set! n (1+ n))
        (for-each (lambda (sym v) (if (not (null? v)) (ly:grob-set-property! g sym v)))
                  '(duration-log style direction voiced-position staff-position font-size)
                  (list-head c 6))
        (if (list? lines) (ly:grob-set-property! staff 'line-positions lines))
        (ly:grob-translate-axis!
         g (* 0.5 (ly:staff-symbol-staff-space g) (- (list-ref c 6) (ly:grob-staff-position g))) Y)
        (write (list (dump (ly:rest::print g)) (ly:rest::height g) (ly:rest::pure-height g 0 0)
                     (ly:rest::width g) (ly:rest::y-offset-callback g) (ly:rest::calc-cross-staff g)))
        (newline)
        (ly:grob-set-property! staff 'line-positions orig)))
  (ly:rest::print g))
\layout { \context { \Voice \override Rest.stencil = #probe } }
{ \repeat unfold 45 r4 }
"#;

/// [`print_vm`] with [`set_layout`]'s staff space, `(head 1)` a rest of the
/// fonts of `(column 0)` on the staff symbol `(staff 0)`, both under
/// `(thing 5)`; and `(rest-case n)`, [`REST_PROBE`]'s line of the case
/// `n`. The rest's font is the one its first case selects, as LilyPond's
/// layout has selected each rest's before the probe: a `font-size` moves a
/// breve's glyph but does not scale it.
fn rest_vm(packs: &PathBuf) -> Option<Vm> {
    let mut vm = print_vm(packs)?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        &format!(
            "(put! (head 1) 'fonts (ly:grob-property (column 0) 'fonts))
             (ly:grob-set-object! (head 1) 'staff-symbol (staff 0))
             (ly:grob-set-parent! (head 1) 1 (thing 5))
             (ly:grob-set-parent! (staff 0) 1 (thing 5))
             (define rest-cases {REST_CASES})
             (define (rest-case n)
               (let ((c (list-ref rest-cases n)) (g (head 1)) (staff (staff 0)))
                 (for-each (lambda (sym v) (put! g sym v))
                           '(duration-log style direction voiced-position staff-position font-size)
                           (list-head c 6))
                 (put! staff 'line-positions
                       (if (list? (list-ref c 7)) (list-ref c 7) '(-4 -2 0 2 4)))
                 (ly:grob-translate-axis!
                  g (* 0.5 (ly:staff-symbol-staff-space g) (- (list-ref c 6) (ly:grob-staff-position g))) 1)
                 (list (%stencil->dump (ly:rest::print g)) (ly:rest::height g)
                       (ly:rest::pure-height g 0 0) (ly:rest::width g)
                       (ly:rest::y-offset-callback g) (ly:rest::calc-cross-staff g))))"
        ),
    );
    Some(vm)
}

#[test]
fn a_rest_s_print_extents_and_y_offset_are_lilypond_s() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the rests are unverified against it");
        return;
    };
    let Some(mut vm) = rest_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-rest-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rest.ly"), REST_PROBE.replace("CASES", REST_CASES)).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "rest.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 41, "{lilypond}");
    for (n, line) in lines.iter().enumerate() {
        let got = caught(&mut vm, &format!("(rest-case {n})"));
        assert_eq!(otf_names(&got), otf_names(line), "case {n}");
    }
    // The print, the height, the pure height and the width each warn;
    // LilyPond's layout reads them again after the probe.
    let diagnostics = diagnosed(&mut vm);
    for warning in ["rest `rests.11' not found", "rest `rests.2xyz' not found"] {
        assert!(stderr.matches(warning).count() >= 4, "{warning}: {stderr}");
        assert_eq!(
            diagnostics.matches(warning).count(),
            4,
            "{warning}: {diagnostics}"
        );
    }
}

#[test]
fn the_rests_print_and_y_offset_are_lilypond_s_on_y1_s_five() {
    // Each Rest of the five's layout.tsv (T19's four whole rests), printed
    // as pages.scm (stencils.ly) draws the glyph at its X, with
    // layout.tsv's extents, and at its Y offset from the staff, within
    // layout.mjs' TOLERANCE.
    let refs = corpus().join("ref");
    if FIVE.iter().any(|id| {
        !refs.join(id).join("layout.tsv").exists() || !refs.join(id).join("pages.scm").exists()
    }) {
        eprintln!("skipped: no layout.tsv or pages.scm in {}", refs.display());
        return;
    }
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is unverified");
        return;
    };
    let Some(mut vm) = rest_vm(packs) else {
        return;
    };
    let extent = |e: &str| match e.split_once(' ') {
        Some((l, r)) => format!("({l} . {r})"),
        None => "(+inf.0 . -inf.0)".into(),
    };
    let mut rests = 0;
    for id in FIVE {
        let pages = std::fs::read_to_string(refs.join(id).join("pages.scm")).unwrap();
        let tsv = std::fs::read_to_string(refs.join(id).join("layout.tsv")).unwrap();
        let fields = |l: &'static str| -> Vec<Vec<String>> {
            tsv.lines()
                .filter(|line| line.starts_with(l))
                .map(|line| line.split('\t').map(String::from).collect())
                .collect()
        };
        let lines = fields("Rest\t");
        if lines.is_empty() {
            continue;
        }
        let staves = fields("StaffSymbol\t");
        assert_eq!(staves.len(), 1, "{id}");
        let staff_y: f64 = staves[0][9].parse().unwrap();
        for l in lines {
            // A rest has no link: its glyph is the one drawn at its X.
            let x: f64 = l[8].parse().unwrap();
            let found: Vec<&str> = pages
                .match_indices("(translate-stencil (")
                .filter(|(i, _)| {
                    let at = &pages[i + "(translate-stencil (".len()..];
                    let drawn: f64 = at[..at.find(' ').unwrap()].parse().unwrap();
                    (drawn - x).abs() <= 5e-5 && at.contains("\"rests.")
                })
                .map(|(i, _)| form(&pages, i))
                .filter(|f| f.contains("\"rests."))
                .map(|f| {
                    let g = f.find("(named-glyph ").unwrap();
                    form(f, g)
                })
                .collect();
            assert_eq!(found.len(), 1, "{id} {l:?}");
            let glyph = found[0];
            let name = glyph.rsplit('"').nth(1).unwrap();
            let log = name.strip_prefix("rests.").unwrap();
            let log = log.replace('M', "-");
            // Its position from its Y offset, as rest-case moves it.
            let y: f64 = l[9].parse().unwrap();
            let pos = 2.0 * (y - staff_y);
            let got = caught(
                &mut vm,
                &format!(
                    "(for-each (lambda (sym v) (put! (head 1) sym v))
                               '(duration-log style direction voiced-position staff-position)
                               '({log} () () 4 ()))
                     (put! (staff 0) 'line-positions '(-4 -2 0 2 4))
                     (ly:grob-translate-axis!
                      (head 1) (* 0.5 (- {pos} (ly:grob-staff-position (head 1)))) 1)
                     (list (ly:rest::y-offset-callback (head 1))
                           (%stencil->dump (ly:rest::print (head 1))))"
                ),
            );
            let expected = format!(
                "({:.4} ({} {} {glyph}))",
                y - staff_y,
                extent(&l[10]),
                extent(&l[11])
            );
            assert!(
                same_within(&otf_names(&got), &expected),
                "{id} {l:?}:\n{got}\n{expected}"
            );
            rests += 1;
        }
    }
    assert_eq!(rests, 4);
    assert_eq!(diagnosed(&mut vm), "()");
}

/// A score whose note columns write, after line breaking, their
/// `ly:note-column::calc-main-extent`, their stem's direction, their
/// heads' duration logs and staff positions and their rest's duration log
/// (`#f` for none): single heads and chords, seconds with the stem up and
/// down, whole notes, breves and rests, whose columns have a stem without
/// heads.
const MAIN_EXTENT_PROBE: &str = r#"\version "2.26.0"
#(define (probe g)
  (let* ((stem (ly:grob-object g 'stem))
         (a (ly:grob-object g 'note-heads))
         (heads (if (ly:grob-array? a) (ly:grob-array->list a) '()))
         (rest (ly:grob-object g 'rest)))
    (write (list (ly:note-column::calc-main-extent g)
                 (if (ly:grob? stem) (ly:grob-property stem 'direction) '())
                 (map (lambda (h) (list (ly:grob-property h 'duration-log) (ly:grob-staff-position h))) heads)
                 (if (ly:grob? rest) (ly:grob-property rest 'duration-log) #f)))
    (newline)))
\layout { \context { \Voice \override NoteColumn.after-line-breaking = #probe } }
{ c'4 <c' e' g'>2 r4 c'1 r1 \stemDown <c'' e''>4 \stemNeutral <c' d'>4 <a'' b''>4 r\breve c'\breve
  b'8 r8 <c' d' e'>2 r16 }
"#;

/// [`print_vm`] with `(main-case line)`: [`MAIN_EXTENT_PROBE`]'s line
/// made again of `(note 0)`, with `(stem 0)` of the line's direction (none
/// for `()`), and fresh pieces as its heads, their X extent their print's,
/// and its rest, their X extent `ly:rest::width`, each on `(staff 0)` at
/// its staff position (no common refpoint) and in `(note 0)` at X 0.
fn main_extent_vm(packs: &PathBuf) -> Option<Vm> {
    let mut vm = print_vm(packs)?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        "(define pieces 0)
         (define (new-piece durlog pos stencil width)
           (let ((g (piece pieces)))
             (set! pieces (1+ pieces))
             (put! g 'fonts (ly:grob-property (column 0) 'fonts))
             (put! g 'duration-log durlog)
             (put! g 'staff-position pos)
             (put! g 'stencil stencil)
             (put! g 'X-extent width)
             (ly:grob-set-object! g 'staff-symbol (staff 0))
             (ly:grob-set-parent! g 0 (note 0))
             g))
         (define (main-case line)
           (let* ((dir (cadr line))
                  (heads (map (lambda (h) (new-piece (car h) (cadr h) ly:note-head::print ly:grob::stencil-width))
                              (caddr line)))
                  (rest (cadddr line)))
             (ly:grob-set-object! (note 0) 'note-heads (apply arr heads))
             (ly:grob-set-object! (note 0) 'rest
                                  (if rest (new-piece rest 0 ly:rest::print ly:rest::width) '()))
             (ly:grob-set-object! (stem 0) 'note-heads (apply arr heads))
             (put! (stem 0) 'direction dir)
             (ly:grob-set-object! (note 0) 'stem (if (null? dir) '() (stem 0)))
             (cons (ly:note-column::calc-main-extent (note 0)) (cdr line))))",
    );
    Some(vm)
}

#[test]
fn the_main_extent_is_lilypond_s() {
    let Some(packs) = print_packs() else {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the main extent is unverified against it"
        );
        return;
    };
    let Some(mut vm) = main_extent_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-main-extent-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.ly"), MAIN_EXTENT_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "main.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 14, "{lilypond}");
    for line in lines {
        assert_eq!(caught(&mut vm, &format!("(main-case '{line})")), line);
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_main_extent_without_a_stem_is_the_first_head_s() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the heads are not printed");
        return;
    };
    let Some(mut vm) = main_extent_vm(packs) else {
        return;
    };
    // No stem: the first head of all, not the lowest; a stem of no
    // direction has no first head, and the rest is taken; nothing, (0 . 0).
    let main = |vm: &mut Vm, line: &str| caught(vm, &format!("(car (main-case '{line}))"));
    assert_eq!(
        main(&mut vm, "(x () ((0 2) (2 -6)) #f)"),
        "(0.0 . 1.9619999999999997)"
    );
    assert_eq!(main(&mut vm, "(x 0 ((0 2) (2 -6)) 2)"), "(0.0 . 0.95)");
    assert_eq!(main(&mut vm, "(x 0 ((0 2)) #f)"), "(0.0 . 0.0)");
    assert_eq!(main(&mut vm, "(x () () #f)"), "(0.0 . 0.0)");
}

#[test]
fn the_dot_column_s_callback_is_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:dot-column::calc-positioning-done";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(music.iter().all(|p| p.name != name));
    let got = caught(&mut vm, &format!("({name} 'x)"));
    assert!(
        got.contains(name) && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
        "{got}"
    );
    for args in ["", "'x 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
}

/// [`DEFS`] with five grob types more: `NoteHd`, a note head, `RestHd`, a
/// rest, `Dot`, the dots of a head, `DotCol`, a dot column, and `Fl`, a
/// flag; and [`MADE`] with seven note heads, a rest, seven dots, a dot
/// column and a flag.
fn dots_vm() -> Option<Vm> {
    let global = DEFS[0].replacen(
        "(Span (meta",
        "(NoteHd (meta (name . NoteHd) (classes Item)
                   (interfaces grob-interface item-interface rhythmic-head-interface
                               note-head-interface staff-symbol-referencer-interface)))
         (RestHd (meta (name . RestHd) (classes Item)
                   (interfaces grob-interface item-interface rhythmic-head-interface
                               rest-interface staff-symbol-referencer-interface)))
         (Dot (meta (name . Dot) (classes Item)
                (interfaces grob-interface item-interface dots-interface
                            staff-symbol-referencer-interface)))
         (DotCol (meta (name . DotCol) (classes Item)
                   (interfaces grob-interface item-interface axis-group-interface
                               dot-column-interface)))
         (Fl (meta (name . Fl) (classes Item) (interfaces grob-interface item-interface)))
         (Span (meta",
        1,
    );
    let made = MADE.replace(
        "(Span . 1))",
        "(Span . 1) (NoteHd . 7) (RestHd . 1) (Dot . 7) (DotCol . 1) (Fl . 1))",
    );
    let mut vm = grobs_vm_of(&[&global, DEFS[1], DEFS[2]], &made)?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        "(put! (staff 0) 'line-positions '(-4 -2 0 2 4))
         (for-each (lambda (g) (ly:grob-set-parent! g 0 (column 0))) (list (stem 0) (made-of 'Fl 0)))
         (define (dot-case line)
           (let* ((ni 0)
                  (heads
                   (map (lambda (h)
                          (let ((g (if (eq? (car h) 'rest)
                                       (made-of 'RestHd 0)
                                       (begin (set! ni (1+ ni)) (made-of 'NoteHd (1- ni))))))
                            (ly:grob-set-parent! g 0 (column 0))
                            (ly:grob-set-parent! g 1 (staff 0))
                            (put! g 'Y-offset (/ (cadr h) 2))
                            (put! g 'X-extent (caddr h))
                            (put! g 'duration-log (list-ref h 4))
                            (ly:grob-set-object! g 'staff-symbol (staff 0))
                            (if (list-ref h 3) (ly:grob-set-object! g 'stem (stem 0)))
                            g))
                        (cadr line)))
                  (col (made-of 'DotCol 0)))
             (for-each
              (lambda (s)
                (put! (stem 0) 'direction (car s))
                (ly:grob-set-object! (stem 0) 'note-heads
                                     (apply arr (map (lambda (i) (list-ref heads i)) (cadr s))))
                (if (caddr s)
                    (let ((f (made-of 'Fl 0)))
                      (ly:grob-set-parent! f 1 (stem 0))
                      (put! f 'X-extent (car (caddr s)))
                      (put! f 'Y-extent (cadr (caddr s)))
                      (ly:grob-set-object! (stem 0) 'flag f))))
              (caddr line))
             (ly:grob-set-parent! col 0 (column 0))
             (put! col 'chord-dots-limit (caar line))
             (put! col 'padding (cadar line))
             (put! col 'positioning-done ly:dot-column::calc-positioning-done)
             (let ((dots
                    (let loop ((ds (cadddr line)) (i 0) (out '()))
                      (if (null? ds)
                          (reverse out)
                          (let* ((d (car ds)) (g (made-of 'Dot i)) (h (list-ref heads (car d))))
                            (ly:grob-set-parent! g 1 h)
                            (ly:grob-set-parent! g 0 col)
                            (put! g 'Y-offset ly:grob::x-parent-positioning)
                            (ly:grob-set-object! g 'staff-symbol (staff 0))
                            (for-each (lambda (sym v) (put! g sym v))
                                      '(staff-position direction style) (cdr d))
                            ;; A rest's dots moved right as add_head moves them.
                            (if (eq? (car (list-ref (cadr line) (car d))) 'rest)
                                (ly:grob-translate-axis! g 1.75 0)
                                (put! g 'X-offset ly:grob::x-parent-positioning))
                            (loop (cdr ds) (1+ i) (cons g out)))))))
               (ly:grob-set-object! col 'dots (apply arr dots))
               (ly:grob-set-object! col 'side-support-elements (apply arr heads))
               (ly:grob-property col 'positioning-done)
               (list (ly:grob-relative-coordinate col (column 0) 0)
                     (map (lambda (d)
                            (and (pair? (ly:grob-basic-properties d))
                                 (list (ly:grob-staff-position d)
                                       (ly:grob-relative-coordinate d col 0))))
                          dots)))))",
    );
    Some(vm)
}

/// A score whose dot columns write, after line breaking, positioned, a
/// line each: their `chord-dots-limit` and `padding`; their supports, each
/// `(kind position X-extent stem duration-log)`, its X extent in its paper
/// column and its stem by number; the stems, each `(direction heads flag)`,
/// its flag's X extent in the column and Y extent in its common refpoint
/// with the stem; the dots, each `(head staff-position direction style)`;
/// and the column's X offset in the paper column with each dot's staff
/// position and X offset in the column after, `#f` for one killed. Notes
/// on lines and in spaces, on ledgers above and below, chords, seconds
/// with the stem up and down, rests of each duration at their positions,
/// flags, double dots, the dots' directions, chord-dots-limit, the kievan
/// style and a padding.
const DOT_PROBE: &str = r#"\version "2.26.0"
#(define (index-of x l)
  (let loop ((l l) (i 0)) (cond ((null? l) #f) ((eq? (car l) x) i) (else (loop (cdr l) (1+ i))))))
#(define (grobs g sym)
  (let ((a (ly:grob-object g sym))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (probe g)
  (ly:grob-property g 'positioning-done)
  (let* ((col (ly:item-get-column g))
         (support (grobs g 'side-support-elements))
         (dots (grobs g 'dots))
         (stems (delete-duplicates
                 (filter ly:grob? (map (lambda (h) (ly:grob-object h 'stem)) support)) eq?))
         (flag (lambda (s) (let ((f (ly:grob-object s 'flag)))
                             (if (ly:grob? f)
                                 (list (ly:grob-extent f col X) (ly:grob-extent f (ly:grob-common-refpoint s f Y) Y))
                                 #f)))))
    (write
     (list (list (ly:grob-property g 'chord-dots-limit) (ly:grob-property g 'padding))
           (map (lambda (h) (list (if (grob::has-interface h 'rest-interface) 'rest 'note)
                                  (ly:grob-staff-position h)
                                  (ly:grob-extent h col X)
                                  (index-of (ly:grob-object h 'stem) stems)
                                  (ly:grob-property h 'duration-log)))
                support)
           (map (lambda (s) (list (ly:grob-property s 'direction)
                                  (map (lambda (h) (index-of h support)) (grobs s 'note-heads))
                                  (flag s)))
                stems)
           (map (lambda (d) (list (index-of (ly:grob-parent d Y) support)
                                  (ly:grob-property d 'staff-position)
                                  (ly:grob-property d 'direction)
                                  (ly:grob-property d 'style)))
                dots)
           (list (ly:grob-relative-coordinate g col X)
                 (map (lambda (d) (if (grob::is-live? d)
                                      (list (ly:grob-staff-position d) (ly:grob-relative-coordinate d g X))
                                      #f))
                      dots))))
    (newline)))
\layout { \context { \Staff \override DotColumn.after-line-breaking = #probe } }
{ \autoBeamOff c'4. d'4. e'4. f'4. g'2. a'2. b'2. c''2. d''1. e''4. f''4. g''4. a''4. b''4. c'''4.
  a4. g4. f4. e4. d4. c4.
  <c' e' g'>4. <c' d'>4. <c' d' e'>2. <f' g' a' b'>4. \stemDown <f' g'>4. <c'' d'' e''>4. \stemNeutral
  <c' e' g' b' d'' f'' a''>4. <c' d' e' f' g' a' b'>2.
  r4. r2. r1. r8. r16. r\breve. r128. \override Rest.staff-position = #-3 r4. r1. \override Rest.staff-position = #5 r2. \revert Rest.staff-position
  c'8. d''8. <e' f'>16. b'8.. c''4..
  \override Dots.direction = #DOWN <e' g'>4. \override Dots.direction = #UP <f' a'>4. \revert Dots.direction
  \override Staff.DotColumn.chord-dots-limit = #1 <c' e' g' b'>4. \override Staff.DotColumn.chord-dots-limit = #0 <c' d' e'>4.
  \override Staff.DotColumn.chord-dots-limit = ##f <c' d' e' f' g'>4. \revert Staff.DotColumn.chord-dots-limit
  \override Dots.style = #'kievan e'4. \revert Dots.style
  \override Staff.DotColumn.padding = #0.5 g'4. \revert Staff.DotColumn.padding
}
"#;

#[test]
fn the_dots_are_positioned_as_lilypond_positions_them() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the dots are unverified against it");
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-dots-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dots.ly"), DOT_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages", "dots.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 51, "{lilypond}");
    let mut killed = 0;
    for (n, line) in lines.iter().enumerate() {
        let Some(mut vm) = dots_vm() else { return };
        eval(&mut vm, &format!("(define c '{line})"));
        let want = caught(&mut vm, "(list-ref c 4)");
        assert_eq!(caught(&mut vm, "(dot-case c)"), want, "case {n}: {line}");
        assert_eq!(diagnosed(&mut vm), "()", "case {n}");
        killed += want.matches("#f").count();
    }
    // Two chords lose dots to their chord-dots-limit.
    assert_eq!(killed, 3);
}

/// The staff symbols of [`ON_LINE_PROBE`]: each `(line-positions
/// ledger-positions ledger-extra ledger-positions-function)`, `()` unset.
const ON_LINE_CASES: &str = "(list
  '((-4 -2 0 2 4) () () ())
  '((-4 -2 0 2 4) () 1 ())
  '((-4 -2 0 2 4) () 2.5 ())
  '((-4 -2 0 2 4) () -1 ())
  '((-3 -1 1 3) () () ())
  '((0) () () ())
  '((-5 -1 3) () () ())
  '((1.5 -0.5) () () ())
  '(() () () ())
  '((-4 -2 0 2 4) (-2 0 2) () ())
  '((-4 -2 0 2 4) ((-3 -1) 1 3) () ())
  '((-4 -2 0 2 4) (-4 -3 -1 1 3 4) 1 ())
  '((-4 -2 0 2 4) (0 0.05) () ())
  '((-4 -2 0 2 4) () () (lambda (g p) (list 7 9 -9))))";

/// A score whose first note head writes, for each of [`ON_LINE_CASES`] set
/// on its staff symbol, a line: whether each position from -15 to 15 is on
/// a line of the staff or a ledger line (`ly:position-on-line?`,
/// `Staff_symbol_referencer::on_line`).
const ON_LINE_PROBE: &str = r#"\version "2.26.0"
#(define cases CASES)
#(define done #f)
#(define (probe g)
  (if (not done)
      (let ((st (ly:grob-object g 'staff-symbol)))
        (set! done #t)
        (for-each (lambda (c)
                    (for-each (lambda (sym v) (ly:grob-set-property! st sym v))
                              '(line-positions ledger-positions ledger-extra ledger-positions-function) c)
                    (write (map (lambda (p) (ly:position-on-line? g p)) (iota 31 -15)))
                    (newline))
                  cases))))
\layout { \context { \Voice \override NoteHead.after-line-breaking = #probe } }
{ c'4 }
"#;

#[test]
fn a_position_is_on_a_line_of_the_staff_or_its_ledgers_as_lilypond_finds_it() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: on_line is unverified against it");
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-on-line-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("on-line.ly"),
        ON_LINE_PROBE.replace("CASES", ON_LINE_CASES),
    )
    .unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages", "on-line.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 14, "{lilypond}");
    let Some(mut vm) = dots_vm() else { return };
    eval(
        &mut vm,
        &format!(
            "(define cases {ON_LINE_CASES})
             (ly:grob-set-object! (made-of 'NoteHd 0) 'staff-symbol (staff 0))
             (define (on-line-case n)
               (for-each (lambda (sym v) (put! (staff 0) sym v))
                         '(line-positions ledger-positions ledger-extra ledger-positions-function)
                         (list-ref cases n))
               (map (lambda (p) (%staff-on-line? (made-of 'NoteHd 0) p)) (iota 31 -15)))"
        ),
    );
    for (n, line) in lines.iter().enumerate() {
        assert_eq!(
            caught(&mut vm, &format!("(on-line-case {n})")),
            *line,
            "case {n}"
        );
    }
    // Without a staff symbol, on no line.
    eval(
        &mut vm,
        "(ly:grob-set-object! (made-of 'NoteHd 0) 'staff-symbol '())",
    );
    assert_eq!(
        caught(&mut vm, "(%staff-on-line? (made-of 'NoteHd 0) 0)"),
        "#f"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

/// A score whose stems each write, after line breaking, as a line: their
/// direction if forced (else `#f`), duration log, `details`,
/// `length-fraction`, `no-stem-extend`, `avoid-note-head` and
/// `neutral-direction`; their staff symbol's `line-positions` and
/// `staff-space`; their heads' staff positions, Y extents and stem
/// attachments; then their `ly:stem::calc-default-direction`, direction,
/// `stem-begin-position`, `length`, and those two's pure forms. Single
/// heads on both sides of the middle, chords, unisons and seconds, forced
/// directions of seven duration logs, whole notes and breves, rests, and
/// each property the length reads overridden, on a five-line and a
/// three-line staff of 1.5 spaces; unbeamed.
const STEM_PROBE: &str = r#"\version "2.26.0"
#(define (probe g)
  (let* ((a (ly:grob-object g 'note-heads))
         (heads (if (ly:grob-array? a) (ly:grob-array->list a) '()))
         (st (ly:grob-object g 'staff-symbol))
         (forced (not (procedure? (cdr (assq 'direction (ly:grob-basic-properties g)))))))
    (write (list (and forced (ly:grob-property g 'direction))
                 (ly:grob-property g 'duration-log)
                 (ly:grob-property g 'details)
                 (ly:grob-property g 'length-fraction)
                 (ly:grob-property g 'no-stem-extend)
                 (ly:grob-property g 'avoid-note-head)
                 (ly:grob-property g 'neutral-direction)
                 (ly:grob-property st 'line-positions)
                 (ly:grob-property st 'staff-space)
                 (map (lambda (h) (list (ly:grob-staff-position h) (ly:grob-extent h h Y)
                                        (ly:grob-property h 'stem-attachment)))
                      heads)
                 (ly:stem::calc-default-direction g)
                 (ly:grob-property g 'direction)
                 (ly:grob-property g 'stem-begin-position)
                 (ly:grob-property g 'length)
                 (ly:stem::pure-calc-stem-begin-position g 0 0)
                 (ly:stem::pure-calc-length g 0 0)))
    (newline)))
\layout { \context { \Voice \override Stem.after-line-breaking = #probe } }
<<
\new Staff { \autoBeamOff
  c'4 d'' a' b' <c' e' g'> <a' c''> <f' e''> <g e''> <c' c'> <c' d'> <a'' b''> c''''
  g''8 c'''16 f'32 a''64 b'128 g,4
  \stemUp a''4 c'''8 e'''16 b'32 \stemDown c'4 f8 g,16 b'4 \stemNeutral
  c'2 c'1 c'\breve r4 r8
  \override Stem.length-fraction = 0.8 e''4 d' \revert Stem.length-fraction
  \override Stem.no-stem-extend = ##t a4 g''' \revert Stem.no-stem-extend
  \override Stem.avoid-note-head = ##t <c' e''>4 <c'' e'''>4 \revert Stem.avoid-note-head
  \override Stem.neutral-direction = #UP b'4 <a' c''> \revert Stem.neutral-direction
  \override Stem.details.lengths = #'(2 (3 . 4) 5) c'4 c'''4 c'8 c'''8 c'16 \stemUp c'''8 \stemNeutral
  \revert Stem.details.lengths
  \override Stem.details.stem-shorten = #'(2 1) \stemDown c'4 c'8 \stemUp c'''2 \stemNeutral
  \revert Stem.details.stem-shorten
}
\new Staff \with { \override StaffSymbol.line-count = 3 \override StaffSymbol.staff-space = 1.5 } {
  \autoBeamOff c'4 a' e'' \stemDown c' f'8 \stemUp c'''16 \stemNeutral <e' f'>4 c'2
}
>>
"#;

/// [`DEFS`]' VM with a staff symbol, 64 stems, 160 pieces and a spanner
/// (`(span 0)`), the layout's
/// `staff-space` 1, and `(stem-case line)`: [`STEM_PROBE`]'s line made
/// again as a fresh stem with its properties, on `(staff 0)` of the
/// line's `line-positions` and `staff-space`, its heads fresh pieces of
/// the line's staff positions (no common refpoint), Y extents and stem
/// attachments.
fn stem_case_vm() -> Option<Vm> {
    let mut vm = grobs_vm_of(DEFS, "'((Staff . 1) (Stem . 64) (Piece . 160) (Span . 1))")?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        "(define stems 0)
         (define pieces 0)
         (define (new-head h)
           (let ((p (piece pieces)))
             (set! pieces (1+ pieces))
             (put! p 'staff-position (car h))
             (put! p 'Y-extent (cadr h))
             (put! p 'stem-attachment (caddr h))
             (ly:grob-set-object! p 'staff-symbol (staff 0))
             p))
         (define (stem-case line)
           (let ((g (stem stems)))
             (set! stems (1+ stems))
             (for-each (lambda (sym i) (put! g sym (list-ref line i)))
                       '(duration-log details length-fraction no-stem-extend avoid-note-head
                         neutral-direction)
                       '(1 2 3 4 5 6))
             (put! (staff 0) 'line-positions (list-ref line 7))
             (put! (staff 0) 'staff-space (list-ref line 8))
             (ly:grob-set-object! g 'staff-symbol (staff 0))
             (ly:grob-set-object! g 'note-heads (apply arr (map new-head (list-ref line 9))))
             g))",
    );
    Some(vm)
}

/// The value `v` written, and set as `g`'s `name`.
fn written_as(vm: &mut Vm, g: Value, name: &str, v: Value) -> String {
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    grob::set_property(vm.heap_mut(), g, name, v);
    out
}

/// The stem `g`'s `stem.rs` callbacks, as [`STEM_PROBE`] writes them:
/// `calc_default_direction`, the direction (`forced`, else
/// `calc_direction`), `calc_stem_begin_position`, `calc_length`,
/// `pure_calc_stem_begin_position` and `pure_calc_length`; the first four
/// set on it as LilyPond's properties would be.
fn stem_values(vm: &mut Vm, g: Value, forced: Option<Value>) -> Result<String, Exception> {
    let v = stem::calc_default_direction(vm, g)?;
    let default = written_as(vm, g, "default-direction", v);
    let v = match forced {
        Some(d) => d,
        None => stem::calc_direction(vm, g)?,
    };
    let dir = written_as(vm, g, "direction", v);
    let v = stem::calc_stem_begin_position(vm, g)?;
    let begin = written_as(vm, g, "stem-begin-position", v);
    let v = stem::calc_length(vm, g)?;
    let length = written_as(vm, g, "length", v);
    let v = stem::pure_calc_stem_begin_position(vm, g)?;
    let pure_begin = shown(vm, v);
    let v = stem::pure_calc_length(vm, g)?;
    let pure_length = shown(vm, v);
    Ok(format!(
        "({default} {dir} {begin} {length} {pure_begin} {pure_length})"
    ))
}

#[test]
fn a_stem_s_direction_length_and_begin_are_lilypond_s() {
    let Some(mut vm) = stem_case_vm() else { return };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-stem-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stem.ly"), STEM_PROBE).unwrap();
    let Ok(out) = Command::new("lilypond")
        .args(["-dno-print-pages", "stem.ly"])
        .current_dir(&dir)
        .output()
    else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the stems are unverified");
        return;
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("warning"), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 56, "{lilypond}");
    for line in lines {
        let g = eval(&mut vm, &format!("(stem-case '{line})"));
        let forced = eval(&mut vm, &format!("(car '{line})"));
        let forced = (forced != Value::FALSE).then_some(forced);
        let got = stem_values(&mut vm, g, forced).unwrap_or_else(|e| e.text(vm.heap()));
        let expected = caught(&mut vm, &format!("(list-tail '{line} 10)"));
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_stems_lengths_and_print_are_lilypond_s_on_y1_s_five() {
    // Each Stem of the five's layout.tsv, its note head's staff position
    // and Y extent layout.tsv's, its duration log the head's glyph's in
    // pages.scm (stencils.ly), its stem attachment the note head's: of a
    // visible stem, the Y extent of its begin position and length is
    // layout.tsv's, and ly:stem::print (MS3 step 38) the box pages.scm
    // draws at its X with layout.tsv's extents, within layout.mjs'
    // TOLERANCE; a whole note's and a rest's stem is invisible, and has
    // none, and prints nothing.
    let refs = corpus().join("ref");
    if FIVE.iter().any(|id| {
        !refs.join(id).join("layout.tsv").exists() || !refs.join(id).join("pages.scm").exists()
    }) {
        eprintln!("skipped: no layout.tsv or pages.scm in {}", refs.display());
        return;
    }
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the stem attachments are not read");
        return;
    };
    let Some(mut heads) = head_vm(packs) else {
        return;
    };
    let Some(mut vm) = stem_case_vm() else { return };
    // The stems' thickness and the layout's blot, as LilyPond's.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'blot-diameter 0.08)",
    );
    let details = caught(
        &mut vm,
        "'((beamed-extreme-minimum-free-lengths 2.0 1.25) (beamed-lengths 3.26 3.5 3.6)
           (beamed-minimum-free-lengths 1.83 1.5 1.25)
           (lengths 3.5 3.5 3.5 4.25 5.0 6.0 7.0 8.0 9.0) (stem-shorten 1.0 0.5 0.25))",
    );
    let (mut visible, mut invisible) = (0, 0);
    for id in FIVE {
        let pages = std::fs::read_to_string(refs.join(id).join("pages.scm")).unwrap();
        let tsv = std::fs::read_to_string(refs.join(id).join("layout.tsv")).unwrap();
        let lines: Vec<Vec<&str>> = tsv.lines().map(|l| l.split('\t').collect()).collect();
        let staves: Vec<&Vec<&str>> = lines.iter().filter(|l| l[0] == "StaffSymbol").collect();
        assert_eq!(staves.len(), 1, "{id}");
        let staff_y: f64 = staves[0][9].parse().unwrap();
        for (i, l) in lines.iter().enumerate().filter(|(_, l)| l[0] == "Stem") {
            // A stem follows its note head, or has a rest.
            let head = (l[1] == "<NoteHead>").then(|| {
                lines[..i]
                    .iter()
                    .rev()
                    .find(|h| h[0] == "NoteHead" && h[2] == l[2])
                    .unwrap()
            });
            let (log, heads_of) = match head {
                Some(h) => {
                    let (_, log) = head_glyph(&pages, h[2]);
                    let y: f64 = h[9].parse().unwrap();
                    let (lo, hi) = h[11].split_once(' ').unwrap();
                    (log, format!("(({} ({lo} . {hi}) ()))", 2.0 * (y - staff_y)))
                }
                None => (2, "()".into()),
            };
            let g = eval(
                &mut vm,
                &format!(
                    "(stem-case '(#f {log} {details} () () () -1 (4 2 0 -2 -4) () {heads_of}))"
                ),
            );
            let v = stem::calc_default_direction(&mut vm, g).unwrap();
            written_as(&mut vm, g, "default-direction", v);
            let v = stem::calc_direction(&mut vm, g).unwrap();
            let dir = written_as(&mut vm, g, "direction", v);
            if head.is_some() {
                let att = caught(
                    &mut heads,
                    &format!(
                        "(put! (head 0) 'duration-log {log})
                         (put! (stem 0) 'direction {dir})
                         (ly:note-head::calc-stem-attachment (head 0))"
                    ),
                );
                eval(
                    &mut vm,
                    &format!(
                        "(put! (ly:grob-array-ref (ly:grob-object (stem (1- stems)) 'note-heads) 0)
                               'stem-attachment '{att})"
                    ),
                );
            }
            let v = stem::calc_stem_begin_position(&mut vm, g).unwrap();
            let begin = vm.heap().to_f64(v).unwrap();
            grob::set_property(vm.heap_mut(), g, "stem-begin-position", v);
            let v = stem::calc_length(&mut vm, g).unwrap();
            let length = vm.heap().to_f64(v).unwrap();
            grob::set_property(vm.heap_mut(), g, "length", v);
            let v = stem::pure_calc_stem_begin_position(&mut vm, g).unwrap();
            assert_eq!(vm.heap().to_f64(v), Some(begin), "{id} {l:?}");
            let v = stem::pure_calc_length(&mut vm, g).unwrap();
            assert_eq!(vm.heap().to_f64(v), Some(length), "{id} {l:?}");
            let print = caught(
                &mut vm,
                "(let ((g (stem (1- stems))))
                   (put! g 'thickness 1.3)
                   (let ((s (ly:stem::print g))) (if (null? s) s (%stencil->dump s))))",
            );
            if stem::is_invisible(&mut vm, g).unwrap() {
                assert_eq!(
                    (l[10], l[11], print.as_str()),
                    ("-", "-", "()"),
                    "{id} {l:?}"
                );
                invisible += 1;
                continue;
            }
            // A stem has no link: its box is the one drawn at its X.
            let x: f64 = l[8].parse().unwrap();
            let boxes: Vec<&str> = pages
                .match_indices("(translate-stencil (")
                .filter(|(i, _)| {
                    let at = &pages[i + "(translate-stencil (".len()..];
                    let drawn: f64 = at[..at.find(' ').unwrap()].parse().unwrap();
                    (drawn - x).abs() <= 5e-5
                })
                .map(|(i, _)| form(&pages, i))
                .filter_map(|f| {
                    let b = f.find("(grob-cause #f (round-filled-box ")?;
                    Some(form(f, b + "(grob-cause #f ".len()))
                })
                .collect();
            assert_eq!(boxes.len(), 1, "{id} {l:?}");
            let (lo, hi) = l[10].split_once(' ').unwrap();
            let (bottom, top) = l[11].split_once(' ').unwrap();
            let expected = format!("(({lo} . {hi}) ({bottom} . {top}) {})", boxes[0]);
            assert!(
                same_within(&print, &expected),
                "{id} {l:?}:\n{print}\n{expected}"
            );
            // `internal_height`, in staff spaces of 1.
            let d: f64 = dir.parse().unwrap();
            let end = d * length + begin;
            let got = format!("({} . {})", begin.min(end) * 0.5, begin.max(end) * 0.5);
            let (lo, hi) = l[11].split_once(' ').unwrap();
            assert!(
                same_within(&got, &format!("({lo} . {hi})")),
                "{id} {l:?}: {got}"
            );
            visible += 1;
        }
    }
    assert_eq!((visible, invisible), (18, 9));
    assert_eq!(diagnosed(&mut vm), "()");
    assert_eq!(diagnosed(&mut heads), "()");
}

#[test]
fn the_strict_direction_is_up_or_down_and_else_warns_and_sets_up() {
    let Some(mut vm) = grobs_vm() else { return };
    let g = grob_of(&mut vm, "(stem 0)");
    for (d, plain, strict) in [
        ("1", 1, 1),
        ("-1", -1, -1),
        ("0", 0, 1),
        ("2", 0, 1),
        ("1.0", 0, 1),
        ("'()", 0, 1),
    ] {
        eval(&mut vm, &format!("(put! (stem 0) 'direction {d})"));
        assert_eq!(
            direction::get_grob_direction(&mut vm, g).unwrap(),
            plain,
            "{d}"
        );
        assert_eq!(
            direction::get_strict_grob_direction(&mut vm, g).unwrap(),
            strict,
            "{d}"
        );
        assert_eq!(
            caught(&mut vm, "(ly:grob-property (stem 0) 'direction)"),
            strict.to_string()
        );
    }
    let warned = diagnosed(&mut vm);
    assert_eq!(
        warned
            .matches("direction of grob Stem must be UP or DOWN; using UP")
            .count(),
        4,
        "{warned}"
    );
    direction::set_grob_direction(&mut vm, g, -1);
    assert_eq!(
        caught(&mut vm, "(ly:grob-property (stem 0) 'direction)"),
        "-1"
    );
}

#[test]
fn a_beamed_stem_takes_its_beam_s_reading_and_reads_its_tremolo() {
    let Some(mut vm) = stem_case_vm() else { return };
    let line = "(#f 2 ((lengths 3.5) (stem-shorten 1.0)) () () () -1 (4 2 0 -2 -4) () \
                ((-6 (-0.5 . 0.5) (1.0 . 0.3))))";
    let g = eval(&mut vm, &format!("(stem-case '{line})"));
    eval(
        &mut vm,
        "(define (beamed g)
           (ly:grob-set-object! g 'beam (span 0))
           (put! (span 0) 'direction 1)
           (put! (span 0) 'quantized-positions '(1 . 2))
           (put! g 'direction 1)
           (put! g 'length 3.25)
           (put! g 'stem-begin-position -2.0))",
    );
    eval(&mut vm, "(beamed (stem 0))");
    // The beam's reading: its direction, the stem's own length and begin;
    // the pure ones as if it had none.
    let got = |vm: &mut Vm, v: Result<Value, Exception>| match v {
        Ok(v) => shown(vm, v),
        Err(e) => e.text(vm.heap()),
    };
    let v = stem::calc_direction(&mut vm, g);
    assert_eq!(got(&mut vm, v), "1");
    let v = stem::calc_stem_begin_position(&mut vm, g);
    assert_eq!(got(&mut vm, v), "-2.0");
    let end = stem::internal_calc_stem_end_position(&mut vm, g, true).unwrap();
    assert_eq!(end, 3.25 - 2.0);
    let v = stem::pure_calc_stem_begin_position(&mut vm, g);
    assert_eq!(got(&mut vm, v), "-5.7");
    let end = stem::internal_calc_stem_end_position(&mut vm, g, false).unwrap();
    assert_eq!(end, 1.0);
    let v = stem::calc_length(&mut vm, g);
    assert_eq!(got(&mut vm, v), "0.0");
    assert!(
        diagnosed(&mut vm)
            .contains("ly:stem::calc-length called but will not be used for beamed stem."),
    );
    // A tremolo flag is read (stem_tremolo.rs, MS4 step 5) unless the
    // beam's reading is taken: one of no stem has an empty stencil, of no
    // length, with LilyPond's programming error.
    eval(
        &mut vm,
        "(ly:grob-set-object! (stem 0) 'tremolo-flag (piece 159))",
    );
    assert!(stem::internal_calc_stem_end_position(&mut vm, g, true).is_ok());
    assert!(!diagnosed(&mut vm).contains("no stem for stem-tremolo"));
    let v = stem::pure_calc_length(&mut vm, g);
    assert!(v.is_ok());
    assert!(diagnosed(&mut vm).contains("no stem for stem-tremolo"));
    let g = eval(&mut vm, &format!("(stem-case '{line})"));
    eval(
        &mut vm,
        "(ly:grob-set-object! (stem 1) 'tremolo-flag (piece 159))",
    );
    put_direction(&mut vm, g, 1);
    let v = stem::calc_length(&mut vm, g);
    assert!(v.is_ok());
    assert!(diagnosed(&mut vm).contains("no stem for stem-tremolo"));
}

/// `g`'s `direction` set to `d`.
fn put_direction(vm: &mut Vm, g: Value, d: i64) {
    let v = vm.heap_mut().integer(d);
    grob::set_property(vm.heap_mut(), g, "direction", v);
}

#[test]
fn a_stem_s_length_reads_its_details_as_lilypond_converts_them() {
    let Some(mut vm) = stem_case_vm() else { return };
    let case = |vm: &mut Vm, details: &str, pos: i32, d: i64| {
        let g = eval(
            vm,
            &format!(
                "(stem-case '(#f 2 {details} () () () -1 (4 2 0 -2 -4) () \
                 (({pos} (-0.5 . 0.5) (1.0 . 0.3)))))"
            ),
        );
        put_direction(vm, g, d);
        match stem::internal_calc_stem_end_position(vm, g, true) {
            Ok(end) => end.to_string(),
            Err(e) => e.text(vm.heap()),
        }
    };
    // No lengths: 7 half spaces; a stem shortened without them reaches
    // `robust_list_ref`'s `car`.
    assert_eq!(case(&mut vm, "()", -6, 1), "1");
    let thrown = case(&mut vm, "()", 2, 1);
    assert!(thrown.contains("car"), "{thrown}");
    // A length that is no real is `from_scm<double>`'s error; a shortening
    // that is none is 0.
    let thrown = case(&mut vm, "((lengths x))", -6, 1);
    assert!(thrown.contains("Wrong type"), "{thrown}");
    assert_eq!(case(&mut vm, "((lengths 3.5) (stem-shorten x))", 2, 1), "9");
    // A pair: up, then down; a stem shortened takes the first length as
    // a real, and a pair is none.
    assert_eq!(case(&mut vm, "((lengths (3 . 4)))", -2, 1), "4");
    assert_eq!(case(&mut vm, "((lengths (3 . 4)))", 2, -1), "-6");
    let thrown = case(&mut vm, "((lengths (3 . 4)))", 0, 1);
    assert!(
        thrown.contains("Wrong type") && thrown.contains("(3 . 4)"),
        "{thrown}"
    );
    // No direction: the stem ends at 0.
    assert_eq!(case(&mut vm, "((lengths 3.5))", 4, 0), "0");
    // No heads: 0, no direction read.
    let g = eval(
        &mut vm,
        "(stem-case '(#f 2 () () () () -1 (4 2 0 -2 -4) () ()))",
    );
    let v = stem::calc_default_direction(&mut vm, g).unwrap();
    assert_eq!(shown(&vm, v), "0");
    let v = stem::calc_direction(&mut vm, g).unwrap();
    assert_eq!(shown(&vm, v), "-1");
    assert_eq!(
        stem::internal_calc_stem_end_position(&mut vm, g, true).unwrap(),
        0.0
    );
    assert_eq!(
        stem::internal_calc_stem_begin_position(&mut vm, g, true).unwrap(),
        0.0
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The stem's callbacks (MS3 step 38), each of the grob alone, and its
/// three pure ones, of the grob, a start and an end.
const STEM_CALLBACKS: [&str; 14] = [
    "ly:stem::calc-cross-staff",
    "ly:stem::calc-default-direction",
    "ly:stem::calc-direction",
    "ly:stem::calc-length",
    "ly:stem::calc-positioning-done",
    "ly:stem::calc-stem-begin-position",
    "ly:stem::calc-stem-info",
    "ly:stem::height",
    "ly:stem::offset-callback",
    "ly:stem::print",
    "ly:stem::width",
    "ly:stem::pure-calc-length",
    "ly:stem::pure-calc-stem-begin-position",
    "ly:stem::pure-height",
];

#[test]
fn the_stem_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (i, name) in STEM_CALLBACKS.into_iter().enumerate() {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let pure = i >= 11;
        // LilyPond's subr, as it prints.
        let formals = if pure { "(_ _ _)" } else { "(_)" };
        assert_eq!(
            caught(&mut vm, name),
            format!("#<procedure {name} {formals}>")
        );
        let x = if pure { "'x 0 0" } else { "'x" };
        let got = caught(&mut vm, &format!("({name} {x})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        let wrong = if pure {
            ["'x", "'x 0 0 0"]
        } else {
            ["", "'x 'x"]
        };
        for args in wrong {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn the_stem_s_beam_and_tremolo_calls_read_the_beam_and_the_tremolo() {
    // The beam's four calls are beam.rs' since MS4 step 3, the tremolo's
    // two stem_tremolo.rs' since MS4 step 5.
    let Some(mut vm) = stem_case_vm() else { return };
    let line = "(#f 3 ((lengths 3.5 3.5) (stem-shorten 1.0) (beamed-lengths 3.26)) () () () -1 \
                (4 2 0 -2 -4) () ((-6 (-0.5 . 0.5) (1.0 . 0.3))))";
    // The key, the subr and the message thrown.
    let thrown = |vm: &mut Vm, text: &str| {
        caught(
            vm,
            &format!("(catch #t (lambda () {text}) (lambda (k s m . r) (list k s m)))"),
        )
    };
    eval(&mut vm, &format!("(define g (stem-case '{line}))"));
    eval(&mut vm, "(put! g 'direction 1)");
    // Unbeamed: the stem info reads the beam's translation all the same,
    // where LilyPond reads through a null: an error naming it; no beam, no
    // cross-staff.
    let got = thrown(&mut vm, "(ly:stem::calc-stem-info g)");
    assert_eq!(
        got,
        "(misc-error \"Beam::get_beam_translation\" \"a stem with no beam\")"
    );
    assert_eq!(caught(&mut vm, "(ly:stem::calc-cross-staff g)"), "#f");
    // Beamed by a beam of no stems: no stem of it on another staff, and the
    // print's corrective half its thickness.
    eval(
        &mut vm,
        "(ly:grob-set-object! g 'beam (span 0))
         (put! (span 0) 'quantized-positions '(1 . 2))
         (put! (span 0) 'beam-thickness 0.48)
         (put! g 'stem-begin-position -5.0)
         (put! g 'length 7.0)
         (put! g 'thickness 1.3)",
    );
    assert_eq!(caught(&mut vm, "(ly:stem::calc-cross-staff g)"), "#f");
    let got = thrown(&mut vm, "(ly:stem::print g)");
    assert!(!got.starts_with("(misc-error"), "{got}");
    // A tremolo flag: its vertical length, from the stem info too.
    eval(&mut vm, "(ly:grob-set-object! g 'tremolo-flag (piece 159))");
    // A tremolo flag of no stem: its vertical length, an empty stencil
    // (LilyPond's programming error), then its beam translation, where
    // LilyPond reads through a null: an error naming it.
    let got = thrown(&mut vm, "(ly:stem::pure-calc-length g 0 0)");
    assert_eq!(
        got,
        "(misc-error \"Stem_tremolo::get_beam_translation\" \"a stem tremolo with no stem\")"
    );
    // The beamed height reads no beam function: from begin and length.
    assert_eq!(caught(&mut vm, "(ly:stem::height g)"), "(-2.5 . 1.0)");
}

/// A score whose stems write, after line breaking, a line each: their
/// direction, duration log, thickness and `note-collision-threshold`;
/// their staff's line positions and staff space; their begin position
/// and length; their heads, each `(staff-position Y-extent
/// stem-attachment X-extent style)`, and their rests' X extents; then
/// their X offset, X extent and Y extent, their heads' X offsets in the
/// note column, and their stencil `(X-extent Y-extent expr)`, `()` for
/// none. Single heads, chords, seconds and clusters both ways, unisons,
/// whole notes and breves with seconds, a harmonic in a whole-note chord,
/// rests, a thicker stem, a wider collision threshold, mensural and
/// petrucci heads and a staff of 1.5 spaces.
const STEM_PRINT_PROBE: &str = r#"\version "2.26.0"
#(define (probe g)
  (let* ((a (ly:grob-object g 'note-heads))
         (heads (if (ly:grob-array? a) (ly:grob-array->list a) '()))
         (r (ly:grob-object g 'rests))
         (rests (if (ly:grob-array? r) (ly:grob-array->list r) '()))
         (st (ly:grob-object g 'staff-symbol))
         (col (ly:grob-parent g X)))
    (write (list (ly:grob-property g 'direction)
                 (ly:grob-property g 'duration-log)
                 (ly:grob-property g 'thickness)
                 (ly:grob-property g 'note-collision-threshold)
                 (ly:grob-property st 'line-positions)
                 (ly:grob-property st 'staff-space)
                 (ly:grob-property g 'stem-begin-position)
                 (ly:grob-property g 'length)
                 (map (lambda (h) (list (ly:grob-staff-position h) (ly:grob-extent h h Y)
                                        (ly:grob-property h 'stem-attachment)
                                        (ly:grob-extent h h X) (ly:grob-property h 'style)))
                      heads)
                 (map (lambda (h) (ly:grob-extent h h X)) rests)
                 (ly:grob-property g 'X-offset)
                 (ly:grob-property g 'X-extent)
                 (ly:grob-property g 'Y-extent)
                 (map (lambda (h) (ly:grob-relative-coordinate h col X)) heads)
                 (let ((s (ly:grob-property g 'stencil)))
                   (if (ly:stencil? s)
                       (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (ly:stencil-expr s))
                       '()))))
    (newline)))
\layout { \context { \Voice \override Stem.after-line-breaking = #probe } }
<<
\new Staff { \autoBeamOff
  c'4 a'' <c' e' g'> <c' d'> <a'' b''> <c' d' e'> <f' g' a' b'> <c' c'> <c'' c''> <g' a'>2 <e'' f''>
  \stemUp <a'' b''>4 <c'' d'' e''> \stemDown <c' d'> <e' f' g'> \stemNeutral
  <c' d'>1 <c' d' e'> <c' g'\harmonic> <c' d'>\breve r4 r8 r1
  \override Stem.thickness = 2 c'4 a'' <c' d'> \revert Stem.thickness
  \override Stem.note-collision-threshold = 2 <c' e'>4 <c' e' g'> \revert Stem.note-collision-threshold
  \override NoteHead.style = #'mensural c'4 a'' \revert NoteHead.style
  \override NoteHead.style = #'petrucci c'4 a'' \revert NoteHead.style
}
\new Staff \with { \override StaffSymbol.staff-space = 1.5 } { \autoBeamOff c'4 <a' b'> a''2 }
>>
"#;

/// [`stem_case_vm`]'s VM with a paper column, and `(print-case line)`:
/// [`STEM_PRINT_PROBE`]'s line made again as a fresh stem with its
/// properties and stencil, its heads fresh pieces in the column with
/// their X extents and styles too, its rests fresh pieces of their X
/// extents; its offset, width, height, heads' X offsets in the column
/// once positioned, and print, as the probe writes them.
fn stem_print_vm() -> Option<Vm> {
    let mut vm = grobs_vm_of(
        DEFS,
        "'((Staff . 1) (Stem . 64) (Piece . 160) (Span . 1) (Column . 1))",
    )?;
    set_layout(&mut vm);
    // LilyPond 2.26.0's line thickness and blot at 20 points, 0.5 and 0.4
    // pt in staff spaces.
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'line-thickness 0.09999999999999999)
         (ly:output-def-set-variable! (ly:grob-layout (staff 0)) 'blot-diameter 0.08)
         (define stems 0)
         (define pieces 0)
         (define (new-piece)
           (set! pieces (1+ pieces))
           (piece (1- pieces)))
         (define (print-case line)
           (let ((g (stem stems)))
             (set! stems (1+ stems))
             (for-each (lambda (sym i) (put! g sym (list-ref line i)))
                       '(direction duration-log thickness note-collision-threshold
                         stem-begin-position length)
                       '(0 1 2 3 6 7))
             (put! g 'stencil ly:stem::print)
             (put! (staff 0) 'line-positions (list-ref line 4))
             (put! (staff 0) 'staff-space (list-ref line 5))
             (ly:grob-set-object! g 'staff-symbol (staff 0))
             (let ((heads
                    (map (lambda (h)
                           (let ((p (new-piece)))
                             (for-each (lambda (sym v) (put! p sym v))
                                       '(staff-position Y-extent stem-attachment X-extent style)
                                       h)
                             (ly:grob-set-object! p 'staff-symbol (staff 0))
                             (ly:grob-set-parent! p 0 (column 0))
                             p))
                         (list-ref line 8)))
                   (rests
                    (map (lambda (x) (let ((p (new-piece))) (put! p 'X-extent x) p))
                         (list-ref line 9))))
               (ly:grob-set-object! g 'note-heads (apply arr heads))
               (ly:grob-set-object! g 'rests (apply arr rests))
               (list (ly:stem::offset-callback g)
                     (ly:stem::width g)
                     (ly:stem::height g)
                     (begin
                       (ly:stem::calc-positioning-done g)
                       (map (lambda (h) (ly:grob-relative-coordinate h (column 0) 0)) heads))
                     (let ((s (ly:stem::print g)))
                       (if (null? s) s (%stencil->dump s)))))))",
    );
    Some(vm)
}

#[test]
fn a_stem_s_positioning_offset_width_height_and_print_are_lilypond_s() {
    let Some(mut vm) = stem_print_vm() else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-stem-print-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stem.ly"), STEM_PRINT_PROBE).unwrap();
    let Ok(out) = Command::new("lilypond")
        .args(["-dno-print-pages", "stem.ly"])
        .current_dir(&dir)
        .output()
    else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the stems' print is unverified");
        return;
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("warning"), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 34, "{lilypond}");
    for line in lines {
        let got = caught(&mut vm, &format!("(print-case '{line})"));
        let expected = caught(&mut vm, &format!("(list-tail '{line} 10)"));
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The accidental's callbacks and the accidental placement's (MS3 step
/// 39), each of the grob alone.
const ACCIDENTAL_CALLBACKS: [&str; 5] = [
    "ly:accidental-interface::height",
    "ly:accidental-interface::horizontal-skylines",
    "ly:accidental-interface::print",
    "ly:accidental-interface::remove-tied",
    "ly:accidental-placement::calc-positioning-done",
];

#[test]
fn the_accidental_s_and_the_placement_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in ACCIDENTAL_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

#[test]
fn a_tied_accidental_is_hidden_and_removed_as_lilypond_s() {
    // A tie is a grob; a broken one is a piece of a non-musical item
    // broken at a line break, whose original is the item.
    let Some(mut vm) = grobs_vm() else {
        return;
    };
    let sys = grob_of(&mut vm, "(group 5)");
    system::make(vm.heap_mut(), sys);
    eval(
        &mut vm,
        "(define sys (group 5))
         (put! (column 0) 'non-musical #t)
         (ly:grob-set-parent! (thing 0) 0 (column 0))
         (ly:grob-set-object! (column 0) 'elements (arr (thing 0)))",
    );
    for t in ["(column 0)", "(thing 0)"] {
        let g = grob_of(&mut vm, t);
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    let column = grob_of(&mut vm, "(column 0)");
    paper_column::set_rank(vm.heap(), column, 3);
    system::make_prebroken_pieces(&mut vm, sys).unwrap();
    eval(
        &mut vm,
        "(define broken
           (car (filter (lambda (p) (and (not (eq? p (thing 0)))
                                         (eq? (ly:grob-original p) (thing 0))))
                        (ly:grob-array->list (ly:grob-object sys 'all-elements)))))
         (define acc 0)
         (define (tied tie forced hide)
           (let ((a (piece acc)))
             (set! acc (1+ acc))
             (put! a 'stencil (ly:make-stencil '() '(0 . 1) '(-1 . 2)))
             (if tie (ly:grob-set-object! a 'tie tie))
             (put! a 'forced forced)
             (put! a 'hide-tied-accidental-after-break hide)
             (list (ly:accidental-interface::height a)
                   (begin (ly:accidental-interface::remove-tied a) (%grob-live? a)))))",
    );
    // The height is the stencil's, but a tied accidental's, not forced,
    // hidden after the break; it dies tied, not forced, if hidden or
    // its tie is not broken. Only #t is true.
    for (tie, forced, hide, expected) in [
        ("#f", "#f", "#f", "((-1.0 . 2.0) #t)"),
        ("#f", "#f", "#t", "((-1.0 . 2.0) #t)"),
        ("(thing 1)", "#f", "#f", "((-1.0 . 2.0) #f)"),
        ("(thing 1)", "#f", "#t", "((+inf.0 . -inf.0) #f)"),
        ("(thing 1)", "#t", "#t", "((-1.0 . 2.0) #t)"),
        ("(thing 1)", "'yes", "#t", "((+inf.0 . -inf.0) #f)"),
        ("(thing 1)", "#f", "'yes", "((-1.0 . 2.0) #f)"),
        ("broken", "#f", "#f", "((-1.0 . 2.0) #t)"),
        ("broken", "#f", "#t", "((+inf.0 . -inf.0) #f)"),
        ("'(1)", "#f", "#t", "((-1.0 . 2.0) #t)"),
    ] {
        assert_eq!(
            caught(&mut vm, &format!("(tied {tie} {forced} {hide})")),
            expected,
            "{tie} {forced} {hide}"
        );
    }
    // A dead accidental has no stencil, and so no height nor skylines.
    assert_eq!(
        caught(
            &mut vm,
            "(let ((a (piece 2)))
               (list (ly:accidental-interface::height a)
                     (ly:accidental-interface::horizontal-skylines a)))"
        ),
        "((+inf.0 . -inf.0) (#<Skyline> . #<Skyline>))"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

/// A score whose accidental placements write, after line breaking, a line
/// each: their X coordinate in the common refpoint of their heads and
/// stems, their `padding` and `right-padding`; their accidentals' note
/// columns' note collisions, each the columns it holds, and the columns,
/// each its collision, its heads' boxes and its stem's (the X extent in
/// that refpoint, the pure Y extent in the accidentals' common refpoint);
/// their `accidental-grobs`' entries, each accidental its pitch, glyph
/// name, `parenthesized`, `restore-first`, `rotation`, column and Y
/// coordinate; then, as placed, each accidental's X coordinate in the
/// placement, the placement's `X-extent`, each accidental's stencil (as
/// `tools/compare/stencils.ly`'s datum), its horizontal skylines' points
/// and its `Y-extent`. Sharps, flats, naturals, double ones, quarter
/// tones, a cautionary one in parentheses, chords of one note name in
/// octaves and in one octave, seconds both ways, restore-first, a
/// rotation, both paddings, two voices in a note collision, a cluster of
/// seven and a staff of 1.5 spaces.
const ACCIDENTAL_PROBE: &str = r#"\version "2.26.0"
#(use-modules (srfi srfi-1))
#(define (objs g sym)
  (let ((a (ly:grob-object g sym))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (index x l)
  (let loop ((l l) (i 0))
    (cond ((null? l) #f) ((eq? (car l) x) i) (else (loop (cdr l) (1+ i))))))
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (probe me)
  (let* ((entries (ly:grob-object me 'accidental-grobs))
         (accs (append-map cdr entries))
         (col-of (lambda (a) (ly:grob-parent (ly:grob-parent a Y) X)))
         (cols0 (delete-duplicates (map col-of accs) eq?))
         (colls (delete-duplicates
                 (filter (lambda (c) (grob::has-interface c 'note-collision-interface))
                         (map (lambda (c) (ly:grob-parent c X)) cols0))
                 eq?))
         (cols (delete-duplicates (append cols0 (append-map (lambda (c) (objs c 'elements)) colls)) eq?))
         (heads (append-map (lambda (c) (objs c 'note-heads)) cols))
         (stems (filter ly:grob? (map (lambda (c) (let ((h (objs c 'note-heads)))
                                                     (if (null? h) #f (ly:grob-object (car h) 'stem))))
                                      cols)))
         (hs (append heads stems))
         (cx (ly:grob-common-refpoint-of-array me (ly:grob-list->grob-array hs) X))
         (cy (ly:grob-common-refpoint-of-array (car accs) (ly:grob-list->grob-array (append accs hs)) Y))
         (box (lambda (g) (list (ly:grob-extent g cx X) (ly:grob-pure-height g cy 0 2147483647)))))
    (write
     (list
      (ly:grob-relative-coordinate me cx X)
      (ly:grob-property me 'padding)
      (ly:grob-property me 'right-padding)
      (map (lambda (c) (map (lambda (col) (index col cols)) (objs c 'elements))) colls)
      (map (lambda (col)
             (let* ((h (objs col 'note-heads))
                    (st (if (null? h) #f (ly:grob-object (car h) 'stem))))
               (list (index (ly:grob-parent col X) colls)
                     (map box h)
                     (if (ly:grob? st) (box st) #f))))
           cols)
      (map (lambda (e)
             (map (lambda (a)
                    (let ((p (ly:event-property (event-cause (ly:grob-parent a Y)) 'pitch)))
                      (list (ly:pitch-octave p) (ly:pitch-notename p) (ly:pitch-alteration p)
                            (ly:grob-property a 'glyph-name)
                            (ly:grob-property a 'parenthesized)
                            (ly:grob-property a 'restore-first)
                            (ly:grob-property a 'rotation)
                            (index (col-of a) cols)
                            (ly:grob-relative-coordinate a cy Y))))
                  (cdr e)))
           entries)
      (map (lambda (a) (ly:grob-relative-coordinate a me X)) accs)
      (ly:grob-property me 'X-extent)
      (map (lambda (a)
             (let ((s (ly:grob-property a 'stencil)))
               (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))))
           accs)
      (map (lambda (a)
             (let ((hs (ly:grob-property a 'horizontal-skylines)))
               (list (ly:skyline->points (car hs) Y) (ly:skyline->points (cdr hs) Y))))
           accs)
      (map (lambda (a) (ly:grob-property a 'Y-extent)) accs)))
    (newline)))
\layout { \context { \Staff \override AccidentalPlacement.after-line-breaking = #probe } }
<<
\new Staff { \autoBeamOff
  cis'4 des' e'! f'? <c' es' g'> <cis' cis''> <c' cis'> <ces' c'> <a' bes' cis'' d''>
  <f' fis'> <bes' ges'' es'''> <c'' eses''> cisis'' cih' ceseh' <cis' es' gis' bes' dis''>
  <ais' bes'> <fis' gis' ais'>
  \set Staff.extraNatural = ##t cisis'4 cis' \unset Staff.extraNatural
  <c' cis' ces'> <c'! c''!> <bes'? des''?> <eses' eses''>2
  \override Accidental.rotation = #'(30 0 0) fis'4 \revert Accidental.rotation
  \override Staff.AccidentalPlacement.right-padding = 0.5 ces''4 \revert Staff.AccidentalPlacement.right-padding
  \override Staff.AccidentalPlacement.padding = 0.4 <fis' gis'>4 \revert Staff.AccidentalPlacement.padding
  \stemUp <des'' es''> \stemDown <des' es'> \stemNeutral
  << { cis''4 ees'' } \\ { aes'4 c' } >>
  <cis' dis' eis' fis' gis' ais' bis'>1
}
\new Staff \with { \override StaffSymbol.staff-space = 1.5 } { \autoBeamOff fis'4 <bes' cis''> ges''2 }
>>
"#;

/// [`print_vm`]'s VM with the grobs [`ACCIDENTAL_PROBE`]'s lines need,
/// `Coll` a note collision, and `(acc-case line)`: the line's placement
/// made again with fresh grobs in a column, `(column 1)`, and on
/// `(thing 0)`: each column a note column, each head and stem an item of
/// its box, each accidental an item of the font of `(column 0)` with its
/// glyph name, `parenthesized`, `restore-first` and `rotation`, and the
/// accidental's callbacks, a child of the placement on X and of an item
/// on Y at its Y coordinate, in its column, caused by its pitch; then the
/// placement positioned, and what the probe writes of it as placed.
fn accidental_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Coll (meta (name . Coll) (classes Item)
                       (interfaces grob-interface item-interface note-collision-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 6) (Thing . 1) (Column . 2) (Staff . 1) (Note . 160) (Placement . 40) \
          (Piece . 1600) (Coll . 4))",
    )?;
    eval(
        &mut vm,
        "(define (coll n) (made-of 'Coll n))
         (define pieces 0)
         (define (new-piece) (set! pieces (1+ pieces)) (piece (1- pieces)))
         (define notes 0)
         (define (new-note) (set! notes (1+ notes)) (note (1- notes)))
         (define colls 0)
         (define (new-coll) (set! colls (1+ colls)) (coll (1- colls)))
         (define placements 0)
         (define (boxed g b)
           (ly:grob-set-parent! g 1 (thing 0))
           (put! g 'X-offset 0.0)
           (put! g 'Y-offset 0.0)
           (put! g 'X-extent (car b))
           (put! g 'Y-extent (cadr b))
           g)
         (define (acc-case line)
           (let* ((me (placement placements))
                  (coll-grobs (map (lambda (e) (new-coll)) (list-ref line 3)))
                  (cols
                   (map (lambda (c)
                          (let* ((col (new-note))
                                 (heads (map (lambda (b)
                                               (let ((h (boxed (new-piece) b)))
                                                 (ly:grob-set-parent! h 0 col)
                                                 h))
                                             (cadr c)))
                                 (st (and (caddr c)
                                          (let ((s (boxed (new-piece) (caddr c))))
                                            (ly:grob-set-parent! s 0 (column 1))
                                            s))))
                            (ly:grob-set-parent!
                             col 0 (if (car c) (list-ref coll-grobs (car c)) (column 1)))
                            (put! col 'X-offset 0.0)
                            (ly:grob-set-object! col 'note-heads (apply arr heads))
                            (if st (for-each (lambda (h) (ly:grob-set-object! h 'stem st)) heads))
                            col))
                        (list-ref line 4))))
             (set! placements (1+ placements))
             (for-each (lambda (c es)
                         (ly:grob-set-parent! c 0 (column 1))
                         (put! c 'X-offset 0.0)
                         (ly:grob-set-object!
                          c 'elements (apply arr (map (lambda (i) (list-ref cols i)) es))))
                       coll-grobs (list-ref line 3))
             (ly:grob-set-parent! me 0 (column 1))
             (put! me 'X-offset (list-ref line 0))
             (if (number? (list-ref line 1)) (put! me 'padding (list-ref line 1)))
             (put! me 'right-padding (list-ref line 2))
             (let* ((entries
                     (map (lambda (e)
                            (map (lambda (d)
                                   (let ((a (new-piece)) (anchor (new-piece)))
                                     (ly:grob-set-parent! anchor 0 (list-ref cols (list-ref d 7)))
                                     (ly:grob-set-parent! anchor 1 (thing 0))
                                     (put! anchor 'X-offset 0.0)
                                     (put! anchor 'Y-offset (list-ref d 8))
                                     (put! anchor 'cause
                                           (ly:make-stream-event
                                            '(note-event)
                                            (list (cons 'pitch (ly:make-pitch (list-ref d 0)
                                                                              (list-ref d 1)
                                                                              (list-ref d 2))))))
                                     (ly:grob-set-parent! a 0 me)
                                     (ly:grob-set-parent! a 1 anchor)
                                     (for-each (lambda (sym i) (put! a sym (list-ref d i)))
                                               '(glyph-name parenthesized restore-first rotation)
                                               '(3 4 5 6))
                                     (put! a 'fonts (ly:grob-property (column 0) 'fonts))
                                     (put! a 'stencil ly:accidental-interface::print)
                                     (put! a 'horizontal-skylines
                                           ly:accidental-interface::horizontal-skylines)
                                     (put! a 'X-offset ly:grob::x-parent-positioning)
                                     (put! a 'X-extent ly:grob::stencil-width)
                                     (put! a 'Y-extent ly:accidental-interface::height)
                                     a))
                                 e))
                          (list-ref line 5)))
                    (accs (apply append entries)))
               (ly:grob-set-object! me 'accidental-grobs
                                    (map cons (iota (length entries)) entries))
               (put! me 'positioning-done ly:accidental-placement::calc-positioning-done)
               (ly:grob-property me 'positioning-done)
               (list (map (lambda (a) (ly:grob-relative-coordinate a me 0)) accs)
                     (ly:grob-property me 'X-extent)
                     (map (lambda (a) (%stencil->dump (ly:grob-property a 'stencil))) accs)
                     (map (lambda (a)
                            (let ((hs (ly:grob-property a 'horizontal-skylines)))
                              (list (ly:skyline->points (car hs) 1)
                                    (ly:skyline->points (cdr hs) 1))))
                          accs)
                     (map (lambda (a) (ly:grob-property a 'Y-extent)) accs)))))",
    );
    Some(vm)
}

#[test]
fn the_accidentals_are_printed_and_placed_as_lilypond_places_them() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the accidentals are unverified");
        return;
    };
    let Some(mut vm) = accidental_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-accidental-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("acc.ly"), ACCIDENTAL_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "acc.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("warning"), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 34, "{lilypond}");
    // The cases the probe means to cover are there.
    for (what, count) in [
        ("(0.0 () 0.15 ((1 0))", 1),
        ("(0.0 () 0.5 ", 1),
        ("(0.0 0.4 0.15 ", 1),
        ("(30 0 0)", 1),
    ] {
        assert_eq!(
            lines.iter().filter(|l| l.contains(what)).count(),
            count,
            "{what}"
        );
    }
    for glyph in [
        "flatflat",
        "doublesharp",
        "leftparen",
        "mirroredflat.flat",
        "natural",
    ] {
        assert!(
            lilypond.contains(&format!("\"accidentals.{glyph}\"")),
            "{glyph}"
        );
    }
    assert!(
        lines.iter().any(|l| l.contains(" () #t () ")),
        "restore-first"
    );
    for line in lines {
        let got = caught(&mut vm, &format!("(acc-case '{line})"));
        let expected = caught(&mut vm, &format!("(list-tail '{line} 6)"));
        assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn an_accidental_without_its_glyph_warns_and_prints_nothing() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = accidental_vm(packs) else {
        return;
    };
    let print = |vm: &mut Vm, props: &str| {
        caught(
            vm,
            &format!(
                "(let ((a (new-piece)))
                   (put! a 'fonts (ly:grob-property (column 0) 'fonts))
                   (for-each (lambda (p) (put! a (car p) (cdr p))) '({props}))
                   (let ((s (ly:accidental-interface::print a)))
                     (list (ly:stencil-empty? s) (ly:stencil-extent s 0))))"
            ),
        )
    };
    // No glyph: LilyPond's warning and nothing printed, though a natural
    // restoring or parentheses are added to nothing.
    assert_eq!(
        print(&mut vm, "(glyph-name . \"accidentals.nonesuch\")"),
        "(#t (+inf.0 . -inf.0))"
    );
    assert_eq!(print(&mut vm, "(glyph-name . 5)"), "(#t (+inf.0 . -inf.0))");
    let got = print(
        &mut vm,
        "(glyph-name . \"accidentals.nonesuch\") (restore-first . #t)",
    );
    assert!(got.starts_with("(#f "), "{got}");
    let warned = diagnosed(&mut vm);
    assert_eq!(
        warned
            .matches("cannot find glyph accidentals.nonesuch")
            .count(),
        2,
        "{warned}"
    );
    assert_eq!(
        warned.matches("cannot find glyph \"").count(),
        1,
        "{warned}"
    );
    // `parenthesized` and `restore-first` only when #t.
    let plain = print(&mut vm, "(glyph-name . \"accidentals.sharp\")");
    assert_eq!(
        print(
            &mut vm,
            "(glyph-name . \"accidentals.sharp\") (parenthesized . yes) (restore-first . 1)"
        ),
        plain
    );
}

/// The accidentals' glyph pages.scm (stencils.ly) draws at `x`, unlinked.
fn accidental_glyph(pages: &str, x: f64) -> &str {
    let found: Vec<&str> = pages
        .match_indices("(translate-stencil (")
        .filter(|(i, _)| {
            let at = &pages[i + "(translate-stencil (".len()..];
            let drawn: f64 = at[..at.find(' ').unwrap()].parse().unwrap();
            (drawn - x).abs() <= 5e-5
        })
        .map(|(i, _)| form(pages, i))
        .filter_map(|f| {
            let g = f.find("(grob-cause #f (named-glyph ")?;
            let glyph = form(f, g + "(grob-cause #f ".len());
            glyph.contains("\"accidentals.").then_some(glyph)
        })
        .collect();
    assert_eq!(found.len(), 1, "{x}");
    found[0]
}

#[test]
fn the_accidentals_print_is_lilypond_s_on_y1_s_five() {
    // Each Accidental of the five's layout.tsv (T19's four), given the
    // glyph pages.scm (stencils.ly) draws at its X: printed, the
    // expression pages.scm's and the extents layout.tsv's, its height
    // the Y extent layout.tsv's, within layout.mjs' TOLERANCE.
    let refs = corpus().join("ref");
    if FIVE.iter().any(|id| {
        !refs.join(id).join("layout.tsv").exists() || !refs.join(id).join("pages.scm").exists()
    }) {
        eprintln!("skipped: no layout.tsv or pages.scm in {}", refs.display());
        return;
    }
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is unverified");
        return;
    };
    let Some(mut vm) = accidental_vm(packs) else {
        return;
    };
    let extent = |e: &str| match e.split_once(' ') {
        Some((l, r)) => format!("({l} . {r})"),
        None => "(+inf.0 . -inf.0)".into(),
    };
    let mut accidentals = Vec::new();
    for id in FIVE {
        let pages = std::fs::read_to_string(refs.join(id).join("pages.scm")).unwrap();
        let tsv = std::fs::read_to_string(refs.join(id).join("layout.tsv")).unwrap();
        for line in tsv.lines().filter(|l| l.starts_with("Accidental\t")) {
            let l: Vec<&str> = line.split('\t').collect();
            let glyph = accidental_glyph(&pages, l[8].parse().unwrap());
            let name = glyph.rsplit('"').nth(1).unwrap();
            let got = caught(
                &mut vm,
                &format!(
                    "(let ((a (new-piece)))
                       (put! a 'fonts (ly:grob-property (column 0) 'fonts))
                       (put! a 'glyph-name \"{name}\")
                       (put! a 'stencil ly:accidental-interface::print)
                       (list (%stencil->dump (ly:accidental-interface::print a))
                             (ly:accidental-interface::height a)))"
                ),
            );
            let expected = format!(
                "(({} {} {glyph}) {})",
                extent(l[10]),
                extent(l[11]),
                extent(l[11])
            );
            assert!(
                same_within(&otf_names(&got), &expected),
                "{id} {line}:\n{got}\n{expected}"
            );
            accidentals.push(format!("{id} {name}"));
        }
    }
    assert_eq!(
        accidentals,
        [
            "T19 accidentals.flat",
            "T19 accidentals.natural",
            "T19 accidentals.flat",
            "T19 accidentals.sharp",
        ]
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

/// The ledger line spanner's callbacks (MS3 step 39), each of the grob
/// alone.
const LEDGER_CALLBACKS: [&str; 2] = [
    "ly:ledger-line-spanner::print",
    "ly:ledger-line-spanner::set-spacing-rods",
];

#[test]
fn the_ledger_line_spanner_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in LEDGER_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

/// A score whose ledger line spanners write a line for their rods, as
/// `springs-and-rods`, and one for each piece's print, as its `stencil`.
/// Each writes its staff symbol (the layout's `staff-space` and
/// `line-thickness`; its `staff-space`, `thickness`, `line-positions`,
/// `ledger-line-thickness`, `ledger-positions`, `ledger-extra` and
/// `ledger-positions-function`; its Y extent) and its heads, each its
/// column's rank, its rounded staff position, whether it is an ambitus's
/// head, its X extent (in its column for the rods, in the common refpoint
/// for the print), its `ledger-positions` and `ledger-extra`, and its
/// accidental's X extent there, glyph name, `parenthesized` and
/// `font-size`. A rods line: `(rods staff minimum-length-fraction ranks
/// heads made live)`, `made` the rods added to each column of `ranks`
/// (each column's `minimum-distances` emptied for the call and the rods
/// then put back as `add_rod` would have added them, so LilyPond lays out
/// as it would); a print line: `(print staff gap length-fraction x heads
/// stencil)`, `x` the spanner's X in the common refpoint, the stencil as
/// `tools/compare/stencils.ly`'s datum. Ledgers above and below, chords
/// across the staff, seconds, whole notes and a breve, accidentals (one
/// cautionary, in parentheses; a double flat; a second's; below the staff,
/// a ledger two positions off the edge of a glyph's shortening range),
/// heads close together, beside heads in the first space beyond the staff
/// without ledgers, a head's own `ledger-positions` and `ledger-extra`, a staff
/// of 1.5 spaces with thicker ledgers and its spanner's three numbers
/// set, one of three lines with `ledger-extra` and a pattern of ledgers,
/// an ambitus, `\tiny` accidentals, odd line positions with a thickness
/// of 2 and a ledger thickness of `(1.1 . 0.07)`; the first staff broken
/// in two.
const LEDGER_PROBE: &str = r#"\version "2.26.0"
#(use-modules (srfi srfi-1))
#(define (objs g sym)
  (let ((a (ly:grob-object g sym))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (rank c) (car (ly:grob-spanned-column-rank-interval c)))
#(define (staff-data me)
  (let ((st (ly:grob-object me 'staff-symbol)) (layout (ly:grob-layout me)))
    (list (ly:output-def-lookup layout 'staff-space)
          (ly:output-def-lookup layout 'line-thickness)
          (map (lambda (p) (ly:grob-property st p))
               '(staff-space thickness line-positions ledger-line-thickness ledger-positions
                 ledger-extra ledger-positions-function))
          (ly:grob-extent st st Y))))
#(define (head-data h cx)
  (let ((acc (ly:grob-object h 'accidental-grob))
        (cx (or cx (ly:item-get-column h))))
    (list (rank (ly:item-get-column h))
          (inexact->exact (round (ly:grob-staff-position h)))
          (grob::has-interface h 'ambitus-interface)
          (ly:grob-extent h cx X)
          (ly:grob-property h 'ledger-positions)
          (ly:grob-property h 'ledger-extra)
          (and (ly:grob? acc)
               (list (ly:grob-extent acc cx X)
                     (ly:grob-property acc 'glyph-name)
                     (ly:grob-property acc 'parenthesized)
                     (ly:grob-property acc 'font-size))))))
#(define (rods-probe me)
  (let* ((heads (objs me 'note-heads))
         (cols (delete-duplicates (map ly:item-get-column heads) eq?))
         (saved (map (lambda (c) (ly:grob-object c 'minimum-distances)) cols))
         (r (begin
              (for-each (lambda (c) (ly:grob-set-object! c 'minimum-distances '())) cols)
              (ly:ledger-line-spanner::set-spacing-rods me)))
         (made (map (lambda (c) (ly:grob-object c 'minimum-distances)) cols)))
    ;; The rods made put back as Spaceable_grob::add_rod would have added
    ;; them to what was there: the larger of two to one column, else first.
    (for-each
     (lambda (c old new)
       (ly:grob-set-object!
        c 'minimum-distances
        (fold (lambda (e mins)
                (let ((there (assq (car e) mins)))
                  (if there
                      (begin (set-cdr! there (max (cdr there) (cdr e))) mins)
                      (cons (cons (car e) (cdr e)) mins))))
              old (reverse new))))
     cols saved made)
    (write (list 'rods (staff-data me)
                 (ly:grob-property me 'minimum-length-fraction)
                 (map rank cols)
                 (map (lambda (h) (head-data h #f)) heads)
                 (map (lambda (m) (map (lambda (e) (cons (rank (car e)) (cdr e))) m)) made)
                 (grob::is-live? me)))
    (newline)
    r))
#(define (print-probe me)
  (let* ((s (ly:ledger-line-spanner::print me))
         (heads (objs me 'note-heads))
         (accs (filter ly:grob? (map (lambda (h) (ly:grob-object h 'accidental-grob)) heads)))
         (cx (ly:grob-common-refpoint-of-array me (ly:grob-list->grob-array (append heads accs)) X)))
    (write (list 'print (staff-data me)
                 (ly:grob-property me 'gap)
                 (ly:grob-property me 'length-fraction)
                 (ly:grob-relative-coordinate me cx X)
                 (map (lambda (h) (head-data h cx)) heads)
                 (if (ly:stencil? s)
                     (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
                     s)))
    (newline)
    s))
\layout {
  \context { \Staff
    \override LedgerLineSpanner.springs-and-rods = #rods-probe
    \override LedgerLineSpanner.stencil = #print-probe
  }
}
<<
  \new Staff { \autoBeamOff
    c'''4 a'' b'' <c d'''> g'' a,4 c'1 <c' d'> <a'' b''> <c''' d'''>2 e'''8 f''' g'''16 a''' b''' c''''
    <g, a,>4 <c e'''>
    cis'4 bes'' aes'''? f'? deses' eses''' <cis' d'> <bes'' ces'''> \breve c''''1
    ais4 bes aes ges eses fisis, cis, ees, aih, beh d''' fis''' bes''' cis''''
  }
  \new Staff { \autoBeamOff
    c'''64 d''' c''' d''' a, b, a, b, <c a''> <d b''> <c a''> <d b''> c''''32 b''' a''' g''' c,64 d, e, f,
    \override NoteHead.ledger-positions = #'(8 10) e'''4 \revert NoteHead.ledger-positions
    \override NoteHead.ledger-extra = 1 g''4 e'4 \revert NoteHead.ledger-extra
    ais''64 b'' cis''' d''' eis''' fis''' gis''' a'''
    a''64 g'' a'' g'' c' d' c' d' b'' g'' b'' g'' a' f' a' f'
  }
  \new Staff \with {
    \override StaffSymbol.staff-space = 1.5
    \override StaffSymbol.ledger-line-thickness = #'(2 . 0.2)
    \override LedgerLineSpanner.gap = 0.3
    \override LedgerLineSpanner.length-fraction = 0.4
    \override LedgerLineSpanner.minimum-length-fraction = 0.5
  } { \autoBeamOff c'''16 d''' e''' f''' a, g, f, e, cis'4 aes'' <c' e'''>2 }
  \new Staff \with {
    \override StaffSymbol.line-count = 3
    \override StaffSymbol.ledger-extra = 1
    \override StaffSymbol.ledger-positions = #'(-6 (-4 -3) 0)
  } { \autoBeamOff c'''8 a'' f'' d'' b' g' e' c' a, c, }
  \new Staff \with { \consists Ambitus_engraver } { \autoBeamOff c'4 a'' d''' a, }
  \new Staff \with { \tiny } { \autoBeamOff cis'8 bes'' aes'''? <cis' d'> eses'''4 c'''' }
  \new Staff \with { \override StaffSymbol.thickness = 2 \override StaffSymbol.line-positions = #'(-3 0 3) \override StaffSymbol.ledger-line-thickness = #'(1.1 . 0.07) } { \autoBeamOff c'4 c''' g, b'' }
>>
"#;

/// [`print_vm`]'s VM with `Ledger`, a ledger line spanner, `Amb`, an
/// ambitus's head, and `(ledger-ranks line)` and `(ledger-case line)` of
/// one of [`LEDGER_PROBE`]'s lines: its columns' ranks, which the test
/// gives `(column 1)` on; and its spanner made again with fresh grobs on
/// `(thing 0)`: a staff symbol of its properties and Y extent, each head
/// an item (an `Amb` for an ambitus's) of its X extent on the column of
/// its rank at its staff position, with its own ledger properties and its
/// accidental, an item of its X extent on `(thing 0)` with the font of
/// `(column 0)`; then for a rods line what `set-spacing-rods` adds to each
/// column and whether the spanner lives, for a print line its stencil.
fn ledger_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Ledger (meta (name . Ledger) (classes Spanner)
                         (interfaces grob-interface spanner-interface
                                     ledger-line-spanner-interface)))
        (Amb (meta (name . Amb) (classes Item)
                   (interfaces grob-interface item-interface ambitus-interface
                               ledgered-interface note-head-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 6) (Thing . 1) (Column . 80) (Staff . 20) (Piece . 800) (Ledger . 20) \
          (Amb . 8))",
    )?;
    eval(
        &mut vm,
        "(define (ledger n) (made-of 'Ledger n))
         (define (amb n) (made-of 'Amb n))
         (define counts '())
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (assq-set! counts kind (1+ n)))
             (made-of kind n)))
         (define (line-heads line) (list-ref line (if (eq? (car line) 'rods) 4 5)))
         (define ranks '())
         (define (ledger-ranks line)
           (set! ranks (sort (delete-duplicates (map car (line-heads line))) <))
           ranks)
         (define (column-of rank) (column (1+ (list-index (lambda (r) (= r rank)) ranks))))
         (define (rank-of col) (car (ly:grob-spanned-column-rank-interval col)))
         (define (ledger-case line)
           (let* ((data (cadr line))
                  (props (caddr data))
                  (st (fresh 'Staff))
                  (me (fresh 'Ledger))
                  (layout (ly:grob-layout (column 0)))
                  (space (* (car data) (if (number? (car props)) (car props) 1.0)))
                  (cols (map column-of ranks)))
             (ly:output-def-set-variable! layout 'staff-space (car data))
             (ly:output-def-set-variable! layout 'line-thickness (cadr data))
             (for-each (lambda (sym v) (if (not (null? v)) (put! st sym v)))
                       '(staff-space thickness line-positions ledger-line-thickness
                         ledger-positions ledger-extra ledger-positions-function)
                       props)
             (put! st 'Y-extent (cadddr data))
             (ly:grob-set-parent! st 1 (thing 0))
             (put! st 'Y-offset 0.0)
             (for-each (lambda (c)
                         (ly:grob-set-parent! c 0 (thing 0))
                         (put! c 'X-offset 0.0)
                         (ly:grob-set-object! c 'minimum-distances '()))
                       cols)
             (let ((heads
                    (map (lambda (d)
                           (let ((h (fresh (if (caddr d) 'Amb 'Piece)))
                                 (acc (list-ref d 6)))
                             (ly:grob-set-parent! h 0 (column-of (car d)))
                             (put! h 'X-offset 0.0)
                             (put! h 'X-extent (cadddr d))
                             (ly:grob-set-parent! h 1 (thing 0))
                             (put! h 'Y-offset (* (cadr d) space 0.5))
                             (ly:grob-set-object! h 'staff-symbol st)
                             (if (pair? (list-ref d 4)) (put! h 'ledger-positions (list-ref d 4)))
                             (if (number? (list-ref d 5)) (put! h 'ledger-extra (list-ref d 5)))
                             (if acc
                                 (let ((a (fresh 'Piece)))
                                   (ly:grob-set-parent! a 0 (thing 0))
                                   (put! a 'X-offset 0.0)
                                   (put! a 'X-extent (car acc))
                                   (if (string? (cadr acc)) (put! a 'glyph-name (cadr acc)))
                                   (put! a 'parenthesized (caddr acc))
                                   (if (number? (cadddr acc)) (put! a 'font-size (cadddr acc)))
                                   (put! a 'fonts (ly:grob-property (column 0) 'fonts))
                                   (ly:grob-set-object! h 'accidental-grob a)))
                             h))
                         (line-heads line))))
               (ly:grob-set-parent! me 0 (thing 0))
               (ly:grob-set-object! me 'note-heads (apply arr heads))
               (ly:grob-set-object! me 'staff-symbol st)
               (if (eq? (car line) 'rods)
                   (begin
                     (if (number? (caddr line)) (put! me 'minimum-length-fraction (caddr line)))
                     (ly:ledger-line-spanner::set-spacing-rods me)
                     (list (map (lambda (c)
                                  (map (lambda (e) (cons (rank-of (car e)) (cdr e)))
                                       (ly:grob-object c 'minimum-distances)))
                                cols)
                           (%grob-live? me)))
                   (begin
                     (if (number? (caddr line)) (put! me 'gap (caddr line)))
                     (if (number? (cadddr line)) (put! me 'length-fraction (cadddr line)))
                     (put! me 'X-offset (list-ref line 4))
                     (let ((s (ly:ledger-line-spanner::print me)))
                       (if (ly:stencil? s) (%stencil->dump s) s)))))))",
    );
    Some(vm)
}

#[test]
fn the_ledger_lines_are_printed_and_spaced_as_lilypond_s() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the ledger lines are unverified");
        return;
    };
    let Some(mut vm) = ledger_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-ledger-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ledger.ly"), LEDGER_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "ledger.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("warning"), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    // Seven staves' rods, and their prints, the first staff's in two.
    let count = |kind: &str| lines.iter().filter(|l| l.starts_with(kind)).count();
    assert_eq!((count("(rods "), count("(print ")), (7, 8), "{lilypond}");
    // The cases the probe means to cover are there: ambitus heads,
    // accidentals in and out of parentheses, a head's own ledgers, rods
    // and ledgers shortened by accidentals and neighbours.
    for what in [
        "(ambitus-interface",
        "\"accidentals.flatflat\" ()",
        "\"accidentals.flat\" #t",
        "(8 10) ()",
        "() 1 #f",
        "(-6 (-4 -3) 0) 1",
        "(1.1 . 0.07)",
    ] {
        assert!(lilypond.contains(what), "{what}");
    }
    for line in lines {
        let ranks = caught(&mut vm, &format!("(ledger-ranks '{line})"));
        for (i, rank) in ranks
            .trim_matches(['(', ')'])
            .split_whitespace()
            .enumerate()
        {
            let col = grob_of(&mut vm, &format!("(column {})", i + 1));
            paper_column::set_rank(vm.heap(), col, rank.parse().unwrap());
        }
        let got = caught(&mut vm, &format!("(ledger-case '{line})"));
        let expected = if line.starts_with("(rods ") {
            caught(&mut vm, &format!("(list-tail '{line} 5)"))
        } else {
            caught(&mut vm, &format!("(list-ref '{line} 6)"))
        };
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_ledger_line_spanner_without_heads_or_a_staff_symbol_draws_nothing() {
    let Some(mut vm) = grobs_vm() else { return };
    eval(
        &mut vm,
        "(define (ledgers me) (list (ly:ledger-line-spanner::print me) (%grob-live? me)))
         (put! (thing 1) 'staff-position 2)",
    );
    // No heads: nothing; heads but no staff symbol: nothing, and the rods
    // kill the spanner.
    assert_eq!(caught(&mut vm, "(ledgers (group 1))"), "(() #t)");
    eval(
        &mut vm,
        "(ly:grob-set-object! (group 1) 'note-heads (arr (thing 1)))",
    );
    assert_eq!(caught(&mut vm, "(ledgers (group 1))"), "(() #t)");
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:ledger-line-spanner::set-spacing-rods (group 1)) (%grob-live? (group 1)))"
        ),
        "(() #f)"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

// --- side positions (MS3 step 40) -------------------------------------------

/// The side position callbacks, each with the arguments LilyPond's subr
/// prints; `::calc-cross-staff` and `::set-axis!` since MS3 step 48.
const SIDE_POSITION_CALLBACKS: [(&str, &str); 6] = [
    ("ly:side-position-interface::calc-cross-staff", "_"),
    ("ly:side-position-interface::move-to-extremal-staff", "_"),
    ("ly:side-position-interface::set-axis!", "_ _"),
    (
        "ly:side-position-interface::pure-y-aligned-side",
        "_ _ _ #:optional _",
    ),
    (
        "ly:side-position-interface::x-aligned-side",
        "_ #:optional _",
    ),
    (
        "ly:side-position-interface::y-aligned-side",
        "_ #:optional _",
    ),
];

#[test]
fn the_side_position_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, args) in SIDE_POSITION_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(
            caught(&mut vm, name),
            format!("#<procedure {name} ({args})>")
        );
        // The grob's type error, named by the method as LilyPond's
        // `LY_ASSERT_SMOB` names it (MS3 step 48): `ly:y-aligned-side`.
        let rest = if name.contains("pure") {
            " 0 0"
        } else if name.contains("set-axis") {
            " 0"
        } else {
            ""
        };
        let got = caught(&mut vm, &format!("({name} 'x{rest})"));
        let subr = format!("In procedure ly:{}:", name.split("::").nth(1).unwrap());
        assert!(
            got.contains(&subr)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        let too_many = if name.contains("pure") {
            "'x 0 0 0 0"
        } else if name.contains("extremal") || name.contains("cross-staff") {
            "'x 'x"
        } else {
            "'x 0 0"
        };
        for args in ["", too_many] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
    // The pure one's columns, `from_scm<vsize>`: Guile's errors from no
    // procedure, the grob checked first.
    let pure = "ly:side-position-interface::pure-y-aligned-side";
    let got = caught(&mut vm, &format!("({pure} (thing 0) 'a 0)"));
    assert!(got.contains("exact integer"), "{got}");
    let got = caught(&mut vm, &format!("({pure} (thing 0) 0 -1)"));
    assert!(got.contains("out of range"), "{got}");
}

/// A score whose side-positioned grobs (`TextScript`, `Fingering` above,
/// below, left and right of their heads, `StringNumber`, `Arpeggio` and
/// `SystemStartBar`) each write, after line breaking, one line for each
/// side position the probe asks of them: on Y pure (from column 0 to
/// `INT_MAX`) and not, on X as `x-aligned-side` takes it. Each grob and
/// its supports have the skylines of their extents
/// (`ly:grob::simple-…-skylines-from-extents`), so that fresh grobs of
/// those extents have theirs. A line: `(name axis pure start end
/// direction props cross-staff on-head x-parent x y x-extent y-extent
/// supports proto staff results)`: `props` the maybe-pure
/// `quantize-position`, `staff-padding`, `horizon-padding`, `padding`,
/// `X-padding`, `minimum-space`, `minimum-X-space` and
/// `add-stem-support`; `on-head` whether its X parent is a note head; then
/// the coordinates `aligned_side` reads in its common refpoints (its X
/// parent's on X, its own on X, its Y parent's maybe-pure on Y or its own
/// pure on X) and its own extents (Y maybe-pure); `supports` its support
/// set, each its `cross-staff`, whether a stem, its `direction` data if a
/// direction (read before the calls) and its `direction`, whether a
/// spanner, its coordinates there and its own extents; `proto` its
/// `side-support-elements`, each an index into `supports` or, for an
/// accidental placement, its `accidental-grobs`' entries of indices;
/// `staff` its staff symbol, if any: whether `aligned_side`'s common
/// refpoint on Y is its own with the staff, the staff's maybe-pure
/// coordinate there and own extent, the staff space and `line-positions`;
/// `results` the callback's offset without a current offset, after 2.5 and
/// after -2.5. Texts up and down, padded, kept a minimum space, with
/// horizon padding, without staff padding and centered (direction 0);
/// fingerings over chords, beamed with their stems' support, quantized
/// between the lines, and beside chords with accidentals (whose placement
/// is a support); string numbers, quantized over their heads too;
/// arpeggios; the system start bar (a spanner, never pure on X); a staff
/// of 1.5 spaces.
const SIDE_PROBE: &str = r#"\version "2.26.0"
#(use-modules (srfi srfi-1))
#(define INF 2147483647)
#(define sv (ly:make-unpure-pure-container ly:grob::simple-vertical-skylines-from-extents
                                           ly:grob::pure-simple-vertical-skylines-from-extents))
#(define sh (ly:make-unpure-pure-container ly:grob::simple-horizontal-skylines-from-extents
                                           ly:grob::pure-simple-horizontal-skylines-from-extents))
#(define (objs g sym)
  (let ((a (ly:grob-object g sym))) (if (ly:grob-array? a) (ly:grob-array->list a) '())))
#(define (support-set me)
  (delete-duplicates
   (append-map (lambda (g)
                 (if (grob::has-interface g 'accidental-placement-interface)
                     (append-map cdr (ly:grob-object g 'accidental-grobs))
                     (list g)))
               (objs me 'side-support-elements))
   eq?))
#(define (common-of gs start a)
  (fold (lambda (g c) (if (ly:grob? c) (ly:grob-common-refpoint c g a) g)) start gs))
#(define (prop me sym pure start end)
  (if pure (ly:grob-pure-property me sym start end) (ly:grob-property me sym)))
#(define (ycoord g c pure start end)
  (if pure (ly:grob-pure-relative-coordinate g c start end) (ly:grob-relative-coordinate g c Y)))
#(define (yext g pure start end)
  (if pure (ly:grob-pure-height g g start end) (ly:grob-extent g g Y)))
#(define (parent-rel g c a)
  (let ((p (ly:grob-parent g a))) (if (ly:grob? p) (ly:grob-relative-coordinate p c a) 0.0)))
#(define (call me a pure start end . cur)
  (cond ((= a X) (apply ly:side-position-interface::x-aligned-side me cur))
        (pure (apply ly:side-position-interface::pure-y-aligned-side me start end cur))
        (else (apply ly:side-position-interface::y-aligned-side me cur))))
#(define (case me a pure start end)
  (let* ((supp (support-set me))
         (dirdata (map (lambda (e) (ly:grob-property-data e 'direction)) supp))
         (sk (if (= a X) 'horizontal-skylines 'vertical-skylines))
         (cached (map (lambda (g) (pair? (ly:grob-property-data g sk))) (cons me supp)))
         (results (list (call me a pure start end) (call me a pure start end 2.5)
                        (call me a pure start end -2.5)))
         (cx (common-of supp (if (= a X) (ly:grob-parent me X) me) X))
         (cy0 (common-of supp (if (= a Y) (ly:grob-parent me Y) me) Y))
         (staff (let ((s (ly:grob-object me 'staff-symbol))) (and (ly:grob? s) s)))
         (quant (eq? #t (prop me 'quantize-position pure start end)))
         (incl (and staff (= a Y) (number? (prop me 'staff-padding pure start end)) (not quant)))
         (cy (if incl (ly:grob-common-refpoint staff cy0 Y) cy0))
         (head (ly:grob-parent me X))
         (index (lambda (g) (list-index (lambda (x) (eq? x g)) supp))))
    (write
     (list
      (grob::name me) a pure start end (ly:grob-property me 'direction)
      (map (lambda (s) (prop me s pure start end))
           '(quantize-position staff-padding horizon-padding padding X-padding minimum-space
             minimum-X-space add-stem-support))
      (ly:grob-property me 'cross-staff)
      (and (ly:grob? head) (grob::has-interface head 'note-head-interface) #t)
      (if (= a X) (parent-rel me cx X) 0.0)
      (if (or (= a X) pure) 0.0 (ly:grob-relative-coordinate me cx X))
      (if (= a X)
          (ly:grob-pure-relative-coordinate me cy start end)
          (ycoord (ly:grob-parent me Y) cy pure start end))
      (ly:grob-extent me me X)
      (yext me pure start end)
      (car cached)
      (ly:grob-extent me me Y)
      (map (lambda (e d c)
             (list (ly:grob-property e 'cross-staff)
                   (and (grob::has-interface e 'stem-interface) #t)
                   (and (number? d) d)
                   (ly:grob-property e 'direction)
                   (ly:spanner? e)
                   (ly:grob-relative-coordinate e cx X)
                   (if (= a X)
                       (ly:grob-pure-relative-coordinate e cy start end)
                       (ycoord e cy pure start end))
                   (ly:grob-extent e e X)
                   (yext e pure start end)
                   c
                   (ly:grob-extent e e Y)))
           supp dirdata (cdr cached))
      (map (lambda (g)
             (if (grob::has-interface g 'accidental-placement-interface)
                 (map (lambda (e) (cons (car e) (map index (cdr e))))
                      (ly:grob-object g 'accidental-grobs))
                 (index g)))
           (objs me 'side-support-elements))
      (and staff
           (list (eq? cy (ly:grob-common-refpoint me staff Y))
                 (ycoord staff cy pure start end)
                 (yext staff pure start end)
                 (ly:staff-symbol-staff-space me)
                 (ly:grob-property staff 'line-positions)))
      results))
    (newline)))
#(define (probe-axis a)
  (lambda (me)
    (if (= a Y)
        (begin (case me Y #t 0 INF) (case me Y #f 0 0))
        (case me X (not (ly:spanner? me)) 0 0))))
#(define (probe-side me)
  ((probe-axis (ly:grob-property me 'side-axis)) me))
\layout {
  \context {
    \Score
    \override NoteHead.vertical-skylines = #sv
    \override NoteHead.horizontal-skylines = #sh
    \override Stem.vertical-skylines = #sv
    \override Stem.horizontal-skylines = #sh
    \override Flag.vertical-skylines = #sv
    \override NoteColumn.vertical-skylines = #sv
    \override NoteColumn.horizontal-skylines = #sh
    \override Dots.vertical-skylines = #sv
    \override Accidental.vertical-skylines = #sv
    \override Accidental.horizontal-skylines = #sh
    \override AccidentalPlacement.vertical-skylines = #sv
    \override Script.vertical-skylines = #sv
    \override Rest.vertical-skylines = #sv
    \override TextScript.vertical-skylines = #sv
    \override Fingering.vertical-skylines = #sv
    \override Fingering.horizontal-skylines = #sh
    \override StringNumber.vertical-skylines = #sv
    \override StringNumber.horizontal-skylines = #sh
    \override Arpeggio.horizontal-skylines = #sh
    \override SystemStartBar.horizontal-skylines = #sh
    \override TextScript.after-line-breaking = #(probe-axis Y)
    \override Fingering.after-line-breaking = #probe-side
    \override StringNumber.after-line-breaking = #probe-side
    \override Arpeggio.after-line-breaking = #(probe-axis X)
    \override SystemStartBar.after-line-breaking = #(probe-axis X)
  }
}
<<
  \new Staff {
    \autoBeamOff
    c'4^"up" d'_"down" a''^"hi" e_"lo"
    <c' e' g'>-1-3-5 <c'' e''>4^"chord" c'''8^"x" g16_"y" r16
    \set fingeringOrientations = #'(left) <c'-1 e'-2>4 <fis'-3 a'-4>
    \set fingeringOrientations = #'(right) <d'-3 f'-4>
    \unset fingeringOrientations
    <c' e' g'>\arpeggio <cis' fis' b'>\arpeggio c'\2 b'\3
    \override TextScript.padding = 1.2 c'4^"pad" b'_"pad"
    \override TextScript.minimum-space = 3 c'^"min" g''_"min" \revert TextScript.minimum-space
    \override TextScript.horizon-padding = 0.3 c'^"hor" \revert TextScript.horizon-padding
    \override TextScript.staff-padding = ##f c'^"nostaff" d'''_"nostaff" \revert TextScript.staff-padding
    \override TextScript.add-stem-support = ##t c'4^"stem" a''^"stem" g'_"stem" \revert TextScript.add-stem-support
    \override TextScript.direction = #CENTER c'4-"center" \revert TextScript.direction
    \override Fingering.quantize-position = ##t c'4-1 e'-2 g'^3 c''-4 g''_5 c'''-1 a-2
    \revert Fingering.quantize-position
    \override StringNumber.quantize-position = ##t c'\2 a''\3 e\4 c''\1 \revert StringNumber.quantize-position
    \override TextScript.padding = -20 c'^"neg" \revert TextScript.padding
    \autoBeamOn c'8[-1 d'-2] a''[_1 g''_2] \override Fingering.add-stem-support = ##t e''8-1 f''-2 \autoBeamOff
  }
  \new Staff \with { \override StaffSymbol.staff-space = 1.5 } {
    \autoBeamOff c'4^"wide" e''_"wide" <c' e'>-1-2 g'4\arpeggio
    \override Fingering.quantize-position = ##t f'-1 a''-2
    \revert Fingering.quantize-position
    \set fingeringOrientations = #'(left) <c'-1 e'-2>4 \unset fingeringOrientations
    \override TextScript.padding = 0.7 c'^"odd" \revert TextScript.padding
    \stemUp e'4_1 \stemDown g''^2 \stemNeutral
  }
>>
"#;

/// [`grobs_vm_of`]'s VM, its layout's `staff-space` 1, with `Nh`, a note
/// head item, and `(side-case line)`: [`SIDE_PROBE`]'s line made again with
/// fresh grobs on one root on both axes, and the three results of the
/// line's callback for the grob made. The grob is a `Delim` spanner for
/// `SystemStartBar`, else a `Piece`; its X parent (a note head if
/// LilyPond's is) and its Y parent are at the line's coordinates, its
/// properties, extents and `direction` the line's; each support is a
/// `Stem` or a `Piece` at its coordinates, with its extents, `cross-staff`
/// and, for a stem, its `direction` data, or a procedure giving its
/// direction; each placement a `Placement` with its `accidental-grobs`;
/// the staff a `Staff` at its coordinate, with its extent, `staff-space`
/// and `line-positions`.
fn side_vm() -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Nh (meta (name . Nh) (classes Item)
                     (interfaces grob-interface item-interface note-head-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = grobs_vm_of(
        &defs,
        "'((Staff . 120) (Stem . 120) (Placement . 10) (Piece . 1600) (Delim . 4) (Nh . 40))",
    )?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        "(define (nh n) (made-of 'Nh n))
         (define counts '())
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (acons kind (1+ n) counts))
             (made-of kind n)))
         (define side-props
           '(quantize-position staff-padding horizon-padding padding X-padding minimum-space
             minimum-X-space add-stem-support))
         (define (side-call me a pure start end . cur)
           (cond ((= a 0) (apply ly:side-position-interface::x-aligned-side me cur))
                 (pure (apply ly:side-position-interface::pure-y-aligned-side me start end cur))
                 (else (apply ly:side-position-interface::y-aligned-side me cur))))
         (define (side-extents! g a xext yext cached uyext)
           (put! g 'X-extent xext)
           (when cached
             (put! g 'Y-extent uyext)
             (if (= a 0)
                 (put! g 'horizontal-skylines (ly:grob::simple-horizontal-skylines-from-extents g))
                 (put! g 'vertical-skylines (ly:grob::simple-vertical-skylines-from-extents g))))
           (put! g 'Y-extent yext))
         (define (side-case line)
           (let* ((ref (lambda (i) (list-ref line i)))
                  (a (ref 1)) (pure (ref 2)) (start (ref 3)) (end (ref 4))
                  (root (fresh 'Piece))
                  (me (fresh (if (eq? (ref 0) 'SystemStartBar) 'Delim 'Piece)))
                  (xp (fresh (if (ref 8) 'Nh 'Piece)))
                  (yp (fresh 'Piece))
                  (supp
                   (map (lambda (s)
                          (let ((e (fresh (if (list-ref s 1) 'Stem 'Piece)))
                                (d (list-ref s 3)))
                            (ly:grob-set-parent! e 0 root)
                            (ly:grob-set-parent! e 1 root)
                            (put! e 'cross-staff (list-ref s 0))
                            (if (list-ref s 1)
                                (put! e 'direction (or (list-ref s 2) (lambda (g) d))))
                            (put! e 'X-offset (list-ref s 5))
                            (put! e 'Y-offset (list-ref s 6))
                            (side-extents! e a (list-ref s 7) (list-ref s 8) (list-ref s 9)
                                           (list-ref s 10))
                            e))
                        (ref 16)))
                  (st (ref 18)))
             (put! root 'X-offset 0.0)
             (put! root 'Y-offset 0.0)
             (ly:grob-set-parent! xp 0 root)
             (put! xp 'X-offset (ref 9))
             (ly:grob-set-parent! yp 1 root)
             (put! yp 'Y-offset (ref 11))
             (ly:grob-set-parent! me 0 xp)
             (ly:grob-set-parent! me 1 yp)
             (put! me 'X-offset (ref 10))
             (put! me 'Y-offset 0.0)
             (put! me 'direction (ref 5))
             (for-each (lambda (s v) (put! me s v)) side-props (ref 6))
             (put! me 'cross-staff (ref 7))
             (side-extents! me a (ref 12) (ref 13) (ref 14) (ref 15))
             (ly:grob-set-object!
              me 'side-support-elements
              (apply arr
                     (map (lambda (p)
                            (if (integer? p)
                                (list-ref supp p)
                                (let ((pl (fresh 'Placement)))
                                  (ly:grob-set-object!
                                   pl 'accidental-grobs
                                   (map (lambda (e)
                                          (cons (car e) (map (lambda (i) (list-ref supp i)) (cdr e))))
                                        p))
                                  pl)))
                          (ref 17))))
             (if st
                 (let ((s (fresh 'Staff)))
                   (ly:grob-set-parent! s 1 root)
                   (put! s 'Y-offset (list-ref st 1))
                   (put! s 'Y-extent (list-ref st 2))
                   (put! s 'staff-space (list-ref st 3))
                   (put! s 'line-positions (list-ref st 4))
                   (ly:grob-set-object! me 'staff-symbol s)))
             (list (side-call me a pure start end)
                   (side-call me a pure start end 2.5)
                   (side-call me a pure start end -2.5))))",
    );
    Some(vm)
}

#[test]
fn side_positions_are_lilypond_s() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the side positions are unverified");
        return;
    }
    let Some(mut vm) = side_vm() else { return };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-side-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("side.ly"), SIDE_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "side.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 113, "{lilypond}");
    // The cases the probe means to cover are there: each grob on its
    // axis, pure and not; a placement among the supports, with its
    // accidental there too and a head twice; quantized fingerings, and
    // quantized string numbers over their note heads; stem support; no
    // direction; a staff of 1.5 spaces; the system start bar.
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    for (what, at_least) in [
        ("(TextScript 1 #t ", 20),
        ("(TextScript 1 #f ", 20),
        ("(Fingering 1 #t ", 10),
        ("(Fingering 0 #t ", 4),
        ("(StringNumber 1 ", 4),
        ("(Arpeggio 0 #t ", 3),
        ("(SystemStartBar 0 #f ", 1),
        (" (((3 . 1) 3)))", 2),
        (" (0 1 2 0 3 ", 1),
        ("(#t 0.5 () 0.5 ", 20),
        ("(#t 0.5 () 0.5 () () () #f) #f #t ", 6),
        ("() () #t) ", 4),
        ("(TextScript 1 #f 0 0 0 ", 1),
        (" 1.5 (4.0 2.0 0.0 -2.0 -4.0)", 8),
    ] {
        assert!(count(what) >= at_least, "{what}: {}", count(what));
    }
    for line in lines {
        // On Y, the staff's coordinates are in the side position's common
        // refpoint; no support is a spanner.
        let checked = caught(
            &mut vm,
            &format!(
                "(let ((l '{line}))
                   (and (or (= (list-ref l 1) 0) (not (list-ref l 18)) (car (list-ref l 18)))
                        (not (memq #t (map (lambda (s) (list-ref s 4)) (list-ref l 16))))))"
            ),
        );
        assert_eq!(checked, "#t", "{line}");
        let got = caught(&mut vm, &format!("(side-case '{line})"));
        let expected = caught(&mut vm, &format!("(list-ref '{line} 19)"));
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn an_improbable_side_position_is_a_programming_error() {
    let Some(mut vm) = side_vm() else { return };
    let line = "(TextScript 1 #f 0 0 1 (() () () 1200 () () () ()) #f #f 0.0 0.0 0.0 \
                (0.0 . 1.0) (0.0 . 1.0) #f (0.0 . 1.0) () () #f)";
    assert_eq!(
        caught(&mut vm, &format!("(side-case '{line})")),
        "(1200.0 1200.0 1200.0)"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Improbable offset for grob Piece: 1200.000000\" \
          \"Improbable offset for grob Piece: 1200.000000\" \
          \"Improbable offset for grob Piece: 1200.000000\")"
    );
    // With `strict-infinity-checking`, LilyPond's error.
    eval(
        &mut vm,
        "(set! %options (acons 'strict-infinity-checking #t %options))",
    );
    let got = caught(&mut vm, &format!("(side-case '{line})"));
    assert!(
        got.contains("aligned_side") && got.contains("Improbable offset."),
        "{got}"
    );
    // Without a direction, the current offset or 0, nothing read.
    let line = "(TextScript 1 #f 0 0 0 (() () () 1200 () () () ()) #f #f 0.0 0.0 0.0 \
                (0.0 . 1.0) (0.0 . 1.0) #f (0.0 . 1.0) () () #f)";
    assert_eq!(
        caught(&mut vm, &format!("(side-case '{line})")),
        "(0.0 2.5 -2.5)"
    );
    // A grob without a Y parent is Guile's type error.
    let got = caught(
        &mut vm,
        "(let ((me (fresh 'Piece)))
           (put! me 'direction 1)
           (ly:side-position-interface::y-aligned-side me))",
    );
    assert!(got.contains("Wrong type"), "{got}");
}

#[test]
fn a_stem_pointing_away_is_no_support_on_y() {
    let Some(mut vm) = side_vm() else { return };
    // A grob above a stem from 0 to 10: on it, padded by nothing, unless
    // the stem points down, away from it; then on nothing, at 0.
    let line = |stem_dir: i32| {
        format!(
            "(TextScript 1 #f 0 0 1 (() () () () () () () ()) #f #f 0.0 0.0 0.0 \
             (0.0 . 1.0) (0.0 . 1.0) #f (0.0 . 1.0) \
             ((#f #t {stem_dir} {stem_dir} #f 0.0 0.0 (0.0 . 1.0) (0.0 . 10.0) #f (0.0 . 10.0))) \
             (0) #f #f)"
        )
    };
    assert_eq!(
        caught(&mut vm, &format!("(side-case '{})", line(1))),
        "(10.0 10.0 10.0)"
    );
    assert_eq!(
        caught(&mut vm, &format!("(side-case '{})", line(-1))),
        "(0.0 2.5 0.0)"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_mark_moves_to_the_extremal_staff_of_its_grouper_or_system() {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Grouper (meta (name . Grouper) (classes Spanner)
                          (interfaces grob-interface spanner-interface staff-grouper-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let made = MADE.replace("(Span . 1)", "(Span . 1) (Grouper . 1)");
    let Some(mut vm) = grobs_vm_of(&defs, &made) else {
        return;
    };
    // The grouper and the system's alignment, thing 5, align groups 0, 1
    // and 2, from 0 to 2, 3 to 5 and 1 to 4 on column 0, which is on the
    // system, group 5; thing 0 is on column 0 from 2 to 3, supported by
    // things 1 (on group 0) and 2 (on group 2).
    eval(
        &mut vm,
        "(define grouper (made-of 'Grouper 0))
         (for-each (lambda (n ext)
                     (ly:grob-set-parent! (group n) 0 (column 0))
                     (put! (group n) 'X-extent ext)
                     (put! (group n) 'axes '(1)))
                   '(0 1 2) '((0 . 2) (3 . 5) (1 . 4)))
         (ly:grob-set-object! grouper 'elements (arr (group 0) (group 1) (group 2)))
         (ly:grob-set-object! (thing 5) 'elements (arr (group 0) (group 1) (group 2)))
         (ly:grob-set-object! (group 5) 'vertical-alignment (thing 5))
         (ly:grob-set-parent! (column 0) 0 (group 5))
         (put! (group 5) 'X-offset 0.0)
         (put! (column 0) 'X-offset 0.0)
         (ly:grob-set-parent! (thing 0) 0 (column 0))
         (put! (thing 0) 'X-offset 0.0)
         (put! (thing 0) 'X-extent '(2 . 3))
         (ly:grob-set-parent! (thing 1) 1 (group 0))
         (ly:grob-set-parent! (thing 2) 1 (group 2))
         (define (moved me)
           (list (ly:side-position-interface::move-to-extremal-staff me)
                 (ly:grob-parent me 1)
                 (map (lambda (g)
                        (let ((a (ly:grob-object g 'elements)))
                          (and (ly:grob-array? a) (memq me (ly:grob-array->list a)) #t)))
                      (list (group 0) (group 1) (group 2)))
                 (ly:grob-array->list (ly:grob-object me 'side-support-elements))))
         (define (reset! me parent dir)
           (ly:grob-set-parent! me 1 parent)
           (put! me 'direction dir)
           (ly:grob-set-object! me 'side-support-elements (arr (thing 1) (thing 2))))",
    );
    let col = grob_of(&mut vm, "(column 0)");
    let sys = grob_of(&mut vm, "(group 5)");
    paper_column::set_system(vm.heap(), col, sys);
    // On the grouper, up (or no direction): its first staff meeting 1 to
    // 4, the supports on other staves dropped.
    eval(&mut vm, "(reset! (thing 0) grouper '())");
    assert_eq!(
        caught(&mut vm, "(moved (thing 0))"),
        "(#t #<Grob Group > (#t #f #f) (#<Grob Thing >))"
    );
    assert_eq!(
        caught(&mut vm, "(eq? (ly:grob-parent (thing 0) 1) (group 0))"),
        "#t"
    );
    assert_eq!(
        caught(
            &mut vm,
            "(eq? (car (ly:grob-array->list (ly:grob-object (thing 0) 'side-support-elements))) (thing 1))"
        ),
        "#t"
    );
    // Down: the last.
    eval(&mut vm, "(reset! (thing 0) grouper -1)");
    caught(&mut vm, "(moved (thing 0))");
    assert_eq!(
        caught(&mut vm, "(eq? (ly:grob-parent (thing 0) 1) (group 2))"),
        "#t"
    );
    assert_eq!(
        caught(
            &mut vm,
            "(map (lambda (g) (eq? g (thing 2))) (ly:grob-array->list (ly:grob-object (thing 0) 'side-support-elements)))"
        ),
        "(#t)"
    );
    // On the system: its vertical alignment's.
    eval(&mut vm, "(reset! (thing 0) (group 5) 1)");
    assert_eq!(caught(&mut vm, "(car (moved (thing 0)))"), "#t");
    assert_eq!(
        caught(&mut vm, "(eq? (ly:grob-parent (thing 0) 1) (group 0))"),
        "#t"
    );
    // On another parent, or meeting no staff: not moved.
    eval(&mut vm, "(reset! (thing 0) (thing 4) 1)");
    assert_eq!(caught(&mut vm, "(car (moved (thing 0)))"), "#f");
    assert_eq!(
        caught(&mut vm, "(eq? (ly:grob-parent (thing 0) 1) (thing 4))"),
        "#t"
    );
    eval(
        &mut vm,
        "(ly:grob-set-parent! (thing 3) 0 (column 0))
         (put! (thing 3) 'X-offset 20.0)
         (put! (thing 3) 'X-extent '(0 . 1))
         (reset! (thing 3) grouper 1)",
    );
    assert_eq!(caught(&mut vm, "(car (moved (thing 3)))"), "#f");
    assert_eq!(
        caught(&mut vm, "(eq? (ly:grob-parent (thing 3) 1) grouper)"),
        "#t"
    );
    // The system without a vertical alignment: not moved.
    eval(
        &mut vm,
        "(reset! (thing 0) (group 5) 1)
         (ly:grob-set-object! (group 5) 'vertical-alignment '())",
    );
    assert_eq!(caught(&mut vm, "(car (moved (thing 0)))"), "#f");
    assert_eq!(diagnosed(&mut vm), "()");
}

// --- clefs and key signatures (MS3 step 40) ---------------------------------

/// The clef's and the key signature's callbacks (MS3 step 40), each of the
/// grob alone.
const CLEF_KEY_CALLBACKS: [&str; 3] = [
    "ly:clef::calc-glyph-name",
    "ly:clef::print",
    "ly:key-signature-interface::print",
];

#[test]
fn the_clef_s_and_the_key_signature_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in CLEF_KEY_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

/// A score whose clefs, key signatures and key cancellations each write a
/// line as their stencil is printed: a clef `(clef dir (glyph non-default
/// full-size-change font-size glyph-name) stencil)`, a key signature `(key
/// dir cancellation staff-space (alteration-alist c0-position
/// alteration-glyph-name-alist padding padding-pairs flat-positions
/// sharp-positions font-size) stencil)`, `dir` its break direction and
/// the stencil as `tools/compare/stencils.ly`'s datum `(x y expr)`.
/// LilyPond prints the prebroken pieces too, so that each change has its
/// three. Keys of one to seven sharps and flats and none, minor, in the
/// treble, bass, alto and tenor clefs, cancelled by each other; clefs
/// changed, transposed, mensural, percussion, resized and full-size; a
/// glyph name not in the font; keys by octave, of quarter tones and double
/// flats, with padding, padding pairs, flat and sharp positions and a font
/// size set; alterations whose glyph is not in the font or not in the
/// alist; a staff of 1.5 spaces.
const CLEF_KEY_PROBE: &str = r#"\version "2.26.0"
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (stil s)
  (if (ly:stencil? s)
      (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
      s))
#(define (clef-probe g)
  (let ((s (ly:clef::print g)))
    (write (list 'clef (ly:item-break-dir g)
                 (map (lambda (p) (ly:grob-property g p))
                      '(glyph non-default full-size-change font-size glyph-name))
                 (stil s)))
    (newline)
    s))
#(define (key-probe g)
  (let ((s (ly:key-signature-interface::print g)))
    (write (list 'key (ly:item-break-dir g)
                 (and (grob::has-interface g 'key-cancellation-interface) #t)
                 (ly:staff-symbol-staff-space g)
                 (map (lambda (p) (ly:grob-property g p))
                      '(alteration-alist c0-position alteration-glyph-name-alist padding padding-pairs
                        flat-positions sharp-positions font-size))
                 (stil s)))
    (newline)
    s))
\layout { \context { \Staff
  \override Clef.stencil = #clef-probe
  \override KeySignature.stencil = #key-probe
  \override KeyCancellation.stencil = #key-probe } }
<<
\new Staff {
  \key g \major c'1 \key d \major c' \key cis \major c' \key ces \major c' \break
  \key e \major c' \key c \major c' \key f \minor c' \clef bass \key b \major c'
  \key bes \major c' \clef alto \key fis \major c' \clef tenor \key fis \major c' \key ges \major c' \break
  \clef treble c'1 \clef bass \break c'1 \key a \major \clef "treble_8" c' \clef "GG" c'
  \clef percussion c' \clef "varC" c' \clef "petrucci-c2" c' \clef "mensural-f" \key c \major c'
  \set Staff.keyAlterations = #`((0 . ,SHARP) (1 . ,FLAT) (6 . ,DOUBLE-FLAT) (3 . ,SEMI-SHARP)) c'
  \set Staff.keyAlterations = #`(((1 . 2) . ,SHARP) ((-1 . 6) . ,FLAT) ((0 . 4) . ,NATURAL)) c'
  \unset Staff.keyAlterations \key a \major c'
  \override Staff.KeySignature.padding = 0.6 \key e \major c' \revert Staff.KeySignature.padding
  \override Staff.KeyCancellation.padding-pairs = #'((("accidentals.natural" . "accidentals.natural") . 0.7))
  \key c \major c' \revert Staff.KeyCancellation.padding-pairs
  \override Staff.KeySignature.flat-positions = #'((-5 . 5)) \key aes \major c' \revert Staff.KeySignature.flat-positions
  \override Staff.KeySignature.sharp-positions = #'(2 (-1 . 3)) \key gis \minor c' \revert Staff.KeySignature.sharp-positions
  \override Staff.KeySignature.font-size = -3 \key b \major c' \revert Staff.KeySignature.font-size
  \override Staff.Clef.font-size = 2 \clef bass c' \revert Staff.Clef.font-size
  \override Staff.Clef.full-size-change = ##t \clef treble c' \revert Staff.Clef.full-size-change
  \override Staff.Clef.glyph-name = "clefs.nonesuch" \clef bass c' \revert Staff.Clef.glyph-name
  \override Staff.KeySignature.alteration-glyph-name-alist =
    #'((1/2 . "accidentals.nonesuch") (0 . "accidentals.natural"))
  \key d \major c' \key bes \major c'
  \revert Staff.KeySignature.alteration-glyph-name-alist
}
\new Staff \with { \override StaffSymbol.staff-space = 1.5 } { \clef bass \key es \major c1 \key e \major c' \key es \major c' }
>>
"#;

/// [`print_vm`]'s VM with `output-lib.scm`'s
/// `key-signature-interface::alteration-positions` as the installed file
/// defines it; `(clef-case line)`: the glyph name `ly:clef::calc-glyph-name`
/// gives of the piece of a clef broken at a line break of the line's
/// direction (the original for 0) given its `glyph`, `non-default` and
/// `full-size-change`, and what `ly:clef::print` prints of a fresh clef of
/// the line's `glyph-name` and `font-size`; and `(key-case line)`, what
/// `ly:key-signature-interface::print` prints of a fresh key signature or
/// cancellation of the line's properties on a staff of its staff space.
fn clef_key_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Clef (meta (name . Clef) (classes Item)
                       (interfaces grob-interface item-interface font-interface clef-interface)))
        (Key (meta (name . Key) (classes Item)
                   (interfaces grob-interface item-interface font-interface
                               key-signature-interface)))
        (Cancel (meta (name . Cancel) (classes Item)
                      (interfaces grob-interface item-interface font-interface
                                  key-signature-interface key-cancellation-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 6) (Thing . 1) (Column . 2) (Staff . 160) (Clef . 60) (Key . 120) \
          (Cancel . 60))",
    )?;
    let text = std::fs::read_to_string(lily_scm().unwrap().join("output-lib.scm")).unwrap();
    let start = text
        .find("(define-public (key-signature-interface::alteration-positions")
        .unwrap();
    eval(&mut vm, form(&text, start));
    // A clef broken at a line break: the column of rank 3 a line break.
    let sys = grob_of(&mut vm, "(group 5)");
    system::make(vm.heap_mut(), sys);
    eval(
        &mut vm,
        "(define sys (group 5))
         (put! (column 1) 'non-musical #t)
         (ly:grob-set-parent! (made-of 'Clef 0) 0 (column 1))
         (ly:grob-set-object! (column 1) 'elements (arr (made-of 'Clef 0)))",
    );
    for t in ["(column 1)", "(made-of 'Clef 0)"] {
        let g = grob_of(&mut vm, t);
        system::typeset_grob(&mut vm, sys, g).unwrap();
    }
    let column = grob_of(&mut vm, "(column 1)");
    paper_column::set_rank(vm.heap(), column, 3);
    system::make_prebroken_pieces(&mut vm, sys).unwrap();
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (column 0)) 'staff-space 1.0)
         (define broken-clef
           (let ((all (ly:grob-array->list (ly:grob-object sys 'all-elements))))
             (map (lambda (d)
                    (find (lambda (p) (and (or (eq? p (made-of 'Clef 0))
                                               (eq? (ly:grob-original p) (made-of 'Clef 0)))
                                           (= (ly:item-break-dir p) d)))
                          all))
                  '(-1 0 1))))
         (define counts (list (cons 'Clef 1)))
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (assq-set! counts kind (1+ n)))
             (made-of kind n)))
         (define (dumped s) (if (ly:stencil? s) (%stencil->dump s) s))
         (define (clef-case line)
           (let ((props (caddr line))
                 (piece (list-ref broken-clef (1+ (cadr line))))
                 (c (fresh 'Clef)))
             (for-each (lambda (sym v) (put! piece sym v))
                       '(glyph non-default full-size-change)
                       (list (car props) (cadr props) (caddr props)))
             (put! c 'fonts (ly:grob-property (column 0) 'fonts))
             (if (number? (cadddr props)) (put! c 'font-size (cadddr props)))
             (put! c 'glyph-name (list-ref props 4))
             (list (ly:clef::calc-glyph-name piece) (dumped (ly:clef::print c)))))
         (define (key-case line)
           (let ((props (list-ref line 4))
                 (st (fresh 'Staff))
                 (k (fresh (if (caddr line) 'Cancel 'Key))))
             (put! st 'staff-space (cadddr line))
             (ly:grob-set-object! k 'staff-symbol st)
             (put! k 'fonts (ly:grob-property (column 0) 'fonts))
             (for-each (lambda (sym v) (if (not (null? v)) (put! k sym v)))
                       '(alteration-alist c0-position alteration-glyph-name-alist padding
                         padding-pairs flat-positions sharp-positions font-size)
                       props)
             (dumped (ly:key-signature-interface::print k))))",
    );
    Some(vm)
}

/// The warnings of [`CLEF_KEY_PROBE`]'s key signatures and clef, each with
/// the number of its lines that give it.
const CLEF_KEY_WARNINGS: [(&str, usize); 3] = [
    ("alteration not found", 6),
    ("No glyph found for alteration: -1/2", 6),
    ("clef `clefs.nonesuch' not found", 3),
];

#[test]
fn clefs_and_key_signatures_are_printed_as_lilypond_prints_them() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the prints are unverified");
        return;
    };
    let Some(mut vm) = clef_key_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-clef-key-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("clef-key.ly"), CLEF_KEY_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "clef-key.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    // LilyPond prints some stencils more than once: each line once.
    let mut lines: Vec<&str> = lilypond.lines().collect();
    let printed = lines.len();
    lines.sort_unstable();
    lines.dedup();
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    // The cases the probe means to cover are there: each break direction
    // of clefs and keys, cancellations, a key by octave, quarter tones,
    // the paddings, the positions, a font size and the staff space.
    for (what, n) in [
        ("(clef -1 ", 11),
        ("(clef 0 ", 11),
        ("(clef 1 ", 15),
        ("(key -1 #t ", 16),
        ("(key 0 #f ", 25),
        ("(key 1 #f ", 30),
        ("((((0 . 4) . 0) ", 5),
        ("(3 . 1/4)", 5),
        ("\"clefs.G\" #t #t", 3),
        ("\"clefs.F\" #t () 2 ", 3),
        ("\"clefs.nonesuch\")", 3),
        ("\")) 0.6 () ", 3),
        ("\"accidentals.natural\") . 0.7)", 2),
        ("((-5 . 5))", 3),
        ("(2 (-1 . 3))", 3),
        (" 1.5 (", 10),
        (" -3) ((", 3),
    ] {
        assert_eq!(count(what), n, "{what}");
    }
    let lines_of = |what: &str| lines.iter().filter(|l| l.starts_with(what)).count();
    assert_eq!([lines_of("(clef "), lines_of("(key ")], [37, 111]);
    for line in &lines {
        if line.starts_with("(clef ") {
            // The glyph name set by the score is not the one calculated:
            // only the print of those.
            let case = if line.contains("\"clefs.nonesuch\"") {
                format!(
                    "(let ((r (clef-case '{line}))) (list (list-ref (caddr '{line}) 4) (cadr r)))"
                )
            } else {
                format!("(clef-case '{line})")
            };
            let got = caught(&mut vm, &case);
            let expected = caught(
                &mut vm,
                &format!("(let ((l '{line})) (list (list-ref (caddr l) 4) (cadddr l)))"),
            );
            assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
        } else {
            let got = caught(&mut vm, &format!("(key-case '{line})"));
            let expected = caught(&mut vm, &format!("(list-ref '{line} 5)"));
            assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
        }
    }
    // Each line's warnings: LilyPond's for each print, the engine's for
    // each line.
    let diagnostics = diagnosed(&mut vm);
    for (warning, count) in CLEF_KEY_WARNINGS {
        assert!(stderr.contains(warning), "{warning}: {stderr}");
        assert_eq!(
            diagnostics.matches(warning).count(),
            count,
            "{warning}: {diagnostics}"
        );
    }
    assert!(printed > lines.len());
}

#[test]
fn a_clef_without_a_glyph_dies_and_one_without_a_glyph_name_prints_nothing() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = clef_key_vm(packs) else {
        return;
    };
    // No glyph: the clef is killed, its glyph name unspecified.
    assert_eq!(
        caught(
            &mut vm,
            "(let ((c (fresh 'Clef)))
               (put! c 'glyph 'treble)
               (list (ly:clef::calc-glyph-name c) (%grob-live? c)))"
        ),
        "(#<unspecified> #f)"
    );
    // `non-default` and `full-size-change` only when #t; a clef at the
    // end of a line or in mid-line changes, one at the start does not.
    for (piece, props, expected) in [
        (0, "'yes '()", "\"clefs.G\""),
        (0, "#t 'yes", "\"clefs.G_change\""),
        (1, "#t #f", "\"clefs.G_change\""),
        (2, "#t #f", "\"clefs.G\""),
    ] {
        let got = caught(
            &mut vm,
            &format!(
                "(let ((p (list-ref broken-clef {piece})))
                   (for-each (lambda (s v) (put! p s v))
                             '(glyph non-default full-size-change)
                             (list \"clefs.G\" {props}))
                   (ly:clef::calc-glyph-name p))"
            ),
        );
        assert_eq!(got, expected, "{piece} {props}");
    }
    // No glyph name: nothing, and no warning.
    assert_eq!(
        caught(
            &mut vm,
            "(let ((c (fresh 'Clef)))
               (put! c 'glyph-name 5)
               (ly:clef::print c))"
        ),
        "()"
    );
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_key_signature_warns_of_a_missing_glyph_as_lilypond_does() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = clef_key_vm(packs) else {
        return;
    };
    let print = |vm: &mut Vm, props: &str| {
        caught(
            vm,
            &format!(
                "(let ((k (fresh 'Key)))
                   (put! k 'fonts (ly:grob-property (column 0) 'fonts))
                   (put! k 'c0-position -6)
                   (for-each (lambda (p) (put! k (car p) (cdr p))) {props})
                   (let ((s (ly:key-signature-interface::print k)))
                     (list (ly:stencil-empty? s) (ly:stencil-extent s 0))))"
            ),
        )
    };
    // An alteration without a glyph, each as Rational::to_string writes
    // it; a glyph not in the font; then the rest printed, aligned left.
    let got = print(
        &mut vm,
        "`((alteration-alist . ((0 . 3/4) (1 . -2) (2 . 0) (3 . 1/2)))
           (alteration-glyph-name-alist . ((1/2 . \"accidentals.sharp\")
                                           (0 . \"accidentals.nonesuch\"))))",
    );
    assert!(got.starts_with("(#f (0.0 . "), "{got}");
    let warned = diagnosed(&mut vm);
    for (warning, n) in [
        ("No glyph found for alteration: 3/4", 1),
        ("No glyph found for alteration: -2", 1),
        ("No glyph found for alteration: ", 2),
        ("alteration not found", 1),
    ] {
        assert_eq!(warned.matches(warning).count(), n, "{warning}: {warned}");
    }
    // No alterations: an empty stencil.
    assert_eq!(print(&mut vm, "'()"), "(#t (+inf.0 . -inf.0))");
    assert_eq!(diagnosed(&mut vm), "()");
}

// --- clef modifiers (MS3 step 42) ------------------------------------------

#[test]
fn the_clef_modifier_s_callback_is_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:clef-modifier::calc-parent-alignment";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(
        music.iter().all(|p| p.name != name),
        "{name} is no placeholder"
    );
    // LilyPond's subr, as it prints.
    assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
    let got = caught(&mut vm, &format!("({name} 'x)"));
    assert!(
        got.contains(name) && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
        "{got}"
    );
    for args in ["", "'x 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
}

/// The clef modifier's cases, `(glyph direction clef-alignments)`, the
/// clef's glyph and the modifier's properties, each of a type LilyPond's
/// type checks let through: down, up, centred and no direction; an entry
/// that is no pair, a list, none, an alist without the clef, one with it
/// twice (after a different key), a change clef's own entry, a glyph
/// without `clefs.`, with it twice, with it around another, and empty.
const CLEF_MODIFIER_CASES: &str = r#"'(("clefs.G" -1 ((G . (-0.2 . 0.1))))
    ("clefs.G" 1 ((G . (-0.2 . 0.1))))
    ("clefs.G" 0 ((G . (-0.2 . 0.1))))
    ("clefs.G" () ((G . (-0.2 . 0.1))))
    ("clefs.F" -1 ((G . (-0.2 . 0.1)) (F . (-0.3 . -0.2)) (C . (0 . 0))))
    ("clefs.F" 1 ((G . (-0.2 . 0.1)) (F . (-0.3 . -0.2)) (C . (0 . 0))))
    ("clefs.C" 1 ((G . (-0.2 . 0.1)) (F . (-0.3 . -0.2)) (C . (0 . 0))))
    ("clefs.G" -1 ((G . 0.5)))
    ("clefs.G" -1 ((G 3 4)))
    ("clefs.G" 1 ((G 3 4)))
    ("clefs.G" -1 ((F . (1 . 2))))
    ("clefs.G" -1 ())
    ("clefs.G" -1 ((F . (1 . 2)) (G . (a . b)) (G . (3 . 4))))
    ("clefs.F_change" 1 ((F_change . (1 . 2)) (F . (3 . 4))))
    ("G" 1 ((G . (1 . 2))))
    ("clefs.clefs.G" 1 ((G . (1 . 2))))
    ("cleclefs.fs.C" 1 ((clefs.C . (1 . 2)) (C . (3 . 4))))
    ("" 1 ((|| . (1 . 2)))))"#;

/// A score whose first clef modifier, as its `parent-alignment-X` is
/// read, writes a line for each of [`CLEF_MODIFIER_CASES`] set on it and
/// its clef: `(case alignment)`, the alignment
/// `ly:clef-modifier::calc-parent-alignment` gives, or the error it
/// throws as `(key args)`.
fn clef_modifier_probe() -> String {
    format!(
        r#"\version "2.26.0"
#(define cases {CLEF_MODIFIER_CASES})
#(define done #f)
#(define (probe g)
  (if (not done)
      (let ((c (ly:grob-parent g X)))
        (set! done #t)
        (for-each
         (lambda (case)
           (write (list case
                        (catch #t
                          (lambda ()
                            (ly:grob-set-property! c 'glyph (car case))
                            (ly:grob-set-property! g 'direction (cadr case))
                            (ly:grob-set-property! g 'clef-alignments (caddr case))
                            (ly:clef-modifier::calc-parent-alignment g))
                          (lambda (key . args) (list key args)))))
           (newline))
         cases)))
  0)
\new Staff {{ \override Staff.ClefModifier.parent-alignment-X = #probe \clef "treble_8" c'1 }}
"#
    )
}

/// [`grobs_vm`] with `(modifier-case case)`: the alignment
/// `ly:clef-modifier::calc-parent-alignment` gives of `(thing 1)`, its X
/// parent `(thing 0)` the clef, the case's properties put on them.
fn clef_modifier_vm() -> Option<Vm> {
    let mut vm = grobs_vm()?;
    eval(
        &mut vm,
        "(ly:grob-set-parent! (thing 1) 0 (thing 0))
         (define (modifier-case case)
           (put! (thing 0) 'glyph (car case))
           (put! (thing 1) 'direction (cadr case))
           (put! (thing 1) 'clef-alignments (caddr case))
           (ly:clef-modifier::calc-parent-alignment (thing 1)))",
    );
    Some(vm)
}

#[test]
fn clef_modifiers_are_aligned_on_their_clefs_as_lilypond_aligns_them() {
    let installed = Command::new("lilypond")
        .arg("--version")
        .output()
        .is_ok_and(|v| String::from_utf8_lossy(&v.stdout).contains("GNU LilyPond 2.26.0"));
    if !installed {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the alignments are unverified");
        return;
    }
    let Some(mut vm) = clef_modifier_vm() else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-clef-modifier-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("clef-modifier.ly"), clef_modifier_probe()).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "clef-modifier.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    // Every case reached the callback: none was ignored by a type check.
    assert!(!stderr.contains("warning"), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 18);
    // The values LilyPond gives for a few: the alignments below and above,
    // centred for no entry or one that is no pair, the first entry's.
    for (n, alignment) in [
        (0, "-0.2"),
        (1, "0.1"),
        (2, "0.1"),
        (3, "-0.2"),
        (4, "-0.3"),
        (7, "0"),
        (9, "(4)"),
        (12, "a"),
        (16, "2"),
        (17, "0"),
    ] {
        assert!(
            lines[n].ends_with(&format!(" {alignment})")),
            "{}",
            lines[n]
        );
    }
    for line in lines {
        let got = caught(&mut vm, &format!("(modifier-case (car '{line}))"));
        let expected = caught(&mut vm, &format!("(cadr '{line})"));
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_clef_modifier_of_odd_properties_or_without_a_clef_throws() {
    let Some(mut vm) = clef_modifier_vm() else {
        return;
    };
    // Values LilyPond's type checks keep from the callback, by hand: a
    // direction that is none is down; a glyph that is no string and an
    // alist that is none are Guile's type errors, as from_scm<std::string>
    // and scm_assq throw them.
    for (direction, alignment) in [("'up", "-0.2"), ("2", "-0.2"), ("1.0", "-0.2")] {
        let case = format!("(list \"clefs.G\" {direction} '((G . (-0.2 . 0.1))))");
        assert_eq!(
            caught(&mut vm, &format!("(modifier-case {case})")),
            alignment,
            "{direction}"
        );
    }
    let got = caught(&mut vm, "(modifier-case '(treble -1 ((G . (1 . 2)))))");
    assert!(
        got.contains("scm_to_utf8_stringn")
            && got.contains("Wrong type argument (expecting string): treble"),
        "{got}"
    );
    for alist in ["5", "((F . 1) . 5)", "(5 (G 1 . 2))"] {
        let got = caught(
            &mut vm,
            &format!("(modifier-case '(\"clefs.G\" -1 {alist}))"),
        );
        assert!(
            got.contains("assq")
                && got.contains(&format!(
                    "Wrong type argument in position 2 (expecting association list): {alist}"
                )),
            "{alist}: {got}"
        );
    }
    // Found before the alist goes wrong: its entry.
    assert_eq!(
        caught(
            &mut vm,
            "(modifier-case '(\"clefs.G\" -1 ((G . (1 . 2)) . 5)))"
        ),
        "1"
    );
    // No clef, which LilyPond dereferences: misc-error.
    let got = caught(
        &mut vm,
        "(ly:clef-modifier::calc-parent-alignment (thing 2))",
    );
    assert!(got.contains("clef modifier without a clef"), "{got}");
}

// --- the system start delimiter (MS3 step 40) -------------------------------

#[test]
fn the_system_start_delimiter_s_callback_is_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:system-start-delimiter::print";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(
        music.iter().all(|p| p.name != name),
        "{name} is no placeholder"
    );
    // LilyPond's subr, as it prints.
    assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
    let got = caught(&mut vm, &format!("({name} 'x)"));
    assert!(
        got.contains(name) && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
        "{got}"
    );
    for args in ["", "'x 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
    // The break-alignable interface's, the image's Scheme.
    for name in [
        "ly:break-alignable-interface::find-parent",
        "ly:break-alignable-interface::self-align-callback",
    ] {
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{got}"
        );
    }
}

/// Scores whose system start delimiters each write a line as their
/// stencil is printed: `(delim name (style collapse-height thickness)
/// (line-thickness output-scale) elements braces live stencil)`, each of
/// the `elements` `(spanner? same-left-bound? y-extent staff-space)`, its
/// Y extent from their common refpoint and its staff space only if it is a
/// spanner beginning where the delimiter does; `braces` each
/// `make-left-brace-markup`'s size and that markup's stencil on the
/// grob; `live` whether the print left the delimiter alive; the stencil as
/// `tools/compare/stencils.ly`'s datum `(x y expr)`. The elements, the name
/// and the properties are read before the print: a killed grob has none.
const DELIM_PROBE: &str = r#"\version "2.26.0"
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (stil s)
  (if (ly:stencil? s)
      (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
      s))
#(define braces '())
#(set! make-left-brace-markup
  (let ((make make-left-brace-markup))
    (lambda (size)
      (let ((m (make size)))
        (set! braces (cons (cons size m) braces))
        m))))
#(define (delim-probe g)
  (set! braces '())
  (let* ((arr (ly:grob-object g 'elements))
         (elts (if (ly:grob-array? arr) (ly:grob-array->list arr) '()))
         (common (if (ly:grob-array? arr) (ly:grob-common-refpoint-of-array g arr Y) g))
         (left (ly:spanner-bound g LEFT))
         (name (grob::name g))
         (props (map (lambda (p) (ly:grob-property g p)) '(style collapse-height thickness)))
         (s (ly:system-start-delimiter::print g)))
    (write (list 'delim name props
                 (map (lambda (v) (ly:output-def-lookup (ly:grob-layout g) v)) '(line-thickness output-scale))
                 (map (lambda (e)
                        (let ((same (and (ly:spanner? e) (eq? (ly:spanner-bound e LEFT) left))))
                          (list (ly:spanner? e) same
                                (and same (ly:grob-extent e common Y))
                                (and same (ly:staff-symbol-staff-space e)))))
                      elts)
                 (map (lambda (b) (list (car b) (stil (ly:text-interface::grob-interpret-markup g (cdr b)))))
                      braces)
                 (grob::is-live? g)
                 (stil s)))
    (newline)
    s))
\layout {
  \context { \Score
    \override SystemStartBar.stencil = #delim-probe
    \override SystemStartBracket.stencil = #delim-probe
    \override SystemStartBrace.stencil = #delim-probe
    \override SystemStartSquare.stencil = #delim-probe }
  \context { \Staff \RemoveEmptyStaves }
}
music = { c'1 \break c'1 \break c'1 }
\score { \new Staff \music }
\score { << \new Staff \music \new Staff \music >> }
\score { <<
  \new StaffGroup << \new Staff \music \new Staff \music >>
  \new GrandStaff << \new Staff \music \new Staff \music >>
  \new StaffGroup \with { systemStartDelimiter = #'SystemStartSquare } << \new Staff \music \new Staff \music >>
  \new ChoirStaff << \new Staff \music >>
>> }
\score { <<
  \new StaffGroup \with { \override SystemStartBracket.thickness = 0.6 } << \new Staff \music \new Staff \music >>
  \new StaffGroup \with { systemStartDelimiter = #'SystemStartSquare \override SystemStartSquare.thickness = 2 }
    << \new Staff \music \new Staff \music >>
  \new GrandStaff \with { \override SystemStartBrace.collapse-height = 30 } << \new Staff \music \new Staff \music >>
  \new GrandStaff << \new Staff \with { \override StaffSymbol.staff-space = 1.5 } \music \new Staff \music >>
  \new StaffGroup \with { \override SystemStartBracket.collapse-height = 16 } << \new Staff \music \new Staff \with { \magnifyStaff #2/3 } \music >>
>>
\layout { \context { \Score \override SystemStartBar.thickness = 3 } } }
\score { <<
  \new Staff { c'1 \break c'1 \break c'1 }
  \new Staff { c'1 \break s1 \break c'1 }
  \new Staff { s1 \break s1 \break c'1 }
>> }
\score { << \new Staff \music \new Staff \music >>
  \layout { \context { \Score \override SystemStartBar.style = #'nonesuch } } }
\score { << \new Staff \music \new Staff \with { \override StaffSymbol.staff-space = 0.5 } \music >>
  \layout { \context { \Score \override SystemStartBar.collapse-height = 12 } } }
"#;

/// [`print_vm`]'s VM with the brace pack too, a spanner type `Elt`, fresh
/// grobs of each kind by `(fresh kind)` and `(delim-case line)`: what
/// `ly:system-start-delimiter::print` prints of a fresh delimiter of the
/// line's properties, its layout's `line-thickness` and `output-scale`,
/// over fresh elements with their Y extents and staff spaces on one Y
/// parent, the brace's markup the stencil the line gives at its size (its
/// expression `brace`); `(live? result)`.
fn delimiter_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Elt (meta (name . Elt) (classes Spanner)
                     (interfaces grob-interface spanner-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 6) (Thing . 2) (Column . 2) (Staff . 200) (Delim . 60) (Elt . 200))",
    )?;
    // The brace font's packs, each continuing the one before it.
    let mut braces: Vec<PathBuf> = std::fs::read_dir(packs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("emmentaler-brace-") && name.ends_with(".pack")
        })
        .collect();
    braces.sort_by_key(|p| {
        let name = p.file_stem().unwrap().to_string_lossy();
        name.rsplit('-').next().unwrap().parse::<u32>().unwrap()
    });
    for path in braces {
        {
            lily_engine_layout::font::put_pack(&mut vm, &std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {}", path.display(), e.text(vm.heap())));
        }
    }
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (column 0)) 'line-thickness 0.1)
         (ly:output-def-set-variable! (ly:grob-layout (column 0)) 'staff-space 1.0)
         (define counts '())
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (assq-set! counts kind (1+ n)))
             (made-of kind n)))
         (define (dumped s)
           (cond ((ly:stencil? s) (%stencil->dump s))
                 ((eq? s *unspecified*) 'unspecified)
                 (else s)))
         (define brace-sizes '())
         (define brace-stencil #f)
         (define-public (make-left-brace-markup size)
           (set! brace-sizes (cons size brace-sizes))
           'brace-markup)
         (set! %grob-interpret-markup (lambda (grob m) brace-stencil))
         (define (delim-case line)
           (let* ((props (caddr line))
                  (layout (ly:grob-layout (column 0)))
                  (d (fresh 'Delim))
                  (parent (thing 0))
                  (braces (list-ref line 5))
                  (elts (map (lambda (e)
                               (let ((g (fresh 'Elt)) (st (fresh 'Staff)))
                                 (ly:grob-set-parent! g 1 parent)
                                 (put! g 'Y-offset 0.0)
                                 (put! g 'Y-extent (caddr e))
                                 (put! st 'staff-space (cadddr e))
                                 (ly:grob-set-object! g 'staff-symbol st)
                                 (%spanner-set-bound! g -1 (column 1))
                                 g))
                             (list-ref line 4))))
             (ly:output-def-set-variable! layout 'line-thickness (car (cadddr line)))
             (ly:output-def-set-variable! layout 'output-scale (cadr (cadddr line)))
             (ly:grob-set-parent! d 1 parent)
             (%spanner-set-bound! d -1 (column 1))
             (for-each (lambda (sym v) (if (not (null? v)) (put! d sym v)))
                       '(style collapse-height thickness) props)
             (put! d 'fonts (ly:grob-property (column 0) 'fonts))
             (ly:grob-set-object! d 'elements (apply arr elts))
             (set! brace-sizes '())
             (set! brace-stencil
                   (and (pair? braces)
                        (ly:make-stencil 'brace (car (cadar braces)) (cadr (cadar braces)))))
             (let ((s (ly:system-start-delimiter::print d)))
               (list (%grob-live? d) (dumped s) (reverse brace-sizes)))))
         (define (brace-expr x expr)
           (cond ((equal? x expr) 'brace)
                 ((pair? x) (cons (brace-expr (car x) expr) (brace-expr (cdr x) expr)))
                 (else x)))
         (define (lilypond-case line)
           (let ((braces (list-ref line 5)) (s (list-ref line 7)))
             (list (list-ref line 6)
                   (if (pair? braces) (brace-expr s (caddr (cadar braces))) s)
                   (map car braces))))",
    );
    Some(vm)
}

#[test]
fn system_start_delimiters_are_printed_as_lilypond_prints_them() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the prints are unverified");
        return;
    };
    let Some(mut vm) = delimiter_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-delimiter-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("delim.ly"), DELIM_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "delim.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lilypond = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = lilypond.lines().collect();
    // The cases the probe means to cover are there: each style, a
    // thickness and a collapse height set, the staff spaces, a staff
    // removed, an unknown style, the killed.
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    for (what, n) in [
        ("(delim SystemStartBar (bar-line 5.0 1.6) ", 12),
        ("(delim SystemStartBar (bar-line 5.0 3) ", 3),
        ("(delim SystemStartBar (bar-line 12 1.6) ", 3),
        ("(delim SystemStartBar (nonesuch 5.0 1.6) ", 3),
        ("(delim SystemStartBracket (bracket 5.0 0.45) ", 6),
        ("(delim SystemStartBracket (bracket 5.0 0.6) ", 3),
        ("(delim SystemStartBracket (bracket 16 0.45) ", 3),
        ("(delim SystemStartBrace (brace 5.0 ()) ", 6),
        ("(delim SystemStartBrace (brace 30 ()) ", 3),
        ("(delim SystemStartSquare (line-bracket 5.0 1.0) ", 3),
        ("(delim SystemStartSquare (line-bracket 5.0 2) ", 3),
        ("(delim #f () ", 0),
        (" 1.5) ", 6),
        (" 0.6666666666666666)", 6),
        (" 0.5)", 3),
        ("(+inf.0 . -inf.0) 1.0)", 1),
        (" #f #<unspecified>)", 16),
        (" () #t ((+inf.0 . -inf.0) (+inf.0 . -inf.0) ()))", 3),
    ] {
        assert_eq!(count(what), n, "{what}");
    }
    assert_eq!(lines.len(), 48);
    for line in &lines {
        // What a killed delimiter's print gives, readable.
        let line = line.replace("#<unspecified>", "unspecified");
        let got = caught(&mut vm, &format!("(delim-case '{line})"));
        let expected = caught(&mut vm, &format!("(lilypond-case '{line})"));
        assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_delimiter_spans_its_spanners_beginning_where_it_does() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = delimiter_vm(packs) else {
        return;
    };
    let print = |vm: &mut Vm, elements: &str| {
        caught(
            vm,
            &format!(
                "(let ((d (fresh 'Delim)) (parent (thing 0)))
                   (ly:grob-set-parent! d 1 parent)
                   (%spanner-set-bound! d -1 (column 1))
                   (put! d 'style 'bar-line)
                   (put! d 'collapse-height 1)
                   (ly:grob-set-object! d 'elements
                     (apply arr (map (lambda (e)
                                       (let ((g (if (car e) (fresh 'Elt) (thing 1))))
                                         (ly:grob-set-parent! g 1 parent)
                                         (put! g 'Y-offset 0.0)
                                         (put! g 'Y-extent (cdr e))
                                         (if (car e) (%spanner-set-bound! g -1 (column (car e))))
                                         g))
                                     {elements})))
                   (let ((s (ly:system-start-delimiter::print d)))
                     (if (ly:stencil? s) (ly:stencil-extent s 1) (%grob-live? d))))"
            ),
        )
    };
    // A spanner beginning at the delimiter's column and one beginning at
    // another, and an item: only the first is spanned.
    assert_eq!(
        print(
            &mut vm,
            "'((1 -6.0 . -2.0) (0 -20.0 . -10.0) (#f -30.0 . 4.0))"
        ),
        "(-6.0 . -2.0)"
    );
    // Two at its column; an empty one between them.
    assert_eq!(
        print(
            &mut vm,
            "'((1 -6.0 . -2.0) (1 +inf.0 . -inf.0) (1 -14.0 . -10.0))"
        ),
        "(-14.0 . -2.0)"
    );
    // None spanned, or no more than its collapse height: killed.
    assert_eq!(print(&mut vm, "'((0 -6.0 . -2.0) (#f -6.0 . -2.0))"), "#f");
    assert_eq!(print(&mut vm, "'((1 -6.0 . -5.0))"), "#f");
    assert_eq!(print(&mut vm, "'()"), "#f");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_break_alignable_grob_aligns_on_its_break_aligned_group() {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Align0 (meta (name . Align0) (classes Item)
                        (interfaces grob-interface item-interface break-alignment-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let Some(mut vm) = grobs_vm_of(&defs, "'((Thing . 2) (Column . 1) (Align0 . 1) (Bit . 4))")
    else {
        return;
    };
    // A break alignment of four groups on a column: Bit 0 and 1 of
    // `clef`, empty and nonempty, Bit 2 of `key`, Bit 3 of `staff-bar`;
    // `thing 0` aligned on it, `thing 1` on the column.
    eval(
        &mut vm,
        "(define ba (made-of 'Align0 0))
         (ly:grob-set-parent! ba 0 (column 0))
         (put! ba 'X-offset 3.0)
         (ly:grob-set-object! ba 'elements (arr (bit 0) (bit 1) (bit 2) (bit 3)))
         (for-each (lambda (g sym ext x anchor)
                     (ly:grob-set-parent! g 0 ba)
                     (put! g 'break-align-symbol sym)
                     (put! g 'X-extent ext)
                     (put! g 'X-offset x)
                     (put! g 'break-align-anchor anchor))
                   (list (bit 0) (bit 1) (bit 2) (bit 3))
                   '(clef clef key staff-bar)
                   '((+inf.0 . -inf.0) (0.0 . 2.0) (0.0 . 1.5) (0.0 . 0.2))
                   '(0.5 1.0 4.0 6.0)
                   '(0.25 0.75 1.25 none))
         (ly:grob-set-parent! (thing 0) 0 ba)
         (put! (thing 0) 'X-offset 0.5)
         (ly:grob-set-parent! (thing 1) 0 (column 0))
         (define (aligned symbols)
           (put! (thing 0) 'break-align-symbols symbols)
           (let ((p (ly:break-alignable-interface::find-parent (thing 0))))
             (list (and p (- 4 (length (memq p (list (bit 0) (bit 1) (bit 2) (bit 3))))))
                   (ly:break-alignable-interface::self-align-callback (thing 0)))))",
    );
    // The first of the symbols with a visible, nonempty group: its X
    // offset less the grob's, plus its anchor (0 if none).
    assert_eq!(caught(&mut vm, "(aligned '(clef key))"), "(1 1.25)");
    assert_eq!(caught(&mut vm, "(aligned '(key clef))"), "(2 4.75)");
    assert_eq!(caught(&mut vm, "(aligned '(staff-bar))"), "(3 5.5)");
    // None visible or nonempty: the first group met; none: 0, exactly.
    eval(&mut vm, "(put! (bit 1) 'break-visibility #(#f #f #f))");
    assert_eq!(caught(&mut vm, "(aligned '(clef))"), "(0 0.25)");
    assert_eq!(caught(&mut vm, "(aligned '(nonesuch))"), "(#f 0)");
    assert_eq!(caught(&mut vm, "(aligned '())"), "(#f 0)");
    // On no break alignment: no parent.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:break-alignable-interface::find-parent (thing 1))
                   (ly:break-alignable-interface::self-align-callback (thing 1)))"
        ),
        "(#f 0)"
    );
}

// --- flags (MS3 step 41) ----------------------------------------------------

/// The flag's callbacks (MS3 step 41), each of the grob alone, and the
/// pure one of the grob and two columns.
const FLAG_CALLBACKS: [(&str, bool); 6] = [
    ("ly:flag::calc-x-offset", false),
    ("ly:flag::calc-y-offset", false),
    ("ly:flag::glyph-name", false),
    ("ly:flag::print", false),
    ("ly:flag::pure-calc-y-offset", true),
    ("ly:flag::width", false),
];

#[test]
fn the_flag_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, pure) in FLAG_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        let formals = if pure { "(_ _ _)" } else { "(_)" };
        assert_eq!(
            caught(&mut vm, name),
            format!("#<procedure {name} {formals}>")
        );
        let args = if pure { "'x 0 1" } else { "'x" };
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for args in ["", "'x 'x 'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

/// A score whose flags each write a line as their stencil is printed:
/// `(flag direction duration-log x-extent y-extent pure-y-extent
/// staff-space line-positions blot-diameter (style stroke-style font-size
/// glyph-name has-staff-symbol) glyph-name stencil width x-offset y-offset
/// pure-y-offset)`, the first eight of its stem (its staff symbol's
/// `staff-space` and `line-positions`, the layout's `blot-diameter`), the
/// stem's extents its own, the pure one from column 0 to `INT_MAX`, then
/// the flag's properties and what its callbacks give, the stencil as
/// `tools/compare/stencils.ly`'s datum `(x y expr)`. An eighth to a 128th
/// up and down, longer and thicker stems and an invisible one; mensural
/// flags at each staff position both ways, on a staff of 1.5 spaces and on
/// one of four uneven lines, a 128th's not in the font; no-flag and
/// stacked; grace strokes, a mensural flag's falling back to the plain
/// one, an unknown stroke and an empty one; an unknown glyph name and one
/// that is no string (LilyPond ignores it); a font size.
const FLAG_PROBE: &str = r##"\version "2.26.0"
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (stil s)
  (if (ly:stencil? s)
      (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
      s))
#(define (flag-probe g)
  (let* ((stem (ly:grob-parent g X))
         (staff (ly:grob-object stem 'staff-symbol))
         (s (ly:flag::print g)))
    (ly:grob-set-property! g 'stencil s)
    (write (list 'flag
                 (ly:grob-property stem 'direction)
                 (ly:grob-property stem 'duration-log)
                 (ly:grob-extent stem stem X)
                 (ly:grob-extent stem stem Y)
                 (ly:grob-pure-height stem stem 0 2147483647)
                 (ly:grob-property staff 'staff-space)
                 (ly:grob-property staff 'line-positions)
                 (ly:output-def-lookup (ly:grob-layout g) 'blot-diameter)
                 (append (map (lambda (p) (ly:grob-property g p)) '(style stroke-style font-size glyph-name))
                         (list (ly:grob? (ly:grob-object g 'staff-symbol))))
                 (ly:flag::glyph-name g)
                 (stil s)
                 (ly:flag::width g)
                 (ly:flag::calc-x-offset g)
                 (ly:flag::calc-y-offset g)
                 (ly:flag::pure-calc-y-offset g 0 2147483647)))
    (newline)
    s))
\layout { \context { \Voice \override Flag.stencil = #flag-probe } }
<<
\new Staff \relative {
  \autoBeamOff \cadenzaOn
  c''8 c16 c32 c64 c128 c,,8 c16 c32 c64 c128
  \stemDown e'8 e16 \stemUp a'8 a16 \stemNeutral
  \override Stem.length = 10 g8 d,16 \revert Stem.length
  \override Flag.style = #'mensural
  c'8 d e f g a b c d e f g a b16 a32 g64 f128
  \stemDown b,,8 c d16 e f32 g a64 b128 \stemNeutral
  \override Flag.stroke-style = "grace" c8 c,8 \revert Flag.stroke-style
  \override Flag.style = #'no-flag c8
  \override Flag.style = #'stacked c8 c''16
  \revert Flag.style
  \grace { c,16 d32 } c8 \acciaccatura e8 f8 \slashedGrace a,8 g8
  \override Flag.stroke-style = "nonesuch" c'8 c,8 \revert Flag.stroke-style
  \override Flag.stroke-style = "" c8 \revert Flag.stroke-style
  \override Flag.glyph-name = "flags.nonesuch" c8 \revert Flag.glyph-name
  \override Flag.glyph-name = #'none c8 \revert Flag.glyph-name
  \override Flag.font-size = 3 c8 c''16 \revert Flag.font-size
  \override Stem.stencil = ##f c,,8 \revert Stem.stencil
  \override Stem.thickness = 3 c8 c''16 \revert Stem.thickness
  \override Flag.style = #'mensural \override Flag.stroke-style = "grace" c8 c,,8
}
\new Staff \with { \override StaffSymbol.staff-space = 1.5 } \relative {
  \autoBeamOff \cadenzaOn
  c''8 c,16 \override Flag.style = #'mensural c'8 d e f g c,, d e
}
\new Staff \with { \override StaffSymbol.line-positions = #'(-5 -1 3 7) } \relative {
  \autoBeamOff \cadenzaOn
  \override Flag.style = #'mensural c'8 d e f g a b c d
}
>>
"##;

/// [`print_vm`]'s VM with fresh flags, stems and staves, and `(flag-case
/// line)`: what the flag's callbacks give of a fresh flag of the line's
/// properties on a fresh stem of its direction, duration log and extents
/// (its Y extent the unpure and the pure one), on a staff of its staff
/// space and lines, with the line's blot diameter.
fn flag_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| {
            d.replace(
                "(Span (meta",
                "(Flag (meta (name . Flag) (classes Item)
                       (interfaces grob-interface item-interface flag-interface font-interface)))
        (Span (meta",
            )
        })
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 1) (Column . 1) (Staff . 90) (Stem . 90) (Flag . 90))",
    )?;
    eval(
        &mut vm,
        "(ly:output-def-set-variable! (ly:grob-layout (column 0)) 'staff-space 1.0)
         (define counts '())
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (assq-set! counts kind (1+ n)))
             (made-of kind n)))
         (define (dumped s) (if (ly:stencil? s) (%stencil->dump s) s))
         (define (flag-case line)
           (let* ((at (lambda (i) (list-ref line i)))
                  (props (at 9)) (unpure (at 4)) (pure (at 5))
                  (st (fresh 'Staff)) (stem (fresh 'Stem)) (f (fresh 'Flag)))
             (if (number? (at 6)) (put! st 'staff-space (at 6)))
             (put! st 'line-positions (at 7))
             (ly:output-def-set-variable! (ly:grob-layout f) 'blot-diameter (at 8))
             (put! stem 'direction (at 1))
             (put! stem 'duration-log (at 2))
             (put! stem 'X-extent (at 3))
             (put! stem 'Y-extent (ly:make-unpure-pure-container (lambda (g) unpure)
                                                                 (lambda (g s e) pure)))
             (ly:grob-set-object! stem 'staff-symbol st)
             (ly:grob-set-parent! f 0 stem)
             (if (list-ref props 4) (ly:grob-set-object! f 'staff-symbol st))
             (put! f 'fonts (ly:grob-property (column 0) 'fonts))
             (for-each (lambda (sym v) (if (not (null? v)) (put! f sym v)))
                       '(style stroke-style font-size glyph-name)
                       (list (car props) (cadr props) (caddr props) (cadddr props)))
             ;; The pure offset first: LilyPond's stem has cached its pure
             ;; height by then, and its unpure extent replaces the container.
             (let* ((pure-y (ly:flag::pure-calc-y-offset f 0 2147483647))
                    (s (ly:flag::print f)))
               (put! f 'stencil s)
               (list (ly:flag::glyph-name f) (dumped s) (ly:flag::width f)
                     (ly:flag::calc-x-offset f) (ly:flag::calc-y-offset f) pure-y))))",
    );
    Some(vm)
}

/// The warnings of [`FLAG_PROBE`]'s flags, each with the number of its
/// lines that give it.
const FLAG_WARNINGS: [(&str, usize); 3] = [
    ("flag `flags.mensurald17' not found", 2),
    ("flag stroke `dnonesuch' not found", 2),
    ("flag `flags.nonesuch' not found", 1),
];

#[test]
fn flags_are_printed_and_placed_as_lilypond_does() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the flags are unverified");
        return;
    };
    let Some(mut vm) = flag_vm(packs) else {
        return;
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-flag-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("flag.ly"), FLAG_PROBE).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", "flag.ly"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let lilypond = String::from_utf8(out.stdout).unwrap();
    // Some flags are alike: each line once.
    let mut lines: Vec<&str> = lilypond.lines().collect();
    let printed = lines.len();
    lines.sort_unstable();
    lines.dedup();
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    // The cases the probe means to cover are there.
    for (what, n) in [
        ("(flag 1 ", 22),
        ("(flag -1 ", 59),
        ("\"flags.mensuralu0", 4),
        ("\"flags.mensuralu1", 5),
        ("\"flags.mensurald0", 24),
        ("\"flags.mensurald1", 11),
        (" 1.5 (", 10),
        ("() (-5 -1 3 7) ", 9),
        ("(no-flag ", 1),
        ("(stacked ", 2),
        ("\"grace\" -3 ", 2),
        ("(mensural \"grace\" ", 3),
        ("\"nonesuch\" ", 2),
        ("(() \"\" ", 1),
        ("\"flags.nonesuch\" #t", 1),
        ("(() () 3 ", 2),
        (" (+inf.0 . -inf.0) (0.0 . 3.8138) ", 1),
        (" (-0.15 . 0.15) ", 2),
    ] {
        assert_eq!(count(what), n, "{what}");
    }
    for line in &lines {
        let got = caught(&mut vm, &format!("(flag-case '{line})"));
        let expected = caught(&mut vm, &format!("(list-tail '{line} 10)"));
        assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
    }
    // Each line's warnings: LilyPond's for each print, the engine's for
    // each line.
    let diagnostics = diagnosed(&mut vm);
    for (warning, count) in FLAG_WARNINGS {
        assert!(stderr.contains(warning), "{warning}: {stderr}");
        assert_eq!(
            diagnostics.matches(warning).count(),
            count,
            "{warning}: {diagnostics}"
        );
    }
    // And no other: a no-flag flag looks for no glyph.
    let all: usize = FLAG_WARNINGS.iter().map(|(_, n)| n).sum();
    assert_eq!(
        diagnostics.matches("\" \"").count() + 1,
        all,
        "{diagnostics}"
    );
    assert!(printed > lines.len());
}

#[test]
fn a_flag_without_a_glyph_name_warns_and_one_without_a_stem_throws() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = flag_vm(packs) else {
        return;
    };
    // A glyph name that is no string: `""`, not in the font.
    let got = caught(
        &mut vm,
        "(let ((stem (fresh 'Stem)) (f (fresh 'Flag)))
           (put! stem 'direction 1)
           (ly:grob-set-parent! f 0 stem)
           (put! f 'fonts (ly:grob-property (column 0) 'fonts))
           (put! f 'glyph-name 5)
           (ly:stencil-empty? (ly:flag::print f)))",
    );
    assert_eq!(got, "#t");
    assert_eq!(diagnosed(&mut vm), "(\"flag `' not found\")");
    // No stem: LilyPond's would dereference it.
    for (name, args) in [
        ("ly:flag::print", ""),
        ("ly:flag::glyph-name", ""),
        ("ly:flag::calc-x-offset", ""),
        ("ly:flag::calc-y-offset", ""),
        ("ly:flag::pure-calc-y-offset", " 0 1"),
    ] {
        let got = caught(&mut vm, &format!("({name} (fresh 'Flag){args})"));
        assert!(got.contains("flag without a stem"), "{name}: {got}");
    }
    // No stencil: no width, before the stem is asked for.
    assert_eq!(
        caught(&mut vm, "(ly:flag::width (fresh 'Flag))"),
        "(0.0 . 0.0)"
    );
}

// --- note collisions (MS3 step 41) ------------------------------------------

#[test]
fn the_note_collision_s_callback_is_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:note-collision-interface::calc-positioning-done";
    let music = lily_engine_music::EXTENSION.prims[1];
    assert!(
        music.iter().all(|p| p.name != name),
        "{name} is no placeholder"
    );
    // LilyPond's subr, as it prints.
    assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
    let got = caught(&mut vm, &format!("({name} 'x)"));
    assert!(
        got.contains(name) && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
        "{got}"
    );
    for args in ["", "'x 'x"] {
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(got.contains("Wrong number of arguments"), "{got}");
    }
}

#[test]
fn a_note_collision_without_note_columns_places_nothing() {
    let Some(mut vm) = grobs_vm() else { return };
    // Its elements none, then one that is no note column: #t, its
    // positioning-done set, the element not moved, no diagnostic.
    for elements in ["'()", "(arr (thing 1))"] {
        let got = caught(
            &mut vm,
            &format!(
                "(let ((me (thing 0)))
                   (ly:grob-set-parent! (thing 1) 0 me)
                   (ly:grob-set-object! me 'elements {elements})
                   (list (ly:note-collision-interface::calc-positioning-done me)
                         (ly:grob-property me 'positioning-done)
                         (ly:grob-relative-coordinate (thing 1) (thing 0) 0)))"
            ),
        );
        assert_eq!(got, "(#t #t 0.0)", "{elements}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

// --- rest collisions (MS3 step 41) ------------------------------------------

#[test]
fn the_rest_collision_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, arity, wrong) in [
        (
            "ly:rest-collision::calc-positioning-done",
            "_",
            ["", "'x 'x"],
        ),
        (
            "ly:rest-collision::force-shift-callback-rest",
            "_ #:optional _",
            ["", "'x 'x 'x"],
        ),
    ] {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(
            caught(&mut vm, name),
            format!("#<procedure {name} ({arity})>")
        );
        let got = caught(&mut vm, &format!("({name} 'x)"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{got}"
        );
        for args in wrong {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{got}");
        }
    }
}

#[test]
fn a_rest_collision_without_two_columns_places_nothing() {
    let Some(mut vm) = grobs_vm() else { return };
    // Its elements none, then one that is no note column: #t, its
    // positioning-done set, the element not moved, no diagnostic.
    for elements in ["'()", "(arr (thing 1))"] {
        let got = caught(
            &mut vm,
            &format!(
                "(let ((me (thing 0)))
                   (ly:grob-set-parent! (thing 1) 1 me)
                   (ly:grob-set-object! me 'elements {elements})
                   (list (ly:rest-collision::calc-positioning-done me)
                         (ly:grob-property me 'positioning-done)
                         (ly:grob-relative-coordinate (thing 1) (thing 0) 1)))"
            ),
        );
        assert_eq!(got, "(#t #t 0.0)", "{elements}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_force_shift_callback_moves_the_rest_by_the_offset_and_gives_0() {
    let Some(mut vm) = grobs_vm() else { return };
    // Without a note column as its X parent, no collision is asked: the
    // offset given moved, one not given (or no number) not, 0.0 returned.
    let got = caught(
        &mut vm,
        "(let ((r (thing 1)))
           (ly:grob-set-parent! r 1 (thing 0))
           (ly:grob-set-parent! r 0 (thing 0))
           (list (ly:rest-collision::force-shift-callback-rest r 1.5)
                 (ly:rest-collision::force-shift-callback-rest r)
                 (ly:rest-collision::force-shift-callback-rest r 'x)
                 (ly:rest-collision::force-shift-callback-rest r -0.5)
                 (ly:grob-relative-coordinate r (thing 0) 1)))",
    );
    assert_eq!(got, "(0.0 0.0 0.0 0.0 1.0)");
    assert_eq!(diagnosed(&mut vm), "()");
}

// --- multi-measure rests (MS3 step 42) --------------------------------------

/// The multi-measure rest's callbacks (MS3 step 42), each of the spanner
/// alone.
const MULTI_MEASURE_REST_CALLBACKS: [&str; 4] = [
    "ly:multi-measure-rest::height",
    "ly:multi-measure-rest::print",
    "ly:multi-measure-rest::set-spacing-rods",
    "ly:multi-measure-rest::set-text-rods",
];

#[test]
fn the_multi_measure_rest_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in MULTI_MEASURE_REST_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        // LilyPond's subr, as it prints.
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        // LY_ASSERT_SMOB (Spanner, …): an item is no spanner either.
        for arg in ["'x", "(thing 0)"] {
            let got = caught(&mut vm, &format!("({name} {arg})"));
            assert!(
                got.contains(name)
                    && got.contains("Wrong type argument in position 1 (expecting Spanner)"),
                "{name}: {got}"
            );
        }
        for args in ["", "'x 'x"] {
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        }
    }
}

/// A score whose multi-measure rests each write a line as their stencil
/// is printed: `(mmr case (stencil height staff-position))`, the case
/// `(properties staff-position-data staff-position staff-space
/// line-positions line-thickness measure-length left right)`: the rest's
/// `measure-count`, `expand-limit`, `usable-duration-logs`,
/// `round-up-exceptions`, `round-up-to-longer-rest`,
/// `max-symbol-separation`, `thick-thickness`, `hair-thickness`,
/// `font-size`, `direction`, `voiced-position` and `spacing-pair`; its
/// `staff-position` as it was made (`'()` unless overridden), its staff
/// position, staff space, its staff's lines and the layout's line
/// thickness when printed; the left bound's `measure-length`; each
/// bound's column as its X relative to the system and its break
/// alignment's X extent relative to it (`#f` for none). Then what the
/// probe's `height` and `print` give, in that order, the stencil as
/// `tools/compare/stencils.ly`'s datum `(x y expr)`, and the
/// `staff-position` after. Church rests of 1 to 10 measures and big rests
/// past `expand-limit`, of the measures of 4/4, 3/4, 2/2, 4/2, 6/4, 3/8 and
/// 5/4; rounded up to a longer rest or for an exception; other usable
/// durations, none (LilyPond's warning) and an improper list (refused by
/// the type check); a smaller `expand-limit`, a smaller and a larger
/// `max-symbol-separation`, other thicknesses, overridden staff
/// positions, a font size, two voices and a direction; on staves of one
/// line, of a staff space of 1.5, of four uneven lines, of none, and with
/// no staff symbol (its lines `#f`).
const MMR_PROBE: &str = r##"\version "2.26.0"
#(define (datum x)
  (cond ((pair? x) (cons (datum (car x)) (datum (cdr x))))
        ((ly:font-metric? x)
         (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
        (else x)))
#(define (stil s)
  (if (ly:stencil? s)
      (list (ly:stencil-extent s X) (ly:stencil-extent s Y) (datum (ly:stencil-expr s)))
      s))
#(define (col-case c)
  (let ((ba (ly:grob-object c 'break-alignment)))
    (list (ly:grob-relative-coordinate c (ly:grob-system c) X)
          (if (ly:grob? ba) (ly:grob-extent ba c X) #f))))
#(define props '(measure-count expand-limit usable-duration-logs round-up-exceptions
                 round-up-to-longer-rest max-symbol-separation thick-thickness
                 hair-thickness font-size direction voiced-position spacing-pair))
#(define sp0 (make-hash-table))
#(define (keep-sp g)
  (hashq-set! sp0 g (ly:grob-property-data g 'staff-position)))
#(define (mmr-probe g)
  (let* ((l (ly:spanner-bound g LEFT))
         (r (ly:spanner-bound g RIGHT))
         (staff (ly:grob-object g 'staff-symbol))
         (sp (hashq-ref sp0 (or (ly:grob-original g) g) 'none))
         (case (list (map (lambda (p) (ly:grob-property g p)) props)
                     sp
                     (ly:grob-staff-position g)
                     (ly:staff-symbol-staff-space g)
                     (and (ly:grob? staff) (ly:grob-property staff 'line-positions))
                     (ly:output-def-lookup (ly:grob-layout g) 'line-thickness)
                     (ly:moment-main (ly:grob-property l 'measure-length))
                     (col-case l)
                     (col-case r)))
         (h (ly:multi-measure-rest::height g))
         (s (ly:multi-measure-rest::print g)))
    (write (list 'mmr case (list (stil s) h (ly:grob-property g 'staff-position))))
    (newline)
    s))
\layout {
  \context { \Voice \override MultiMeasureRest.stencil = #mmr-probe
              \override MultiMeasureRest.before-line-breaking = #keep-sp }
}
\new Staff \relative {
  \compressEmptyMeasures
  R1 R1*2 R1*3 R1*4 R1*5 R1*6 R1*7 R1*8 R1*9 R1*10 R1*11 R1*20
  \time 3/4 R2. R2.*2 R2.*5
  \time 2/2 R1*3
  \time 4/2 R\breve*2 R\breve*3
  \time 6/4 R1.*3
  \time 3/8 R4.*4
  \time 5/4 R4*5*2
  \time 3/2
  \override MultiMeasureRest.round-up-to-longer-rest = ##t R1.*3 R1.
  \revert MultiMeasureRest.round-up-to-longer-rest
  \override MultiMeasureRest.round-up-exceptions = #'((3 . 2)) R1.*2
  \revert MultiMeasureRest.round-up-exceptions
  \time 4/4
  \override MultiMeasureRest.usable-duration-logs = #'(0 -1) R1*7
  \override MultiMeasureRest.usable-duration-logs = #'(1 2) R1*3
  \override MultiMeasureRest.usable-duration-logs = #'() R1*3
  \override MultiMeasureRest.usable-duration-logs = #'(0 . 1) R1*2
  \revert MultiMeasureRest.usable-duration-logs
  \override MultiMeasureRest.expand-limit = 3 R1*3 R1*4
  \revert MultiMeasureRest.expand-limit
  \override MultiMeasureRest.max-symbol-separation = 0.5 R1*7
  \override MultiMeasureRest.max-symbol-separation = 20 R1*7
  \revert MultiMeasureRest.max-symbol-separation
  \override MultiMeasureRest.thick-thickness = 3
  \override MultiMeasureRest.hair-thickness = 4 R1*15
  \revert MultiMeasureRest.thick-thickness \revert MultiMeasureRest.hair-thickness
  \override MultiMeasureRest.staff-position = 2 R1*3 R1
  \override MultiMeasureRest.staff-position = -6 R1*6
  \revert MultiMeasureRest.staff-position
  \override MultiMeasureRest.font-size = -3 R1*7 R1*20
  \revert MultiMeasureRest.font-size
  << { R1*3 R1 } \\ { R1*3 R1 } >>
  \oneVoice \override MultiMeasureRest.direction = #DOWN R1*5 R1
  \revert MultiMeasureRest.direction
}
\new Staff \with { \override StaffSymbol.line-count = 1 } {
  \compressEmptyMeasures
  R1 R1*2 R1*5 R1*9 R1*12
  << { R1*3 R1 } \\ { R1*3 R1 } >>
}
\new Staff \with { \override StaffSymbol.staff-space = 1.5 } {
  \compressEmptyMeasures R1*3 R1*7 R1 R1*30
}
\new Staff \with { \override StaffSymbol.line-positions = #'(-5 -1 3 7) } {
  \compressEmptyMeasures R1 R1*3 R1*7
}
\new Staff \with { \override StaffSymbol.line-count = 0 } {
  \compressEmptyMeasures R1 R1*6
}
\new Staff \with { \remove Staff_symbol_engraver } {
  \compressEmptyMeasures R1 R1*3 R1*12
}
"##;

/// A score whose multi-measure rests' and numbers' `springs-and-rods`
/// each write a line as they set their rods: `(rods name (measure-count
/// space-increment bound-padding minimum-length expand-limit)
/// (measure-length full-measure-extra-space left-spacing? right-spacing?)
/// stencil-x before after)`, the left bound column's properties and
/// whether either column has a `spacing`, the grob's stencil's X extent,
/// and the rod from the left bound's column to the right one's before and
/// after. The columns have no `horizontal-skylines`, so that the rods are
/// the lengths alone; a number's rods are set on a column cleared of the
/// rest's, then the rest's put back.
const MMR_RODS_PROBE: &str = r##"\version "2.26.0"
#(define (dist from to)
  (let ((d (assq to (ly:grob-object from 'minimum-distances))))
    (and d (cdr d))))
#(define (rods-probe g)
  (let* ((l (ly:spanner-bound g LEFT))
         (r (ly:spanner-bound g RIGHT))
         (props (map (lambda (p) (ly:grob-property g p))
                     '(measure-count space-increment bound-padding minimum-length expand-limit)))
         (cols (list (ly:moment-main (ly:grob-property l 'measure-length))
                     (ly:grob-property l 'full-measure-extra-space)
                     (ly:grob? (ly:grob-object l 'spacing))
                     (ly:grob? (ly:grob-object r 'spacing))))
         (before (dist l r))
         (after #f))
    (if (eq? (grob::name g) 'MultiMeasureRest)
        (ly:multi-measure-rest::set-spacing-rods g)
        (let ((kept (ly:grob-object l 'minimum-distances)))
          (set! before #f)
          (ly:grob-set-object! l 'minimum-distances '())
          (ly:multi-measure-rest::set-text-rods g)
          (set! after (dist l r))
          (ly:grob-set-object! l 'minimum-distances kept)))
    (write (list 'rods (grob::name g) props cols
                 (let ((s (ly:grob-property g 'stencil)))
                   (if (ly:stencil? s) (ly:stencil-extent s X) #f))
                 before (or after (dist l r))))
    (newline)))
\layout {
  \context { \Score \override NonMusicalPaperColumn.horizontal-skylines = ##f }
  \context { \Voice \override MultiMeasureRest.springs-and-rods = #rods-probe
              \override MultiMeasureRestNumber.springs-and-rods = #rods-probe }
}
\new Staff {
  \compressEmptyMeasures
  R1 R1*2 R1*3 R1*7 R1*11 R1*20
  \time 3/4 R2.*5
  \time 4/2 R\breve*2
  \time 3/8 R4.*4
  \time 4/4
  \override MultiMeasureRest.space-increment = 5 R1*6
  \override MultiMeasureRest.bound-padding = 2 R1*6
  \override MultiMeasureRest.minimum-length = 30 R1*6
  \override MultiMeasureRest.minimum-length = 3 R1*6
  \override MultiMeasureRest.expand-limit = 3 R1*4
  \override MultiMeasureRestNumber.bound-padding = 3 R1*12
  \override MultiMeasureRestNumber.minimum-length = 20 R1*12
  \override MultiMeasureRestNumber.stencil = ##f R1*12
}
"##;

/// The probes' fresh grobs: `MMRest`, a multi-measure rest spanner, and
/// `MMNumber`, its number.
const MMR_GROBS: &str = "(MMRest (meta (name . MMRest) (classes Spanner)
                       (interfaces grob-interface spanner-interface font-interface
                                   multi-measure-rest-interface rest-interface
                                   staff-symbol-referencer-interface)))
        (MMNumber (meta (name . MMNumber) (classes Spanner)
                         (interfaces grob-interface spanner-interface font-interface
                                     multi-measure-rest-number-interface text-interface)))
        (Span (meta";

/// [`print_vm`]'s VM with [`set_layout`]'s staff space and fresh rests,
/// numbers, columns on `(group 0)`, staves and things, and `(mmr-case
/// case)`: the height, print and `staff-position` of a fresh rest of
/// [`MMR_PROBE`]'s case, on a fresh staff, between fresh columns at the
/// case's X with break alignments of its extents, translated to its staff
/// position; and `(rods-case line)`: the rod from the left to the right
/// fresh column that a fresh rest or number of [`MMR_RODS_PROBE`]'s line
/// sets, the rest of `define-grobs.scm`'s properties on the staff of five
/// lines, the number of a stencil of the line's X extent. The columns are
/// not broken: the rods to and from their pieces are LilyPond's loop.
fn mmr_vm(packs: &PathBuf) -> Option<Vm> {
    let defs: Vec<String> = DEFS
        .iter()
        .map(|d| d.replace("(Span (meta", MMR_GROBS))
        .collect();
    let defs: Vec<&str> = defs.iter().map(String::as_str).collect();
    let mut vm = print_vm_of(
        packs,
        &defs,
        "'((Group . 1) (Thing . 300) (Column . 300) (Staff . 140) (MMRest . 140) (MMNumber . 40))",
    )?;
    set_layout(&mut vm);
    eval(
        &mut vm,
        "(define counts '((Column . 1)))
         (define (fresh kind)
           (let ((n (or (assq-ref counts kind) 0)))
             (set! counts (assq-set! counts kind (1+ n)))
             (made-of kind n)))
         (define (dumped s) (if (ly:stencil? s) (%stencil->dump s) s))
         (define (put-all! g syms vs)
           (for-each (lambda (sym v) (if (not (null? v)) (put! g sym v))) syms vs))
         (define (col-of c)
           (let ((col (fresh 'Column)))
             (ly:grob-set-parent! col 0 (group 0))
             (put! col 'X-offset (car c))
             (if (cadr c)
                 (let ((ba (fresh 'Thing)))
                   (ly:grob-set-parent! ba 0 col)
                   (put! ba 'X-extent (cadr c))
                   (ly:grob-set-object! col 'break-alignment ba)))
             col))
         (define (rest-between g st l r)
           (let ((y (fresh 'Thing)))
             (put! g 'fonts (ly:grob-property (column 0) 'fonts))
             (ly:grob-set-object! g 'staff-symbol st)
             (ly:grob-set-parent! g 1 y)
             (ly:grob-set-parent! st 1 y)
             (ly:spanner-set-bound! g -1 l)
             (ly:spanner-set-bound! g 1 r)))
         (define (mmr-case case)
           (let* ((at (lambda (i) (list-ref case i)))
                  (g (fresh 'MMRest)) (st (fresh 'Staff))
                  (l (col-of (at 7))) (r (col-of (at 8))))
             (put-all! g '(measure-count expand-limit usable-duration-logs round-up-exceptions
                           round-up-to-longer-rest max-symbol-separation thick-thickness
                           hair-thickness font-size direction voiced-position spacing-pair)
                       (at 0))
             (put-all! g '(staff-position) (list (at 1)))
             (put! st 'staff-space (at 3))
             (put! st 'line-positions (or (at 4) '()))
             (ly:output-def-set-variable! (ly:grob-layout g) 'line-thickness (at 5))
             (put! l 'measure-length (ly:make-moment (at 6)))
             (rest-between g st l r)
             (if (not (at 4)) (ly:grob-set-object! g 'staff-symbol '()))
             (ly:grob-translate-axis!
              g (* 0.5 (ly:staff-symbol-staff-space g) (- (at 2) (ly:grob-staff-position g))) 1)
             (let* ((h (ly:multi-measure-rest::height g))
                    (s (ly:multi-measure-rest::print g)))
               (list (dumped s) h (ly:grob-property g 'staff-position)))))
         (define (rods-case line)
           (let* ((at (lambda (i) (list-ref line i)))
                  (rest? (eq? (at 1) 'MultiMeasureRest))
                  (g (fresh (if rest? 'MMRest 'MMNumber)))
                  (l (fresh 'Column)) (r (fresh 'Column))
                  (cols (at 3)))
             (put-all! g '(measure-count space-increment bound-padding minimum-length expand-limit)
                       (at 2))
             (put! l 'measure-length (ly:make-moment (car cols)))
             (put! l 'full-measure-extra-space (cadr cols))
             (if (caddr cols) (ly:grob-set-object! l 'spacing (fresh 'Thing)))
             (if (cadddr cols) (ly:grob-set-object! r 'spacing (fresh 'Thing)))
             (if rest?
                 (let ((st (fresh 'Staff)))
                   (put-all! g '(thick-thickness hair-thickness max-symbol-separation
                                 usable-duration-logs voiced-position spacing-pair)
                             '(6.6 2.0 8.0 (-3 -2 -1 0) 4 (break-alignment . break-alignment)))
                   (put! st 'line-positions '(4.0 2.0 0.0 -2.0 -4.0))
                   (ly:output-def-set-variable! (ly:grob-layout g) 'line-thickness
                                                0.09999999999999999)
                   (rest-between g st l r)
                   (ly:multi-measure-rest::set-spacing-rods g))
                 (begin
                   (ly:spanner-set-bound! g -1 l)
                   (ly:spanner-set-bound! g 1 r)
                   (put! g 'stencil (and (at 4) (ly:make-stencil '() (at 4) '(0 . 0))))
                   (ly:multi-measure-rest::set-text-rods g)))
             (let ((d (assq r (ly:grob-object l 'minimum-distances))))
               (and d (cdr d)))))",
    );
    Some(vm)
}

/// LilyPond's output of `probe` (`stdout`) and its diagnostics
/// (`stderr`), run in `layout-mmr-probe`.
fn run_probe(name: &str, probe: &str) -> (String, String) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("layout-mmr-probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), probe).unwrap();
    let out = Command::new("lilypond")
        .args(["-dno-print-pages", name])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{stderr}");
    (String::from_utf8(out.stdout).unwrap(), stderr)
}

#[test]
fn multi_measure_rests_are_printed_as_lilypond_does() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the rests are unverified");
        return;
    };
    let Some(mut vm) = mmr_vm(packs) else {
        return;
    };
    let (lilypond, stderr) = run_probe("mmr.ly", MMR_PROBE);
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 65, "{lilypond}");
    let count = |what: &str| lines.iter().filter(|l| l.contains(what)).count();
    // The cases the probe means to cover are there: church rests and big
    // ones, the measures' lengths, the staves.
    for (what, n) in [
        ("\"rests.M3\"", 4),
        ("\"rests.M2\"", 20),
        ("\"rests.M1\"", 28),
        ("\"rests.M1o\"", 4),
        ("\"rests.0\"", 30),
        ("\"rests.0o\"", 10),
        ("\"rests.1\"", 1),
        ("round-filled-box", 8),
        (" 3/4 (", 3),
        (" 2 (", 2),
        (" 3/2 (", 4),
        (" 3/8 (", 1),
        (" 5/4 (", 1),
        (" (0.0) 0.09", 9),
        (" () 0.09", 2),
        (" 1.5 (", 4),
        (" (-5 -1 3 7) ", 3),
        (" #f 0.09", 3),
    ] {
        assert_eq!(count(what), n, "{what}");
    }
    for line in &lines {
        let got = caught(&mut vm, &format!("(mmr-case (cadr '{line}))"));
        let expected = caught(&mut vm, &format!("(caddr '{line})"));
        assert_eq!(otf_names(&got), otf_names(&expected), "{line}");
    }
    // The case without usable durations: LilyPond's warning at its
    // height and its print, which LilyPond's layout also ran.
    let warning = "usable-duration-logs must be a non-empty list.  Falling back to whole rests.";
    assert_eq!(stderr.matches(warning).count(), 4, "{stderr}");
    let diagnostics = diagnosed(&mut vm);
    assert_eq!(diagnostics.matches(warning).count(), 2, "{diagnostics}");
    assert_eq!(diagnostics.matches("\" \"").count(), 1, "{diagnostics}");
}

#[test]
fn multi_measure_rests_and_their_numbers_set_rods_as_lilypond_does() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the rods are unverified");
        return;
    };
    let Some(mut vm) = mmr_vm(packs) else {
        return;
    };
    let (lilypond, _) = run_probe("rods.ly", MMR_RODS_PROBE);
    let lines: Vec<&str> = lilypond.lines().collect();
    assert_eq!(lines.len(), 33, "{lilypond}");
    // A rest's and its number's rods, a number without a stencil, the
    // lengths of 4/4, 3/4, 4/2 and 3/8, and minimum lengths.
    for (what, n) in [
        ("(rods MultiMeasureRest ", 17),
        ("(rods MultiMeasureRestNumber ", 16),
        (" #f #f ", 1),
        (" 30 10) ", 1),
        (" 20 ()) ", 2),
        (" 3 20 ", 2),
    ] {
        assert_eq!(
            lines.iter().filter(|l| l.contains(what)).count(),
            n,
            "{what}"
        );
    }
    for line in &lines {
        let got = caught(&mut vm, &format!("(rods-case '{line})"));
        let expected = caught(&mut vm, &format!("(list-ref '{line} 6)"));
        assert_eq!(got, expected, "{line}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_multi_measure_rest_s_odd_properties_are_read_as_lilypond_reads_them() {
    let Some(packs) = print_packs() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the print is not run");
        return;
    };
    let Some(mut vm) = mmr_vm(packs) else {
        return;
    };
    eval(
        &mut vm,
        "(define (odd-rest . props)
           (let ((g (fresh 'MMRest)) (st (fresh 'Staff))
                 (l (col-of '(0.0 #f))) (r (col-of '(20.0 #f))))
             (put-all! g '(measure-count expand-limit usable-duration-logs voiced-position
                           spacing-pair)
                       '(3 10 (-3 -2 -1 0) 4 (break-alignment . break-alignment)))
             (put! st 'line-positions '(4 2 0 -2 -4))
             (rest-between g st l r)
             (let loop ((p props))
               (if (pair? p) (begin (put! g (car p) (cadr p)) (loop (cddr p)))))
             g))",
    );
    // No measure count, or none positive: nothing, no staff position.
    for count in ["'()", "0", "-2"] {
        let got = caught(
            &mut vm,
            &format!(
                "(let ((g (odd-rest 'measure-count {count})))
                   (list (ly:multi-measure-rest::height g)
                         (ly:stencil-expr (ly:multi-measure-rest::print g))
                         (ly:grob-property g 'staff-position)))"
            ),
        );
        assert_eq!(got, "((+inf.0 . -inf.0) () ())", "{count}");
    }
    // A measure count or a limit that is no int: scm_to_int's error.
    for (prop, v) in [("measure-count", "2.5"), ("expand-limit", "'x")] {
        let got = caught(
            &mut vm,
            &format!("(ly:multi-measure-rest::print (odd-rest '{prop} {v}))"),
        );
        assert!(
            got.contains("Wrong type (expecting exact integer)"),
            "{prop}: {got}"
        );
    }
    // Usable durations that are no proper list: LilyPond's warning, and
    // measures of whole rests: three of them a breve's and a whole one.
    let got = caught(
        &mut vm,
        "(let ((g (odd-rest 'usable-duration-logs '(-1 . 0))))
           (ly:stencil-expr (ly:multi-measure-rest::print g)))",
    );
    assert_eq!(got.matches("\"rests.").count(), 2, "{got}");
    assert!(
        got.contains("\"rests.M1\"") && got.contains("\"rests.0\""),
        "{got}"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"usable-duration-logs must be a non-empty list.  Falling back to whole rests.\")"
    );
    // A spacing-pair that is no pair: the columns' staff bars, none here,
    // so their coordinates: the symbol across 20 staff spaces.
    let got = caught(
        &mut vm,
        "(let ((g (odd-rest 'spacing-pair #f 'measure-count 20)))
           (ly:stencil-extent (ly:multi-measure-rest::print g) 0))",
    );
    assert_eq!(got, "(3.0 . 17.0)");
    // A rest without both bounds sets no rod, with LilyPond's programming
    // error.
    let got = caught(
        &mut vm,
        "(let ((g (fresh 'MMRest)))
           (put! g 'measure-count 2)
           (ly:multi-measure-rest::set-text-rods g))",
    );
    assert_eq!(got, "#<unspecified>");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"Multi measure rest seems misplaced.\")"
    );
}

// --- hairpins and brackets (MS3 step 49) ------------------------------------

/// The spanner callbacks of `hairpin.cc`, `volta-bracket.cc`,
/// `ottava-bracket.cc` and `horizontal-bracket.cc`.
const BRACKET_CALLBACKS: [&str; 5] = [
    "ly:hairpin::broken-bound-padding",
    "ly:hairpin::print",
    "ly:horizontal-bracket::print",
    "ly:ottava-bracket::print",
    "ly:volta-bracket-interface::print",
];

/// The six are layout's, not the music crate's placeholders: LilyPond's
/// subrs as they print, each checking its argument; their stencils are
/// LilyPond's (`tools/compare/test/layout.test.mjs`' `bracket-prints.ly`),
/// and the pure height is T18's hairpins' (its layout.tsv).
#[test]
fn the_hairpin_s_and_the_brackets_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in BRACKET_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        // Where LilyPond's dereference what is no spanner, the spanner
        // checked under the full name.
        for arg in ["'x", "(thing 0)"] {
            let got = caught(&mut vm, &format!("({name} {arg})"));
            assert!(
                got.contains(name)
                    && got.contains("Wrong type argument in position 1 (expecting Spanner)"),
                "{name}: {got}"
            );
        }
        let got = caught(&mut vm, &format!("({name})"));
        assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
    }
    let name = "ly:hairpin::pure-height";
    assert!(music.iter().all(|p| p.name != name));
    assert_eq!(
        caught(&mut vm, name),
        format!("#<procedure {name} (_ _ _)>")
    );
    // LY_ASSERT_SMOB (Grob, smob, 1), under the method's name.
    let got = caught(&mut vm, &format!("({name} 'x 0 1)"));
    assert!(
        got.contains("ly:pure-height")
            && got.contains("Wrong type argument in position 1 (expecting Grob)"),
        "{got}"
    );
}

// --- line spanners (MS3 step 52) --------------------------------------------

/// The callbacks of `line-spanner.cc`: the seven that were the music
/// crate's placeholders, and the left bound info with its text of a line
/// spanner that is not horizontal, which no grob reads.
const LINE_SPANNER_CALLBACKS: [&str; 8] = [
    "ly:horizontal-line-spanner::calc-left-bound-info",
    "ly:horizontal-line-spanner::calc-left-bound-info-and-text",
    "ly:horizontal-line-spanner::calc-right-bound-info",
    "ly:line-spanner::calc-cross-staff",
    "ly:line-spanner::calc-left-bound-info",
    "ly:line-spanner::calc-left-bound-info-and-text",
    "ly:line-spanner::calc-right-bound-info",
    "ly:line-spanner::print",
];

/// The eight are layout's, not the music crate's placeholders: LilyPond's
/// subrs as they print. A bound info checks its spanner as
/// `calc_bound_info`'s `LY_ASSERT_SMOB (Spanner, smob, 0)` does, under
/// the method's name at no position; the print under its full name, where
/// LilyPond's dereferences what is none; the cross-staff of what is no
/// spanner, or of one without bounds, is `#f`. Their values are
/// LilyPond's (`tools/compare/test/layout.test.mjs`' line-spanners
/// fixture and `line-spanner-prints.ly`).
#[test]
fn the_line_spanner_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in LINE_SPANNER_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        let got = caught(&mut vm, &format!("({name})"));
        assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        for arg in ["'x", "(thing 0)"] {
            let got = caught(&mut vm, &format!("({name} {arg})"));
            if name.ends_with("calc-cross-staff") {
                assert_eq!(got, "#f", "{name} {arg}");
            } else if name.ends_with("print") {
                assert!(
                    got.contains(name)
                        && got.contains("Wrong type argument in position 1 (expecting Spanner)"),
                    "{name}: {got}"
                );
            } else {
                assert_eq!(
                    got,
                    format!(
                        "In procedure ly:calc-bound-info: Wrong type (expecting Spanner): {}",
                        if arg == "'x" { "x" } else { "#<Grob Thing >" }
                    ),
                    "{name} {arg}"
                );
            }
        }
    }
    assert_eq!(
        caught(&mut vm, "(ly:line-spanner::calc-cross-staff (span 0))"),
        "#f"
    );
}

/// `ly:line-interface::line` on a grob (OPEN A23): its arguments checked
/// as `LY_ASSERT_SMOB (Grob, grob, 1)` and `LY_ASSERT_TYPE (scm_is_number,
/// …)` check them, in order. The line it draws on a grob, by the grob's
/// staff symbol and properties, is LilyPond's on note heads
/// (`line-spanner-prints.ly`).
#[test]
fn the_line_interface_s_line_is_drawn_on_a_grob() {
    let Some(mut vm) = grobs_vm() else { return };
    let name = "ly:line-interface::line";
    assert_eq!(
        caught(&mut vm, name),
        format!("#<procedure {name} (_ _ _ _ _)>")
    );
    assert_eq!(
        caught(&mut vm, &format!("({name} 'x 0 'a 1 1)")),
        format!("In procedure {name}: Wrong type argument in position 1 (expecting Grob): x")
    );
    assert_eq!(
        caught(&mut vm, &format!("({name} (thing 0) 0 'a 1 1)")),
        format!("In procedure {name}: Wrong type argument in position 3 (expecting number): a")
    );
}

// --- scripts and fingerings (MS3 step 53) -----------------------------------

/// The callbacks of `script-interface.cc`, `script-column.cc` and
/// `fingering-column.cc`, the music crate's placeholders until then.
const SCRIPT_CALLBACKS: [&str; 5] = [
    "ly:fingering-column::calc-positioning-done",
    "ly:script-column::before-line-breaking",
    "ly:script-column::row-before-line-breaking",
    "ly:script-interface::calc-cross-staff",
    "ly:script-interface::calc-positioning-done",
];

/// The five are layout's, not the music crate's placeholders: LilyPond's
/// subrs as they print, each checking its grob as `LY_ASSERT_SMOB (Grob,
/// smob, 1)` does, under the method's name. A script with no X parent
/// is positioned and crosses no staff; a column with no scripts orders
/// none. Their values on scripts and fingerings are LilyPond's
/// (`tools/compare/test/layout.test.mjs`' scripts and fingerings
/// fixtures).
#[test]
fn the_script_s_and_the_fingering_column_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for name in SCRIPT_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        assert_eq!(caught(&mut vm, name), format!("#<procedure {name} (_)>"));
        let got = caught(&mut vm, &format!("({name})"));
        assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        let method = format!("ly:{}", name.split("::").nth(1).unwrap());
        assert_eq!(
            caught(&mut vm, &format!("({name} 'x)")),
            format!("In procedure {method}: Wrong type argument in position 1 (expecting Grob): x"),
            "{name}"
        );
    }
    for (call, value) in [
        ("(ly:script-interface::calc-cross-staff (thing 0))", "#f"),
        (
            "(ly:script-interface::calc-positioning-done (thing 0))",
            "#t",
        ),
        (
            "(ly:script-column::before-line-breaking (thing 0))",
            "#<unspecified>",
        ),
        (
            "(ly:script-column::row-before-line-breaking (thing 0))",
            "#<unspecified>",
        ),
    ] {
        assert_eq!(caught(&mut vm, call), value, "{call}");
    }
}

// --- arpeggios (MS3 step 54) ------------------------------------------------

/// The callbacks of `arpeggio.cc`, the music crate's placeholders until
/// then, with whether each is the pure one.
const ARPEGGIO_CALLBACKS: [(&str, bool); 9] = [
    ("ly:arpeggio::calc-cross-staff", false),
    ("ly:arpeggio::calc-positions", false),
    ("ly:arpeggio::print", false),
    ("ly:arpeggio::pure-height", true),
    ("ly:arpeggio::width", false),
    ("ly:chord-bracket::print", false),
    ("ly:chord-bracket::width", false),
    ("ly:chord-slur::print", false),
    ("ly:chord-slur::width", false),
];

/// The nine are layout's, not the music crate's placeholders: LilyPond's
/// subrs as they print, each checking its grob as `LY_ASSERT_SMOB (Grob,
/// smob, 1)` does, under the method's name. A grob of no stems crosses no
/// staff and spans no positions. Their values on arpeggios, chord
/// brackets and chord slurs are LilyPond's
/// (`tools/compare/test/layout.test.mjs`' arpeggios fixture).
#[test]
fn the_arpeggio_s_the_chord_bracket_s_and_the_chord_slur_s_callbacks_are_layout_s() {
    let Some(mut vm) = grobs_vm() else { return };
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, pure) in ARPEGGIO_CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let formals = if pure { "(_ _ _)" } else { "(_)" };
        assert_eq!(
            caught(&mut vm, name),
            format!("#<procedure {name} {formals}>")
        );
        let got = caught(&mut vm, &format!("({name})"));
        assert!(got.contains("Wrong number of arguments"), "{name}: {got}");
        let method = format!("ly:{}", name.split("::").nth(1).unwrap());
        let args = if pure { " 0 0" } else { "" };
        assert_eq!(
            caught(&mut vm, &format!("({name} 'x{args})")),
            format!("In procedure {method}: Wrong type argument in position 1 (expecting Grob): x"),
            "{name}"
        );
    }
    for (call, value) in [
        ("(ly:arpeggio::calc-cross-staff (thing 0))", "#f"),
        (
            "(ly:arpeggio::calc-positions (thing 0))",
            "(+inf.0 . -inf.0)",
        ),
    ] {
        assert_eq!(caught(&mut vm, call), value, "{call}");
    }
}
