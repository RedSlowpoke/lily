//! `paper-column.cc` (part 3's, MS3): what `Score_engraver`, `System` and
//! `Paper_column_engraver` call of it, ported first (MS3 step 15): the two
//! constructors (`paper-column.cc:84–90`, `:92–97`), `set_rank`
//! (`:60–64`) and `is_breakable` (`:138–142`), 23 lines; and what
//! `ly:staff-symbol::print` calls (MS3 step 27), `break_align_width`
//! (`:166–218`, 53) and `is_musical` (`:108–113`, 6), the image's Scheme
//! ([`PRELUDE`]); and what `ly:self-alignment-interface::aligned-on-x-parent`
//! calls (MS3 step 27), `get_interface_extent` (`:263–276`, 14), the
//! image's Scheme too. And what the spacing and breaking call of it (MS3
//! step 30), Rust with no Scheme face: `set_system` (`:72–76`, 5),
//! `when_mom` (`:99–106`, 8), `is_musical` over the image's
//! `%paper-column-musical?`, `is_used` (`:115–136`, 22),
//! `minimum_distance` (`:144–164`, 21) and
//! `is_extraneous_column_from_ligature` (`:424–444`, 21).
//!
//! A column is an item of the class `Paper_column` (`item.rs`) whose
//! `system_` and `rank_` layout keeps with its grob ([`Column`]). Its rank
//! is `VPOS` until `System::add_column` gives it one (`system.rs`); its
//! system is none until breaking sets it ([`set_system`], part 5).
//! `Paper_column_engraver` makes the columns (`paper_column_engraver.rs`),
//! and `item.rs` answers an item's system and rank from its column's.
//! Elsewhere: `get_column` is `item.rs`', `internal_set_as_bound_of_spanner`
//! `spanner.rs`' `set_bound`, and `rank_less` `simple_spacer.rs`'
//! `lower_bound_rank`, its one use. And `ly:paper-column::print`
//! (`:278–417`, 140), the image's Scheme ([`PRELUDE`]) over Rust
//! helpers (MS3 step 30).
//! `make_sticky_same_type`, LilyPond's error for a grob made to stick to a
//! column, has no caller: nothing here makes sticky grobs from an engraver
//! (`ly:engraver-make-sticky` is not ported).

use alloc::format;
use alloc::string::String;

use lily_engine_music::{Host, Moment};
use lily_engine_scheme::{Cx, Exception, Heap, Tag, Value};
use lily_engine_skyline::{Direction, Skyline};

use crate::R;
use crate::geometry::fma;
use crate::grob::{self, with_grob, with_grob_mut};
use crate::grob_array;
use crate::item::{self, Class, VPOS};
use crate::separation_item;
use crate::skylines::skyline_pair_in;

/// A column's `system_` and `rank_`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// The handle of its system, none until breaking sets it.
    pub system: Option<Value>,
    /// Its rank among its system's columns, [`VPOS`] until `add_column`.
    pub rank: u64,
}

impl Default for Column {
    fn default() -> Column {
        Column {
            system: None,
            rank: VPOS,
        }
    }
}

/// `Paper_column::Paper_column (SCM)`: the grob `grob` made a column, of
/// no system and the rank `VPOS`, with `paper-column-interface` after
/// `Item`'s `item-interface` ([`item::set_class`]).
pub fn make(heap: &mut Heap, grob: Value) {
    item::set_class(heap, grob, Class::PaperColumn);
    with_grob_mut(heap, grob, |g| g.column = Column::default());
}

/// `Paper_column::Paper_column (Paper_column const &)`: the column's own
/// part of `copy`, a copy of the column `src`: no system, `src`'s rank.
/// The grob's copy (`Item`'s) is breaking's (part 5).
pub fn copied(heap: &Heap, copy: Value, src: Value) {
    let rank = rank(heap, src);
    with_grob_mut(heap, copy, |g| g.column = Column { system: None, rank });
}

/// `set_rank`.
pub fn set_rank(heap: &Heap, column: Value, rank: u64) {
    with_grob_mut(heap, column, |g| g.column.rank = rank);
}

/// `get_rank`: the column's rank, [`VPOS`] for what is not a grob.
pub fn rank(heap: &Heap, column: Value) -> u64 {
    with_grob(heap, column, |g| g.column.rank).unwrap_or(VPOS)
}

/// `(%paper-column-rank col)`: [`rank`], which page layout reads of a
/// system's left bound (`page_layout_problem.rs`); `#f` for `VPOS` and
/// for what is not a grob, where LilyPond reads through a null.
pub(crate) fn rank_prim(cx: &mut Cx, args: &[Value]) -> R {
    match rank(cx.heap(), args[0]) {
        VPOS => Ok(Value::FALSE),
        r => Ok(cx.heap_mut().integer(r as i64)),
    }
}

/// `get_system`: the handle of the column's system, if it has one.
pub fn system(heap: &Heap, column: Value) -> Option<Value> {
    with_grob(heap, column, |g| g.column.system).flatten()
}

/// `set_system`: the column put on the system `system`.
pub fn set_system(heap: &Heap, column: Value, system: Value) {
    with_grob_mut(heap, column, |g| g.column.system = Some(system));
}

/// `when_mom`: the grob's `when`, if it is a moment, else 0.
pub fn when_mom(host: &mut dyn Host, me: Value) -> Result<Moment, Exception> {
    let m = grob::property(host, me, "when")?;
    Ok(host
        .heap()
        .foreign::<Moment>(m)
        .copied()
        .unwrap_or_default())
}

/// `is_musical`: [`PRELUDE`]'s `%paper-column-musical?` of the grob.
pub fn is_musical(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let musical = crate::lily_procedure(host, "%paper-column-musical?")?;
    Ok(host.call(musical, &[me])? != Value::FALSE)
}

/// `is_used`: whether the column has `elements`, is `bounded-by-me`, is
/// breakable, is `used` (`#t` only, `from_scm<bool>`) or has `labels`
/// that are a pair.
pub fn is_used(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    for name in ["elements", "bounded-by-me"] {
        if !grob_array::extract_grob_array(host, me, name)?.is_empty() {
            return Ok(true);
        }
    }
    if is_breakable(host, me)? || grob::property(host, me, "used")? == Value::TRUE {
        return Ok(true);
    }
    let labels = grob::property(host, me, "labels")?;
    Ok(host.heap().car(labels).is_some())
}

/// `is_breakable`: whether the grob's `line-break-permission` is a symbol.
pub fn is_breakable(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let v = grob::property(host, me, "line-break-permission")?;
    Ok(v.tag() == Tag::Symbol)
}

/// `minimum_distance`: how far the column `right` must be from the column
/// `left` for the skyline bounding `left`'s `horizontal-skylines` on the
/// right not to meet the one bounding `right`'s on the left, merged with
/// `right`'s `conditional_skyline` given `left`; at least 0. A column
/// whose `horizontal-skylines` is no skyline pair has an empty skyline
/// there; a pair the wrong way round is `from_scm`'s error.
pub fn minimum_distance(host: &mut dyn Host, left: Value, right: Value) -> Result<f64, Exception> {
    let cols = [left, right];
    let mut skys = [
        Skyline::empty(Direction::Up),
        Skyline::empty(Direction::Down),
    ];
    // LEFT then RIGHT, each taking its pair's side facing the other.
    for (d, facing) in [Direction::Up, Direction::Down].into_iter().enumerate() {
        let sp_scm = grob::property(host, cols[d], "horizontal-skylines")?;
        let heap = host.heap();
        match skyline_pair_in(heap, sp_scm) {
            Ok(Some(sp)) => skys[d] = sp[facing].clone(),
            Ok(None) => {}
            Err(message) => return Err(crate::misc_error(heap, "from_scm", message)),
        }
    }
    let cond = separation_item::conditional_skyline(host, right, left)?;
    skys[1].merge(&cond);
    let d = skys[0].distance(&skys[1], 0.0);
    Ok(if 0.0 < d { d } else { 0.0 })
}

/// `is_extraneous_column_from_ligature`: whether the column is musical
/// and has rhythmic heads among its `elements`, none of them in it (a
/// ligature engraver moved them). A rhythmic head that is no item, where
/// LilyPond's `dynamic_cast` gives a null it reads through, is in no
/// column.
pub fn is_extraneous_column_from_ligature(
    host: &mut dyn Host,
    me: Value,
) -> Result<bool, Exception> {
    if !is_musical(host, me)? {
        return Ok(false);
    }
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    let heap = host.heap();
    let mut has_notehead = false;
    for e in elts {
        if grob::has_interface(heap, e, "rhythmic-head-interface") {
            has_notehead = true;
            if item::is_item(heap, e) && item::get_column(heap, e) == Some(me) {
                return Ok(false);
            }
        }
    }
    Ok(has_notehead)
}

// --- what `print` computes with --------------------------------------------------

/// `(%paper-column-rank-string me)`: the column's rank as `std::to_string`
/// writes it, `VPOS`'s too; `ly:paper-column::print`'s wrong-type-arg for
/// what is no paper column, where LilyPond reads through a null.
pub(crate) fn rank_string(cx: &mut Cx, args: &[Value]) -> R {
    let me = args[0];
    if item::class(cx.heap_mut(), me) != Some(Class::PaperColumn) {
        return Err(cx.wrong_type("ly:paper-column::print", 1, me, "Paper_column"));
    }
    let r = format!("{}", rank(cx.heap(), me));
    Ok(cx.heap_mut().string(&r))
}

/// `(%paper-column-when-string when)`: `Moment::to_string` of a moment,
/// `?/?` for anything else.
pub(crate) fn when_string(cx: &mut Cx, args: &[Value]) -> R {
    let mut when = String::new();
    match cx.heap().foreign::<Moment>(args[0]) {
        Some(m) => m.write(&mut when),
        None => when.push_str("?/?"),
    }
    Ok(cx.heap_mut().string(&when))
}

/// `(%paper-column-distance-string x)`: `form_string ("%5.2lf", x)`. Rust
/// rounds the exact value to two places, ties to even, as Apple's libc
/// does; only its not-a-number is spelt otherwise.
pub(crate) fn distance_string(cx: &mut Cx, args: &[Value]) -> R {
    let x = crate::number_arg(cx, "%paper-column-distance-string", 1, args[0])?;
    let s = if x.is_nan() {
        format!("{:>5}", "nan")
    } else {
        format!("{x:5.2}")
    };
    Ok(cx.heap_mut().string(&s))
}

/// `(%paper-column-arrow-y j height base)`: the `j`th arrow's Y, `base`
/// less `j` times `height` and both paddings, the multiply fused as the
/// binary's `fmsub` does it.
pub(crate) fn arrow_y(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%paper-column-arrow-y";
    let j = crate::number_arg(cx, SUBR, 1, args[0])?;
    let height = crate::number_arg(cx, SUBR, 2, args[1])?;
    let base = crate::number_arg(cx, SUBR, 3, args[2])?;
    let (small_pad, big_pad) = (0.15, 0.35);
    Ok(cx
        .heap_mut()
        .flonum(fma(-j, height + small_pad + big_pad, base)))
}

/// `%break-align-width`, `break_align_width` (and LilyPond's
/// `ly:paper-column::break-align-width` over it, since MS8 step 7's
/// part 2, `centered-spanner-interface::calc-x-offset`'s): the X extent of the column
/// `me`'s break-align group of the first of `align-syms` (a symbol or a
/// list of them) whose group has one, relative to the column's X parent,
/// or of its whole break alignment for `break-alignment`; the column's
/// coordinate twice if none has, if it has no break alignment, or, after a
/// programming error, if it is musical. And `%paper-column-musical?`,
/// `is_musical`: a `shortest-starter-duration` that is an exact real or an
/// infinite one, and not 0 (`from_scm<Rational>`). And
/// `%paper-column-interface-extent`, `get_interface_extent`: the union of
/// the robust extents on `a`, each relative to itself, of the column's
/// `elements` with the interface `iface`; empty for none.
///
/// And `ly:paper-column::print` (`:278–417`, 140), the debugging stencil
/// of a column: its rank over its `when`, a line down from it, and an
/// arrow for each spring of its `ideal-distances` to a column with a
/// system (blue, from 2.5 below the line's top) and each distance of its
/// `minimum-distances` to a column of its own system (red, from 3), each
/// arrow labelled with its length (`%5.2lf`), one under the other.
/// `Stencil` methods are the `ly:stencil-…` that call them, but
/// `add_stencil` (`%stencil-add-stencil`), which `ly:stencil-add` does not
/// nest the same way. Where LilyPond reads through a null, a `p` that is
/// no paper column is `ly:paper-column::print`'s wrong-type-arg, and a
/// spring that is none `%spring-ideal-distance`'s.
pub(crate) const PRELUDE: &str = r#"
(define (%paper-column-length i)
  (if (> (car i) (cdr i)) 0.0 (- (cdr i) (car i))))
(define (%paper-column-arrow me musfont dist j base dir pad color)
  (let* ((arrowhead (ly:stencil-scale (ly:font-get-glyph musfont "arrowheads.open.01") 1 1.66))
         (head-len (%paper-column-length (ly:stencil-extent arrowhead 0)))
         (number (ly:stencil-scale
                  (%grob-interpret-markup me (%paper-column-distance-string dist)) 1 1.1))
         (num-height (%paper-column-length (ly:stencil-extent number 1)))
         (num-len (%paper-column-length (ly:stencil-extent number 0)))
         (number (ly:stencil-aligned-to number 1 dir))
         (y (%paper-column-arrow-y j num-height base))
         (num-off (cons (/ (- (- dist num-len) head-len) 2) (+ y pad)))
         (p2 (cons dist y))
         (id (%points-to-line-stencil 0.1 (list (cons 0 y) p2)))
         (id (%stencil-add-stencil id (ly:stencil-translate arrowhead p2)))
         (id (%stencil-add-stencil id (ly:stencil-translate number num-off))))
    (stencil-with-color id color)))
(define-public (ly:paper-column::print p)
  (let* ((r (%paper-column-rank-string p))
         (when (%paper-column-when-string (ly:grob-property p 'when)))
         (musfont (ly:grob-default-font p))
         (t (%grob-interpret-markup p r))
         (when-mol (%grob-interpret-markup p when))
         (t (ly:stencil-scale t 1.2 1.4))
         (t (ly:stencil-combine-at-edge t 1 -1 when-mol 0.1))
         (t (ly:stencil-aligned-to t 0 -1))
         ;; compensate for font serifs and half letter-distance
         (t (ly:stencil-translate t '(-0.1 . 0)))
         (t (ly:stencil-aligned-to t 1 -1))
         (l (ly:round-filled-box '(0 . 0.02) '(-8 . -1) 0.0)))
    ;; j: the number of arrows drawn, of both loops.
    (let ideal ((s (ly:grob-object p 'ideal-distances)) (j 0) (l l))
      (if (pair? s)
          (let ((sp (caar s)) (other (cdar s)))
            (if (or (not (ly:grob? other)) (null? (ly:grob-system other)))
                (ideal (cdr s) j l)
                (ideal (cdr s) (1+ j)
                       (%stencil-add-stencil
                        l (%paper-column-arrow p musfont (%spring-ideal-distance sp) (1+ j)
                                               -2.5 -1 0.15 '(0.2 0.4 1.0 1.0))))))
          (let minimum ((s (ly:grob-object p 'minimum-distances)) (j j) (l l))
            (if (pair? s)
                (let* ((dist (%scm->double (cdar s))) (other (caar s)))
                  (if (or (not (ly:grob? other))
                          (not (eq? (ly:grob-system other) (ly:grob-system p))))
                      (minimum (cdr s) j l)
                      (minimum (cdr s) (1+ j)
                               (%stencil-add-stencil
                                l (%paper-column-arrow p musfont dist (1+ j)
                                                       -3.0 1 -0.15 '(1.0 0.25 0.25 1.0))))))
                (%stencil-add-stencil t l)))))))
(define (%paper-column-interface-extent column iface a)
  (let loop ((elts (%grob-set column 'elements)) (ext '(+inf.0 . -inf.0)))
    (cond ((null? elts) ext)
          ((%grob-has-interface? (car elts) iface)
           (let ((e (%grob-robust-relative-extent (car elts) (car elts) a)))
             (loop (cdr elts) (cons (min (car ext) (car e)) (max (cdr ext) (cdr e))))))
          (else (loop (cdr elts) ext)))))
(define (%paper-column-musical? me)
  (let ((st (ly:grob-property me 'shortest-starter-duration)))
    (and (real? st) (or (exact? st) (inf? st)) (not (zero? st)))))
(define (%break-align-width me align-syms)
  (let* ((sys (ly:grob-parent me 0))
         (coord (%grob-relative-coordinate me sys 0))
         (align-syms (if (symbol? align-syms) (list align-syms) align-syms)))
    (if (%paper-column-musical? me)
        (begin
          (%grob-programming-error me "tried to get break-align-width of a musical column")
          (cons coord coord))
        (let ((break-alignment (ly:grob-object me 'break-alignment)))
          (if (not (ly:item? break-alignment))
              (cons coord coord)
              (let* ((align (let loop ((s align-syms))
                              (cond ((not (pair? s)) #f)
                                    ((eq? (car s) 'break-alignment) break-alignment)
                                    ((%find-nonempty-break-align-group break-alignment (car s)))
                                    (else (loop (cdr s))))))
                     (ext (if align (%grob-extent align me 0) '(+inf.0 . -inf.0))))
                (if (> (car ext) (cdr ext))
                    (cons coord coord)
                    (cons (+ (car ext) coord) (+ (cdr ext) coord)))))))))
(define-public (ly:paper-column::break-align-width col align-syms)
  (%check-type "ly:paper-column::break-align-width" 1 ly:grob? "Grob" col)
  (%check-type "ly:paper-column::break-align-width" 2
               (lambda (x) (or (symbol? x) (list? x))) "symbol or list" align-syms)
  (%break-align-width col align-syms))
"#;
