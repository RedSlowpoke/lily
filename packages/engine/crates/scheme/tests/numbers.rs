//! S1's numbers (chapter 03 §5.1, R13): the tower's arithmetic, comparison
//! and conversion primitives, `number->string` and `string->number`. The
//! tables' expected texts are Guile 3.0.11's; with `guile` installed, every
//! case is also run by guile and compared. The bignums are also checked
//! against `i128`, and the flonums' printing against Rust's shortest digits.
use std::process::Command;

use lily_engine_scheme::{BigInt, Heap, Num, NumError, Rounding, Tag, Value, parse_number};

// --- the cases, run by the heap and by guile -------------------------------------

/// Expressions with their arguments as literals, and what Guile 3.0.11 writes
/// for their values, or the key of the error it throws.
const CASES: &[(&str, &str)] = &[
    ("(+ 1 2)", "3"),
    ("(+ 2305843009213693951 1)", "2305843009213693952"),
    ("(- -2305843009213693952 1)", "-2305843009213693953"),
    (
        "(* 2305843009213693951 2305843009213693951)",
        "5316911983139663487003542222693990401",
    ),
    ("(* 4294967296 4294967296)", "18446744073709551616"),
    ("(- 2305843009213693952 1)", "2305843009213693951"),
    ("(+ 1/2 1/3)", "5/6"),
    ("(+ 1/2 0.5)", "1.0"),
    ("(+ 1/2 1/2)", "1"),
    ("(- 1/2 1/2)", "0"),
    ("(* 1/3 3)", "1"),
    ("(* 2/3 3/4)", "1/2"),
    ("(/ 6 4)", "3/2"),
    ("(/ 6 3)", "2"),
    ("(/ -6 4)", "-3/2"),
    ("(/ 6 -4)", "-3/2"),
    ("(/ 1 3)", "1/3"),
    ("(/ 1/2)", "2"),
    ("(/ 0.0)", "+inf.0"),
    ("(/ 0)", "#<numerical-overflow>"),
    ("(/ 1 0)", "#<numerical-overflow>"),
    ("(/ 1.0 0)", "#<numerical-overflow>"),
    ("(/ 0 0.0)", "+nan.0"),
    ("(/ 1 0.0)", "+inf.0"),
    ("(/ -1 0.0)", "-inf.0"),
    ("(/ 7 2.0)", "3.5"),
    (
        "(/ 123456789012345678901234567890 10)",
        "12345678901234567890123456789",
    ),
    (
        "(/ 123456789012345678901234567890 7)",
        "17636684144620811271604938270",
    ),
    ("(- 0.0)", "-0.0"),
    ("(- 0)", "0"),
    ("(- 1/2)", "-1/2"),
    ("(- 0.0+1.0i)", "-0.0-1.0i"),
    ("(+ 0 -0.0)", "0.0"),
    ("(* -1 0.0)", "-0.0"),
    ("(* 0 1.5)", "0.0"),
    ("(* 0 +inf.0)", "+nan.0"),
    ("(* 1.5 1.0+1.0i)", "1.5+1.5i"),
    ("(* 0.0+1.0i 0.0+1.0i)", "-1.0+0.0i"),
    ("(+ 1.0+2.0i 1.0-2.0i)", "2.0+0.0i"),
    ("(- 1.0+1.0i 1.0+1.0i)", "0.0+0.0i"),
    ("(- 1 1.0+1.0i)", "0.0-1.0i"),
    ("(/ 1 1.0+1.0i)", "0.5-0.5i"),
    ("(/ 1.0+1.0i 0.0)", "+inf.0+inf.0i"),
    (
        "(/ 1.0+1.0i 2.0-3.0i)",
        "-0.07692307692307694+0.3846153846153846i",
    ),
    ("(/ 1.0+1.0i 0)", "#<numerical-overflow>"),
    ("(1+ 2305843009213693951)", "2305843009213693952"),
    ("(1- -2305843009213693952)", "-2305843009213693953"),
    ("(1- 0.5)", "-0.5"),
    ("(abs -5/3)", "5/3"),
    ("(abs -2305843009213693952)", "2305843009213693952"),
    ("(abs -0.0)", "0.0"),
    (
        "(abs -123456789012345678901234567890)",
        "123456789012345678901234567890",
    ),
    ("(= 1 1.0)", "#t"),
    ("(= 1/2 0.5)", "#t"),
    ("(= 9007199254740993 9007199254740992.0)", "#f"),
    ("(= 1.0+0.0i 1)", "#t"),
    ("(= 1.0+1.0i 1.0+1.0i)", "#t"),
    ("(= +nan.0 +nan.0)", "#f"),
    ("(= 0.0 -0.0)", "#t"),
    ("(< 1/3 0.3333333333333333)", "#f"),
    ("(> 1/3 0.3333333333333333)", "#t"),
    ("(< 1 +inf.0)", "#t"),
    ("(< -inf.0 -123456789012345678901234567890)", "#t"),
    ("(< 1 +nan.0)", "#f"),
    ("(<= 2 2.0)", "#t"),
    ("(>= 1/2 1/3)", "#t"),
    (
        "(< 123456789012345678901234567890 123456789012345678901234567891)",
        "#t",
    ),
    (
        "(< -123456789012345678901234567891 -123456789012345678901234567890)",
        "#t",
    ),
    ("(< 1.0+1.0i 2)", "#<wrong-type-arg>"),
    ("(max 1 2.0)", "2.0"),
    ("(min 1 2.0)", "1.0"),
    ("(max 1/2 1/3)", "1/2"),
    ("(min 1/2 1/3)", "1/3"),
    ("(max 1/2 0.4)", "0.5"),
    ("(max 1 +nan.0)", "+nan.0"),
    ("(min +nan.0 1)", "+nan.0"),
    ("(max 0 -0.0)", "0.0"),
    ("(max -0.0 0)", "0.0"),
    ("(min 0 -0.0)", "-0.0"),
    ("(min 0.0 -0.0)", "-0.0"),
    (
        "(max 123456789012345678901234567890 1)",
        "123456789012345678901234567890",
    ),
    ("(quotient 7 2)", "3"),
    ("(quotient -7 2)", "-3"),
    ("(remainder -7 2)", "-1"),
    ("(modulo -7 2)", "1"),
    ("(modulo 7 -2)", "-1"),
    ("(modulo -7 2.0)", "1.0"),
    ("(quotient 7 0)", "#<numerical-overflow>"),
    ("(quotient 1.5 2)", "#<wrong-type-arg>"),
    (
        "(remainder 123456789012345678901234567890 1000000007)",
        "197434842",
    ),
    (
        "(modulo -123456789012345678901234567890 1000000007)",
        "802565165",
    ),
    (
        "(quotient 123456789012345678901234567890 -98765432109876543210)",
        "-1249999988",
    ),
    ("(floor/ 7 -2)", "-4 -1"),
    ("(floor/ -7 2)", "-4 1"),
    ("(ceiling/ 7 2)", "4 -1"),
    ("(truncate/ -7 2)", "-3 -1"),
    ("(round/ 7 2)", "4 -1"),
    ("(round/ 5 2)", "2 1"),
    ("(round/ -5 2)", "-2 -1"),
    ("(euclidean/ -7 2)", "-4 1"),
    ("(euclidean/ -7 -2)", "4 1"),
    ("(euclidean/ 7 -2)", "-3 1"),
    ("(centered/ 7 2)", "4 -1"),
    ("(centered/ -7 2)", "-3 -1"),
    ("(centered/ 5 -2)", "-3 -1"),
    ("(floor/ 16/3 -10/7)", "-4 -8/21"),
    ("(euclidean/ 16/3 -10/7)", "-3 22/21"),
    ("(floor/ -123.2 -63.5)", "1.0 -59.7"),
    ("(centered/ -123.2 -63.5)", "2.0 3.799999999999997"),
    ("(round/ 7.5 2)", "4.0 -0.5"),
    ("(truncate/ 7 0.0)", "#<numerical-overflow>"),
    ("(floor 7/2)", "3"),
    ("(floor -7/2)", "-4"),
    ("(ceiling 7/2)", "4"),
    ("(ceiling -7/2)", "-3"),
    ("(truncate -7/2)", "-3"),
    ("(round 7/2)", "4"),
    ("(round 5/2)", "2"),
    ("(round -5/2)", "-2"),
    ("(round 1/2)", "0"),
    ("(round 2.5)", "2.0"),
    ("(round 3.5)", "4.0"),
    ("(round -0.5)", "0.0"),
    ("(round -2.5)", "-2.0"),
    ("(round 1.5)", "2.0"),
    ("(truncate -2.7)", "-2.0"),
    ("(floor -0.5)", "-1.0"),
    ("(ceiling -0.5)", "-0.0"),
    ("(floor 1e300)", "1.0e300"),
    ("(floor 2.5+0.0i)", "#<wrong-type-arg>"),
    ("(gcd 12 -18)", "6"),
    ("(gcd 0 0)", "0"),
    ("(gcd 0.0 4)", "4.0"),
    ("(gcd 123456789012345678901234567890 9876543210)", "90"),
    ("(lcm 4 6)", "12"),
    ("(lcm 0 5)", "0"),
    ("(lcm -4 6.0)", "12.0"),
    ("(gcd 1.5 2)", "#<wrong-type-arg>"),
    ("(numerator 6/4)", "3"),
    ("(denominator 6/4)", "2"),
    ("(numerator 5)", "5"),
    ("(denominator 5)", "1"),
    ("(numerator 0.5)", "1.0"),
    ("(denominator 0.75)", "4.0"),
    ("(numerator 0.3)", "5404319552844595.0"),
    ("(numerator +inf.0)", "+inf.0"),
    ("(denominator -0.0)", "1.0"),
    ("(numerator -1/3)", "-1"),
    ("(exact->inexact 1/3)", "0.3333333333333333"),
    ("(exact->inexact 2/3)", "0.6666666666666666"),
    (
        "(exact->inexact 123456789012345678901234567890)",
        "1.2345678901234568e29",
    ),
    ("(exact->inexact 9007199254740993)", "9007199254740992.0"),
    (
        "(exact->inexact 12345678901234567890123/1000)",
        "12345678901234567000.0",
    ),
    ("(exact->inexact -1/3)", "-0.3333333333333333"),
    ("(inexact->exact 0.1)", "3602879701896397/36028797018963968"),
    ("(inexact->exact -0.75)", "-3/4"),
    ("(inexact->exact 1e20)", "100000000000000000000"),
    ("(inexact->exact 1e30)", "1000000000000000019884624838656"),
    (
        "(inexact->exact 5e-324)",
        "1/202402253307310618352495346718917307049556649764142118356901358027430339567995346891960383701437124495187077864316811911389808737385793476867013399940738509921517424276566361364466907742093216341239767678472745068562007483424692698618103355649159556340810056512358769552333414615230502532186327508646006263307707741093494784",
    ),
    ("(inexact->exact +inf.0)", "#<out-of-range>"),
    ("(inexact->exact +nan.0)", "#<out-of-range>"),
    ("(inexact->exact 2.0+0.0i)", "2"),
    ("(inexact->exact 1.0+1.0i)", "#<wrong-type-arg>"),
    ("(inexact->exact 1/3)", "1/3"),
    ("(expt 2 100)", "1267650600228229401496703205376"),
    ("(expt 2 -2)", "1/4"),
    ("(expt -2 3)", "-8"),
    ("(expt 2/3 3)", "8/27"),
    ("(expt 2/3 -3)", "27/8"),
    ("(expt 0 0)", "1"),
    ("(expt 0.0 0)", "1"),
    ("(expt 0 0.0)", "1.0"),
    ("(expt 0 -1)", "+nan.0"),
    ("(expt 0.0 -1)", "+nan.0"),
    ("(expt 2.0 3)", "8.0"),
    ("(expt 2 3.0)", "8.0"),
    ("(expt 1/2 3)", "1/8"),
    ("(expt 4 1/2)", "2.0"),
    ("(expt 2 0.5)", "1.4142135623730951"),
    ("(expt 2 1.5)", "2.8284271247461903"),
    ("(expt -1 0.5)", "6.123233995736766e-17+1.0i"),
    ("(expt -8 1/3)", "1.0+1.732050807568877i"),
    ("(expt 1 1267650600228229401496703205376)", "1"),
    ("(expt -1 1267650600228229401496703205377)", "-1"),
    (
        "(expt 2 1267650600228229401496703205376)",
        "#<numerical-overflow>",
    ),
    ("(expt 2.0 1267650600228229401496703205376)", "+inf.0"),
    ("(expt 0.0+1.0i 2)", "-1.0+0.0i"),
    ("(sqrt 16)", "4"),
    ("(sqrt 15)", "3.872983346207417"),
    ("(sqrt 1/4)", "1/2"),
    ("(sqrt 2/9)", "0.4714045207910317"),
    ("(sqrt -4)", "0.0+2.0i"),
    ("(sqrt -4.0)", "0.0+2.0i"),
    ("(sqrt -1/4)", "0.0+0.5i"),
    ("(sqrt 1e-320)", "9.99994433575849e-161"),
    ("(sqrt 0.0+1.0i)", "0.7071067811865476+0.7071067811865475i"),
    ("(sqrt -1.0-1.0i)", "0.45508986056222733-1.09868411346781i"),
    (
        "(sqrt 100000000000000000000000000000000000000000)",
        "3.1622776601683794e20",
    ),
    (
        "(sqrt 1000000000000000000000000000000000000000000)",
        "1000000000000000000000",
    ),
    ("(sqrt -0.0)", "-0.0"),
    ("(exact-integer-sqrt 17)", "4 1"),
    (
        "(exact-integer-sqrt 100000000000000000000000000000000000000000)",
        "316227766016837933199 562477137586013626399",
    ),
    ("(exact-integer-sqrt -1)", "#<wrong-type-arg>"),
    ("(exact-integer-sqrt 4.0)", "#<wrong-type-arg>"),
    ("(exp 0)", "1.0"),
    ("(exp 1)", "2.718281828459045"),
    ("(exp -1/2)", "0.6065306597126334"),
    ("(exp 1.0+1.0i)", "1.4686939399158851+2.2873552871788423i"),
    ("(log 0)", "#<numerical-overflow>"),
    ("(log 0.0)", "-inf.0"),
    ("(log 1)", "0.0"),
    ("(log -1)", "0.0+3.141592653589793i"),
    ("(log 1/3)", "-1.0986122886681096"),
    ("(log -1/2)", "-0.6931471805599453+3.141592653589793i"),
    (
        "(log 100000000000000000000000000000000000000000000000000)",
        "115.12925464970228",
    ),
    ("(log 1.0+1.0i)", "0.34657359027997264+0.7853981633974483i"),
    ("(sin 0)", "0"),
    ("(sin 1)", "0.8414709848078965"),
    ("(cos 0)", "1"),
    ("(cos 1.0)", "0.5403023058681398"),
    ("(tan 0)", "0"),
    ("(tan 1/2)", "0.5463024898437905"),
    ("(asin 0)", "0"),
    ("(asin 1/2)", "0.5235987755982988"),
    ("(asin 2)", "1.5707963267948966-1.3169578969248166i"),
    ("(acos 1)", "0"),
    ("(acos 0)", "1.5707963267948966"),
    ("(acos 2)", "0.0+1.3169578969248166i"),
    ("(atan 0)", "0"),
    ("(atan 1)", "0.7853981633974483"),
    ("(atan 1 1)", "0.7853981633974483"),
    ("(atan -1 -1)", "-2.356194490192345"),
    ("(atan 0.0+2.0i)", "1.5707963267948966+0.5493061443340549i"),
    ("(sin 1.0+1.0i)", "1.2984575814159773+0.6349639147847361i"),
    ("(make-rectangular 1 0)", "1"),
    ("(make-rectangular 1 0.0)", "1.0+0.0i"),
    ("(make-rectangular 1/2 -3)", "0.5-3.0i"),
    ("(make-rectangular 1.0+1.0i 2)", "#<wrong-type-arg>"),
    ("(make-polar 1 0)", "1"),
    ("(make-polar 0 2)", "0"),
    (
        "(make-polar 2 3.141592653589793)",
        "-2.0+2.4492935982947064e-16i",
    ),
    (
        "(make-polar 1.0 1)",
        "0.5403023058681398+0.8414709848078965i",
    ),
    ("(real-part 3)", "3"),
    ("(real-part 1.5-2.5i)", "1.5"),
    ("(imag-part 1.5)", "0"),
    ("(imag-part 1.5-2.5i)", "-2.5"),
    ("(magnitude -3.0+4.0i)", "5.0"),
    ("(magnitude -5/3)", "5/3"),
    ("(angle -1)", "3.141592653589793"),
    ("(angle -1.0)", "3.141592653589793"),
    ("(angle 1)", "0.0"),
    ("(angle -0.0)", "3.141592653589793"),
    ("(angle 0.0+1.0i)", "1.5707963267948966"),
    ("(ash 1 100)", "1267650600228229401496703205376"),
    ("(ash -5 -1)", "-3"),
    ("(ash -1 -100)", "-1"),
    ("(ash 3 -1000)", "0"),
    ("(ash 1267650600228229401496703205376 -98)", "4"),
    ("(ash -1267650600228229401496703205377 -100)", "-2"),
    (
        "(ash 1 1267650600228229401496703205376)",
        "#<numerical-overflow>",
    ),
    ("(ash 0 1267650600228229401496703205376)", "0"),
    ("(ash 1.0 2)", "#<wrong-type-arg>"),
    (
        "(logand -1 1180591620717411303424)",
        "1180591620717411303424",
    ),
    (
        "(logand -8 1180591620717411303423)",
        "1180591620717411303416",
    ),
    ("(logior -8 18446744073709551616)", "-8"),
    ("(logxor -1 5)", "-6"),
    (
        "(logxor 1180591620717411303424 -1180591620717411303424)",
        "-2361183241434822606848",
    ),
    ("(lognot 18446744073709551616)", "-18446744073709551617"),
    ("(lognot -18446744073709551617)", "18446744073709551616"),
    ("(logtest 4 5)", "#t"),
    (
        "(logtest 1180591620717411303424 -1180591620717411303424)",
        "#f",
    ),
    ("(logbit? 3 8)", "#t"),
    ("(logbit? 100 -1)", "#t"),
    ("(logbit? 70 1180591620717411303424)", "#t"),
    ("(logbit? 71 -1180591620717411303424)", "#t"),
    ("(logcount -2)", "1"),
    ("(logcount 1267650600228229401496703205375)", "100"),
    ("(logcount -1267650600228229401496703205376)", "100"),
    ("(integer-length -1)", "0"),
    ("(integer-length 0)", "0"),
    ("(integer-length 255)", "8"),
    ("(integer-length -256)", "8"),
    ("(integer-length 18446744073709551616)", "65"),
    ("(number? 1/2)", "#t"),
    ("(complex? 1.0+2.0i)", "#t"),
    ("(real? 1.0+0.0i)", "#f"),
    ("(real? 1/2)", "#t"),
    ("(rational? +inf.0)", "#f"),
    ("(rational? 0.5)", "#t"),
    ("(integer? 2.0)", "#t"),
    ("(integer? 5/2)", "#f"),
    ("(integer? +inf.0)", "#f"),
    ("(exact-integer? 2.0)", "#f"),
    ("(exact-integer? 123456789012345678901234567890)", "#t"),
    ("(exact? 1/2)", "#t"),
    ("(exact? 0.5)", "#f"),
    ("(inexact? 1.0+1.0i)", "#t"),
    ("(nan? +nan.0)", "#t"),
    ("(nan? 1)", "#f"),
    ("(inf? -inf.0)", "#t"),
    ("(finite? 1/2)", "#t"),
    ("(finite? +nan.0)", "#f"),
    ("(zero? -0.0)", "#t"),
    ("(zero? 0/5)", "#t"),
    ("(zero? 0.0+0.0i)", "#t"),
    ("(positive? +nan.0)", "#f"),
    ("(positive? 1/2)", "#t"),
    ("(negative? -123456789012345678901234567890)", "#t"),
    ("(negative? -0.0)", "#f"),
    ("(odd? 3.0)", "#t"),
    ("(odd? 123456789012345678901234567891)", "#t"),
    ("(even? 0)", "#t"),
    ("(even? 1.5)", "#<wrong-type-arg>"),
    ("(eqv? 1 1.0)", "#f"),
    ("(eqv? 2.0 2.0)", "#t"),
    ("(eqv? 0.0 -0.0)", "#f"),
    ("(eqv? +nan.0 +nan.0)", "#t"),
    (
        "(eqv? 123456789012345678901234567890 123456789012345678901234567890)",
        "#t",
    ),
    ("(eqv? 1/2 1/2)", "#t"),
    ("(eqv? 1.0+2.0i 1.0+2.0i)", "#t"),
    ("(equal? 1/2 0.5)", "#f"),
    ("(+ 1 'a)", "#<wrong-type-arg>"),
    ("(< 'a 1)", "#<wrong-type-arg>"),
    ("(number->string 255 16)", r#""ff""#),
    ("(number->string -255 2)", r#""-11111111""#),
    ("(number->string 1/3 2)", r#""1/11""#),
    (
        "(number->string 0.1 2)",
        r#""1.100110011001100110011001100110011001100110011001101e-100""#,
    ),
    ("(number->string 1.5 16)", r#""1.8""#),
    ("(number->string 3.75 2)", r#""11.11""#),
    (
        "(number->string 0.1 3)",
        r#""0.0022002200220022002200220022002201""#,
    ),
    ("(number->string 1e21 16)", r#""3.635c9adc5deae11""#),
    ("(number->string -1.5 36)", r#""-1.i""#),
    (
        "(number->string 1e-10 2)",
        r#""1.1011011111001101111111011001110101111011110110111011e-100010""#,
    ),
    ("(number->string 1e100 7)", r#""1.6201341553122251064e226""#),
    ("(number->string 12345.678 16)", r#""3039.ad916872b""#),
    ("(number->string 1.0+2.5i 2)", r#""1.0+10.1i""#),
    (
        "(number->string 123456789012345678901234567890 36)",
        r#""byw97um9s91dlz68tsi""#,
    ),
    ("(number->string 10 1)", "#<out-of-range>"),
    ("(number->string 10 37)", "#<out-of-range>"),
    ("(number->string 'a 1)", "#<out-of-range>"),
    (
        "(ash -2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376 -1099511627776)",
        "-1",
    ),
];

/// Texts `string->number` reads, and what Guile 3.0.11 writes for the result.
const READ: &[(&str, &str)] = &[
    ("1e2", "100.0"),
    ("1E2", "100.0"),
    ("1d2", "100.0"),
    ("1s2", "100.0"),
    ("1f2", "100.0"),
    ("1l2", "100.0"),
    ("#e1.5", "3/2"),
    ("#i3/4", "0.75"),
    ("#x-ff", "-255"),
    ("#xFF", "255"),
    ("#b101", "5"),
    ("#o17", "15"),
    ("#e#x10", "16"),
    ("#x#e10", "16"),
    ("#e1e400", "#<out-of-range>"),
    ("1e400", "#<out-of-range>"),
    ("1e308", "1.0e308"),
    ("1e309", "#<out-of-range>"),
    ("1e-400", "#<out-of-range>"),
    ("-1e-400", "#<out-of-range>"),
    ("1e-324", "0.0"),
    ("1e-330", "#<out-of-range>"),
    ("12#", "120.0"),
    ("12#.5", "#f"),
    ("1#.#", "10.0"),
    ("1.#", "1.0"),
    (".5", "0.5"),
    ("-.5", "-0.5"),
    ("+.5", "0.5"),
    ("5.", "5.0"),
    (".", "#f"),
    ("+", "#f"),
    ("-", "#f"),
    ("+i", "0.0+1.0i"),
    ("-i", "0.0-1.0i"),
    ("+I", "0.0+1.0i"),
    ("1+i", "1.0+1.0i"),
    ("1-2i", "1.0-2.0i"),
    ("1@0", "1"),
    ("1@2", "-0.4161468365471424+0.9092974268256817i"),
    ("-1@-2", "0.4161468365471424+0.9092974268256817i"),
    ("1/2+3/4i", "0.5+0.75i"),
    ("#e1.5+2.5i", "1.5+2.5i"),
    ("#i1/2", "0.5"),
    ("+inf.0", "+inf.0"),
    ("-inf.0", "-inf.0"),
    ("+nan.0", "+nan.0"),
    ("-nan.0", "+nan.0"),
    ("inf.0", "#f"),
    ("+inf.0i", "0.0+inf.0i"),
    ("1+inf.0i", "1.0+inf.0i"),
    ("+nan.1", "#f"),
    ("+ian.0", "+nan.0"),
    ("1/0", "#f"),
    ("0/5", "0"),
    ("1/2e2", "#f"),
    ("1e2.5", "#f"),
    ("#x1.5", "#f"),
    ("#xa/b", "10/11"),
    ("1e", "#f"),
    ("1e+", "#f"),
    ("e1", "#f"),
    ("1/", "#f"),
    ("/2", "#f"),
    ("12abc", "#f"),
    ("#e+inf.0", "#f"),
    ("#i#x10", "16.0"),
    ("#x#x10", "#f"),
    ("-0", "0"),
    ("-0.0", "-0.0"),
    ("0.0e5", "0.0"),
    (" 1", "#f"),
    ("", "#f"),
    ("#", "#f"),
    ("#e", "#f"),
    ("#e1", "1"),
    ("1.5e3", "1500.0"),
    (
        "123456789012345678901234567890",
        "123456789012345678901234567890",
    ),
    (
        "-123456789012345678901234567890/3",
        "-41152263004115226300411522630",
    ),
    ("0.1e1", "1.0"),
    ("1e+5", "100000.0"),
    ("1e-5", "1.0e-5"),
    ("#d10", "10"),
    ("#b2", "#f"),
    ("1i", "#f"),
    ("i", "#f"),
    ("1+2", "#f"),
    ("1@", "#f"),
    ("@1", "#f"),
    ("+1/2i", "0.0+0.5i"),
    ("#xabcdef0123456789abcdef", "207698809136909011942886895"),
    ("1.000000000000000000000000000001", "1.0"),
    ("0.30000000000000004", "0.30000000000000004"),
    ("2.2250738585072011e-308", "2.225073858507201e-308"),
    ("4.9e-324", "5.0e-324"),
    ("2.4703282292062328e-324", "5.0e-324"),
    ("2.4703282292062327e-324", "0.0"),
    ("9007199254740993", "9007199254740993"),
    ("9007199254740993.0", "9007199254740992.0"),
    ("1e23", "1.0e23"),
    ("8.98846567431158e307", "8.98846567431158e307"),
    ("#i1e400", "#<out-of-range>"),
    ("1.e2", "100.0"),
    ("#x1e2", "482"),
    ("#b1e2", "#f"),
    ("+1@+1", "0.5403023058681398+0.8414709848078965i"),
    ("1@+i", "#f"),
    ("1+i+i", "#f"),
    ("+inf.0+nan.0i", "+inf.0+nan.0i"),
    ("-nan.0i", "0.0+nan.0i"),
    ("#e1.2e-3", "3/2500"),
    ("1/2#", "0.05"),
    ("1#/2", "5.0"),
    ("#e1#", "10"),
    ("-1-i", "-1.0-1.0i"),
    ("1-i", "1.0-1.0i"),
    ("-i+1", "#f"),
    ("+5", "5"),
    ("-1.5e3", "-1500.0"),
    ("...", "#f"),
    ("+a", "#f"),
    ("1e21", "1.0e21"),
    ("1e20", "1.0e20"),
    ("123456789.0", "123456789.0"),
    ("1234567.0", "1234567.0"),
    ("12345678.0", "12345678.0"),
    ("10000000.0", "1.0e7"),
    ("100000000.0", "1.0e8"),
    ("1e7", "1.0e7"),
    ("0.001", "0.001"),
    ("0.0001", "1.0e-4"),
    ("1.5e-7", "1.5e-7"),
    ("5e-324", "5.0e-324"),
    ("1.7976931348623157e308", "1.7976931348623157e308"),
    ("-0.0", "-0.0"),
    ("100.0", "100.0"),
    ("1e10", "1.0e10"),
    ("1.23e10", "1.23e10"),
    ("0.1", "0.1"),
    ("0.3333333333333333", "0.3333333333333333"),
    ("2e-3", "0.002"),
    ("2.5e-4", "2.5e-4"),
    ("4.35", "4.35"),
    ("1e22", "1.0e22"),
    ("9007199254740992.0", "9007199254740992.0"),
    ("2.2250738585072014e-308", "2.2250738585072014e-308"),
    ("2.225073858507201e-308", "2.225073858507201e-308"),
    ("4.9406564584124654e-324", "5.0e-324"),
    ("1e23", "1.0e23"),
    ("0.30000000000000004", "0.30000000000000004"),
    ("123.456", "123.456"),
    ("-987654.321", "-987654.321"),
    ("1234567.5", "1234567.5"),
    ("12345678.5", "12345678.5"),
    ("0.000123", "1.23e-4"),
    ("1e-3", "0.001"),
    ("9.999999999999999e22", "1.0e23"),
    ("3.141592653589793", "3.141592653589793"),
    ("2.718281828459045", "2.718281828459045"),
    ("1e-5", "1.0e-5"),
    ("6.02214076e23", "6.02214076e23"),
    ("1.602176634e-19", "1.602176634e-19"),
    ("65536.0", "65536.0"),
    ("0.5", "0.5"),
    ("0.25", "0.25"),
    ("0.125", "0.125"),
    ("1e300", "1.0e300"),
    ("1e-300", "1.0e-300"),
    ("1.5", "1.5"),
    ("-2.5", "-2.5"),
    ("7e7", "7.0e7"),
    ("7.5e7", "7.5e7"),
    ("1e6", "1000000.0"),
    ("999999.0", "999999.0"),
    ("9999999.0", "9999999.0"),
    ("99999999.0", "99999999.0"),
];

/// The arguments of a case: numbers, `'symbols` and `"strings"`.
fn tokens(expr: &str) -> Vec<String> {
    let inner = &expr[1..expr.len() - 1];
    let mut tokens = Vec::new();
    let mut chars = inner.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c == ' ' {
            chars.next();
        } else if c == '"' {
            let mut token = String::from(chars.next().unwrap());
            for c in chars.by_ref() {
                token.push(c);
                if c == '"' {
                    break;
                }
            }
            tokens.push(token);
        } else {
            let mut token = String::new();
            while let Some(&c) = chars.peek().filter(|&&c| c != ' ') {
                token.push(c);
                chars.next();
            }
            tokens.push(token);
        }
    }
    tokens
}

fn key(error: NumError) -> &'static str {
    match error {
        NumError::WrongType(_) => "#<wrong-type-arg>",
        NumError::OutOfRange(_) | NumError::Exponent { .. } => "#<out-of-range>",
        NumError::Overflow => "#<numerical-overflow>",
    }
}

/// Applies a case's procedure as the VM will: the heap's primitive, and its
/// values or its error.
fn apply(heap: &mut Heap, op: &str, args: &[Value]) -> Result<Vec<Value>, NumError> {
    let one = |r: Result<Value, NumError>| r.map(|v| vec![v]);
    let truth = |r: Result<bool, NumError>| r.map(|b| vec![Value::bool(b)]);
    let two = |r: Result<(Value, Value), NumError>| r.map(|(q, r)| vec![q, r]);
    let radix = |heap: &Heap, v: Value| heap.to_i64(v).unwrap() as u32;
    match (op, args) {
        ("+", &[x, y]) => one(heap.add(x, y)),
        ("-", &[x, y]) => one(heap.sub(x, y)),
        ("-", &[x]) => one(heap.negate(x)),
        ("*", &[x, y]) => one(heap.mul(x, y)),
        ("/", &[x, y]) => one(heap.div(x, y)),
        ("/", &[x]) => one(heap.reciprocal(x)),
        ("1+", &[x]) => one(heap.one_plus(x)),
        ("1-", &[x]) => one(heap.one_minus(x)),
        ("abs", &[x]) => one(heap.abs(x)),
        ("=", &[x, y]) => truth(heap.num_eq(x, y)),
        ("<", &[x, y]) => truth(heap.less(x, y)),
        (">", &[x, y]) => truth(heap.greater(x, y)),
        ("<=", &[x, y]) => truth(heap.less_eq(x, y)),
        (">=", &[x, y]) => truth(heap.greater_eq(x, y)),
        ("max", &[x, y]) => one(heap.max(x, y)),
        ("min", &[x, y]) => one(heap.min(x, y)),
        ("quotient", &[x, y]) => one(heap.quotient(x, y)),
        ("remainder", &[x, y]) => one(heap.remainder(x, y)),
        ("modulo", &[x, y]) => one(heap.modulo(x, y)),
        ("floor/", &[x, y]) => two(heap.divide(Rounding::Floor, x, y)),
        ("ceiling/", &[x, y]) => two(heap.divide(Rounding::Ceiling, x, y)),
        ("truncate/", &[x, y]) => two(heap.divide(Rounding::Truncate, x, y)),
        ("round/", &[x, y]) => two(heap.divide(Rounding::Round, x, y)),
        ("euclidean/", &[x, y]) => two(heap.divide(Rounding::Euclidean, x, y)),
        ("centered/", &[x, y]) => two(heap.divide(Rounding::Centered, x, y)),
        ("floor", &[x]) => one(heap.round(Rounding::Floor, x)),
        ("ceiling", &[x]) => one(heap.round(Rounding::Ceiling, x)),
        ("truncate", &[x]) => one(heap.round(Rounding::Truncate, x)),
        ("round", &[x]) => one(heap.round(Rounding::Round, x)),
        ("gcd", &[x, y]) => one(heap.gcd(x, y)),
        ("lcm", &[x, y]) => one(heap.lcm(x, y)),
        ("numerator", &[x]) => one(heap.numerator(x)),
        ("denominator", &[x]) => one(heap.denominator(x)),
        ("exact->inexact", &[x]) => one(heap.inexact(x)),
        ("inexact->exact", &[x]) => one(heap.exact(x)),
        ("expt", &[x, y]) => one(heap.expt(x, y)),
        ("sqrt", &[x]) => one(heap.sqrt(x)),
        ("exact-integer-sqrt", &[x]) => two(heap.exact_integer_sqrt(x)),
        ("exp", &[x]) => one(heap.exp(x)),
        ("log", &[x]) => one(heap.log(x)),
        ("sin", &[x]) => one(heap.sin(x)),
        ("cos", &[x]) => one(heap.cos(x)),
        ("tan", &[x]) => one(heap.tan(x)),
        ("asin", &[x]) => one(heap.asin(x)),
        ("acos", &[x]) => one(heap.acos(x)),
        ("atan", &[x]) => one(heap.atan(x)),
        ("atan", &[y, x]) => one(heap.atan2(y, x)),
        ("make-rectangular", &[x, y]) => one(heap.make_rectangular(x, y)),
        ("make-polar", &[x, y]) => one(heap.make_polar(x, y)),
        ("real-part", &[x]) => one(heap.real_part(x)),
        ("imag-part", &[x]) => one(heap.imag_part(x)),
        ("magnitude", &[x]) => one(heap.magnitude(x)),
        ("angle", &[x]) => one(heap.angle(x)),
        ("ash", &[x, y]) => one(heap.ash(x, y)),
        ("logand", &[x, y]) => one(heap.logand(x, y)),
        ("logior", &[x, y]) => one(heap.logior(x, y)),
        ("logxor", &[x, y]) => one(heap.logxor(x, y)),
        ("lognot", &[x]) => one(heap.lognot(x)),
        ("logtest", &[x, y]) => truth(heap.logtest(x, y)),
        ("logbit?", &[x, y]) => truth(heap.logbit(x, y)),
        ("logcount", &[x]) => one(heap.logcount(x)),
        ("integer-length", &[x]) => one(heap.integer_length(x)),
        ("number?", &[x]) | ("complex?", &[x]) => Ok(vec![Value::bool(heap.is_number(x))]),
        ("real?", &[x]) => Ok(vec![Value::bool(heap.is_real(x))]),
        ("rational?", &[x]) => Ok(vec![Value::bool(heap.is_rational(x))]),
        ("integer?", &[x]) => Ok(vec![Value::bool(heap.is_integer(x))]),
        ("exact-integer?", &[x]) => Ok(vec![Value::bool(heap.is_exact_integer(x))]),
        ("exact?", &[x]) => truth(heap.is_exact(x)),
        ("inexact?", &[x]) => truth(heap.is_inexact(x)),
        ("nan?", &[x]) => truth(heap.is_nan(x)),
        ("inf?", &[x]) => truth(heap.is_inf(x)),
        ("finite?", &[x]) => truth(heap.is_finite(x)),
        ("zero?", &[x]) => truth(heap.is_zero(x)),
        ("positive?", &[x]) => truth(heap.is_positive(x)),
        ("negative?", &[x]) => truth(heap.is_negative(x)),
        ("odd?", &[x]) => truth(heap.is_odd(x)),
        ("even?", &[x]) => truth(heap.is_even(x)),
        ("eqv?", &[x, y]) => Ok(vec![Value::bool(heap.eqv(x, y))]),
        ("equal?", &[x, y]) => Ok(vec![Value::bool(heap.equal(x, y))]),
        ("number->string", &[x, r]) => {
            let r = radix(heap, r);
            heap.number_to_string(x, r).map(|s| vec![heap.string(s)])
        }
        _ => panic!("no procedure {op} of {} arguments in the test", args.len()),
    }
}

/// What the driver prints for a case: its values written and joined by
/// spaces, or `#<key>` for an error.
fn run(heap: &mut Heap, expr: &str) -> String {
    let tokens = tokens(expr);
    let args: Vec<Value> = tokens[1..]
        .iter()
        .map(|token| match token.strip_prefix('\'') {
            Some(name) => heap.intern(name),
            None => match heap.string_to_number(token, 10) {
                Ok(v) if v != Value::FALSE => v,
                _ => panic!("{token} in {expr} is not a number"),
            },
        })
        .collect();
    match apply(heap, &tokens[0], &args) {
        Ok(values) => {
            let printed: Vec<String> = values
                .iter()
                .map(|&v| {
                    let mut out = String::new();
                    heap.write(v, &mut out);
                    out
                })
                .collect();
            printed.join(" ")
        }
        Err(error) => key(error).into(),
    }
}

fn read(heap: &mut Heap, text: &str) -> String {
    match heap.string_to_number(text, 10) {
        Ok(v) => {
            let mut out = String::new();
            heap.write(v, &mut out);
            out
        }
        Err(error) => key(error).into(),
    }
}

/// The lines guile prints for the cases and the texts, if it is installed.
fn guile(cases: &[&str], texts: &[&str]) -> Option<Vec<String>> {
    let mut program = String::from(
        r##"(define (show thunk)
  (catch #t
    (lambda () (call-with-values thunk (lambda vals (display (string-join (map (lambda (v) (call-with-output-string (lambda (p) (write v p)))) vals) " ")))))
    (lambda (key . args) (display "#<") (display key) (display ">")))
  (newline))
"##,
    );
    // Each case is read and given to `eval`, as the tables were made: what
    // guile's compiler inlines can throw other keys (`ash`).
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    for case in cases {
        program.push_str(&format!(
            "(show (lambda () (eval (read (open-input-string \"{}\")) (interaction-environment))))\n",
            quote(case)
        ));
    }
    for text in texts {
        program.push_str(&format!(
            "(show (lambda () (string->number \"{}\")))\n",
            quote(text)
        ));
    }
    let out = Command::new("guile")
        .args(["--no-auto-compile", "-c", &program])
        .output()
        .ok()?;
    assert!(
        out.status.success(),
        "guile failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(String::from)
            .collect(),
    )
}

/// The procedures whose last bits come from a libm: Guile's is the
/// platform's, and the engine's is the libm crate's (musl's algorithms).
const LIBM: [&str; 12] = [
    "exp",
    "log",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "expt",
    "sqrt",
    "make-polar",
    "angle",
];

/// The doubles in a printed number, in order.
fn doubles(text: &str) -> Vec<f64> {
    match parse_number(text, 10) {
        Ok(Some(Num::Real(x))) => vec![x],
        Ok(Some(Num::Complex(re, im))) => vec![re, im],
        _ => Vec::new(),
    }
}

/// Whether two printed numbers are doubles at most `ulps` apart, part by
/// part.
fn close(a: &str, b: &str, ulps: u64) -> bool {
    let (a, b) = (doubles(a), doubles(b));
    let order = |x: f64| {
        let bits = x.to_bits() as i64;
        if bits < 0 { i64::MIN - bits } else { bits }
    };
    !a.is_empty()
        && a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(&x, &y)| order(x).abs_diff(order(y)) <= ulps)
}

#[test]
fn primitives_compute_as_guile_does() {
    let mut heap = Heap::new();
    let (mut failures, mut approximate) = (Vec::new(), Vec::new());
    for &(expr, expected) in CASES {
        let got = run(&mut heap, expr);
        if got == expected {
            continue;
        }
        let op = &tokens(expr)[0];
        if LIBM.contains(&op.as_str()) && close(&got, expected, 4) {
            approximate.push(format!("{expr}: {got}, Guile {expected}"));
        } else {
            failures.push(format!("{expr}: {got}, Guile {expected}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // What the libm crate rounds otherwise than macOS's libm, within 4 ulps.
    assert!(approximate.len() <= 7, "{}", approximate.join("\n"));
    eprintln!("within 4 ulps of Guile's libm:\n{}", approximate.join("\n"));
}

#[test]
fn string_to_number_reads_as_guile_does() {
    let mut heap = Heap::new();
    let mut failures = Vec::new();
    for &(text, expected) in READ {
        let got = read(&mut heap, text);
        if got != expected {
            failures.push(format!("{text:?}: {got}, Guile {expected}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_tables_are_what_guile_prints() {
    let cases: Vec<&str> = CASES.iter().map(|&(expr, _)| expr).collect();
    let texts: Vec<&str> = READ.iter().map(|&(text, _)| text).collect();
    let Some(lines) = guile(&cases, &texts) else {
        eprintln!("guile is not installed: the tables were not compared with it");
        return;
    };
    let expected = CASES.iter().chain(READ).map(|&(_, printed)| printed);
    let inputs = cases.iter().chain(&texts);
    assert_eq!(lines.len(), CASES.len() + READ.len());
    for ((line, expected), input) in lines.iter().zip(expected).zip(inputs) {
        assert_eq!(line, expected, "guile on {input}");
    }
}

// --- bignums ---------------------------------------------------------------------

/// A deterministic stream of bits.
struct Bits(u64);

impl Bits {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// An i128 of a random width, so that small and large ones both come.
    fn i128(&mut self) -> i128 {
        let width = self.next() % 127;
        let n = ((self.next() as u128) << 64 | self.next() as u128) >> (127 - width);
        if self.next() & 1 == 1 {
            -(n as i128)
        } else {
            n as i128
        }
    }

    /// A positive integer of `limbs` random 32-bit limbs.
    fn big(&mut self, limbs: usize) -> BigInt {
        (0..limbs).fold(BigInt::zero(), |n, _| {
            n.mul_add_small(1 << 16, 0)
                .mul_add_small(1 << 16, self.next() as u32 | 1)
        })
    }
}

fn big(n: i128) -> BigInt {
    BigInt::from_i128(n)
}

#[test]
fn bignums_agree_with_i128() {
    let mut bits = Bits(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let (a, b) = (bits.i128() >> 1, bits.i128() >> 1);
        let (x, y) = (big(a), big(b));
        assert_eq!(x.add(&y), big(a + b), "{a} + {b}");
        assert_eq!(x.sub(&y), big(a - b), "{a} - {b}");
        let (s, t) = (a >> 64, b >> 63);
        assert_eq!(big(s).mul(&big(t)), big(s * t), "{s} × {t}");
        if b != 0 {
            assert_eq!(x.div_rem(&y), (big(a / b), big(a % b)), "{a} / {b}");
            let (q, r) = (a.div_euclid(b), a.rem_euclid(b));
            let (q, r) = if r != 0 && b < 0 {
                (q - 1, r + b)
            } else {
                (q, r)
            };
            assert_eq!(x.div_rem_floor(&y), (big(q), big(r)), "floor {a} / {b}");
        }
        let (mut g, mut h) = (a.unsigned_abs(), b.unsigned_abs());
        while h != 0 {
            (g, h) = (h, g % h);
        }
        assert_eq!(x.gcd(&y), big(g as i128), "gcd {a} {b}");
        assert_eq!(x.and(&y), big(a & b), "{a} and {b}");
        assert_eq!(x.or(&y), big(a | b), "{a} or {b}");
        assert_eq!(x.xor(&y), big(a ^ b), "{a} xor {b}");
        assert_eq!(x.not(), big(!a), "not {a}");
        let shift = (bits.next() % 70) as usize;
        assert_eq!(big(a >> 2).shl(shift).shr_floor(shift as u64), big(a >> 2));
        assert_eq!(x.shr_floor(shift as u64), big(a >> shift), "{a} >> {shift}");
        let width = 128 - if a < 0 { !a } else { a }.leading_zeros();
        assert_eq!(x.bit_length(), width as u64, "integer-length {a}");
        let ones = if a < 0 {
            (!a).count_ones()
        } else {
            a.count_ones()
        };
        assert_eq!(x.count_ones(), ones as u64, "logcount {a}");
        assert_eq!(
            x.bit(shift as u64),
            a >> shift & 1 == 1,
            "bit {shift} of {a}"
        );
        assert_eq!(x.to_f64(), a as f64, "{a} as a double");
        assert_eq!(x.to_string_radix(10), a.to_string());
        let sign = if a < 0 { "-" } else { "" };
        let m = a.unsigned_abs();
        assert_eq!(x.to_string_radix(16), format!("{sign}{m:x}"));
        assert_eq!(x.to_string_radix(2), format!("{sign}{m:b}"));
        assert_eq!(x.to_i64(), i64::try_from(a).ok());
        let (root, rest) = big(a.abs() >> 1).isqrt();
        let root = root.to_string_radix(10).parse::<u128>().unwrap();
        let n = a.unsigned_abs() >> 1;
        assert!(root * root <= n && (root + 1) * (root + 1) > n, "isqrt {n}");
        assert_eq!(rest, big((n - root * root) as i128));
    }
}

#[test]
fn bignums_beyond_128_bits() {
    let mut bits = Bits(42);
    let mut heap = Heap::new();
    for round in 0..300 {
        let (a, b) = (bits.big(2 + round % 12), bits.big(1 + round % 7));
        let a = if round % 3 == 0 { a.neg() } else { a };
        let (q, r) = a.div_rem(&b);
        assert_eq!(q.mul(&b).add(&r), a);
        assert!(r.abs() < b && (r.is_zero() || r.is_negative() == a.is_negative()));
        assert_eq!(a.mul(&b).div_rem(&b), (a.clone(), BigInt::zero()));
        let (fq, fr) = a.div_rem_floor(&b.neg());
        assert_eq!(fq.mul(&b.neg()).add(&fr), a);
        assert!(fr.is_zero() || fr.is_negative());
        assert_eq!(a.xor(&b).xor(&b), a);
        assert!(a.and(&a.not()).is_zero());
        assert_eq!(a.shl(77).shr_floor(77), a);
        let (s, rest) = a.abs().isqrt();
        assert_eq!(s.mul(&s).add(&rest), a.abs());
        assert!(rest <= s.shl(1));
        // The digits read back, and the double is the one Rust reads.
        for radix in [2, 7, 10, 16, 36] {
            let text = a.to_string_radix(radix);
            let read = heap.string_to_number(&text, radix).unwrap();
            assert_eq!(heap.num(read), Some(Num::Int(a.clone())), "{text}");
        }
        let decimal = a.to_string_radix(10);
        assert_eq!(a.to_f64(), decimal.parse::<f64>().unwrap(), "{decimal}");
    }
}

// --- flonums -----------------------------------------------------------------------

/// The significant digits and the decimal exponent of a printed double.
fn digits(text: &str) -> (String, i64) {
    let text = text.trim_start_matches('-');
    let (mantissa, exponent) = match text.split_once('e') {
        Some((m, e)) => (m, e.parse::<i64>().unwrap()),
        None => (text, 0),
    };
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i64;
    let all: String = mantissa.chars().filter(|&c| c != '.').collect();
    let lead = all.len() - all.trim_start_matches('0').len();
    let significant = all.trim_matches('0').to_string();
    (significant, exponent + point - lead as i64)
}

#[test]
fn flonums_print_the_shortest_digits_and_read_back() {
    let mut bits = Bits(7);
    let mut heap = Heap::new();
    for i in 0..30_000 {
        let x = match i % 3 {
            0 => f64::from_bits(bits.next()),
            // Near-integers and small numbers, where the notation changes.
            1 => (bits.next() % 1_000_000_000) as f64 / 1e3,
            _ => f64::from_bits(bits.next() >> 2),
        };
        if !x.is_finite() {
            continue;
        }
        let value = heap.flonum(x);
        let printed = heap.number_to_string(value, 10).unwrap();
        let read = heap.string_to_number(&printed, 10).unwrap();
        assert_eq!(
            heap.as_flonum(read).map(f64::to_bits),
            Some(x.to_bits()),
            "{x:e} printed {printed}"
        );
        // Both are the shortest digits that read back, but where two are as
        // near, Burger and Dybvig's last digit is even and Rust's is not
        // always.
        let (ours, rust) = (digits(&printed), digits(&format!("{x:e}")));
        if ours != rust {
            let last = ours.0.bytes().last().unwrap();
            assert!(
                ours.0.len() == rust.0.len() && ours.1 == rust.1 && last % 2 == 0,
                "{x:e} printed {printed}"
            );
        }
    }
}

// --- the heap ------------------------------------------------------------------------

#[test]
fn numbers_are_kept_in_their_smallest_form() {
    let mut heap = Heap::new();
    let n = heap.number(Num::ratio(BigInt::from_i64(6), BigInt::from_i64(-4)));
    assert_eq!(
        heap.as_ratio(n)
            .map(|(a, b)| (a.as_fixnum(), b.as_fixnum())),
        Some((Some(-3), Some(2)))
    );
    let two = heap.number(Num::ratio(BigInt::from_i64(6), BigInt::from_i64(3)));
    assert_eq!(two.as_fixnum(), Some(2));
    let max = heap.integer(Value::FIXNUM_MAX);
    assert_eq!(max.tag(), Tag::Fixnum);
    let big = heap.one_plus(max).unwrap();
    assert_eq!(big.tag(), Tag::Bignum);
    let back = heap.one_minus(big).unwrap();
    assert_eq!(back, max, "a bignum that fits again is a fixnum");
    let min = heap.integer(i64::MIN);
    assert_eq!((min.tag(), heap.to_i64(min)), (Tag::Bignum, Some(i64::MIN)));
    let half = heap.string_to_number("1/2", 10).unwrap();
    assert_eq!(heap.to_f64(half), Some(0.5));
    assert_eq!(heap.to_f64(Value::fixnum(3).unwrap()), Some(3.0));
    assert_eq!(heap.to_f64(Value::NULL), None);
    let i = heap.string_to_number("+i", 10).unwrap();
    assert_eq!(
        (heap.to_f64(i), heap.as_complex(i)),
        (None, Some((0.0, 1.0)))
    );
}

#[test]
fn numbers_survive_collection_only_when_reachable() {
    let mut heap = Heap::new();
    let big = heap
        .string_to_number("123456789012345678901234567891", 10)
        .unwrap();
    let ratio = heap.div(big, Value::fixnum(2).unwrap()).unwrap();
    assert_eq!(ratio.tag(), Tag::Ratio);
    let flonum = heap.flonum(1.5);
    let complex = heap.complex(1.0, 2.0);
    let depth = heap.root_depth();
    heap.root(ratio);
    let before = heap.objects();
    heap.collect(&[]);
    // The ratio and its bignum numerator stay; `big`, the flonum and the
    // complex number go.
    assert_eq!(heap.stats().freed, 3, "{before} objects before");
    let mut out = String::new();
    heap.write(ratio, &mut out);
    assert_eq!(out, "123456789012345678901234567891/2");
    let _ = (flonum, complex);
    heap.unroot_to(depth);
    heap.collect(&[]);
    assert_eq!(heap.objects(), 0);
}

#[test]
fn the_printer_writes_numbers_inside_data() {
    let mut heap = Heap::new();
    let items: Vec<Value> = [
        "1/2",
        "-2.5",
        "+i",
        "123456789012345678901234567890",
        "1e21",
        "-0.0",
    ]
    .iter()
    .map(|text| heap.string_to_number(text, 10).unwrap())
    .collect();
    let list = heap.list(&items);
    let vector = heap.vector(items);
    let pair = heap.cons(list, vector);
    let (mut written, mut displayed) = (String::new(), String::new());
    heap.write(pair, &mut written);
    heap.display(pair, &mut displayed);
    let expected = "((1/2 -2.5 0.0+1.0i 123456789012345678901234567890 1.0e21 -0.0) . #(1/2 -2.5 0.0+1.0i 123456789012345678901234567890 1.0e21 -0.0))";
    assert_eq!((written.as_str(), displayed.as_str()), (expected, expected));
}

#[test]
fn an_exponent_out_of_range_names_its_text() {
    let text = "#e1.5e-400";
    match parse_number(text, 10) {
        Err(NumError::Exponent { start, end }) => assert_eq!(&text[start..end], "-400"),
        other => panic!("{other:?}"),
    }
    let text = "λ+1e999";
    assert!(
        matches!(parse_number(text, 10), Ok(None)),
        "not a number before the exponent"
    );
    let text = "1+1e999i";
    match parse_number(text, 10) {
        Err(NumError::Exponent { start, end }) => assert_eq!(&text[start..end], "999"),
        other => panic!("{other:?}"),
    }
    let mut heap = Heap::new();
    assert_eq!(heap.string_to_number("10", 1), Err(NumError::OutOfRange(2)));
    assert_eq!(
        heap.string_to_number("z", 36).map(|v| v.as_fixnum()),
        Ok(Some(35))
    );
}

#[test]
fn eqv_is_equal_on_numbers() {
    let mut heap = Heap::new();
    let mut read = |text: &str| heap.string_to_number(text, 10).unwrap();
    let pairs = [
        (
            "123456789012345678901234567890",
            "123456789012345678901234567890",
            true,
        ),
        ("1/3", "2/6", true),
        ("1/3", "1/4", false),
        ("1.5", "1.5", true),
        ("+nan.0", "-nan.0", true),
        ("0.0", "-0.0", false),
        ("1+2i", "1.0+2.0i", true),
        ("1", "1.0", false),
    ];
    let pairs: Vec<(Value, Value, bool)> = pairs
        .iter()
        .map(|&(a, b, same)| (read(a), read(b), same))
        .collect();
    for (a, b, same) in pairs {
        assert_eq!(heap.eqv(a, b), same, "{a:?} {b:?}");
        assert_eq!(heap.equal(a, b), same);
        let vectors = (heap.vector(vec![a]), heap.vector(vec![b]));
        assert_eq!(heap.equal(vectors.0, vectors.1), same);
    }
}
