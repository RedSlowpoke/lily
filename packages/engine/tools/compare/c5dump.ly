%% Chapter 05's dump (§3, Appendix A), for C5: after page breaking, one line per
%% system (page, its offset on the page, the moment of its first column, its
%% columns) and one line per live grob caused by an event (type, the event's
%% file:line:char, system, X and Y relative to the system, a shape digest: a
%% beam's positions, a slur's or tie's control points, a stem's extent; and the
%% index of the line, VerticalAxisGroup, of the system it is in), tab-separated,
%% into C5_OUT. capture.ly includes it; capture.mjs sets C5_OUT per score.
%% Systems are numbered across books, so a file with several has no repeats.
#(begin
  (define c5-port (open-file (or (getenv "C5_OUT") "c5.out") "w"))
  (define c5-sysno 0)
  (define (c5-line . xs)
    (display (string-join (map (lambda (x) (format #f "~a" x)) xs) "\t") c5-port)
    (newline c5-port))
  (define (c5-r x) (if (number? x) (/ (round (* x 1000)) 1000.0) x))
  ;; The line (VerticalAxisGroup) a grob is in: its index among the system's lines, or -1.
  (define (c5-line-of g lines)
    (let loop ((p g))
      (cond ((not (ly:grob? p)) -1)
            ((eq? (grob::name p) 'VerticalAxisGroup)
             (let ((tail (memq (or (ly:grob-original p) p) lines))) (if tail (- (length lines) (length tail)) -1)))
            (else (loop (ly:grob-parent p Y))))))
  (define (c5-shape g)
    (let ((n (grob::name g)))
      (cond ((eq? n 'Beam) (let ((p (ly:grob-property g 'positions #f))) (if (pair? p) (list (c5-r (car p)) (c5-r (cdr p))) "")))
            ((memq n '(Slur Tie PhrasingSlur)) (map (lambda (o) (cons (c5-r (car o)) (c5-r (cdr o)))) (ly:grob-property g 'control-points '())))
            ((eq? n 'Stem) (let ((e (ly:grob-extent g g Y))) (list (c5-r (car e)) (c5-r (cdr e)))))
            (else ""))))
  (ly:output-def-set-variable! $defaultpaper 'page-post-process
   (lambda (paper pages)
     (for-each
      (lambda (page pageno)
        (for-each
         (lambda (line)
           (let ((sys (ly:prob-property line 'system-grob #f)))
             (when (ly:grob? sys)
               (let* ((cols (ly:grob-array->list (ly:grob-object sys 'columns)))
                      (when (if (pair? cols) (ly:grob-property (car cols) 'when #f) #f)))
                 (c5-line "S" c5-sysno pageno (c5-r (ly:prob-property line 'Y-offset 0))
                          (if (ly:moment? when) (ly:moment-main when) "-") (length cols)))
               (let ((lines (map (lambda (v) (or (ly:grob-original v) v))
                                 (filter (lambda (g) (eq? (grob::name g) 'VerticalAxisGroup))
                                         (ly:grob-array->list (ly:grob-object sys 'all-elements))))))
                 (for-each
                  (lambda (g)
                    (let ((ev (and (grob::name g) (event-cause g))))
                      (when (ly:stream-event? ev)
                        (let ((o (ly:event-property ev 'origin #f)))
                          (when (ly:input-location? o)
                            (let ((l (ly:input-file-line-char-column o)))
                              (c5-line "G" (grob::name g) (format #f "~a:~a:~a" (basename (car l)) (cadr l) (caddr l))
                                       c5-sysno (c5-r (ly:grob-relative-coordinate g sys X))
                                       (c5-r (ly:grob-relative-coordinate g sys Y)) (c5-shape g) (c5-line-of g lines))))))))
                  (ly:grob-array->list (ly:grob-object sys 'all-elements))))
               (set! c5-sysno (1+ c5-sysno)))))
         (ly:prob-property page 'lines '())))
      pages (iota (length pages)))
     (force-output c5-port))))
