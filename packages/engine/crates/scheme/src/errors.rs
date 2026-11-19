//! Guile 3's errors (chapter 03 §5.3, R7): the templates that Guile 3.0.11's
//! C code, VM and `boot-9.scm` throw with, and `print-exception`, which
//! words a throw as Guile prints it after `ERROR:` and as LilyPond prints it
//! under "Guile signaled an error for the expression beginning here".
//!
//! An error is a throw of a key with the arguments `(subr message args
//! rest)`: the procedure's name or `#f`, the message with `~A` and `~S` for
//! `args`, and `rest`, which is `#f` or, for a wrong type or a value out of
//! range, a list of the culprit. `tests/errors.rs` checks the wording of
//! every primitive against guile.
//!
//! Where Guile words an error twice, the engine does too. The procedures
//! `car`, `cdr`, `vector-length`, `vector-ref` and `vector-set!` say `Wrong
//! type (expecting pair)` and the like, at toplevel, through `map` or
//! `apply`; but a call of one by its name inside a procedure or a `let` with
//! bindings is compiled code in Guile's evaluator (`ice-9/eval.scm`), whose
//! checks say `Wrong type argument in position 1 (expecting pair)`. The
//! compiler marks those calls ([`crate::vm::Cx::direct`]).

use alloc::string::String;
use alloc::vec::Vec;

use crate::bignum::BigInt;
use crate::heap::Heap;
use crate::number::NumError;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown};

/// A message Guile throws: its key, its text, and whether its `rest` is a
/// list of the last argument (the culprit) or `#f`.
pub(crate) struct Template {
    pub key: &'static str,
    pub message: &'static str,
    pub culprit: bool,
}

const fn t(key: &'static str, message: &'static str, culprit: bool) -> Template {
    Template {
        key,
        message,
        culprit,
    }
}

/// `scm_wrong_type_arg_msg`: a C procedure's argument of the wrong type.
pub(crate) const WRONG_TYPE_EXPECTING: Template = t(
    "wrong-type-arg",
    "Wrong type argument in position ~A (expecting ~A): ~S",
    true,
);
/// `scm_wrong_type_arg`: the same without what was expected.
pub(crate) const WRONG_TYPE_AT: Template = t(
    "wrong-type-arg",
    "Wrong type argument in position ~A: ~S",
    true,
);
/// The VM's type checks, and C procedures that pass no position.
pub(crate) const WRONG_TYPE: Template = t("wrong-type-arg", "Wrong type (expecting ~A): ~S", true);
/// `boot-9.scm`'s checks in `catch`, `throw` and `scm-error`.
pub(crate) const WRONG_TYPE_SCHEME: Template = t(
    "wrong-type-arg",
    "Wrong type argument in position ~a: ~a",
    true,
);
/// `with-fluid*`'s check of its fluid.
pub(crate) const WRONG_TYPE_PLAIN: Template = t("wrong-type-arg", "Wrong type: ~S", true);
pub(crate) const NOT_A_PARAMETER: Template = t("wrong-type-arg", "Not a parameter: ~S", false);
pub(crate) const WRONG_TYPE_TO_APPLY: Template =
    t("wrong-type-arg", "Wrong type to apply: ~S", true);
pub(crate) const APPLY_TO_NON_LIST: Template = t("wrong-type-arg", "Apply to non-list: ~S", true);
/// `scm_out_of_range_pos`.
pub(crate) const ARGUMENT_OUT_OF_RANGE: Template =
    t("out-of-range", "Argument ~A out of range: ~S", true);
/// `scm_out_of_range`, and the VM's index checks.
pub(crate) const VALUE_OUT_OF_RANGE: Template = t("out-of-range", "Value out of range: ~S", true);
/// The C procedures' checks of a size or index against its bounds.
pub(crate) const VALUE_OUT_OF_BOUNDS: Template =
    t("out-of-range", "Value out of range ~S to< ~S: ~S", true);
/// An array index outside its dimension's bounds, both inclusive.
pub(crate) const INDEX_OUT_OF_BOUNDS: Template =
    t("out-of-range", "Value out of range ~S to ~S: ~S", true);
pub(crate) const UNBOUND_VARIABLE: Template = t("unbound-variable", "Unbound variable: ~S", false);
pub(crate) const WRONG_NUMBER_OF_ARGS: Template = t(
    "wrong-number-of-args",
    "Wrong number of arguments to ~A",
    false,
);
pub(crate) const ZERO_VALUES: Template = t(
    "misc-error",
    "Zero values returned to single-valued continuation",
    false,
);
/// A call of an escape procedure whose extent has ended; Guile's `abort`
/// passes the prompt's tag as an argument the message does not use, so its
/// printer fails after the text, as Guile's does.
pub(crate) const ABORT_TO_UNKNOWN_PROMPT: Template =
    t("misc-error", "Abort to unknown prompt", false);
/// The engine's own: Guile 3 grows its stack until memory runs out.
pub(crate) const STACK_OVERFLOW: Template = t("stack-overflow", "Stack overflow", false);
/// The engine's own: the host's cancel check said stop (R14).
pub(crate) const CANCELLED: Template = t("cancelled", "Cancelled", false);

/// Guile's largest `size_t` and `SCM_MAX_VECTOR_LENGTH`, the upper bounds of
/// its size checks.
pub(crate) const SIZE_MAX: u64 = u64::MAX;
pub(crate) const MAX_VECTOR_LENGTH: u64 = (1 << 56) - 1;

impl Cx {
    /// How much has been written to the standard error port and not
    /// taken, as [`crate::Vm::error_len`] says.
    pub fn error_len(&self) -> usize {
        self.errors.len()
    }

    /// Throws `template` from `subr` with `args`.
    #[cold]
    pub(crate) fn raise(
        &mut self,
        template: &Template,
        subr: Option<&str>,
        args: &[Value],
    ) -> Thrown {
        self.raise_message(template.key, subr, template.message, args, template.culprit)
    }

    /// A `misc-error` from `subr` with `message` and its `args`.
    #[cold]
    pub(crate) fn misc_error(
        &mut self,
        subr: Option<&str>,
        message: &str,
        args: &[Value],
    ) -> Thrown {
        self.raise_message("misc-error", subr, message, args, false)
    }

    /// Throws `key` from `subr` with Guile's arguments of an error:
    /// `message`, in which `~A` and `~S` take `args` in turn, and no data.
    #[cold]
    pub fn error(
        &mut self,
        key: &str,
        subr: Option<&str>,
        message: &str,
        args: &[Value],
    ) -> Thrown {
        self.raise_message(key, subr, message, args, false)
    }

    #[cold]
    fn raise_message(
        &mut self,
        key: &str,
        subr: Option<&str>,
        message: &str,
        args: &[Value],
        culprit: bool,
    ) -> Thrown {
        let key = self.heap.intern(key);
        let subr = match subr {
            Some(subr) => self.heap.string(subr),
            None => Value::FALSE,
        };
        let message = self.heap.string(message);
        let rest = match args.last() {
            Some(&culprit_value) if culprit => self.heap.list(&[culprit_value]),
            _ => Value::FALSE,
        };
        let args = self.heap.list(args);
        let args = self.heap.list(&[subr, message, args, rest]);
        self.error = Some(Exception { key, args });
        Thrown
    }

    /// The check of Guile's compiled code for a direct call
    /// ([`crate::vm::Cx::direct`]): the same text as [`Cx::wrong_type`], with
    /// the position and what was expected written into the message.
    #[cold]
    pub(crate) fn wrong_type_compiled(
        &mut self,
        subr: &str,
        position: usize,
        arg: Value,
        expecting: &str,
    ) -> Thrown {
        let message = alloc::format!(
            "Wrong type argument in position {position} (expecting {expecting}): ~S"
        );
        self.raise_message("wrong-type-arg", Some(subr), &message, &[arg], true)
    }

    /// The same for an index out of range: `Argument N out of range: arg`.
    #[cold]
    pub(crate) fn out_of_range_compiled(
        &mut self,
        subr: &str,
        position: usize,
        arg: Value,
    ) -> Thrown {
        let message = alloc::format!("Argument {position} out of range: ~S");
        self.raise_message("out-of-range", Some(subr), &message, &[arg], true)
    }

    /// `Wrong type argument in position N (expecting what): arg`.
    #[cold]
    pub fn wrong_type(
        &mut self,
        subr: &str,
        position: usize,
        arg: Value,
        expecting: &str,
    ) -> Thrown {
        let position = self.heap.integer(position as i64);
        let expecting = self.heap.string(expecting);
        self.raise(
            &WRONG_TYPE_EXPECTING,
            Some(subr),
            &[position, expecting, arg],
        )
    }

    /// `Wrong type argument in position N: arg`.
    #[cold]
    pub(crate) fn wrong_type_at(&mut self, subr: &str, position: usize, arg: Value) -> Thrown {
        let position = self.heap.integer(position as i64);
        self.raise(&WRONG_TYPE_AT, Some(subr), &[position, arg])
    }

    /// `Wrong type (expecting what): arg`.
    #[cold]
    pub fn wrong_type_vm(&mut self, subr: Option<&str>, arg: Value, expecting: &str) -> Thrown {
        let expecting = self.heap.string(expecting);
        self.raise(&WRONG_TYPE, subr, &[expecting, arg])
    }

    /// `Argument N out of range: arg`.
    #[cold]
    pub(crate) fn out_of_range(&mut self, subr: &str, position: usize, arg: Value) -> Thrown {
        let position = self.heap.integer(position as i64);
        self.raise(&ARGUMENT_OUT_OF_RANGE, Some(subr), &[position, arg])
    }

    /// `Value out of range: arg`.
    #[cold]
    pub(crate) fn value_out_of_range(&mut self, subr: Option<&str>, arg: Value) -> Thrown {
        self.raise(&VALUE_OUT_OF_RANGE, subr, &[arg])
    }

    /// `Value out of range low to< high: arg`, from no procedure.
    #[cold]
    pub fn out_of_bounds(&mut self, low: i64, high: u64, arg: Value) -> Thrown {
        let low = self.heap.integer(low);
        let high = match i64::try_from(high) {
            Ok(n) => self.heap.integer(n),
            Err(_) => self.heap.integer_big(BigInt::from_u64(high)),
        };
        self.raise(&VALUE_OUT_OF_BOUNDS, None, &[low, high, arg])
    }

    /// A count or index of a C procedure, an exact integer from 0 to `high`,
    /// checked as `scm_to_size_t` checks it.
    pub(crate) fn size(&mut self, arg: Value, high: u64) -> Result<usize, Thrown> {
        match self.heap.to_i64(arg) {
            Some(n) if n >= 0 && n as u64 <= high => Ok(n as usize),
            Some(_) => Err(self.out_of_bounds(0, high, arg)),
            None if self.heap.is_exact_integer(arg) => Err(self.out_of_bounds(0, high, arg)),
            None => Err(self.wrong_type_vm(None, arg, "exact integer")),
        }
    }

    /// A number primitive's error; `args` are its arguments, as `NumError`
    /// counts them from 1.
    #[cold]
    pub fn num_error(&mut self, subr: &str, error: NumError, args: &[Value]) -> Thrown {
        let arg = |n: u8| {
            args.get(n as usize - 1)
                .copied()
                .unwrap_or(Value::UNSPECIFIED)
        };
        match error {
            NumError::WrongType(n) => self.wrong_type_at(subr, n as usize, arg(n)),
            NumError::OutOfRange(n) => self.out_of_range(subr, n as usize, arg(n)),
            NumError::Overflow => {
                // Guile's division names the procedure that divides.
                let subr = match subr {
                    "/" => "divide",
                    "quotient" => "truncate-quotient",
                    "remainder" => "truncate-remainder",
                    "modulo" => "floor-remainder",
                    other => other,
                };
                let key = self.heap.intern("numerical-overflow");
                let subr = self.heap.string(subr);
                let message = self.heap.string("Numerical overflow");
                let args = self.heap.list(&[subr, message, Value::FALSE, Value::FALSE]);
                self.error = Some(Exception { key, args });
                Thrown
            }
            NumError::Exponent { .. } => self.value_out_of_range(Some(subr), arg(1)),
        }
    }
}

// --- print-exception -----------------------------------------------------------------

/// The keys `boot-9.scm` prints with `scm-error-printer`.
const SCM_ERROR_KEYS: &[&str] = &[
    "goops-error",
    "host-not-found",
    "misc-error",
    "no-data",
    "no-recovery",
    "null-pointer-error",
    "out-of-memory",
    "out-of-range",
    "program-error",
    "read-error",
    "regular-expression-syntax",
    "signal",
    "stack-overflow",
    "system-error",
    "try-again",
    "unbound-variable",
    "wrong-number-of-args",
    "wrong-type-arg",
];

/// That printing failed, as Guile's printer reports it.
struct Failed;

impl Exception {
    /// The throw as Guile 3's `print-exception` prints it with no frame,
    /// without the newline: `In procedure car: Wrong type (expecting pair):
    /// ()`, `Unbound variable: x`, `Throw to key `foo' with args `(1)'.`
    pub fn text(&self, heap: &Heap) -> String {
        let mut out = String::new();
        let key = if self.key.tag() == Tag::Symbol {
            heap.name(self.key)
        } else {
            None
        };
        let items = list_items(heap, self.args);
        let printed = match key {
            Some(k) if SCM_ERROR_KEYS.contains(&k) => match items.as_deref() {
                Some([subr, message, args, ..]) => {
                    scm_error(heap, *subr, *message, *args, &mut out)
                }
                Some(_) => default(heap, self, &mut out),
                None => Err(Failed),
            },
            Some("syntax-error") => match items.as_deref() {
                Some([who, what, place, form, subform, ..]) => {
                    syntax_error(heap, [*who, *what, *place, *form, *subform], &mut out);
                    Ok(())
                }
                _ => default(heap, self, &mut out),
            },
            Some("keyword-argument-error") => match items.as_deref() {
                Some([_, message, _, faulty, ..]) if heap.car(*faulty).is_some() => {
                    heap.display(*message, &mut out);
                    out.push_str(": ");
                    heap.write(heap.car(*faulty).unwrap_or(Value::FALSE), &mut out);
                    Ok(())
                }
                _ => Err(Failed),
            },
            _ => default(heap, self, &mut out),
        };
        if printed.is_err() {
            out.push_str("Error while printing exception.");
        }
        out
    }
}

fn default(heap: &Heap, e: &Exception, out: &mut String) -> Result<(), Failed> {
    out.push_str("Throw to key `");
    heap.display(e.key, out);
    out.push_str("' with args `");
    heap.write(e.args, out);
    out.push_str("'.");
    Ok(())
}

/// `scm-error-printer`: `In procedure subr: ` and the message formatted.
fn scm_error(
    heap: &Heap,
    subr: Value,
    message: Value,
    args: Value,
    out: &mut String,
) -> Result<(), Failed> {
    if subr != Value::FALSE {
        out.push_str("In procedure ");
        heap.display(subr, out);
        out.push_str(": ");
    }
    let args = if args == Value::FALSE {
        Some(Vec::new())
    } else {
        list_items(heap, args)
    };
    let (Some(message), Some(args)) = (heap.str(message), args) else {
        return Err(Failed);
    };
    simple_format(heap, message.as_str(), &args, out)
}

/// `syntax-error-printer`: `Syntax error:`, the place, who, what, and the
/// form or subform.
fn syntax_error(heap: &Heap, [who, what, place, form, subform]: [Value; 5], out: &mut String) {
    out.push_str("Syntax error:\n");
    if place != Value::FALSE {
        let field = |name: &str| {
            let mut at = place;
            while let (Some(entry), Some(next)) = (heap.car(at), heap.cdr(at)) {
                if heap.car(entry).and_then(|k| heap.name(k)) == Some(name) {
                    return heap.cdr(entry);
                }
                at = next;
            }
            None
        };
        match field("filename") {
            Some(file) if file != Value::FALSE => heap.display(file, out),
            _ => out.push_str("unknown file"),
        }
        out.push(':');
        match field("line").and_then(|l| l.as_fixnum()) {
            Some(line) => out.push_str(&alloc::format!("{}", line + 1)),
            None => out.push_str("#f"),
        }
        out.push(':');
        heap.display(field("column").unwrap_or(Value::FALSE), out);
        out.push_str(": ");
    } else {
        out.push_str("unknown location: ");
    }
    if who != Value::FALSE {
        heap.display(who, out);
        out.push_str(": ");
    }
    heap.display(what, out);
    if subform != Value::FALSE {
        out.push_str(" in subform ");
        heap.write(subform, out);
        out.push_str(" of ");
        heap.write(form, out);
    } else if form != Value::FALSE {
        out.push_str(" in form ");
        heap.write(form, out);
    }
}

/// Guile's `simple-format`: `~A` displays the next argument, `~S` writes it,
/// `~%` is a newline and `~~` a tilde. Another directive, too few arguments
/// or too many fail. The text before a directive is written with it, so a
/// failing directive loses it; the text at the end is written before the
/// check for arguments left over.
fn simple_format(
    heap: &Heap,
    message: &str,
    args: &[Value],
    out: &mut String,
) -> Result<(), Failed> {
    let mut args = args.iter();
    let mut chunk = String::new();
    let mut chars = message.chars();
    while let Some(c) = chars.next() {
        if c != '~' {
            chunk.push(c);
            continue;
        }
        let directive = chars.next();
        let arg = match directive {
            Some('A' | 'a' | 'S' | 's') => Some(*args.next().ok_or(Failed)?),
            Some('%' | '~') => None,
            _ => return Err(Failed),
        };
        out.push_str(&core::mem::take(&mut chunk));
        match (directive, arg) {
            (Some('A' | 'a'), Some(arg)) => heap.display(arg, out),
            (Some('S' | 's'), Some(arg)) => heap.write(arg, out),
            (Some('%'), _) => out.push('\n'),
            _ => out.push('~'),
        }
    }
    out.push_str(&chunk);
    match args.next() {
        Some(_) => Err(Failed),
        None => Ok(()),
    }
}

/// The items of a proper list; `None` for anything else.
pub(crate) fn list_items(heap: &Heap, mut list: Value) -> Option<Vec<Value>> {
    let mut items = Vec::new();
    while let (Some(car), Some(cdr)) = (heap.car(list), heap.cdr(list)) {
        items.push(car);
        list = cdr;
    }
    (list == Value::NULL).then_some(items)
}
