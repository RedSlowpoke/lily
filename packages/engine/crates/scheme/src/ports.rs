//! Ports (chapter 03 §2.6): string ports for input and output, the
//! standard output and error ports, which the host takes
//! ([`crate::Vm::take_output`], [`crate::Vm::take_errors`]), and the
//! procedures that read and write characters and data on them.
//!
//! The current output, error and input ports are fluids that `display` and
//! the others read when no port is given ([`Cx::output`]); Scheme sees them
//! as parameters, `current-output-port` and the rest ([`PRELUDE`]), so
//! `with-output-to-string` and `parameterize` rebind them. A file Scheme
//! writes (`with-output-to-file`, `call-with-output-file`) is kept for the
//! host by its name, in `%written-files` (MS2 step 26).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::control::Fluid;
use crate::heap::{Foreign, Heap};
use crate::image::{ImageReader, ImageWriter};
use crate::reader::{Embedded, LilyScan, read};
use crate::value::Value;
use crate::vm::{Cx, Source, Thrown, read_exception};

type R = Result<Value, Thrown>;

pub(crate) enum Kind {
    /// What the host takes as output.
    Stdout,
    /// What the host takes as errors.
    Stderr,
    Output(String),
    /// The text and the byte to read next.
    Input(String, usize),
    /// A bytevector output port's bytes (`library/ports.rs`).
    Bytes(Vec<u8>),
}

pub(crate) struct Port {
    pub(crate) kind: Kind,
    pub(crate) open: bool,
}

impl Foreign for Port {
    fn type_name(&self) -> &'static str {
        "port"
    }

    /// `#<output: string>`, without the address Guile prints.
    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str(match (&self.kind, self.open) {
            (_, false) => "#<closed: string>",
            (Kind::Stdout, _) => "#<output: file /dev/stdout>",
            (Kind::Stderr, _) => "#<output: file /dev/stderr>",
            (Kind::Output(_), _) => "#<output: string>",
            (Kind::Bytes(_), _) => "#<output: file>",
            (Kind::Input(..), _) => "#<input: string>",
        });
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.bool(self.open);
        match &self.kind {
            Kind::Stdout => out.u8(0),
            Kind::Stderr => out.u8(1),
            Kind::Output(text) => {
                out.u8(2);
                out.str(text);
            }
            Kind::Input(text, at) => {
                out.u8(3);
                out.str(text);
                out.count(*at);
            }
            Kind::Bytes(bytes) => {
                out.u8(4);
                out.bytes(bytes);
            }
        }
        true
    }
}

pub(crate) fn load_port(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let open = r.bool()?;
    let kind = match r.u8()? {
        0 => Kind::Stdout,
        1 => Kind::Stderr,
        2 => Kind::Output(r.str()?.into()),
        3 => Kind::Input(r.str()?.into(), r.u32()? as usize),
        4 => Kind::Bytes(r.bytes()?.into()),
        _ => return None,
    };
    Some(Box::new(Port { kind, open }))
}

impl Port {
    fn is_output(&self) -> bool {
        !matches!(self.kind, Kind::Input(..))
    }
}

/// The ports a VM starts with: standard output, standard error and an empty
/// input, each in a fluid.
pub(crate) fn standard(heap: &mut Heap) -> [Value; 3] {
    [Kind::Stdout, Kind::Stderr, Kind::Input(String::new(), 0)].map(|kind| {
        let port = heap.wrap(Port { kind, open: true });
        heap.wrap(Fluid { value: port })
    })
}

fn fluid_value(heap: &Heap, fluid: Value) -> Value {
    heap.foreign::<Fluid>(fluid)
        .map_or(Value::FALSE, |f| f.value)
}

/// The output port `given`, or the current one: an open output port, or
/// `subr`'s error about argument `position`.
pub(crate) fn output(cx: &mut Cx, subr: &str, position: usize, given: Option<Value>) -> R {
    let port = given.unwrap_or_else(|| fluid_value(&cx.heap, cx.output));
    match cx.heap.foreign::<Port>(port) {
        Some(p) if p.open && p.is_output() => Ok(port),
        _ => Err(cx.wrong_type_at(subr, position, port)),
    }
}

/// The current error port.
pub(crate) fn current_error(cx: &Cx) -> Value {
    fluid_value(&cx.heap, cx.error_port)
}

/// Writes `text` to `port`, an open output port. A bytevector port takes
/// it in Latin-1, as Guile's does, with `?` for what Latin-1 lacks, where
/// Guile throws an `encoding-error`.
pub(crate) fn emit(cx: &mut Cx, port: Value, text: &str) {
    let kind = cx.heap.foreign_mut::<Port>(port).map(|p| &mut p.kind);
    match kind {
        Some(Kind::Stdout) => cx.out.push_str(text),
        Some(Kind::Stderr) => cx.errors.push_str(text),
        Some(Kind::Output(s)) => s.push_str(text),
        Some(Kind::Bytes(bytes)) => bytes.extend(
            text.chars()
                .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')),
        ),
        _ => {}
    }
}

/// The column of `port`'s line that output goes to next: what follows its
/// last newline, of what the port holds. The standard ports' output the
/// host took is counted as ending a line.
pub(crate) fn column(cx: &Cx, port: Value) -> usize {
    let text = match cx.heap.foreign::<Port>(port).map(|p| &p.kind) {
        Some(Kind::Stdout) => cx.out.as_str(),
        Some(Kind::Stderr) => cx.errors.as_str(),
        Some(Kind::Output(s)) => s.as_str(),
        _ => "",
    };
    text.rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
}

/// `display`, `write`, `newline` and `write-char`, each with an optional
/// port.
pub(crate) fn display(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "display", 2, args.get(1).copied())?;
    let mut text = String::new();
    cx.heap.display(args[0], &mut text);
    emit(cx, port, &text);
    Ok(Value::UNSPECIFIED)
}

pub(crate) fn write(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "write", 2, args.get(1).copied())?;
    let mut text = String::new();
    cx.heap.write(args[0], &mut text);
    emit(cx, port, &text);
    Ok(Value::UNSPECIFIED)
}

pub(crate) fn newline(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "newline", 1, args.first().copied())?;
    emit(cx, port, "\n");
    Ok(Value::UNSPECIFIED)
}

pub(crate) fn write_char(cx: &mut Cx, args: &[Value]) -> R {
    let Some(c) = args[0].as_char() else {
        return Err(cx.wrong_type("write-char", 1, args[0], "character"));
    };
    let port = output(cx, "write-char", 2, args.get(1).copied())?;
    let mut buffer = [0; 4];
    emit(cx, port, c.encode_utf8(&mut buffer));
    Ok(Value::UNSPECIFIED)
}

/// `(force-output [port])`. Output is never buffered here, but Guile's
/// standard error port is, and LilyPond's own messages are not: what the
/// error port got before a flush is printed before the messages that follow
/// it, and the rest after them, so the host is told how much was flushed
/// ([`crate::Vm::take_error_port`]).
pub(crate) fn force_output(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "force-output", 1, args.first().copied())?;
    if matches!(
        cx.heap.foreign::<Port>(port).map(|p| &p.kind),
        Some(Kind::Stderr)
    ) {
        cx.flushed = cx.errors.len();
    }
    Ok(Value::UNSPECIFIED)
}

/// `(object->string obj [printer])`: `write`'s text, or `display`'s when
/// the printer is `display`.
pub(crate) fn object_to_string(cx: &mut Cx, args: &[Value]) -> R {
    let mut text = String::new();
    let display = crate::prims::primitive("display");
    if args.get(1) == Some(&display) {
        cx.heap.display(args[0], &mut text);
    } else {
        cx.heap.write(args[0], &mut text);
    }
    Ok(cx.heap.string(text))
}

pub(crate) fn open_output_string(cx: &mut Cx, _: &[Value]) -> R {
    Ok(cx.heap.wrap(Port {
        kind: Kind::Output(String::new()),
        open: true,
    }))
}

pub(crate) fn open_input_string(cx: &mut Cx, args: &[Value]) -> R {
    let Some(text) = cx.heap.str(args[0]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("open-input-string", 1, args[0], "string"));
    };
    Ok(cx.heap.wrap(Port {
        kind: Kind::Input(text, 0),
        open: true,
    }))
}

pub(crate) fn get_output_string(cx: &mut Cx, args: &[Value]) -> R {
    let text = match cx.heap.foreign::<Port>(args[0]) {
        Some(Port {
            kind: Kind::Output(s),
            open: true,
        }) => s.clone(),
        _ => {
            return Err(cx.wrong_type("get-output-string", 1, args[0], "open output string port"));
        }
    };
    Ok(cx.heap.string(text))
}

fn port_test(cx: &mut Cx, v: Value, test: fn(&Port) -> bool) -> R {
    Ok(Value::bool(cx.heap.foreign::<Port>(v).is_some_and(test)))
}

pub(crate) fn is_port(cx: &mut Cx, args: &[Value]) -> R {
    port_test(cx, args[0], |_| true)
}
pub(crate) fn is_input_port(cx: &mut Cx, args: &[Value]) -> R {
    port_test(cx, args[0], |p| !p.is_output())
}
pub(crate) fn is_output_port(cx: &mut Cx, args: &[Value]) -> R {
    port_test(cx, args[0], Port::is_output)
}

pub(crate) fn is_port_closed(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Port>(args[0]) {
        Some(p) => Ok(Value::bool(!p.open)),
        None => Err(cx.wrong_type("port-closed?", 1, args[0], "port")),
    }
}

/// `close-port`: `#t` if the port was open. `close-input-port` and
/// `close-output-port` too.
pub(crate) fn close_port(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign_mut::<Port>(args[0]) {
        Some(p) => Ok(Value::bool(core::mem::replace(&mut p.open, false))),
        None => Err(cx.wrong_type("close-port", 1, args[0], "port")),
    }
}

/// The input port `given`, or the current one, and its text from where it
/// is.
pub(crate) fn input<'a>(
    cx: &'a mut Cx,
    subr: &str,
    given: Option<Value>,
) -> Result<(Value, &'a str), Thrown> {
    let port = given.unwrap_or_else(|| fluid_value(&cx.heap, cx.input));
    let open = matches!(
        cx.heap.foreign::<Port>(port),
        Some(Port {
            kind: Kind::Input(..),
            open: true
        })
    );
    if !open {
        return Err(cx.wrong_type(subr, 1, port, "open input port"));
    }
    match cx.heap.foreign::<Port>(port) {
        Some(Port {
            kind: Kind::Input(text, at),
            ..
        }) => Ok((port, &text[*at..])),
        _ => unreachable!(),
    }
}

pub(crate) fn advance(cx: &mut Cx, port: Value, bytes: usize) {
    if let Some(Port {
        kind: Kind::Input(_, at),
        ..
    }) = cx.heap.foreign_mut::<Port>(port)
    {
        *at += bytes;
    }
}

pub(crate) fn read_char(cx: &mut Cx, args: &[Value]) -> R {
    let (port, rest) = input(cx, "read-char", args.first().copied())?;
    let Some(c) = rest.chars().next() else {
        return Ok(Value::EOF);
    };
    advance(cx, port, c.len_utf8());
    Ok(Value::char(c))
}

pub(crate) fn peek_char(cx: &mut Cx, args: &[Value]) -> R {
    let (_, rest) = input(cx, "peek-char", args.first().copied())?;
    Ok(rest.chars().next().map_or(Value::EOF, Value::char))
}

pub(crate) fn char_ready(cx: &mut Cx, args: &[Value]) -> R {
    input(cx, "char-ready?", args.first().copied())?;
    Ok(Value::TRUE)
}

pub(crate) fn is_eof_object(_: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(args[0] == Value::EOF))
}

/// `(read [port])`: the next datum, as the reader reads it at the port's
/// place.
pub(crate) fn read_datum(cx: &mut Cx, args: &[Value]) -> R {
    let (port, _) = input(cx, "read", args.first().copied())?;
    let Some(Port {
        kind: Kind::Input(text, at),
        ..
    }) = cx.heap.foreign::<Port>(port)
    else {
        unreachable!()
    };
    let (text, at) = (text.clone(), *at);
    let mut scan = LilyScan { file: None };
    let embedded = match cx.lily_braces {
        true => Some(&mut scan as &mut dyn Embedded),
        false => None,
    };
    match read(&mut cx.heap, &text, at, embedded) {
        Ok(r) => {
            advance(cx, port, r.end - at);
            Ok(r.datum)
        }
        Err(e) => {
            cx.error = Some(read_exception(&mut cx.heap, &e, &Source::new(&text, None)));
            Err(Thrown)
        }
    }
}

/// `(read-hash-extend chr proc)`, which LilyPond calls with `#\{` and its
/// `read-lily-expression` (`parser-ly-from-scheme.scm`): from then on `#{`
/// reads as that procedure reads it, which the reader does natively
/// ([`LilyScan`]). Another character is not extended.
pub(crate) fn read_hash_extend(cx: &mut Cx, args: &[Value]) -> R {
    if args[0] != Value::char('{') {
        return Err(cx.misc_error(
            Some("read-hash-extend"),
            "the engine's reader extends # with LilyPond's #{ only",
            &[],
        ));
    }
    cx.lily_braces = true;
    Ok(Value::UNSPECIFIED)
}

/// The ports' parameters and the procedures that rebind them.
pub(crate) const PRELUDE: &str = r#"
(define (%port-parameter fluid)
  (%make-parameter fluid (lambda (x) x)
    (lambda args
      (if (null? args)
          (fluid-ref fluid)
          (let ((prev (fluid-ref fluid)))
            (fluid-set! fluid (car args))
            prev)))))
(define current-output-port (%port-parameter %output-fluid))
(define current-error-port (%port-parameter %error-fluid))
(define current-input-port (%port-parameter %input-fluid))
(define (with-output-to-port port thunk) (with-fluid* %output-fluid port thunk))
(define (with-error-to-port port thunk) (with-fluid* %error-fluid port thunk))
(define (with-input-from-port port thunk) (with-fluid* %input-fluid port thunk))
(define (call-with-output-string proc)
  (let ((port (open-output-string)))
    (proc port)
    (let ((s (get-output-string port))) (close-port port) s)))
(define (with-output-to-string thunk)
  (call-with-output-string (lambda (port) (with-output-to-port port thunk))))
(define (with-error-to-string thunk)
  (call-with-output-string (lambda (port) (with-error-to-port port thunk))))
(define (call-with-input-string s proc) (proc (open-input-string s)))
(define (with-input-from-string s thunk)
  (with-input-from-port (open-input-string s) thunk))
;; A file is written into `%written-files`, its name and text, the last
;; first, which the host takes (`lily_engine_core`): the engine has no
;; file system.
(define %written-files (list '()))
(define (call-with-output-file file proc)
  (let* ((port (open-output-string))
         (result (proc port)))
    (set-car! %written-files (cons (cons file (get-output-string port)) (car %written-files)))
    (close-port port)
    result))
(define (with-output-to-file file thunk)
  (call-with-output-file file (lambda (port) (with-output-to-port port thunk))))
(define (call-with-port port proc)
  (call-with-values (lambda () (proc port))
    (lambda vals (close-port port) (apply values vals))))
"#;
