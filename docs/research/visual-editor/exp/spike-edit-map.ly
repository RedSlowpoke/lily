%% Spike: per-notehead metadata for a visual editor.
#(define edit-map-rows '())
#(define (edit-map-engraver context)
  (make-engraver
   (acknowledgers
    ((note-head-interface engraver grob source-engraver)
     (let* ((event (event-cause grob))
            (origin (and event (ly:event-property event 'origin)))
            (loc (and origin (ly:input-file-line-char-column origin)))
            (pitch (ly:event-property event 'pitch))
            (dur (ly:event-property event 'duration))
            (now (ly:context-current-moment context))
            (staff (ly:context-find context 'Staff)))
       (set! edit-map-rows
             (cons (format #f "{\"at\":\"~a:~a:~a\",\"pitch\":[~a,~a,~a],\"dur\":\"~a\",\"moment\":\"~a\",\"voice\":\"~a\",\"staff\":\"~a\",\"pos\":~a}"
                           (basename (car loc)) (cadr loc) (caddr loc)
                           (ly:pitch-octave pitch) (ly:pitch-notename pitch) (ly:pitch-alteration pitch)
                           (ly:duration->string dur)
                           (ly:moment-main now)
                           (ly:context-id context)
                           (if staff (ly:context-id staff) "")
                           (ly:grob-property grob 'staff-position 0))
                   edit-map-rows)))))
   ((finalize engraver)
    (let ((port (open-output-file "edit-map.json")))
      (display (string-append "[\n" (string-join (reverse edit-map-rows) ",\n") "\n]\n") port)
      (close-port port)))))
\layout { \context { \Voice \consists #edit-map-engraver } }
