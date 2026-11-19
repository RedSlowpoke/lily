;;; b3-loop.scm, prints 2999997 #t
;; tail calls: a named let and two mutually recursive procedures, 1e6 iterations each
(define (my-even? n) (if (= n 0) #t (my-odd? (- n 1))))
(define (my-odd? n) (if (= n 0) #f (my-even? (- n 1))))
(display (let loop ((i 0) (acc 0)) (if (= i 1000000) acc (loop (+ i 1) (+ acc (remainder i 7))))))
(display " ") (display (my-even? 1000000)) (newline)
