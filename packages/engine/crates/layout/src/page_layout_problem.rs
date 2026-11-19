//! `page-layout-problem.cc` (part 5's, MS3): `is_spaceable`
//! (`page-layout-problem.cc:1173–1177`, 5 lines), which the staff grouper,
//! the alignment and the system call, ported first (MS3 step 27); its
//! spacing specs are `spacing_spec.rs`'.
//!
//! And the page-layout problem of one system with no page and no paper
//! book (`Page_layout_problem (NULL, SCM_EOL, systems)`, then
//! `solution`), which `ly:align-interface::align-to-ideal-distances`
//! solves (`align.rs`), ported first from step 34 by MS3 step 28
//! ([`PRELUDE`]): the constructor's systems (`:406–564` less the page's
//! and the paper's parts), `append_system` (`:578–731`),
//! `solve_rod_spring_problem` (`:779–824`), `find_system_offsets`
//! (`:837–1018` less the titles), `distribute_loose_lines`
//! (`:1024–1054`), `solution` (`:1063–1068`), `build_system_skyline`
//! (`:1079–1127`), `get_details` (`:1158–1171`), `mark_as_spaceable`,
//! `alter_spring_from_spacing_spec` and `filter_dead_elements`
//! (`:1179–1183`, `:1344–1374`), ≈ 560 lines; over `spring.rs`' springs
//! and `simple_spacer.rs`' solver (step 29's, ported first too).
//!
//! Step 34's: the constructor's page and paper parts (`:406–564`), the
//! page's head and foot, its height, and the paper's spacing variables,
//! margins and in-note padding and direction; the titles and markups
//! between systems (`append_prob`, `:733–770`, and the titles' part of
//! `find_system_offsets`), `force` (`:826–830`), the footnotes of a
//! page's lines (`get_footnote_grobs`, `get_footnote_count` and
//! `get_footnotes_from_lines`, `:42–117`), their numbers and stencils
//! stacked onto each line (`add_footnotes_to_lines`, `:127–352`), and
//! stacked onto the page's foot under the separator
//! (`get_footnote_separator_stencil` and `add_footnotes_to_footer`,
//! `:354–404`). And step 35's: `fixed_force_solution` (`:1057–1061`) and
//! the fixed-force branch of `solve_rod_spring_problem` (`:788–798`),
//! which `Page_breaking::draw_page` (`page.rs`) solves a ragged last page
//! with; it makes the problem of a page with a paper book, after
//! `make_pages` adds the footnotes to its lines. The alignment's problem
//! has neither.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::grob;

/// `is_spaceable`: whether the staff `g` is spaced as a staff, which one
/// without a number as its `staff-affinity` is; a loose line, one with
/// one, is placed against its neighbours instead.
pub fn is_spaceable(host: &mut dyn Host, g: Value) -> Result<bool, Exception> {
    let affinity = grob::property(host, g, "staff-affinity")?;
    Ok(host.heap().num(affinity).is_none())
}

/// The page-layout problem in Scheme:
///
/// - `(ly:get-spacing-spec from to)`, page-layout-problem-scheme.cc's
///   (`:24–35`, MS3 step 63): `get_spacing_spec (from, to, false, 0, 0)`,
///   the spacing spec between the two grobs (`%spacing-spec`), with
///   `LY_ASSERT_SMOB`'s type error for what is no grob.
/// - `%alter-spring-from-spacing-spec spec spring`: the spring's ideal
///   and minimum distances from `spec`'s `basic-distance` and
///   `minimum-distance`, its default strengths, then its stretch from
///   `stretchability`.
/// - `%filter-dead-elements input`: `input`'s live grobs, each hara-kiri
///   group given `%consider-suicide` first.
/// - `%element-details staves`, `get_details (Element)`: the
///   `line-break-system-details` of the left bound of the last staff's
///   system, `'()` for no staves; and for no system or no bound, where
///   LilyPond 2.26.0 would crash.
/// - `%build-system-skyline staves translations`, `build_system_skyline`:
///   `(up . down)`, the staves' skylines packed at their translations, the
///   upper relative to the first staff (`is_spaceable`), the lower to the
///   last.
/// - `%system-rank sys`: `System::get_rank` (`system.rs`), which breaking
///   into systems sets; 0 for an unbroken system, whose rank LilyPond
///   leaves uninitialized.
/// - `%line-indent sys rank`, `line_dimension_interval (layout,
///   rank)[LEFT]`: the layout's `indent` for the first line, else its
///   `short-indent`, its `line-width` read first.
/// - `%distribute-loose-lines lines mins first last`,
///   `distribute_loose_lines`: the loose lines between the two spaced
///   ones that end `lines` (`#f` for none) spaced by their spacing specs,
///   at least `mins` apart, between the page offsets `first` and `last`.
/// - `%interval-length i`, `Interval::length`: 0 for an empty interval.
/// - `%footnote-grobs lines`, `get_footnote_grobs`: the footnotes of
///   `lines`, in order: a system's `footnotes-after-line-breaking`, and
///   `#f` (LilyPond's null) for each of a `Prob`'s `footnotes`; a grob
///   that is no system is LilyPond's programming error, anything else is
///   skipped. `%footnote-count lines`, `get_footnote_count`: their number.
/// - `%footnotes-from-lines lines`, `get_footnotes_from_lines`: each
///   line's `footnote-stencil`, `'()` for no lines; and with LilyPond's
///   programming errors, `'()` when the first is neither a grob nor a
///   `Prob` or has no `footnote-stencil` yet, and a later line that is
///   neither left out.
/// - `%layout-extract-page-properties layout`, `titling.scm`'s
///   `layout-extract-page-properties`: the alist chain of the layout's
///   `line-width` (`ly:paper-get-number`, `%scm->double` of the
///   variable), its `property-defaults` and `((font-encoding . latin1))`.
/// - `%add-footnotes-to-lines! lines counter paper top-paper`,
///   `add_footnotes_to_lines (lines, counter, pb)`, `paper` the paper
///   book's paper (`#f` for none: LilyPond's programming error, and
///   nothing done) and `top-paper` its top paper book's: each footnote of
///   `%footnote-grobs`, from `counter` on, is given its number, the
///   markup of the paper's `footnote-numbering-function`, after a grob's
///   `numbering-assertion-function` is called with it; the numbers'
///   stencils are moved right to end where the widest ends. Each
///   system's footnotes after line breaking are its `footnote-text`
///   (the first broken piece's for a broken spanner), interpreted with
///   the page's properties; one `automatically-numbered` (or a broken
///   piece of it) takes the next number: the markup as its `text` (and
///   its pieces'), the stencil raised by `footnote-number-raise` to its
///   top and put at its left. A stencil that is not empty is stacked
///   under the system's `footnote-stencil`, by `footnote-padding`, for a
///   `footnote` of `#t`, else under its `in-note-stencil`, by
///   `in-note-padding`, and the system given both. A `Prob`'s
///   `footnotes`, each `(key numbered stencil)`, are stacked the same way
///   into its `footnote-stencil`, and each key given its number's
///   stencil (the empty stencil if not numbered), consed in front of the
///   top paper's `number-footnote-table` (none if no pair), which is set
///   at the end. The numbers are taken in order by the numbered
///   footnotes, as LilyPond takes them. A paper variable LilyPond reads
///   as a number is 0 if none. LilyPond's checks that a number's markup
///   gives a stencil, which interpreting a markup always does, are left
///   out; a `Prob`'s footnote stencil that is none, where LilyPond would
///   crash, is the type error of the stencil procedure given it.
/// - `%footnote-separator-stencil paper`,
///   `get_footnote_separator_stencil`: the paper's
///   `footnote-separator-markup` interpreted with the page's properties,
///   the empty stencil if it is no markup.
/// - `%add-footnotes-to-footer footnotes foot paper`,
///   `add_footnotes_to_footer`: `foot` with each of `footnotes` that is a
///   stencil and not empty stacked over it, the last first, by the
///   paper's `footnote-footer-padding` for the first stacked and its
///   `footnote-padding` for the rest; with one stacked, the separator
///   over them by `footnote-padding` unless it is empty.
/// - `%page-layout-problem paper page systems ragged [fixed-force]`:
///   `Page_layout_problem (pb, page, systems)`, then `solution (ragged)`
///   and `force ()`, as `(offsets . force)`: the systems' offsets on the
///   page and the force that spaced them, `-inf.0` for a compressed page;
///   `%page-layout-solution` the offsets alone. With `fixed-force`,
///   `fixed_force_solution (fixed-force)` (`:1057–1061`) when `ragged` is
///   `#t`, as `Page_breaking::draw_page` calls it for a ragged last page:
///   the springs spaced by `fixed-force` if the page holds them so, else
///   filled as a page that is not ragged (`%spacer-solve`'s fixed force);
///   its force is then 0, the constructor's, which that branch of
///   `solve_rod_spring_problem` leaves as it is, unless the page is
///   compressed. A `fixed-force` that is infinite is `solution (ragged)`.
///   `paper` is the paper book's paper (`#f` for no paper book or none)
///   and `page` its page (a `Prob`, else none). The page gives the
///   head's and foot's heights (`%interval-length`) and the height,
///   `paper-height`, 100 if no number; without a paper, LilyPond's warning that footnotes cannot
///   be placed; with one, the foot is the page's `foot-stencil` (the
///   empty stencil if none) with the footnotes of `%footnotes-from-lines`
///   added by `%add-footnotes-to-footer`, whose height the footer's is.
///   The paper gives the spacing specs, `top-markup-spacing` at the top
///   when the first system is a `Prob`, the margins taken off the height,
///   the head's padding from `top-system-spacing` and the foot's from
///   `last-bottom-spacing`, the in-note system padding (0.5 if no number)
///   and direction (`UP` if none); a variable the paper has not
///   is `'()`, read as LilyPond reads `SCM_UNDEFINED`.
///   Without a paper every spec is `'()`, and the in-note system padding
///   and direction, which LilyPond 2.26.0 leaves uninitialized then, are
///   0.5 and `UP`; the in-note padding, which it never sets, is 0. A
///   system's spec is the top's for the first, the markup's after a
///   title, the score's for one whose left bound is the column of rank 0
///   (`#f` for none, where LilyPond reads through a null), else the
///   system's. A `Prob` among the systems is a title or a markup: its
///   spec is the top's for the first, `markup-markup-spacing` after a
///   title, else `score-markup-spacing`; it is spaced by its
///   `vertical-skylines`, else its stencil's extent, under the last
///   system, held there by a `tight-spacing` of `#t`, and its
///   `bottom-padding` is taken off the height when it is last. Its
///   offset is its spring's; the loose lines above it are placed against
///   its stencil's top, and a loose line at the top of the system under
///   it against its stencil's bottom. A `Prob` with no stencil, where
///   LilyPond would crash, has the empty extent of LilyPond's empty
///   stencil.
///   Each staff is moved to its place on Y and given its
///   `system-Y-offset`; the loose lines between staves are distributed.
///   A system's `vertical-skylines` that are no pair, where LilyPond
///   would crash, are empty. The over-full page's warning is
///   `(ice-9 format)`'s `~,1f` for LilyPond's `%.1f`. `set_property` is
///   `%grob-set-property-data!` (C++ `set_property`, unchecked).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:get-spacing-spec from-scm to-scm)
  (%check-type "ly:get-spacing-spec" 1 ly:grob? "Grob" from-scm)
  (%check-type "ly:get-spacing-spec" 2 ly:grob? "Grob" to-scm)
  (%spacing-spec from-scm to-scm #f 0 0))
(define (%alter-spring-from-spacing-spec spec spring)
  (let ((space (%read-spacing-spec spec 'basic-distance)))
    (if space (%spring-set! spring 'ideal-distance space)))
  (let ((min-dist (%read-spacing-spec spec 'minimum-distance)))
    (if min-dist (%spring-set! spring 'min-distance min-dist)))
  (%spring-set! spring 'default-strength 0)
  (let ((stretch (%read-spacing-spec spec 'stretchability)))
    (if stretch (%spring-set! spring 'inverse-stretch-strength stretch))))
(define (%filter-dead-elements input)
  (let loop ((es input) (out '()))
    (if (null? es)
        (reverse out)
        (let ((g (car es)))
          (if (%grob-has-interface? g 'hara-kiri-group-spanner-interface)
              (%consider-suicide g))
          (loop (cdr es) (if (%grob-live? g) (cons g out) out))))))
(define (%element-details staves)
  (if (null? staves)
      '()
      (let* ((sys (ly:grob-system (car (last-pair staves))))
             (bound (and (ly:grob? sys) (ly:spanner-bound sys -1))))
        (if (ly:grob? bound)
            (ly:grob-property bound 'line-break-system-details)
            '()))))
(define (%build-system-skyline staves translations)
  (if (null? translations)
      (cons (%skyline-empty 1) (%skyline-empty -1))
      (let ((first (car translations)))
        (let loop ((gs staves) (ts translations) (up (%skyline-empty 1)) (down (%skyline-empty -1))
                   (found #f) (first-dy 0.0) (last-dy 0.0))
          (if (null? gs)
              (cons (%skyline-raised up (- first-dy)) (%skyline-raised down (- last-dy)))
              (let* ((dy (- (car ts) first))
                     (sky (ly:grob-property (car gs) 'vertical-skylines))
                     (pair (%skyline-pair? sky))
                     (up (if pair
                             (%skyline-raised (%skyline-merged (%skyline-raised up (- dy)) (cdr sky)) dy)
                             up))
                     (down (if pair
                               (%skyline-raised (%skyline-merged (%skyline-raised down (- dy)) (car sky)) dy)
                               down))
                     (spaceable (%spaceable? (car gs))))
                (loop (cdr gs) (cdr ts) up down (or found spaceable)
                      (if (and spaceable (not found)) dy first-dy)
                      (if spaceable dy last-dy))))))))
(define (%line-indent sys rank)
  (%layout-dimension sys 'line-width)
  (%layout-dimension sys (if (eqv? rank 0) 'indent 'short-indent)))
(define (%distribute-loose-lines lines mins first-translation last-translation)
  (let* ((v (list->vector lines))
         (springs (let loop ((i 0) (ms mins) (acc '()))
                    (if (< (1+ i) (vector-length v))
                        (let ((spec (%spacing-spec (vector-ref v i) (vector-ref v (1+ i)) #f 0 2147483647))
                              (spring (%make-spring 1.0 0.0)))
                          (%alter-spring-from-spacing-spec spec spring)
                          (%spring-set! spring 'ensure-min-distance (car ms))
                          (loop (1+ i) (cdr ms) (cons spring acc)))
                        (reverse acc))))
         (solution (list->vector (caddr (%spacer-solve springs (- first-translation last-translation) #f)))))
    (do ((i 1 (1+ i))) ((>= (1+ i) (vector-length solution)))
      (let ((g (vector-ref v i)))
        (if g
            (let ((system-offset (%scm->double (ly:grob-property g 'system-Y-offset))))
              (ly:grob-translate-axis! g (- (- first-translation (vector-ref solution i)) system-offset) 1)))))))
(define (%interval-length i)
  (if (<= (cdr i) (car i)) 0.0 (- (cdr i) (car i))))
(define (%footnote-grobs lines)
  (let loop ((s lines) (out '()))
    (if (pair? s)
        (let ((g (car s)))
          (cond ((ly:grob? g)
                 (if (%system? g)
                     (loop (cdr s) (let add ((fs (%grob-set g 'footnotes-after-line-breaking)) (out out))
                                     (if (pair? fs) (add (cdr fs) (cons (car fs) out)) out)))
                     (begin
                       (ly:programming-error "got a grob for footnotes that wasn't a System")
                       (loop (cdr s) out))))
                ((ly:prob? g)
                 (loop (cdr s) (let add ((st (ly:prob-property g 'footnotes)) (out out))
                                 (if (pair? st) (add (cdr st) (cons #f out)) out))))
                (else (loop (cdr s) out))))
        (reverse out))))
(define (%footnote-count lines)
  (length (%footnote-grobs lines)))
(define %not-a-line (list 'not-a-line))
(define (%line-footnote-stencil line)
  (cond ((ly:grob? line) (ly:grob-property line 'footnote-stencil))
        ((ly:prob? line) (ly:prob-property line 'footnote-stencil))
        (else %not-a-line)))
(define (%footnotes-from-lines lines)
  (if (not (pair? lines))
      '()
      (let ((first (%line-footnote-stencil (car lines))))
        (cond ((eq? first %not-a-line)
               (ly:programming-error "Systems on a page must be a prob or grob.")
               '())
              ((null? first)
               (ly:programming-error "Footnotes must be added to lines before they are retrieved.")
               '())
              (else
               (let loop ((s lines) (out '()))
                 (if (pair? s)
                     (let ((st (%line-footnote-stencil (car s))))
                       (if (eq? st %not-a-line)
                           (begin
                             (ly:programming-error "Systems on a page must be a prob or grob.")
                             (loop (cdr s) out))
                           (loop (cdr s) (cons st out))))
                     (reverse out))))))))
(define (%layout-extract-page-properties layout)
  (list (append (list (cons 'line-width (%scm->double (ly:output-def-lookup layout 'line-width))))
                (ly:output-def-lookup layout 'property-defaults)
                '((font-encoding . latin1)))))
(define (%footnote-numbered footnote annotation raise)
  (let ((top (+ (cdr (ly:stencil-extent footnote 1)) raise)))
    (ly:stencil-combine-at-edge
     footnote 0 -1
     (ly:stencil-translate-axis annotation (- top (cdr (ly:stencil-extent annotation 1))) 1)
     0.0)))
(define (%add-footnotes-to-lines! lines counter paper top-paper)
  (if (not paper)
      (ly:programming-error "Cannot get footnotes because there is no valid paper block.")
      (let* ((table (let ((t (ly:output-def-lookup top-paper 'number-footnote-table)))
                      (if (pair? t) t '())))
             (numbering (ly:output-def-lookup paper 'footnote-numbering-function))
             (props (%layout-extract-page-properties paper))
             (padding (%real-or (ly:output-def-lookup paper 'footnote-padding) 0.0))
             (in-note-padding (%real-or (ly:output-def-lookup paper 'in-note-padding) 0.0))
             (raise (%real-or (ly:output-def-lookup paper 'footnote-number-raise) 0.0))
             (empty (lambda () (ly:make-stencil '()))))
        (let number ((gs (%footnote-grobs lines)) (counter counter)
                     (markups '()) (stencils '()) (max-length -inf.0))
          (if (pair? gs)
              (begin
                (if (car gs)
                    (let ((assertion (ly:grob-property (car gs) 'numbering-assertion-function)))
                      (if (procedure? assertion) (assertion counter))))
                (let* ((markup (numbering counter))
                       (stencil (%interpret-markup paper props markup))
                       (x (ly:stencil-extent stencil 0)))
                  (number (cdr gs) (1+ counter) (cons markup markups) (cons stencil stencils)
                          (if (or (%interval-empty? x) (not (< max-length (cdr x))))
                              max-length
                              (cdr x)))))
              (let line ((s lines) (table table) (in-text (reverse markups))
                         (numbers (let align ((ss stencils) (out '()))
                                    (if (pair? ss)
                                        (let ((x (ly:stencil-extent (car ss) 0)))
                                          (align (cdr ss)
                                                 (cons (if (%interval-empty? x)
                                                           (car ss)
                                                           (ly:stencil-translate-axis (car ss) (- max-length (cdr x)) 0))
                                                       out)))
                                        out))))
                (cond
                 ((not (pair? s))
                  (ly:output-def-set-variable! top-paper 'number-footnote-table table))
                 ((ly:grob? (car s))
                  (let ((sys (car s)))
                    (if (not (%system? sys))
                        (begin
                          (ly:programming-error "got a grob for footnotes that wasn't a System")
                          (line (cdr s) table in-text numbers))
                        (let note ((fs (%grob-set sys 'footnotes-after-line-breaking))
                                   (mol (empty)) (in-note-mol (empty)) (in-text in-text) (numbers numbers))
                          (if (pair? fs)
                              (let* ((footnote (car fs))
                                     (pieces (if (ly:spanner? footnote) (ly:spanner-broken-into footnote) '()))
                                     (markup (let ((m (ly:grob-property footnote 'footnote-text)))
                                               (if (pair? pieces) (ly:grob-property (car pieces) 'footnote-text) m)))
                                     (stencil (%interpret-markup paper (%layout-extract-page-properties paper) markup))
                                     (numbered (or (eq? #t (ly:grob-property footnote 'automatically-numbered))
                                                   (let any ((ps pieces))
                                                     (and (pair? ps)
                                                          (or (eq? #t (ly:grob-property (car ps) 'automatically-numbered))
                                                              (any (cdr ps)))))))
                                     (stencil (if numbered
                                                  (begin
                                                    (%grob-set-property-data! footnote 'text (car in-text))
                                                    (if (ly:spanner? footnote)
                                                        (begin
                                                          (%grob-set-property-data! footnote 'text (car in-text))
                                                          (for-each (lambda (p) (%grob-set-property-data! p 'text (car in-text)))
                                                                    pieces)))
                                                    (%footnote-numbered stencil (car numbers) raise))
                                                  stencil))
                                     (in-text (if numbered (cdr in-text) in-text))
                                     (numbers (if numbered (cdr numbers) numbers)))
                                (cond ((ly:stencil-empty? stencil)
                                       (note (cdr fs) mol in-note-mol in-text numbers))
                                      ((eq? #t (ly:grob-property footnote 'footnote))
                                       (note (cdr fs) (ly:stencil-combine-at-edge mol 1 -1 stencil padding)
                                             in-note-mol in-text numbers))
                                      (else
                                       (note (cdr fs) mol
                                             (ly:stencil-combine-at-edge in-note-mol 1 -1 stencil in-note-padding)
                                             in-text numbers))))
                              (begin
                                (%grob-set-property-data! sys 'in-note-stencil in-note-mol)
                                (%grob-set-property-data! sys 'footnote-stencil mol)
                                (line (cdr s) table in-text numbers)))))))
                 ((ly:prob? (car s))
                  (let note ((st (ly:prob-property (car s) 'footnotes)) (mol (empty))
                             (table table) (in-text in-text) (numbers numbers))
                    (if (pair? st)
                        (let* ((entry (car st))
                               (numbered (eq? #t (cadr entry)))
                               (in-text-stencil (if numbered (%interpret-markup paper props (car in-text)) (empty)))
                               (stencil (if numbered
                                            (%footnote-numbered (caddr entry) (car numbers) raise)
                                            (caddr entry))))
                          (note (cdr st)
                                (if (ly:stencil-empty? stencil)
                                    mol
                                    (ly:stencil-combine-at-edge mol 1 -1 stencil padding))
                                (cons (cons (car entry) in-text-stencil) table)
                                (if numbered (cdr in-text) in-text)
                                (if numbered (cdr numbers) numbers)))
                        (begin
                          (ly:prob-set-property! (car s) 'footnote-stencil mol)
                          (line (cdr s) table in-text numbers)))))
                 (else (line (cdr s) table in-text numbers)))))))))
(define (%footnote-separator-stencil paper)
  (let ((props (%layout-extract-page-properties paper))
        (markup (ly:output-def-lookup paper 'footnote-separator-markup)))
    (if (or (string? markup) (%markup? markup))
        (%interpret-markup paper props markup)
        (ly:make-stencil '()))))
(define (%add-footnotes-to-footer footnotes foot paper)
  (let ((padding (%real-or (ly:output-def-lookup paper 'footnote-padding) 0.0))
        (footer-padding (%real-or (ly:output-def-lookup paper 'footnote-footer-padding) 0.0)))
    (let loop ((s (reverse footnotes)) (foot foot) (found #f))
      (cond ((pair? s)
             (let ((stencil (car s)))
               (if (and (ly:stencil? stencil) (not (ly:stencil-empty? stencil)))
                   (loop (cdr s) (ly:stencil-combine-at-edge foot 1 1 stencil (if found padding footer-padding)) #t)
                   (loop (cdr s) foot found))))
            (found
             (let ((separator (%footnote-separator-stencil paper)))
               (if (ly:stencil-empty? separator)
                   foot
                   (ly:stencil-combine-at-edge foot 1 1 separator padding))))
            (else foot)))))
(define (%page-layout-problem paper page systems ragged . fixed-force)
  (let ((springs '())
        (elements '())
        (header-height 0.0)
        (footer-height 0.0)
        (header-padding 0.0)
        (footer-padding 0.0)
        (in-note-padding 0.0)
        (in-note-system-padding 0.5)
        (in-note-direction 1)
        (bottom-loose-baseline 0.0)
        (page-height 100.0)
        (bottom-skyline #f)
        (system-system-spacing '())
        (score-system-spacing '())
        (markup-system-spacing '())
        (score-markup-spacing '())
        (markup-markup-spacing '())
        (top-system-spacing '())
        (last-bottom-spacing '()))
    (define (push-spring! s) (set! springs (cons s springs)))
    (define (append-system! sys spring indent padding)
      (let ((align (ly:grob-object sys 'vertical-alignment)))
        (when (ly:grob? align)
          (%grob-set-property-data! align 'positioning-done #t)
          (let* ((elts (%filter-dead-elements (%grob-set align 'elements)))
                 (minimum-offsets (%align-minimum-translations align elts 1 #f #f 0 0))
                 (with-min (list->vector (%align-minimum-translations align elts 1 #t #f 0 0)))
                 (skylines (%build-system-skyline elts (vector->list with-min)))
                 (up (%skyline-shifted (car skylines) indent))
                 (down (%skyline-shifted (cdr skylines) indent))
                 (in-note (ly:grob-property sys 'in-note-stencil))
                 (in-note-length (if (ly:stencil? in-note)
                                     (let ((e (ly:stencil-extent in-note 1))) (- (cdr e) (car e)))
                                     0.0)))
            (when (> in-note-length 0)
              (%grob-set-property-data! sys 'in-note-padding in-note-padding)
              (%grob-set-property-data! sys 'in-note-system-padding in-note-system-padding)
              (%grob-set-property-data! sys 'in-note-direction in-note-direction)
              (let ((raise (lambda (sky)
                             (%skyline-with-minimum-height
                              sky (+ (%skyline-max-height sky)
                                     (* in-note-direction (+ in-note-system-padding in-note-length)))))))
                (if (eqv? in-note-direction 1)
                    (set! up (raise up))
                    (set! down (raise down)))))
            (let ((minimum-distance
                   (+ (%skyline-distance up bottom-skyline
                                         (%real-or (ly:grob-property sys 'skyline-horizontal-padding) 0.0))
                      padding)))
              (%spring-set! spring 'ensure-min-distance minimum-distance)
              (push-spring! spring)
              (if (and (pair? elts) (not (%spaceable? (car elts))))
                  (let* ((sky (ly:grob-property (car elts) 'vertical-skylines))
                         (first-skyline (%skyline-shifted
                                         (if (%skyline-pair? sky)
                                             (%skyline-merged (%skyline-empty 1) (cdr sky))
                                             (%skyline-empty 1))
                                         indent)))
                    (set! minimum-distance
                          (- (%skyline-distance first-skyline bottom-skyline 0.0) bottom-loose-baseline))))
              (set! bottom-skyline down)
              (set! elements (cons (vector elts (list->vector minimum-offsets) minimum-distance padding #f)
                                   elements)))
            (let loop ((i 0) (es elts) (last 0) (found #f)
                       (manual (%assoc-get 'alignment-distances (%element-details elts) '())))
              (if (pair? es)
                  (cond ((not (%spaceable? (car es))) (loop (1+ i) (cdr es) last found manual))
                        ((not found)
                         (if (> i 0)
                             (%spring-set! (car springs) 'ensure-min-distance
                                           (+ (- bottom-loose-baseline (vector-ref with-min i)) padding)))
                         (loop (1+ i) (cdr es) i #t manual))
                        (else
                         (let* ((spring (%make-spring 0.5 0.0))
                                (last-staff (list-ref elts last))
                                (spec (ly:grob-property last-staff 'staff-staff-spacing))
                                (spec (if (list? spec) spec (cons (car spec) '())))
                                (default-spacing (ly:grob-property last-staff 'default-staff-staff-spacing))
                                (spec (if (and (pair? default-spacing) (list? spec))
                                          (let fill ((s default-spacing) (spec spec))
                                            (if (pair? s)
                                                (fill (cdr s) (if (%assq (caar s) spec) spec (cons (car s) spec)))
                                                spec))
                                          spec)))
                           (%alter-spring-from-spacing-spec spec spring)
                           (push-spring! spring)
                           (%spring-set! spring 'ensure-min-distance
                                         (- (vector-ref with-min last) (vector-ref with-min i)))
                           (if (and (pair? manual) (number? (car manual)))
                               (let ((dy (%scm->double (car manual))))
                                 (%spring-set! spring 'ideal-distance dy)
                                 (%spring-set! spring 'min-distance dy)
                                 (%spring-set! spring 'inverse-stretch-strength 0.0)))
                           (loop (1+ i) (cdr es) i #t (if (pair? manual) (cdr manual) manual)))))
                  (begin
                    (set! bottom-loose-baseline
                          (if found
                              (- (vector-ref with-min last)
                                 (vector-ref with-min (1- (vector-length with-min))))
                              0.0))
                    (if (and (not found) (pair? elts))
                        (%grob-set-property-data! (car elts) 'staff-affinity #f)))))))))
    (define (append-prob! prob spring padding)
      (let ((sky (ly:prob-property prob 'vertical-skylines))
            (stencil (ly:prob-property prob 'stencil))
            (tight-spacing (eq? (ly:prob-property prob 'tight-spacing) #t))
            (minimum-distance 0.0))
        (cond ((%skyline-pair? sky)
               (let ((d (%skyline-distance (cdr sky) bottom-skyline 0.0)))
                 (set! minimum-distance (if (< d bottom-loose-baseline) bottom-loose-baseline d)))
               (set! bottom-skyline (car sky)))
              ((ly:stencil? stencil)
               (let ((iv (ly:stencil-extent stencil 1)))
                 (set! minimum-distance (- (cdr iv) (%skyline-max-height bottom-skyline)))
                 (set! bottom-skyline (%skyline-with-minimum-height (%skyline-empty -1) (car iv))))))
        (set! bottom-loose-baseline 0.0)
        (if tight-spacing
            (begin
              (%spring-set! spring 'min-distance minimum-distance)
              (%spring-set! spring 'inverse-stretch-strength 0.0)
              (%spring-set! spring 'ideal-distance 0.0))
            (%spring-set! spring 'ensure-min-distance (+ minimum-distance padding)))
        (push-spring! spring)
        (set! elements (cons (vector '() #() 0.0 padding prob) elements))))
    (define (prob-extent prob)
      (let ((stencil (ly:prob-property prob 'stencil)))
        (if (ly:stencil? stencil) (ly:stencil-extent stencil 1) (cons +inf.0 -inf.0))))
    (define (find-system-offsets solution)
      (let ((idx 1) (loose '()) (loose-mins '()) (offsets '())
            (last-spaceable-line #f) (last-spaceable-translation 0.0)
            (last-title-extent (cons +inf.0 -inf.0)))
        (define (push-loose! g) (set! loose (cons g loose)))
        (define (push-min! d) (set! loose-mins (cons d loose-mins)))
        (define (distribute! last-translation)
          (%distribute-loose-lines (reverse loose) (reverse loose-mins)
                                   last-spaceable-translation last-translation)
          (set! loose '())
          (set! loose-mins '()))
        (for-each
         (lambda (el)
           (if (vector-ref el 4)
               (let ((position (vector-ref solution idx))
                     (extent (prob-extent (vector-ref el 4))))
                 (set! offsets (cons position offsets))
                 (when (pair? loose)
                   (let ((loose-extent (%grob-extent (car loose) (car loose) 1)))
                     (push-min! (+ (+ (- (car loose-extent)) (cdr extent)) (vector-ref el 3)))
                     (push-loose! #f)
                     (distribute! (- position))))
                 (set! last-spaceable-line #f)
                 (set! last-spaceable-translation (- position))
                 (set! last-title-extent extent)
                 (set! idx (1+ idx)))
               (let* ((min-offsets (vector-ref el 1))
                      (first-min (if (> (vector-length min-offsets) 0) (vector-ref min-offsets 0) 0.0))
                      (system-position (+ (vector-ref solution idx) first-min))
                      (found #f))
                 (let staff-loop ((ss (vector-ref el 0)) (staff-idx 0))
                   (when (pair? ss)
                     (let ((staff (car ss)))
                       (%grob-set-property-data! staff 'system-Y-offset (- system-position))
                       (if (%spaceable? staff)
                           (begin
                             (ly:grob-translate-axis! staff (- system-position (vector-ref solution idx)) 1)
                             (when (pair? loose)
                               (if (> staff-idx 0)
                                   (push-min! (- (vector-ref min-offsets (1- staff-idx))
                                                 (vector-ref min-offsets staff-idx)))
                                   (begin
                                     (push-min! 0.0)
                                     (push-loose! #f)
                                     (push-min! (- (vector-ref el 3) (vector-ref min-offsets 0)))))
                               (push-loose! staff)
                               (distribute! (- (vector-ref solution idx))))
                             (set! last-spaceable-line staff)
                             (set! last-spaceable-translation (- (vector-ref solution idx)))
                             (set! found #t)
                             (set! idx (1+ idx)))
                           (let ((ext (%grob-extent staff staff 1)))
                             (when (not (%interval-empty? ext))
                               (if (null? loose) (push-loose! last-spaceable-line))
                               (if (> staff-idx 0)
                                   (push-min! (- (vector-ref min-offsets (1- staff-idx))
                                                 (vector-ref min-offsets staff-idx)))
                                   (push-min!
                                    (cond ((car loose)
                                           (let ((d (+ (vector-ref el 2) (vector-ref el 3))))
                                             (push-min! 0.0)
                                             (push-loose! #f)
                                             d))
                                          ((not (%interval-empty? last-title-extent))
                                           (+ (- (cdr ext) (car last-title-extent)) (vector-ref el 3)))
                                          (else (+ (+ header-padding header-height) (cdr ext))))))
                               (push-loose! staff))))
                       (staff-loop (cdr ss) (1+ staff-idx)))))
                 (if (not found) (set! idx (1+ idx)))
                 (set! offsets (cons system-position offsets)))))
         (reverse elements))
        (when (pair? loose)
          (let ((last-ext (%grob-extent (car loose) (car loose) 1)))
            (push-min! (+ (+ (- (car last-ext)) footer-height) footer-padding))
            (push-loose! #f)
            (distribute! (- page-height))))
        (reverse offsets)))
    (when (ly:prob? page)
      (let* ((head (ly:prob-property page 'head-stencil))
             (foot (ly:prob-property page 'foot-stencil))
             (foot-stencil (if (ly:stencil? foot) foot (ly:make-stencil '())))
             (foot-stencil (if (ly:output-def? paper)
                               (%add-footnotes-to-footer (%footnotes-from-lines systems) foot-stencil paper)
                               (begin
                                 (ly:warning "A page layout problem has been initiated that cannot accommodate footnotes.")
                                 foot-stencil))))
        (set! header-height (if (ly:stencil? head) (%interval-length (ly:stencil-extent head 1)) 0.0))
        (set! footer-height (%interval-length (ly:stencil-extent foot-stencil 1)))
        (set! page-height (%real-or (ly:prob-property page 'paper-height) 100.0))))
    (set! bottom-skyline (%skyline-with-minimum-height (%skyline-empty -1) (- header-height)))
    (when (ly:output-def? paper)
      (let ((var (lambda (sym) (ly:output-def-lookup paper sym))))
        (set! system-system-spacing (var 'system-system-spacing))
        (set! score-system-spacing (var 'score-system-spacing))
        (set! markup-system-spacing (var 'markup-system-spacing))
        (set! score-markup-spacing (var 'score-markup-spacing))
        (set! markup-markup-spacing (var 'markup-markup-spacing))
        (set! last-bottom-spacing (var 'last-bottom-spacing))
        (set! top-system-spacing (var 'top-system-spacing))
        (if (and (pair? systems) (ly:prob? (car systems)))
            (set! top-system-spacing (var 'top-markup-spacing)))
        (set! page-height (- page-height (+ (%real-or (var 'top-margin) 0.0)
                                            (%real-or (var 'bottom-margin) 0.0))))
        (set! header-padding (or (%read-spacing-spec top-system-spacing 'padding) header-padding))
        (set! footer-padding (or (%read-spacing-spec last-bottom-spacing 'padding) footer-padding))
        (set! in-note-system-padding (%real-or (var 'in-note-system-padding) 0.5))
        (set! in-note-direction (%direction-or (var 'in-note-direction) 1))))
    (let loop ((s systems) (last-system-was-title #f))
      (when (pair? s)
        (let ((g (car s)))
          (cond ((ly:prob? g)
                 (let ((spec (cond ((eq? s systems) top-system-spacing)
                                   (last-system-was-title markup-markup-spacing)
                                   (else score-markup-spacing)))
                       (spring (%make-spring 0.0 0.0)))
                   (%alter-spring-from-spacing-spec spec spring)
                   (append-prob! g spring (or (%read-spacing-spec spec 'padding) 0.0))
                   (loop (cdr s) #t)))
                ((not (ly:grob? g))
                 (ly:programming-error "got a system that was neither a Grob nor a Prob")
                 (loop (cdr s) last-system-was-title))
                ((not (%system? g))
                 (ly:programming-error "got a grob for vertical spacing that wasn't a System")
                 (loop (cdr s) last-system-was-title))
                (else
                 (let ((spec (cond ((eq? s systems) top-system-spacing)
                                   (last-system-was-title markup-system-spacing)
                                   ((eqv? (%paper-column-rank (ly:spanner-bound g -1)) 0) score-system-spacing)
                                   (else system-system-spacing)))
                       (spring (%make-spring 0.0 0.0))
                       (indent (%line-indent g (%system-rank g))))
                   (%alter-spring-from-spacing-spec spec spring)
                   (append-system! g spring indent (or (%read-spacing-spec spec 'padding) 0.0))
                   (%queue-check 18)
                   (loop (cdr s) #f)))))))
    (let ((last-spring (%make-spring 0.0 0.0)))
      (%alter-spring-from-spacing-spec last-bottom-spacing last-spring)
      (%spring-set! last-spring 'ensure-min-distance
                    (+ (- (or (%read-spacing-spec last-bottom-spacing 'padding) 0.0)
                          (%skyline-max-height bottom-skyline))
                       footer-height))
      (push-spring! last-spring))
    (if (pair? elements)
        (let ((staves (vector-ref (car elements) 0))
              (prob (vector-ref (car elements) 4)))
          (set! page-height
                (- page-height
                   (cond (prob (%real-or (ly:prob-property prob 'bottom-padding) 0.0))
                         ((pair? staves)
                          (%real-or (%assoc-get 'bottom-padding (%element-details staves) #f) 0.0))
                         (else 0.0))))))
    (let* ((fixed (and ragged (pair? fixed-force) (not (inf? (car fixed-force))) (car fixed-force)))
           (sol (if fixed
                    (%spacer-solve (reverse springs) page-height ragged fixed)
                    (%spacer-solve (reverse springs) page-height ragged)))
           (force (if fixed 0.0 (car sol)))
           (solution (list->vector (caddr sol))))
      (if (not (cadr sol))
          (let ((overflow (- (cadddr sol) page-height)))
            (if (and ragged (< overflow 1e-6))
                (ly:warning "ragged-bottom was specified, but page must be compressed")
                (let* ((space-count (vector-length solution))
                       (increment (/ overflow (exact->inexact (- space-count 2)))))
                  (ly:warning ((@ (ice-9 format) format) #f "compressing over-full page by ~,1f staff-spaces"
                                                       overflow))
                  (set! force -inf.0)
                  (do ((i 2 (1+ i))) ((>= i space-count))
                    (vector-set! solution i
                                 (- (vector-ref solution i)
                                    (* (exact->inexact (- i 1)) increment))))))))
      (cons (find-system-offsets solution) force))))
(define (%page-layout-solution paper page systems ragged)
  (car (%page-layout-problem paper page systems ragged)))
"#;
