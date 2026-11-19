//! `note-head.cc` and `note-head-scheme.cc` (part 3's, MS3 step 37): the
//! note head's callbacks `ly:note-head::print` (`note-head.cc:114–122`, 9
//! lines) over `internal_print` (`:37–96`, 60), `::stem-x-shift`
//! (`:101–112`, 12), `::include-ledger-line-height` (`:124–148`, 25),
//! `::calc-stem-attachment` (`:198–214`, 17) and
//! `::calc-tab-stem-attachment` (`:221–234`, 14), the music crate's
//! placeholders until then (D67); `get_stem_attachment` (`:164–196`, 33)
//! with what it reads of the font, `Open_type_font::attachment_point`
//! (`open-type-font.cc:332–369`, 38) and `Modified_font_metric`'s
//! (`modified-font-metric.cc:91–97`, 7), under its Scheme name
//! `ly:note-head::stem-attachment` (`note-head-scheme.cc:24–48`, 25); and
//! `stem_attachment_coordinate` (`:150–156`, 7), which `stem.rs` ported
//! first (MS3 step 31).
//!
//! The callbacks are the image's Scheme, as layout's others are: they read
//! properties through their callbacks, which a primitive cannot call.
//! `internal_print` asks two functions of other files first:
//! `get_strict_grob_direction` (`directional-element-interface.cc:25–36`
//! and `:59–63`, part 3's with the stem, MS3 step 38), here as
//! `%strict-grob-direction`, and `Staff_symbol_referencer::on_line`
//! (`staff-symbol-referencer.cc:26–31`) for the mensural styles, over
//! `Staff_symbol::on_line` and its ledgers (`staff_symbol.rs`, ported
//! first for the dot column, MS3 step 37; OPEN A6).
//!
//! Where LilyPond's C++ would crash, the doc comments of [`PRELUDE`] say
//! what the port does instead.

use alloc::string::String;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Heap, Value};

use crate::font;
use crate::geometry::{Axis, Box, Direction, Interval, Offset, X, Y};
use crate::grob;
use crate::pango;

type R = Result<Value, lily_engine_scheme::Thrown>;

const UP: Direction = 1;
const DOWN: Direction = -1;

/// `stem_attachment_coordinate`: the head's `stem-attachment` on `a`, `(0
/// . 0)` if it is no pair of numbers.
pub fn stem_attachment_coordinate(
    host: &mut dyn Host,
    me: Value,
    a: Axis,
) -> Result<f64, Exception> {
    let v = grob::property(host, me, "stem-attachment")?;
    let heap = host.heap();
    let end = if a == X { heap.car(v) } else { heap.cdr(v) };
    match (heap.car(v), heap.cdr(v)) {
        (Some(x), Some(y)) if heap.is_number(x) && heap.is_number(y) => {
            let e = end.unwrap_or(Value::NULL);
            grob::to_double(heap, e)
        }
        _ => Ok(0.0),
    }
}

/// `attachment_point` of the font `m` for the glyph `name`: its `LILC`
/// entry's `attachment-down` for a down-stem, else its `attachment`
/// (whether it was taken for a down-stem, to be rotated), in points scaled
/// by the font's magnification; `(0, 0)` for a glyph without an entry or
/// a direction of 0, and with LilyPond's warning for one without an
/// `attachment`.
fn attachment_point(
    m: font::Metric<'_>,
    name: &str,
    d: Direction,
) -> (Offset, bool, Option<String>) {
    let Some(entry) = m.font.metrics(name) else {
        return ((0.0, 0.0), false, None);
    };
    let mut att = Some([0.0, 0.0]);
    let mut rotate = false;
    if d == DOWN {
        att = entry.attachment_down;
        if att.is_none() {
            rotate = true;
        }
    }
    if d == UP || rotate {
        att = entry.attachment;
        if att.is_none() {
            let text = alloc::format!("no stem attachment found in font for glyph {name}");
            return ((0.0, 0.0), false, Some(text));
        }
    }
    let [x, y] = att.unwrap_or([0.0, 0.0]);
    let o = (lily_engine_fonts::POINT * x, lily_engine_fonts::POINT * y);
    ((o.0 * m.magnification, o.1 * m.magnification), rotate, None)
}

/// `get_stem_attachment`: where on the glyph `key` of `fm` a stem in the
/// direction `dir` attaches, each axis from -1 at the box's left or bottom
/// edge to 1 at its right or top one, rotated about the centre where the
/// font gives no attachment for a down-stem; `(0, 0)` for a glyph the font
/// has not (of a Pango font, any), and the warning `attachment_point`
/// gives.
fn get_stem_attachment(
    heap: &Heap,
    fm: Value,
    key: &str,
    dir: Direction,
) -> (Offset, Option<String>) {
    let mut att = [0.0, 0.0];
    let key = key.replace('-', "M");
    let Some(m) = font::metric(heap, fm) else {
        return ((0.0, 0.0), None);
    };
    let Some(k) = m.font.name_to_index(&key) else {
        return ((0.0, 0.0), None);
    };
    let c = m.font.char_dimensions(k);
    let mut b = Box::new(
        Interval::new(c.x.left, c.x.right),
        Interval::new(c.y.left, c.y.right),
    );
    b.scale(m.magnification);
    let (wxwy, rotate, warning) = attachment_point(m, &key, dir);
    for (a, w) in [(X, wxwy.0), (Y, wxwy.1)] {
        let v = if a == X { b.x() } else { b.y() };
        if !v.is_empty() {
            att[a] = 2.0 * (w - v.center()) / v.length();
        }
    }
    if rotate {
        att = [-att[0], -att[1]];
    }
    ((att[0], att[1]), warning)
}

/// `(ly:note-head::stem-attachment font-metric glyph-name [direction])`:
/// [`get_stem_attachment`], up without a direction.
pub(crate) fn ly_stem_attachment(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:note-head::stem-attachment";
    let heap = cx.heap();
    if font::metric(heap, args[0]).is_none() && pango::pango_font(heap, args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Font_metric"));
    }
    let Some(key) = cx.heap().str(args[1]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type(SUBR, 2, args[1], "string"));
    };
    let dir = match args.get(2) {
        None => UP,
        Some(&d) => crate::direction_arg(cx, SUBR, 3, d)?,
    };
    let (att, warning) = get_stem_attachment(cx.heap(), args[0], &key, dir);
    let value = crate::offset(cx.heap_mut(), att);
    crate::warned(cx, value, warning.as_slice())
}

/// The callbacks, and what they compute with:
///
/// - `%int-or`, `from_scm<int> (v, fallback)`: an exact integer of an
///   `int`'s range, else the fallback.
/// - `%strict-grob-direction`, `get_strict_grob_direction`: the grob's
///   `direction` if up or down; else LilyPond's warning, and the grob set
///   up.
/// - `%staff-on-line?`, `Staff_symbol_referencer::on_line`: whether the
///   position `pos` is on one of the staff symbol's lines or the ledger
///   lines a head there needs (`staff_symbol.rs`' `on_line`, MS3 step
///   37), `#f` without a staff symbol.
/// - `%note-head-internal-print`, `internal_print`: the head's stencil
///   and the glyph's name, `""` where none is found.
///
/// `::calc-stem-attachment` and `::calc-tab-stem-attachment` of a head
/// without a stem take an up-stem, where LilyPond reads the direction of
/// no grob. `internal_print` of a `kievan` head of an eighth without a
/// stem takes it as unbeamed.
pub(crate) const PRELUDE: &str = r#"
(define (%int-or v fallback)
  (if (and (exact-integer? v) (<= -2147483648 v 2147483647)) v fallback))
(define (%strict-grob-direction me)
  (let ((d (%direction-or (ly:grob-property me 'direction) 0)))
    (if (eqv? d 0)
        (begin
          (%grob-warning me (string-append "direction of grob " (%grob-name me)
                                           " must be UP or DOWN; using UP"))
          (ly:grob-set-property! me 'direction 1)
          1)
        d)))
(define (%staff-on-line? me pos)
  (let ((st (%staff-symbol me)))
    (and st (%host-call 'staff-symbol-on-line st pos))))
(define (%note-head-internal-print me)
  (let* ((style (let ((s (ly:grob-property me 'style)))
                  (if (symbol? s) (symbol->string s) "default")))
         (suffix (number->string (min (%int-or (ly:grob-property me 'duration-log) 2) 2)))
         (suffix (if (string=? style "default")
                     suffix
                     (let ((g (ly:grob-property me 'glyph-name))) (if (string? g) g ""))))
         (fm (ly:grob-default-font me))
         (symmetric "noteheads.s")
         (directed "")
         (either symmetric)
         (out (ly:font-get-glyph fm (string-append either suffix))))
    (if (ly:stencil-empty? out)
        (let ((dir (%strict-grob-direction me)))
          (set! directed (if (eqv? dir 1) "noteheads.u" "noteheads.d"))
          (set! either directed)
          (set! out (ly:font-get-glyph fm (string-append either suffix)))))
    (if (and (member style '("mensural" "neomensural" "petrucci" "baroque" "kievan"))
             (not (%staff-on-line? me (%int-or (ly:grob-property me 'staff-position) 0))))
        (let ((test (ly:font-get-glyph fm (string-append either "r" suffix))))
          (if (not (ly:stencil-empty? test))
              (begin
                (set! either (string-append either "r"))
                (set! out test)))))
    (if (and (string=? style "kievan")
             (eqv? 3 (%int-or (ly:grob-property me 'duration-log) 2)))
        (let ((stem (ly:grob-object me 'stem)))
          (if (and (ly:grob? stem) (ly:grob? (ly:grob-object stem 'beam)))
              (set! out (ly:font-get-glyph fm (string-append either "2kievan"))))))
    (set! either (string-append either suffix))
    (if (ly:stencil-empty? out)
        (begin
          (%grob-warning me (string-append "none of note heads `" symmetric
                                           "' or `" directed "' found"))
          (cons (ly:make-stencil '() '(0.0 . 0.0) '(0.0 . 0.0)) ""))
        (cons out either))))
(define-public (ly:note-head::print grob)
  (%check-type "ly:note-head::print" 1 ly:grob? "Grob" grob)
  (car (%note-head-internal-print grob)))
(define-public (ly:note-head::stem-x-shift grob)
  (%check-type "ly:note-head::stem-x-shift" 1 ly:grob? "Grob" grob)
  (let ((stem (ly:grob-object grob 'stem)))
    (if (ly:grob? stem) (ly:grob-property stem 'positioning-done))
    0))
(define-public (ly:note-head::include-ledger-line-height grob)
  (%check-type "ly:note-head::include-ledger-line-height" 1 ly:grob? "Grob" grob)
  (let ((staff (%staff-symbol grob)))
    (if staff
        (let* ((ss (%staff-symbol-staff-space staff))
               (f (/ ss 2.0))
               (span (%staff-symbol-line-span staff))
               (lines (cond ((> (car span) (cdr span)) span)
                            ((< f 0) (cons (* (cdr span) f) (* (car span) f)))
                            (else (cons (* (car span) f) (* (cdr span) f)))))
               (my-pos (/ (* (%staff-position grob #f) ss) 2.0))
               (e (%grob-extent grob grob 1))
               (my-ext (cons (+ (car e) my-pos) (+ (cdr e) my-pos)))
               (lo (+ (- (cdr lines) (car my-ext)) 1))
               (hi (- (- (car lines) (cdr my-ext)) 1)))
          (cons (if (< lo 0.0) lo 0.0) (if (< 0.0 hi) hi 0.0)))
        (cons 0.0 0.0))))
(define-public (ly:note-head::calc-stem-attachment grob)
  (%check-type "ly:note-head::calc-stem-attachment" 1 ly:grob? "Grob" grob)
  (let* ((stem (ly:grob-object grob 'stem))
         (fm (ly:grob-default-font grob))
         (key (cdr (%note-head-internal-print grob)))
         (dir (if (ly:grob? stem) (%direction-or (ly:grob-property stem 'direction) 0) 0)))
    (ly:note-head::stem-attachment fm key (if (eqv? dir 0) 1 dir))))
(define-public (ly:note-head::calc-tab-stem-attachment grob)
  (%check-type "ly:note-head::calc-tab-stem-attachment" 1 ly:grob? "Grob" grob)
  (let* ((stem (ly:grob-object grob 'stem))
         (dir (if (ly:grob? stem) (%direction-or (ly:grob-property stem 'direction) 0) 0)))
    (cons 0.0 (* (if (eqv? dir 0) 1 dir) 1.35))))
"#;
