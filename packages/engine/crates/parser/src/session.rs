//! The session (chapter 03 §1, `lily.scm`): what an init file defines is
//! recorded once, as `session-save` records the variables of
//! `declarations-init.ly`'s module, and each file's module is given them,
//! as `session-replay` gives them. An image written after [`parse_init`]
//! holds the record, so a restored VM parses no init file (02 §5.8).
//!
//! A record is `(name #t module . value)` in `(lily)`'s
//! `lilypond-declarations`, beside `define-session`'s `(name #f module .
//! value)`. LilyPond records the variable itself and adds it to each file's
//! module; the engine has no variable objects (`lily.rs`), so a file's
//! module uses the init file's module, searched before `(lily)` (MS2 step
//! 16): the variables are shared, not copied (D66), and `set!` of one
//! changes the init file's, which `session-terminate` restores after the
//! file.

use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Value, Vm};

use crate::classify::Include;
use crate::{Env, NoteNames, Parse, parse_in};

/// Parses `text` as the init file `name`, with the pitch names of `names`'
/// default language, in a module of its own ([`Env::bare`]) that uses
/// what the init files before it recorded, as LilyPond's init files share
/// `declarations-init.ly`'s module; `\include` reads what `include` has of
/// each file it looks for, which for an init file is first its own name.
/// Then records every variable it defined or changed, as `session-save`
/// does, unless it failed.
pub fn parse_init(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    vm: &mut Vm,
    include: Include<'_>,
) -> Parse {
    let mut env = Env::bare(vm);
    env.include = Some(include);
    let module = env.module();
    replay(env.vm(), module);
    let parse = parse_in(name, text, names, env, false, false);
    if !parse.failed() {
        save(vm, parse.module);
    }
    parse
}

/// `session-terminate`, as `lilypond-all` runs it after each file: each
/// recorded variable given its value again, and the thunks of
/// `call-after-session` run. False if there is no session or it failed.
pub fn end_session(vm: &mut Vm) -> bool {
    vm.eval_text("(@@ (lily) session-terminate)", None)
        .and_then(|end| vm.apply(end, &[]))
        .is_ok()
}

/// `session-save`'s `module-for-each`: each variable of `module` put in
/// front of `lilypond-declarations`, but one recorded already with the
/// same value; and each `define-session` record given its variable's value
/// now, as `session-save` takes the changes the init made.
fn save(vm: &mut Vm, module: Value) {
    let Some(slot) = vm.slot("lily", "lilypond-declarations") else {
        return;
    };
    let lily = vm.slot_module(slot);
    let mut declarations = vm.slot_value(slot).unwrap_or(Value::NULL);
    let mut at = declarations;
    while let Some((record, next)) = vm.heap().car(at).zip(vm.heap().cdr(at)) {
        let heap = vm.heap();
        let flag = heap.cdr(record).and_then(|r| heap.car(r));
        let tail = heap.cdr(record).and_then(|r| heap.cdr(r));
        let owner = tail.and_then(|r| heap.car(r));
        let name = heap
            .car(record)
            .and_then(|n| heap.name(n))
            .map(String::from);
        if let (Some(Value::FALSE), Some(tail), Some(owner), Some(name)) = (flag, tail, owner, name)
            && let Some(value) = vm.module_value(owner, &name)
        {
            vm.heap_mut().set_cdr(tail, value);
        }
        at = next;
    }
    let recorded = records(vm, declarations);
    let mut locals = vm.module_locals(module).unwrap_or_default();
    locals.retain(|&(name, value)| {
        let name = vm.heap().name(name);
        !recorded
            .iter()
            .any(|(n, _, v)| Some(n.as_str()) == name && *v == value)
    });
    let heap = vm.heap_mut();
    for (name, value) in locals {
        let record = heap.cons(module, value);
        let record = heap.cons(Value::TRUE, record);
        let record = heap.cons(name, record);
        declarations = heap.cons(record, declarations);
    }
    vm.module_define(lily, "lilypond-declarations", declarations);
}

/// `session-replay`'s `module-add!`: `module` uses the module of each
/// init file recorded, the newest searched first.
pub(crate) fn replay(vm: &mut Vm, module: Value) {
    let Some(slot) = vm.slot("lily", "lilypond-declarations") else {
        return;
    };
    let declarations = vm.slot_value(slot).unwrap_or(Value::NULL);
    let mut used: Vec<Value> = Vec::new();
    // The list is newest first; the module used last is searched first.
    for (_, owner, _) in records(vm, declarations).into_iter().rev() {
        if !used.contains(&owner) {
            used.push(owner);
            vm.module_use(module, owner);
        }
    }
}

/// The init files' records of `declarations`, newest first: each name,
/// module and value.
fn records(vm: &Vm, declarations: Value) -> Vec<(String, Value, Value)> {
    let mut at = declarations;
    let mut records = Vec::new();
    let heap = vm.heap();
    while let (Some(record), Some(next)) = (heap.car(at), heap.cdr(at)) {
        let parser = heap.cdr(record).and_then(|r| heap.car(r));
        let tail = heap.cdr(record).and_then(|r| heap.cdr(r));
        if parser == Some(Value::TRUE)
            && let (Some(name), Some(owner), Some(value)) = (
                heap.car(record),
                tail.and_then(|r| heap.car(r)),
                tail.and_then(|r| heap.cdr(r)),
            )
            && let Some(name) = heap.name(name)
        {
            records.push((String::from(name), owner, value));
        }
        at = next;
    }
    records
}
