//! The figured bass crate (MS4 step 16, D223): its classes shadow the
//! music crate's placeholders of their names, with LilyPond 2.26.0's
//! events, acknowledgers and properties; and its primitives replace the
//! music crate's placeholders in `(lily)` under the same names, each
//! checking its grob as LilyPond's subr does where the placeholder threw
//! `misc-error`. The figures themselves are the tools' tests'
//! (grobs.test.mjs, layout.test.mjs: fixtures/figures.ly, T06 and T21).
use lily_engine_scheme::{Value, Vm};

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

fn show(vm: &Vm, v: Value) -> String {
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

#[test]
fn the_classes_shadow_the_placeholders_of_their_names() {
    for class in lily_engine_figured_bass::CLASSES {
        let placeholder = lily_engine_music::CLASSES
            .iter()
            .find(|c| c.name == class.name)
            .unwrap_or_else(|| panic!("{}: no placeholder", class.name));
        assert!(placeholder.make.is_none(), "{}", class.name);
        assert!(class.make.is_some(), "{}", class.name);
    }
    let [engraver, position] = lily_engine_figured_bass::CLASSES else {
        panic!("two classes");
    };
    assert_eq!(engraver.name, "Figured_bass_engraver");
    assert_eq!(engraver.hears, ["rest-event", "bass-figure-event"]);
    assert_eq!(
        engraver.reads,
        [
            "figuredBassAlterationDirection",
            "figuredBassCenterContinuations",
            "figuredBassFormatter",
            "implicitBassFigures",
            "useBassFigureExtenders",
            "ignoreFiguredBassRest",
        ]
    );
    assert_eq!(position.name, "Figured_bass_position_engraver");
    assert_eq!(
        position.acknowledges,
        [
            ("note-column-interface", false),
            ("slur-interface", false),
            ("stem-interface", false),
            ("slur-interface", true),
            ("tie-interface", true),
            ("bass-figure-alignment-interface", false),
            ("bass-figure-alignment-interface", true),
        ]
    );
}

#[test]
fn the_primitives_replace_the_placeholders() {
    let calls = [
        "(ly:enclosing-bracket::print 1)",
        "(ly:enclosing-bracket::width 1)",
        "(ly:figured-bass-continuation::center-on-figures 1)",
    ];
    let caught = |vm: &mut Vm, call: &str| {
        let text = format!(
            "(catch #t (lambda () {call}) (lambda (key subr message args rest) (list key subr)))"
        );
        let v = eval(vm, &text);
        show(vm, v)
    };

    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    eval(&mut vm, "(set-current-module (resolve-module '(lily)))");
    for call in calls {
        let got = caught(&mut vm, call);
        assert!(got.starts_with("(misc-error "), "{call}: {got}");
    }

    lily_engine_figured_bass::install(&mut vm).unwrap();
    for call in calls {
        let name = &call[1..call.find(' ').unwrap()];
        assert_eq!(
            caught(&mut vm, call),
            format!("(wrong-type-arg \"{name}\")"),
            "{call}"
        );
    }
}
