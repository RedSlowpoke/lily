;;; b4-alist.scm, prints 4679166
;; grob-property-like lookups: a chain of alists searched front to back (ly:assoc-get, ly:chain-assoc-get)
(define (make-props prefix n)
  (let loop ((i 0) (acc '()))
    (if (= i n) acc
        (loop (+ i 1) (cons (cons (string->symbol (string-append prefix (number->string i))) i) acc)))))
(define chain (list (make-props "override-" 8) (make-props "prop-" 40) (make-props "default-" 60)))
(define keys (list->vector (append (map car (car chain)) (map car (cadr chain)) (map car (caddr chain)) (list 'missing))))
(define (chain-get key chain default)
  (if (null? chain) default
      (let ((hit (assq key (car chain)))) (if hit (cdr hit) (chain-get key (cdr chain) default)))))
(define (run n)
  (let loop ((i 0) (sum 0) (chain chain))
    (if (= i n) sum
        (let ((k (vector-ref keys (remainder i (vector-length keys)))))
          (loop (+ i 1) (+ sum (chain-get k chain 0))
                (if (= (remainder i 1000) 0) (cons (list (cons 'tmp i)) (cdr chain)) chain))))))
(display (run 200000)) (newline)
