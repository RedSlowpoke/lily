;;; b1-fib.scm, prints 832040
(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
(display (fib 30)) (newline)
