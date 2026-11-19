//! `(ice-9 match)` in `syntax-rules`, written for the engine from the
//! documentation of Guile 3's `match` (Alex Shinn's matcher): `match`,
//! `match-lambda`, `match-lambda*`, `match-let` (also named) and
//! `match-let*`, with `=>` and its failure procedure.
//!
//! Patterns: `_`, an identifier (bound, or compared with `equal?` to its
//! value if the pattern bound it already), a datum, `'datum`, `` `qp ``
//! with `,p` inside, `()`, `(p . q)`, `(p ...)` and `(p ___)` followed by
//! fixed patterns, `(p ..1)`, `#(p ...)`, `(? pred p ...)`, `(= proc p)`,
//! `(and p ...)`, `(or p ...)` and `(not p)`. Not provided: `$` and records,
//! `get!` and `set!`, `**1`, `=..`, `..=`, tree patterns, `match-letrec`.
//!
//! The matcher is in continuation-passing style: a success continuation is
//! a macro use `(k arg ...)` that receives the identifiers bound so far as
//! its last operand, so that a later identifier knows whether it is new.
//! Whether a pattern is an identifier, and whether an identifier is one of
//! those, is told by a `syntax-rules` of the pattern itself, as the
//! portable matchers tell it.

/// The module's Scheme.
pub(crate) const PRELUDE: &str = r#"
(define (%match-error v) (throw 'match-error "match" "no matching pattern" v))
(define-syntax match
  (syntax-rules ()
    ((_ e clause ...) (let ((v e)) (%match-next v clause ...)))))
(define-syntax %match-next
  (syntax-rules (=>)
    ((_ v) (%match-error v))
    ((_ v (pat (=> fail) body ...) clause ...)
     (let ((fail (lambda () (%match-next v clause ...))))
       (%match-one v pat (%match-body (let () body ...)) (fail) ())))
    ((_ v (pat body ...) clause ...)
     (let ((fail (lambda () (%match-next v clause ...))))
       (%match-one v pat (%match-body (let () body ...)) (fail) ())))))
(define-syntax %match-body
  (syntax-rules ()
    ((_ e ids) e)))
(define-syntax %match-one
  (syntax-rules ::: (_ ... ___ ..1 quote quasiquote ? = and or not)
    ((_ v _ (k a :::) fk ids) (k a ::: ids))
    ((_ v (quote d) (k a :::) fk ids) (if (equal? v 'd) (k a ::: ids) fk))
    ((_ v (quasiquote q) sk fk ids) (%match-quasi v q sk fk ids))
    ((_ v (? pred) (k a :::) fk ids) (if (pred v) (k a ::: ids) fk))
    ((_ v (? pred p :::) sk fk ids)
     (if (pred v) (%match-one v (and p :::) sk fk ids) fk))
    ((_ v (= proc p) sk fk ids) (let ((w (proc v))) (%match-one w p sk fk ids)))
    ((_ v (and) (k a :::) fk ids) (k a ::: ids))
    ((_ v (and p q :::) sk fk ids)
     (%match-one v p (%match-one v (and q :::) sk fk) fk ids))
    ((_ v (or) sk fk ids) fk)
    ((_ v (or p q :::) sk fk ids)
     (%match-one v p sk (%match-one v (or q :::) sk fk ids) ids))
    ((_ v (not p) (k a :::) fk ids)
     (%match-one v p (%match-body fk) (k a ::: ids) ids))
    ((_ v (p ..1) sk fk ids)
     (if (pair? v) (%match-tail v p () sk fk ids) fk))
    ((_ v (p ... . tail) sk fk ids) (%match-tail v p tail sk fk ids))
    ((_ v (p ___ . tail) sk fk ids) (%match-tail v p tail sk fk ids))
    ((_ v (p . q) sk fk ids)
     (if (pair? v)
         (let ((head (car v)) (rest (cdr v)))
           (%match-one head p (%match-one rest q sk fk) fk ids))
         fk))
    ((_ v () (k a :::) fk ids) (if (null? v) (k a ::: ids) fk))
    ((_ v #(p :::) sk fk ids)
     (if (vector? v)
         (let ((items (vector->list v))) (%match-one items (p :::) sk fk ids))
         fk))
    ((_ v x sk fk ids)
     (%match-ident? x (%match-bind v x sk fk ids) (%match-datum v x sk fk ids)))))
(define-syntax %match-datum
  (syntax-rules ()
    ((_ v x (k a ...) fk ids) (if (equal? v x) (k a ... ids) fk))))
(define-syntax %match-ident?
  (syntax-rules ()
    ((_ (x . y) sk fk) fk)
    ((_ #(x ...) sk fk) fk)
    ((_ x sk fk)
     (let-syntax ((ident? (syntax-rules () ((_ x ks kf) ks) ((_ y ks kf) kf))))
       (ident? abracadabra sk fk)))))
(define-syntax %match-bind
  (syntax-rules ()
    ((_ v x (k a ...) fk (id ...))
     (let-syntax ((new? (syntax-rules (id ...) ((_ x ks kf) ks) ((_ y ks kf) kf))))
       (new? abracadabra
             (let ((x v)) (k a ... (x id ...)))
             (if (equal? v x) (k a ... (id ...)) fk))))))
(define-syntax %match-quasi
  (syntax-rules ::: (unquote)
    ((_ v (unquote p) sk fk ids) (%match-one v p sk fk ids))
    ((_ v (a . b) sk fk ids)
     (if (pair? v)
         (let ((head (car v)) (rest (cdr v)))
           (%match-quasi head a (%match-quasi rest b sk fk) fk ids))
         fk))
    ((_ v #(a :::) sk fk ids)
     (if (vector? v)
         (let ((items (vector->list v))) (%match-quasi items (a :::) sk fk ids))
         fk))
    ((_ v d (k a :::) fk ids) (if (equal? v 'd) (k a ::: ids) fk))))
(define-syntax %match-tail
  (syntax-rules ()
    ((_ v p () sk fk ids)
     (if (list? v) (%match-vars p (%match-each v p sk fk ids) ()) fk))
    ((_ v p tail sk fk ids)
     (if (list? v)
         (let ((n (- (length v) (length 'tail))))
           (if (>= n 0)
               (let ((head (list-head v n)) (rest (list-tail v n)))
                 (%match-tail head p () (%match-one rest tail sk fk) fk ids))
               fk))
         fk))))
(define-syntax %match-each
  (syntax-rules ()
    ((_ v p sk fk ids (x ...)) (%match-temps (x ...) () v p sk fk ids))))
(define-syntax %match-temps
  (syntax-rules ()
    ((_ () ((x t) ...) v p (k a ...) fk ids)
     (let loop ((l v) (t '()) ...)
       (if (null? l)
           (let ((x (reverse t)) ...) (k a ... (x ... . ids)))
           (let ((item (car l)))
             (%match-one item p (%match-body (loop (cdr l) (cons x t) ...)) fk ids)))))
    ((_ (y . ys) (pair ...) v p sk fk ids)
     (%match-temps ys (pair ... (y tmp)) v p sk fk ids))))
(define-syntax %match-vars
  (syntax-rules ::: (_ ... ___ ..1 quote quasiquote ? = and or not)
    ((_ _ (k a :::) vs) (k a ::: vs))
    ((_ (quote d) (k a :::) vs) (k a ::: vs))
    ((_ (quasiquote q) k vs) (%match-vars-quasi q k vs))
    ((_ (? pred p :::) k vs) (%match-vars (p :::) k vs))
    ((_ (= proc p) k vs) (%match-vars p k vs))
    ((_ (and p :::) k vs) (%match-vars (p :::) k vs))
    ((_ (or) (k a :::) vs) (k a ::: vs))
    ((_ (or p q :::) k vs) (%match-vars p k vs))
    ((_ (not p) (k a :::) vs) (k a ::: vs))
    ((_ (p ..1) k vs) (%match-vars p k vs))
    ((_ (p ... . tail) k vs) (%match-vars (p . tail) k vs))
    ((_ (p ___ . tail) k vs) (%match-vars (p . tail) k vs))
    ((_ (p . q) k vs) (%match-vars p (%match-vars q k) vs))
    ((_ #(p :::) k vs) (%match-vars (p :::) k vs))
    ((_ x (k a :::) (v :::)) (%match-ident? x (k a ::: (x v :::)) (k a ::: (v :::))))))
(define-syntax %match-vars-quasi
  (syntax-rules ::: (unquote)
    ((_ (unquote p) k vs) (%match-vars p k vs))
    ((_ (a . b) k vs) (%match-vars-quasi a (%match-vars-quasi b k) vs))
    ((_ #(a :::) k vs) (%match-vars-quasi (a :::) k vs))
    ((_ d (k a :::) vs) (k a ::: vs))))
(define-syntax match-lambda
  (syntax-rules ()
    ((_ clause ...) (lambda (x) (match x clause ...)))))
(define-syntax match-lambda*
  (syntax-rules ()
    ((_ clause ...) (lambda x (match x clause ...)))))
(define-syntax match-let*
  (syntax-rules ()
    ((_ () body ...) (let () body ...))
    ((_ ((pat e) binding ...) body ...)
     (match e (pat (match-let* (binding ...) body ...))))))
(define-syntax match-let
  (syntax-rules ()
    ((_ (binding ...) body ...) (%match-let (binding ...) () body ...))
    ((_ name ((pat e) ...) body ...)
     (%match-let-named name ((pat e) ...) () body ...))))
(define-syntax %match-let
  (syntax-rules ()
    ((_ () ((pat t e) ...) body ...)
     (let ((t e) ...) (match-let* ((pat t) ...) body ...)))
    ((_ ((pat e) binding ...) (done ...) body ...)
     (%match-let (binding ...) (done ... (pat tmp e)) body ...))))
(define-syntax %match-let-named
  (syntax-rules ()
    ((_ name () ((pat t e) ...) body ...)
     (let name ((t e) ...) (match-let* ((pat t) ...) body ...)))
    ((_ name ((pat e) binding ...) (done ...) body ...)
     (%match-let-named name (binding ...) (done ... (pat tmp e)) body ...))))
"#;
