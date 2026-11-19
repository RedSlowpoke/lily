%% MS4 step 3's probe of the Beam's callbacks before quanting: included
%% before a score (layout.test.mjs writes a file that includes this, then
%% the score), it records each Beam's values in LilyPond 2.26.0 and in the
%% engine alike, one line a read, into beam-probe.txt in the current
%% directory (the engine's outputs), the file written again with every
%% line, as the engine has no files to append to:
%%
%%   link  before  direction  knee  shorten  cross-staff  normal-stems  stems
%%   link  after   X-positions  beam-segments
%%
%% link          the origin of the event the beam's cause leads to,
%%               file:line:char:column as grobs.ly writes it
%% before        read in the Beam's before-line-breaking (it has none of its
%%               own): its direction, knee, shorten and cross-staff, how
%%               many normal stems it has, and each stem's direction and
%%               beaming once the beam's beaming is read, | between them
%% after         read in its after-line-breaking (none of its own either):
%%               its X-positions and each segment's vertical count and
%%               horizontal extent, | between them
%%
%% Numbers are written whole, as `number->string' writes them, so that a
%% last bit shows, and the rest as `display' writes it, so that an exact 0
%% and 0.0 differ. The reads come
%% before the beam is quanted: its positions and stem lengths are not read.
#(begin
  (define beam-probe-lines '())
  (define (beam-probe-line . xs)
    (set! beam-probe-lines (cons (string-join xs "\t") beam-probe-lines))
    (with-output-to-file "beam-probe.txt"
      (lambda ()
        (for-each (lambda (l) (display l) (newline)) (reverse beam-probe-lines)))))
  (define (beam-probe-number x)
    (number->string x))
  (define (beam-probe-interval e)
    (if (pair? e)
        (string-append (beam-probe-number (car e)) " " (beam-probe-number (cdr e)))
        (format #f "~a" e)))
  (define (beam-probe-link grob)
    (let* ((cause (let loop ((cause (ly:grob-property grob 'cause)))
                    (if (ly:grob? cause) (loop (ly:grob-property cause 'cause)) cause)))
           (origin (and (ly:stream-event? cause) (ly:event-property cause 'origin #f))))
      (if (ly:input-location? origin)
          (let ((l (ly:input-file-line-char-column origin)))
            (format #f "~a:~a:~a:~a" (basename (car l)) (cadr l) (caddr l) (cadddr l)))
          "-")))
  (define (beam-probe-stems beam)
    (let ((stems (ly:grob-object beam 'stems #f)))
      (if (ly:grob-array? stems) (ly:grob-array->list stems) '())))
  (define (beam-probe-before beam)
    (let* ((direction (ly:grob-property beam 'direction))
           (knee (ly:grob-property beam 'knee))
           (shorten (ly:grob-property beam 'shorten))
           (cross (ly:grob-property beam 'cross-staff))
           (normal (ly:grob-object beam 'normal-stems #f))
           (beaming (ly:grob-property beam 'beaming)))
      (beam-probe-line
       (beam-probe-link beam) "before"
       (format #f "~a" direction) (format #f "~a" knee) (format #f "~a" shorten) (format #f "~a" cross)
       (format #f "~a" (if (ly:grob-array? normal) (ly:grob-array-length normal) normal))
       (string-join
        (map (lambda (s)
               (format #f "~a ~a" (ly:grob-property s 'direction) (ly:grob-property s 'beaming)))
             (beam-probe-stems beam))
        "|"))))
  (define (beam-probe-after beam)
    (let ((x (ly:grob-property beam 'X-positions))
          (segments (ly:grob-property beam 'beam-segments)))
      (beam-probe-line
       (beam-probe-link beam) "after" (beam-probe-interval x)
       (string-join
        (map (lambda (s)
               (format #f "~a ~a" (assq-ref s 'vertical-count) (beam-probe-interval (assq-ref s 'horizontal))))
             segments)
        "|")))))
\layout {
  \context {
    \Score
    \override Beam.before-line-breaking = #beam-probe-before
    \override Beam.after-line-breaking = #beam-probe-after
  }
}
