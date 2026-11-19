//! `text-interface.cc` (MS3 step 8): markup interpreted into stencils, under
//! LilyPond's names and arities, moved here from the music crate's
//! placeholders (D67).
//!
//! - `ly:text-interface::interpret-markup` is Scheme ([`PRELUDE`]), as a
//!   primitive cannot call the markup commands: a string is
//!   `interpret_string`'s, a markup its command applied to the layout, the
//!   chain and its arguments under `max-markup-depth`, anything else
//!   LilyPond's programming error and the empty stencil. A string has its
//!   ASCII white space made spaces, its font selected, and then either the
//!   chain's outermost `string-transformers` applied and the result
//!   interpreted with the others, or its stencil made by `%text-stencil`
//!   ([`text_stencil`]): the font's, from its `font-encoding` and
//!   `font-features`, a Pango font's text (`pango.rs`).
//! - `ly:perform-text-replacements` ([`perform_text_replacements`]):
//!   `replacement-alist`'s strings replaced, the longest at each place, into
//!   a `\concat`.
//! - `ly:text-interface::print` and `::grob-interpret-markup` (MS3 step
//!   19) are Scheme ([`PRELUDE`]) for the same reason: the grob's markup,
//!   its `text` for `print` (the empty stencil if reading it killed the
//!   grob), interpreted in the grob's layout with
//!   `Font_interface::text_font_alist_chain`, its alist chain ending in
//!   `((font-encoding . latin1))`.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Bound;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Tag, Value};

use crate::R;
use crate::font::metric;
use crate::font_select::chain_get;
use crate::geometry::Box;
use crate::pango::pango_font;
use crate::stencil::Stencil;

/// `Text_interface::interpret_markup` and `interpret_string`, and
/// `print` and `grob_interpret_markup` on a grob, in `(lily)`.
pub(crate) const PRELUDE: &str = r#"
(define %markup-depth 0)
(define (%markup? x)
  (and (pair? x)
       (markup-command-signature (car x))
       (not (markup-list-function? (car x)))))
(define (%white-space->space c)
  (if (memv c '(#\tab #\newline #\vtab #\page #\return)) #\space c))
(define (%interpret-string layout props str)
  (let* ((str (string-map %white-space->space str))
         (font (ly:paper-get-font layout props))
         (transformers (ly:chain-assoc-get 'string-transformers props '())))
    (if (pair? transformers)
        (let* ((reversed (reverse transformers))
               (transformed ((car reversed) layout props str)))
          (%interpret-markup layout
                             (cons (list (cons 'string-transformers (reverse (cdr reversed)))) props)
                             transformed))
        (%text-stencil font layout props str))))
(define (%interpret-markup layout props markup)
  (cond ((string? markup) (%interpret-string layout props markup))
        ((%markup? markup)
         (let* ((option (ly:get-option 'max-markup-depth))
                (max-depth (if (and (integer? option) (exact? option) (>= option 0)) option 1024)))
           (dynamic-wind
            (lambda () #f)
            (lambda ()
              (set! %markup-depth (1+ %markup-depth))
              (if (> %markup-depth max-depth)
                  (begin
                    (%diagnose 'error #f
                               (format #f "Markup depth exceeds maximal value of ~a; Markup: ~a"
                                       max-depth (procedure-name (car markup))))
                    (ly:make-stencil '()))
                  (let ((stencil (apply (car markup) layout props (cdr markup))))
                    (if (ly:stencil? stencil)
                        stencil
                        (begin
                          (ly:programming-error "markup interpretation must yield stencil")
                          (ly:make-stencil '()))))))
            (lambda () (set! %markup-depth (1- %markup-depth))))))
        (else
         (ly:programming-error "Trying to interpret a non-markup object: ~a"
                               (call-with-output-string (lambda (port) (write markup port))))
         (ly:make-stencil '()))))
(define-public (ly:text-interface::interpret-markup layout props markup)
  (%check-type "ly:text-interface::interpret-markup" 1 ly:output-def? "Output_def" layout)
  (%interpret-markup layout props markup))
(define (%text-font-alist-chain grob)
  (ly:grob-alist-chain grob (list (cons 'font-encoding 'latin1))))
(define (%grob-interpret-markup grob markup)
  (%interpret-markup (ly:grob-layout grob) (%text-font-alist-chain grob) markup))
(define-public (ly:text-interface::grob-interpret-markup grob markup)
  (%check-type "ly:text-interface::grob-interpret-markup" 1 ly:grob? "Grob" grob)
  (%grob-interpret-markup grob markup))
(define-public (ly:text-interface::print grob)
  (%check-type "ly:text-interface::print" 1 ly:grob? "Grob" grob)
  (let ((text (ly:grob-property grob 'text)))
    (if (%grob-live? grob)
        (%grob-interpret-markup grob text)
        (ly:make-stencil '()))))
"#;

/// `Text_interface::is_markup` (`text-interface.cc:363–371`): a string,
/// or a markup command's application by the prelude's `%markup?`.
pub fn is_markup(host: &mut dyn Host, x: Value) -> Result<bool, Exception> {
    match x.tag() {
        Tag::String => Ok(true),
        Tag::Pair => match host.lily("%markup?") {
            Some(markup) => Ok(host.call(markup, &[x])? != Value::FALSE),
            None => Ok(false),
        },
        _ => Ok(false),
    }
}

/// The value of `(lily)`'s option `name`, as `ly:get-option` reads it.
fn option(cx: &mut Cx, name: &str) -> Value {
    let Some(options) = cx.global("lily", "%options") else {
        return Value::FALSE;
    };
    let key = cx.heap_mut().intern(name);
    let heap = cx.heap();
    let mut at = options;
    while let (Some(entry), Some(rest)) = (heap.car(at), heap.cdr(at)) {
        if heap.car(entry) == Some(key) {
            return heap.cdr(entry).unwrap_or(Value::FALSE);
        }
        at = rest;
    }
    Value::FALSE
}

/// `(%text-stencil font layout props str)`: the end of `interpret_string`,
/// the font's `text_stencil` of `str` with the chain's `font-features`,
/// a music string if its `font-encoding` is one of
/// `all-music-font-encodings`; a font of no text, LilyPond's programming
/// error and an empty stencil.
pub(crate) fn text_stencil(cx: &mut Cx, args: &[Value]) -> R {
    const FUNCTION: &str = "interpret_string";
    let [font, _layout, props, str] = [args[0], args[1], args[2], args[3]];
    let heap = cx.heap_mut();
    let encoding = chain_get(heap, "font-encoding", props).unwrap_or(Value::FALSE);
    let features = chain_get(heap, "font-features", props).unwrap_or(Value::FALSE);
    let mut features_str = String::new();
    if heap.car(features).is_some() {
        let mut at = features;
        while let (Some(feature), Some(rest)) = (heap.car(at), heap.cdr(at)) {
            let Some(feature) = heap.str(feature).map(|s| String::from(s.as_str())) else {
                return Err(cx.error(
                    "misc-error",
                    Some(FUNCTION),
                    "Found non-string in font-features list",
                    &[],
                ));
            };
            if !features_str.is_empty() {
                features_str.push(',');
            }
            features_str.push_str(&feature);
            at = rest;
        }
    } else if features != Value::FALSE {
        return Err(cx.error(
            "misc-error",
            Some(FUNCTION),
            "Expecting a list for font-features value",
            &[],
        ));
    }
    let mut encodings = cx
        .global("lily", "all-music-font-encodings")
        .unwrap_or(Value::NULL);
    let heap = cx.heap();
    let mut music = false;
    while let (Some(e), Some(rest)) = (heap.car(encodings), heap.cdr(encodings)) {
        music |= e == encoding;
        encodings = rest;
    }
    let text = String::from(heap.str(str).map(|s| s.as_str()).unwrap_or(""));
    if pango_font(heap, font).is_none() {
        let errors = match metric(heap, font) {
            Some(_) => alloc::vec![String::from("Cannot get a text stencil from this font")],
            None => Vec::new(),
        };
        let stencil = cx.heap_mut().wrap(Stencil::new(Box::EMPTY, Value::NULL));
        return crate::programming_errored(cx, stencil, &errors);
    }
    let to_paths = option(cx, "music-strings-to-paths") != Value::FALSE;
    let faces = crate::font::text_faces(cx, font, &text, music)?;
    let mut warnings = Vec::new();
    let stencil = crate::pango::text_stencil(
        cx.heap_mut(),
        font,
        &faces,
        &text,
        music,
        to_paths,
        &features_str,
        &mut warnings,
    );
    let stencil = cx.heap_mut().wrap(stencil);
    crate::warned(cx, stencil, &warnings)
}

/// `(ly:perform-text-replacements layout props input-string)`: each place
/// of `input-string` where a string of the chain's `replacement-alist`
/// begins, the longest, replaced by its value (the first of a string that
/// is there twice), never inside a UTF-8 character and never again in what
/// replaced it, as `(make-concat-markup (list before replacement … rest))`;
/// the string itself if there is nothing to replace.
pub(crate) fn perform_text_replacements(cx: &mut Cx, args: &[Value]) -> R {
    let input = args[2];
    let Some(text) = cx.heap().str(input).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type_vm(Some("scm_to_utf8_stringn"), input, "string"));
    };
    let alist = chain_get(cx.heap_mut(), "replacement-alist", args[1]).unwrap_or(Value::NULL);
    if alist == Value::NULL || text.is_empty() {
        return Ok(input);
    }
    let heap = cx.heap();
    let mut table: BTreeMap<Vec<u8>, Value> = BTreeMap::new();
    let mut longest = 0;
    let mut at = alist;
    while let (Some(entry), Some(rest)) = (heap.car(at), heap.cdr(at)) {
        if let (Some(k), Some(v)) = (heap.car(entry), heap.cdr(entry))
            && let Some(k) = heap.str(k)
            && !k.as_str().is_empty()
        {
            let k = k.as_str().as_bytes().to_vec();
            longest = longest.max(k.len());
            table.entry(k).or_insert(v);
        }
        at = rest;
    }
    let bytes = text.as_bytes();
    let mut parts: Vec<Value> = Vec::new();
    let (mut last, mut i) = (0, 0);
    let piece = |cx: &mut Cx, from: usize, to: usize| {
        let s = String::from_utf8_lossy(&bytes[from..to]).into_owned();
        cx.heap_mut().string(s)
    };
    while i < bytes.len() {
        while i < bytes.len() {
            if bytes[i] & 0xc0 == 0x80 {
                break;
            }
            let probe = &bytes[i..(i + longest).min(bytes.len())];
            let Some((key, &value)) = table
                .range::<[u8], _>((Bound::Unbounded, Bound::Included(probe)))
                .next_back()
            else {
                break;
            };
            if !bytes[i..].starts_with(key) {
                break;
            }
            let before = piece(cx, last, i);
            parts.push(before);
            parts.push(value);
            i += key.len();
            last = i;
        }
        i += 1;
    }
    let rest = piece(cx, last, bytes.len().min(i));
    parts.push(rest);
    let list = cx.heap_mut().list(&parts);
    let Some(concat) = cx.global("lily", "make-concat-markup") else {
        return Err(cx.error("misc-error", None, "make-concat-markup is not defined", &[]));
    };
    Ok(cx.tail_call(concat, &[list]))
}
