//! `(ice-9 pretty-print)`'s `pretty-print`, written for the engine from
//! the layout Guile 3 gives (Marc Feeley's `genwrite`, which Guile's is
//! derived from): an object that fits in what is left of the line is
//! written on it; a list that does not is broken, its items under each
//! other, special forms with their bodies indented by two, `if` with its
//! branches under its test, and a call whose head is at most five
//! characters with its arguments after the head.

use alloc::string::String;
use alloc::vec::Vec;

use crate::heap::Heap;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

/// How the items of a broken list are laid out.
#[derive(Clone, Copy)]
enum Layout {
    /// As expressions: special forms, calls and lists.
    Expr,
    /// As a list of expressions, each on its own line.
    ExprList,
}

struct Printer<'a> {
    heap: &'a mut Heap,
    out: String,
    column: usize,
    /// The last column a line may reach: the width less the prefix.
    max_column: usize,
    display: bool,
    prefix: &'a str,
}

/// The prefix a datum `(head x)` is written with, if it is a quote form.
fn quote_prefix(heap: &Heap, obj: Value) -> Option<(&'static str, Value)> {
    let head = heap.car(obj)?;
    let rest = heap.cdr(obj)?;
    if heap.cdr(rest)? != Value::NULL || head.tag() != Tag::Symbol {
        return None;
    }
    let prefix = match heap.name(head)? {
        "quote" => "'",
        "quasiquote" => "`",
        "unquote" => ",",
        "unquote-splicing" => ",@",
        _ => return None,
    };
    Some((prefix, heap.car(rest)?))
}

/// The items of `list` up to its first non-pair, and that end.
fn items(heap: &Heap, mut list: Value) -> (Vec<Value>, Value) {
    let mut items = Vec::new();
    while let (Some(car), Some(cdr)) = (heap.car(list), heap.cdr(list)) {
        items.push(car);
        list = cdr;
    }
    (items, list)
}

impl Printer<'_> {
    fn put(&mut self, text: &str) {
        match text.rfind('\n') {
            Some(at) => self.column = text[at + 1..].chars().count(),
            None => self.column += text.chars().count(),
        }
        self.out.push_str(text);
    }

    /// `write` (or `display`), with quote forms abbreviated anywhere in a
    /// proper list.
    fn wr(&self, obj: Value, out: &mut String) {
        if let Some((prefix, x)) = quote_prefix(self.heap, obj) {
            out.push_str(prefix);
            self.wr(x, out);
            return;
        }
        let (list, end) = items(self.heap, obj);
        if !list.is_empty() && end == Value::NULL {
            out.push('(');
            for (i, &item) in list.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                self.wr(item, out);
            }
            out.push(')');
        } else if self.display {
            self.heap.display(obj, out);
        } else {
            self.heap.write(obj, out);
        }
    }

    fn write(&mut self, obj: Value) {
        let mut text = String::new();
        self.wr(obj, &mut text);
        self.put(&text);
    }

    fn spaces(&mut self, n: usize) {
        for _ in 0..n {
            self.out.push(' ');
        }
        self.column += n;
    }

    /// To column `to`, on a new line if the line is past it.
    fn indent(&mut self, to: usize) {
        if to < self.column {
            self.out.push('\n');
            self.out.push_str(self.prefix);
            self.column = self.prefix.chars().count();
            self.spaces(to.saturating_sub(self.column));
        } else {
            self.spaces(to - self.column);
        }
    }

    fn pr(&mut self, obj: Value, layout: Layout) {
        if let Some(items) = self.heap.vector_items(obj) {
            let items = items.to_vec();
            let list = self.heap.list(&items);
            self.put("#");
            self.pr(list, layout);
            return;
        }
        if obj.tag() != Tag::Pair {
            self.write(obj);
            return;
        }
        if let Some((prefix, x)) = quote_prefix(self.heap, obj) {
            self.put(prefix);
            self.pr(x, layout);
            return;
        }
        let mut text = String::new();
        self.wr(obj, &mut text);
        let room = self.max_column.saturating_sub(self.column);
        let fits =
            self.max_column >= self.column && !text.contains('\n') && text.chars().count() <= room;
        if fits {
            self.put(&text);
        } else {
            match layout {
                Layout::Expr => self.pp_expr(obj),
                Layout::ExprList => self.pp_list(obj, Layout::Expr),
            }
        }
    }

    fn pp_expr(&mut self, expr: Value) {
        let (list, end) = items(self.heap, expr);
        let head = list[0];
        let name = match head.tag() {
            Tag::Symbol => self.heap.name(head),
            _ => None,
        };
        let n = list.len();
        let mut tails = Vec::with_capacity(4);
        let mut at = expr;
        for _ in 0..4 {
            tails.push(at);
            at = self.heap.cdr(at).unwrap_or(Value::NULL);
        }
        let rest = |k: usize| tails[k];
        match name {
            Some("lambda" | "lambda*") if n >= 3 => {
                self.pp1(head, list[1], rest(2), Layout::ExprList)
            }
            Some("let") if n >= 4 && list[1].tag() == Tag::Symbol => self.pp2(
                head,
                list[1],
                list[2],
                rest(3),
                Layout::Expr,
                Layout::ExprList,
            ),
            Some("let" | "let*" | "letrec" | "letrec*" | "let-syntax" | "letrec-syntax")
                if n >= 3 =>
            {
                self.pp1(head, list[1], rest(2), Layout::ExprList)
            }
            Some("define" | "define*" | "define-public" | "define-syntax") if n >= 3 => {
                self.pp1(head, list[1], rest(2), Layout::ExprList)
            }
            Some("if") if (n == 3 || n == 4) && end == Value::NULL => self.pp_if(head, &list),
            Some("cond" | "begin") => self.pp0(head, rest(1)),
            Some("case") if n >= 2 => self.pp1(head, list[1], rest(2), Layout::Expr),
            Some("do") if n >= 3 => self.pp2(
                head,
                list[1],
                list[2],
                rest(3),
                Layout::ExprList,
                Layout::ExprList,
            ),
            Some("syntax-rules") if n >= 2 => self.pp1(head, list[1], rest(2), Layout::ExprList),
            Some("syntax-case") if n >= 3 => self.pp2(
                head,
                list[1],
                list[2],
                rest(3),
                Layout::Expr,
                Layout::ExprList,
            ),
            Some(name) if name.chars().count() <= 5 => {
                self.put("(");
                self.write(head);
                let column = self.column + 1;
                self.pp_down(rest(1), column, Layout::Expr);
            }
            _ => self.pp_list(expr, Layout::Expr),
        }
    }

    fn pp0(&mut self, head: Value, body: Value) {
        let body_column = self.column + 2;
        self.put("(");
        self.write(head);
        self.pp_down(body, body_column, Layout::Expr);
    }

    fn pp1(&mut self, head: Value, param: Value, body: Value, layout: Layout) {
        let body_column = self.column + 2;
        self.put("(");
        self.write(head);
        self.put(" ");
        self.pr(param, layout);
        self.pp_down(body, body_column, Layout::Expr);
    }

    fn pp2(
        &mut self,
        head: Value,
        param0: Value,
        param1: Value,
        body: Value,
        layout0: Layout,
        layout1: Layout,
    ) {
        let body_column = self.column + 2;
        self.put("(");
        self.write(head);
        self.put(" ");
        self.pr(param0, layout0);
        self.put(" ");
        self.pr(param1, layout1);
        self.pp_down(body, body_column, Layout::Expr);
    }

    /// `if`'s branches go under its test.
    fn pp_if(&mut self, head: Value, list: &[Value]) {
        self.put("(");
        self.write(head);
        self.put(" ");
        let body_column = self.column;
        self.pr(list[1], Layout::Expr);
        let body = self.heap.list(&list[2..]);
        self.pp_down(body, body_column, Layout::Expr);
    }

    fn pp_list(&mut self, list: Value, layout: Layout) {
        self.put("(");
        let column = self.column;
        self.pp_down(list, column, layout);
    }

    fn pp_down(&mut self, list: Value, column: usize, layout: Layout) {
        let (items, end) = items(self.heap, list);
        for item in items {
            self.indent(column);
            self.pr(item, layout);
        }
        if end != Value::NULL {
            self.indent(column);
            self.put(".");
            self.indent(column);
            self.pr(end, layout);
        }
        self.put(")");
    }
}

/// `(%pretty-print obj width display? prefix column)`: the text
/// `pretty-print` writes, from `column` of the port's line.
pub(crate) fn pretty_print(cx: &mut Cx, args: &[Value]) -> Result<Value, Thrown> {
    let width = cx.size(args[1], u32::MAX as u64)?;
    let Some(prefix) = cx.heap.str(args[3]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("pretty-print", 1, args[3], "string"));
    };
    let column = cx.size(args[4], u32::MAX as u64)?;
    let mut printer = Printer {
        heap: &mut cx.heap,
        out: String::new(),
        column,
        max_column: width.saturating_sub(prefix.chars().count()),
        display: args[2] != Value::FALSE,
        prefix: &prefix,
    };
    printer.put(&prefix);
    printer.pr(args[0], Layout::Expr);
    printer.out.push('\n');
    let text = printer.out;
    Ok(cx.heap.string(text))
}
