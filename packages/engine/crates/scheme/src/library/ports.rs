//! `(ice-9 textual-ports)` and `(ice-9 binary-ports)`'s bytevector output
//! port, over `ports.rs`' ports.
//!
//! The textual procedures read and write the string ports and the standard
//! ports: `get-char`, `lookahead-char`, `get-line`, `get-string-all`,
//! `get-string-n`, `put-char` and `put-string`. A bytevector output port
//! collects bytes (`put-u8`, `put-bytevector`, and text in Latin-1) until
//! its procedure takes them.

use alloc::string::String;
use alloc::vec::Vec;

use crate::ports::{Kind, Port, advance, emit, input, output};
use crate::reader::Bytevector;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

type R = Result<Value, Thrown>;

pub(crate) fn get_char(cx: &mut Cx, args: &[Value]) -> R {
    let (port, rest) = input(cx, "get-char", Some(args[0]))?;
    let Some(c) = rest.chars().next() else {
        return Ok(Value::EOF);
    };
    advance(cx, port, c.len_utf8());
    Ok(Value::char(c))
}

pub(crate) fn lookahead_char(cx: &mut Cx, args: &[Value]) -> R {
    let (_, rest) = input(cx, "lookahead-char", Some(args[0]))?;
    Ok(rest.chars().next().map_or(Value::EOF, Value::char))
}

/// `(get-line port)`: the text to the next newline, which is read and not
/// returned; the end of file if there is no text.
pub(crate) fn get_line(cx: &mut Cx, args: &[Value]) -> R {
    let (port, rest) = input(cx, "get-line", Some(args[0]))?;
    if rest.is_empty() {
        return Ok(Value::EOF);
    }
    let (line, used) = match rest.find('\n') {
        Some(at) => (String::from(&rest[..at]), at + 1),
        None => (String::from(rest), rest.len()),
    };
    advance(cx, port, used);
    Ok(cx.heap.string(line))
}

/// `(get-string-all port)`: the rest of the text, empty at the end.
pub(crate) fn get_string_all(cx: &mut Cx, args: &[Value]) -> R {
    let (port, rest) = input(cx, "get-string-all", Some(args[0]))?;
    let text = String::from(rest);
    advance(cx, port, text.len());
    Ok(cx.heap.string(text))
}

/// `(get-string-n port count)`: at most `count` characters; the end of file
/// if there are none.
pub(crate) fn get_string_n(cx: &mut Cx, args: &[Value]) -> R {
    let count = cx.size(args[1], u64::MAX)?;
    let (port, rest) = input(cx, "get-string-n", Some(args[0]))?;
    if rest.is_empty() && count > 0 {
        return Ok(Value::EOF);
    }
    let end = rest
        .char_indices()
        .nth(count)
        .map_or(rest.len(), |(at, _)| at);
    let text = String::from(&rest[..end]);
    advance(cx, port, end);
    Ok(cx.heap.string(text))
}

pub(crate) fn put_char(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "put-char", 1, Some(args[0]))?;
    let Some(c) = args[1].as_char() else {
        return Err(cx.wrong_type("put-char", 2, args[1], "character"));
    };
    let mut buffer = [0; 4];
    emit(cx, port, c.encode_utf8(&mut buffer));
    Ok(Value::UNSPECIFIED)
}

/// `(put-string port string [start [count]])`.
pub(crate) fn put_string(cx: &mut Cx, args: &[Value]) -> R {
    let port = output(cx, "put-string", 1, Some(args[0]))?;
    let Some(text) = cx.heap.str(args[1]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("put-string", 2, args[1], "string"));
    };
    let length = text.chars().count();
    let start = match args.get(2) {
        Some(&v) => cx.size(v, length as u64)?,
        None => 0,
    };
    let count = match args.get(3) {
        Some(&v) => cx.size(v, (length - start) as u64)?,
        None => length - start,
    };
    let part: String = text.chars().skip(start).take(count).collect();
    emit(cx, port, &part);
    Ok(Value::UNSPECIFIED)
}

/// `(%open-bytevector-output-port)`: the port; `open-bytevector-output-port`
/// gives it with the procedure that takes its bytes.
pub(crate) fn open_bytevector_output_port(cx: &mut Cx, _: &[Value]) -> R {
    Ok(cx.heap.wrap(Port {
        kind: Kind::Bytes(Vec::new()),
        open: true,
    }))
}

fn bytes_port<'a>(cx: &'a mut Cx, subr: &str, port: Value) -> Result<&'a mut Vec<u8>, Thrown> {
    let ok = matches!(
        cx.heap.foreign::<Port>(port),
        Some(Port {
            kind: Kind::Bytes(_),
            open: true
        })
    );
    if !ok {
        return Err(cx.wrong_type(subr, 1, port, "open bytevector output port"));
    }
    match cx.heap.foreign_mut::<Port>(port) {
        Some(Port {
            kind: Kind::Bytes(bytes),
            ..
        }) => Ok(bytes),
        _ => unreachable!(),
    }
}

/// `(%take-bytevector port)`: what the port got since the last time.
pub(crate) fn take_bytevector(cx: &mut Cx, args: &[Value]) -> R {
    let bytes = core::mem::take(bytes_port(cx, "get-bytevector", args[0])?);
    Ok(cx.heap.wrap(Bytevector(bytes)))
}

pub(crate) fn put_u8(cx: &mut Cx, args: &[Value]) -> R {
    let byte = cx.size(args[1], 255)? as u8;
    bytes_port(cx, "put-u8", args[0])?.push(byte);
    Ok(Value::UNSPECIFIED)
}

pub(crate) fn put_bytevector(cx: &mut Cx, args: &[Value]) -> R {
    let Some(bytes) = cx.heap.foreign::<Bytevector>(args[1]).map(|b| b.0.clone()) else {
        return Err(cx.wrong_type("put-bytevector", 2, args[1], "bytevector"));
    };
    bytes_port(cx, "put-bytevector", args[0])?.extend_from_slice(&bytes);
    Ok(Value::UNSPECIFIED)
}

/// `(%port-column port)`, for `pretty-print`.
pub(crate) fn port_column(cx: &mut Cx, args: &[Value]) -> R {
    let column = crate::ports::column(cx, args[0]);
    Ok(cx.heap.integer(column as i64))
}

/// `(array-copy array)` of `(ice-9 arrays)`: a fresh copy of a vector, a
/// string or a bytevector, the arrays of one dimension from 0. Arrays of
/// more dimensions are not provided.
pub(crate) fn array_copy(cx: &mut Cx, args: &[Value]) -> R {
    let a = args[0];
    if let Some(items) = cx.heap.vector_items(a) {
        let items = items.to_vec();
        return Ok(cx.heap.vector(items));
    }
    if a.tag() == Tag::String {
        let text = cx
            .heap
            .str(a)
            .map(|s| String::from(s.as_str()))
            .unwrap_or_default();
        return Ok(cx.heap.string(text));
    }
    if let Some(bytes) = cx.heap.foreign::<Bytevector>(a).map(|b| b.0.clone()) {
        return Ok(cx.heap.wrap(Bytevector(bytes)));
    }
    if let Some(array) = cx.heap.foreign::<crate::reader::Array>(a).cloned() {
        return Ok(cx.heap.wrap(array));
    }
    if let Some(copy) = crate::array::copy_shared(cx, a) {
        return Ok(copy);
    }
    Err(cx.wrong_type("array-copy", 1, a, "array"))
}
