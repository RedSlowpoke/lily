//! The chord names crate (MS4 step 17, D223): its primitive replaces the
//! music crate's placeholder in `(lily)` under the same name, checking its
//! grob as LilyPond's subr does where the placeholder threw `misc-error`.
//! The chord names themselves are the tools' tests'
//! (fixtures/tablature.ly's ChordNames and FretBoards with `chordChanges`).
use lily_engine_scheme::{Value, Vm};

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

fn caught(vm: &mut Vm, call: &str) -> String {
    let text = format!(
        "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr)))"
    );
    let v = eval(vm, &text);
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

#[test]
fn the_primitive_replaces_the_placeholder() {
    let call = "(ly:chord-name::after-line-breaking 1)";
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    eval(&mut vm, "(set-current-module (resolve-module '(lily)))");
    assert!(caught(&mut vm, call).starts_with("(misc-error "));

    lily_engine_chord_names::install(&mut vm).unwrap();
    assert_eq!(
        caught(&mut vm, call),
        "(wrong-type-arg \"ly:chord-name::after-line-breaking\")"
    );
}
