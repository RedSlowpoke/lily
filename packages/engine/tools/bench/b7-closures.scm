;;; b7-closures.scm, prints 25004150000
;; higher-order code: closures, rest arguments, apply, multi-list map (music functions, callbacks)
(define (compose . fs) (if (null? fs) (lambda (x) x) (let ((f (car fs)) (g (apply compose (cdr fs)))) (lambda (x) (f (g x))))))
(define (make-adder n) (lambda (x) (+ x n)))
(define (sum . xs) (let loop ((xs xs) (acc 0)) (if (null? xs) acc (loop (cdr xs) (+ acc (car xs))))))
(define fs (map make-adder '(1 2 3 4 5 6 7 8)))
(define h (apply compose fs))
(define (run n)
  (let loop ((i 0) (acc 0))
    (if (= i n) acc
        (loop (+ i 1) (+ acc (h i) (apply sum (map (lambda (a b) (* a b)) (list i 1 2) (list 3 i 4))))))))
(display (run 100000)) (newline)
