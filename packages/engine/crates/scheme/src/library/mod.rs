//! The Guile modules that LilyPond's library files import (chapter 03
//! §2.6), made with every VM as `(srfi srfi-1)` and `(ice-9 format)` are
//! (`prims::MODULES`): each uses `(guile)` and exports what it defines.
//!
//! - `(srfi srfi-2)` and `(ice-9 and-let-star)`: `and-let*`.
//! - `(srfi srfi-11)`: `let-values` and `let*-values`.
//! - `(srfi srfi-13)`, empty: Guile 3 defines its string procedures in
//!   `(guile)`, where the engine has those it has.
//! - `(srfi srfi-39)`: `with-parameters*`; it re-exports `make-parameter`
//!   and the port parameters, while `parameterize` is the expander's own
//!   and seen everywhere.
//! - `(ice-9 receive)`: `receive`.
//! - `(ice-9 list)`: `rassoc`, `rassv` and `rassq`.
//! - `(ice-9 optargs)`: `let-optional`, `let-optional*`, `let-keywords`
//!   and `let-keywords*` over `lambda*`; `define*`, `lambda*` and
//!   `define*-public` are the expander's.
//! - `(ice-9 match)` in `syntax-rules` ([`r#match`]).
//! - `(ice-9 pretty-print)`: `pretty-print` ([`pretty`]).
//! - `(ice-9 textual-ports)` and `(ice-9 binary-ports)`' bytevector output
//!   port ([`ports`]).
//! - `(ice-9 arrays)`: `array-copy`.
//! - `(oop goops)`: `define-class` as a record (`goops.rs`).
//! - `(ice-9 control)`, empty: `define-markup-commands.scm` imports it and
//!   uses none of its names.

pub(crate) mod r#match;
pub(crate) mod ports;
pub(crate) mod pretty;

/// The primitives of each module.
pub(crate) const TEXTUAL_PORTS: &[&str] = &[
    "get-char",
    "lookahead-char",
    "get-line",
    "get-string-all",
    "get-string-n",
    "put-char",
    "put-string",
];
pub(crate) const BINARY_PORTS: &[&str] = &[
    "%open-bytevector-output-port",
    "%take-bytevector",
    "put-u8",
    "put-bytevector",
];
pub(crate) const PRETTY_PRINT: &[&str] = &["%pretty-print", "%port-column"];
pub(crate) const ARRAYS: &[&str] = &["array-copy"];

/// A clause is `(var expr)`, `(expr)` or a variable; with no body, the
/// value of the last clause.
pub(crate) const SRFI_2: &str = r#"
(define-syntax and-let*
  (syntax-rules ()
    ((_ ()) #t)
    ((_ () body0 body ...) (let () body0 body ...))
    ((_ ((var expr))) (let ((var expr)) var))
    ((_ ((expr))) expr)
    ((_ ((var expr) clause ...) body ...)
     (let ((var expr)) (if var (and-let* (clause ...) body ...) #f)))
    ((_ ((expr) clause ...) body ...)
     (if expr (and-let* (clause ...) body ...) #f))
    ((_ (var)) var)
    ((_ (var clause ...) body ...)
     (if var (and-let* (clause ...) body ...) #f))))
"#;

pub(crate) const SRFI_11: &str = r#"
(define-syntax let-values
  (syntax-rules ()
    ((_ (binding ...) body0 body ...)
     (%let-values (binding ...) () (let () body0 body ...)))))
(define-syntax %let-values
  (syntax-rules ()
    ((_ () (temp ...) body) (let (temp ...) body))
    ((_ ((formals e) binding ...) temps body)
     (%let-values-formals formals e () (binding ...) temps body))))
(define-syntax %let-values-formals
  (syntax-rules ()
    ((_ () e args bindings temps body)
     (call-with-values (lambda () e)
       (lambda args (%let-values bindings temps body))))
    ((_ (a . b) e (arg ...) bindings (temp ...) body)
     (%let-values-formals b e (arg ... x) bindings (temp ... (a x)) body))
    ((_ a e (arg ...) bindings (temp ...) body)
     (call-with-values (lambda () e)
       (lambda (arg ... . x) (%let-values bindings (temp ... (a x)) body))))))
(define-syntax let*-values
  (syntax-rules ()
    ((_ () body0 body ...) (let () body0 body ...))
    ((_ (binding0 binding ...) body0 body ...)
     (let-values (binding0) (let*-values (binding ...) body0 body ...)))))
"#;

pub(crate) const SRFI_39: &str = r#"
(re-export make-parameter current-input-port current-output-port current-error-port)
(define (with-parameters* params values thunk)
  (if (null? params)
      (thunk)
      (parameterize (((car params) (car values)))
        (with-parameters* (cdr params) (cdr values) thunk))))
"#;

/// The entry whose cdr is `key`.
pub(crate) const LIST: &str = r#"
(define (%rassoc key alist same?)
  (let loop ((rest alist))
    (and (pair? rest)
         (if (same? key (cdar rest)) (car rest) (loop (cdr rest))))))
(define* (rassoc key alist #:optional (same? equal?)) (%rassoc key alist same?))
(define (rassv key alist) (%rassoc key alist eqv?))
(define (rassq key alist) (%rassoc key alist eq?))
"#;

pub(crate) const RECEIVE: &str = r#"
(define-syntax-rule (receive vars vals . body)
  (call-with-values (lambda () vals) (lambda vars . body)))
"#;

/// `let-keywords` evaluates its defaults outside the bindings, as Guile's
/// does, `let-keywords*` inside, as `lambda*` does; `#:allow-other-keys`
/// is the literal `#t` or `#f` after the rest argument.
pub(crate) const OPTARGS: &str = r#"
(define-syntax let-optional
  (syntax-rules ()
    ((_ rest (binding ...) body0 body ...)
     (%let-defaults rest (binding ...) () () #:optional (body0 body ...)))))
(define-syntax let-optional*
  (syntax-rules ()
    ((_ rest (binding ...) body0 body ...)
     (apply (lambda* (#:optional binding ... . rest) body0 body ...) rest))))
(define-syntax let-keywords
  (syntax-rules ()
    ((_ rest aok (binding ...) body0 body ...)
     (%let-defaults rest (binding ...) () () (#:key aok) (body0 body ...)))))
(define-syntax let-keywords*
  (syntax-rules ()
    ((_ rest #f (binding ...) body0 body ...)
     (apply (lambda* (#:key binding ...) body0 body ...) rest))
    ((_ rest #t (binding ...) body0 body ...)
     (apply (lambda* (#:key binding ... #:allow-other-keys) body0 body ...) rest))))
(define-syntax %let-defaults
  (syntax-rules ()
    ((_ rest () (temp ...) (formal ...) k body)
     (let (temp ...) (%let-apply k rest (formal ...) body)))
    ((_ rest ((v init) . more) (temp ...) (formal ...) k body)
     (%let-defaults rest more (temp ... (t (lambda () init))) (formal ... (v (t))) k body))
    ((_ rest (v . more) temps (formal ...) k body)
     (%let-defaults rest more temps (formal ... (v #f)) k body))))
(define-syntax %let-apply
  (syntax-rules ()
    ((_ #:optional rest (formal ...) (body ...))
     (apply (lambda* (#:optional formal ... . rest) body ...) rest))
    ((_ (#:key #f) rest (formal ...) (body ...))
     (apply (lambda* (#:key formal ...) body ...) rest))
    ((_ (#:key #t) rest (formal ...) (body ...))
     (apply (lambda* (#:key formal ... #:allow-other-keys) body ...) rest))))
"#;

pub(crate) const PRETTY_PRINT_PRELUDE: &str = r#"
(define* (pretty-print obj #:optional port* #:key (port (or port* (current-output-port)))
                       (width 79) (max-expr-width 50) (display? #f) (per-line-prefix ""))
  (display (%pretty-print obj width display? per-line-prefix (%port-column port)) port))
"#;

pub(crate) const BINARY_PORTS_PRELUDE: &str = r#"
(define (open-bytevector-output-port . transcoder)
  (let ((port (%open-bytevector-output-port)))
    (values port (lambda () (%take-bytevector port)))))
"#;
