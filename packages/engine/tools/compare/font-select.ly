%% O2's selection capture (01 §3.7, MS3 step 5): the music fonts a score's
%% grobs and markups select, as the settings file of a compile with the
%% capture's flags (capture.mjs: --svg -dpoint-and-click):
%%
%%   lilypond --svg -dpoint-and-click -dinclude-settings=font-select.ly score.ly
%%
%% One line per selection into FONT_SELECT_OUT (default fonts.out),
%% tab-separated, each value as `write` writes it: who selected (a grob's
%% name, or `markup`), then what select_font reads (the layout's
%% staff-height, and from the chain font-size, font-encoding, font-family
%% and font-name, #f where the chain has none), then the font it gave: its
%% file name, design size and magnification. A grob's font is the `font`
%% property Font_interface::get_default_font caches, read after the pages
%% are made, its chain music_font_alist_chain's; a markup's is what
%% ly:paper-get-font gives a markup command. Fonts that are no OpenType
%% font's (Pango's text fonts) are not written. tools/bench/test/
%% fonts.test.mjs selects the same fonts with ly:paper-get-font in LilyPond
%% and in the engine.
#(begin
  (define font-select-port (open-file (or (getenv "FONT_SELECT_OUT") "fonts.out") "w"))
  (define (font-select-line who layout size encoding family name font)
    (when (ly:otf-font? font)
      (display
       (string-join
        (map (lambda (x) (format #f "~s" x))
             (list who (ly:output-def-lookup layout 'staff-height) size encoding family name
                   (ly:font-file-name font) (ly:font-design-size font) (ly:font-magnification font)))
        "\t")
       font-select-port)
      (newline font-select-port)
      (force-output font-select-port)))
  ;; A markup command's ly:paper-get-font, called by name from (lily).
  (define font-select-get-font ly:paper-get-font)
  (module-set! (resolve-module '(lily)) 'ly:paper-get-font
   (lambda (layout chain)
     (let ((font (font-select-get-font layout chain))
           (get (lambda (key) (ly:chain-assoc-get key chain #f))))
       (font-select-line 'markup layout (get 'font-size) (get 'font-encoding) (get 'font-family)
                         (get 'font-name) font)
       font)))
  ;; Each grob's cached font, with its chain's values and
  ;; music_font_alist_chain's defaults after them.
  (ly:output-def-set-variable! $defaultpaper 'page-post-process
   (lambda (paper pages)
     (for-each
      (lambda (page)
        (for-each
         (lambda (line)
           (let ((sys (ly:prob-property line 'system-grob #f)))
             (when (ly:grob? sys)
               (for-each
                (lambda (g)
                  (let ((font (ly:grob-property g 'font #f))
                        (get (lambda (key default) (ly:grob-property g key default))))
                    (when (ly:font-metric? font)
                      (font-select-line (grob::name g) (ly:grob-layout g) (get 'font-size #f)
                                        (get 'font-encoding 'fetaMusic) (get 'font-family 'music)
                                        (get 'font-name #f) font))))
                (ly:grob-array->list (ly:grob-object sys 'all-elements))))))
         (ly:prob-property page 'lines '())))
      pages))))
