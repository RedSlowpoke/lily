//! The tablature crate (MS4 steps 7a and 17, D223): its classes shadow the
//! music crate's placeholders of their names, with LilyPond 2.26.0's
//! events, acknowledgers and properties. The heads, the staff symbols and
//! the fret diagrams themselves are the tools' tests' (grobs.test.mjs,
//! layout.test.mjs: fixtures/tab-note-heads.ly, fixtures/tablature.ly and
//! T22).

#[test]
fn the_classes_shadow_the_placeholders_of_their_names() {
    let names: Vec<&str> = lily_engine_tablature::CLASSES
        .iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        names,
        [
            "Tab_note_heads_engraver",
            "Tab_staff_symbol_engraver",
            "Fretboard_engraver"
        ]
    );
    for class in lily_engine_tablature::CLASSES {
        let placeholder = lily_engine_music::CLASSES
            .iter()
            .find(|c| c.name == class.name)
            .unwrap_or_else(|| panic!("{}: no placeholder", class.name));
        assert!(placeholder.make.is_none(), "{}", class.name);
        assert!(class.make.is_some(), "{}", class.name);
    }
    let fretboard = lily_engine_tablature::CLASSES[2];
    assert_eq!(
        fretboard.hears,
        ["note-event", "string-number-event", "fingering-event"]
    );
    assert_eq!(
        fretboard.reads,
        [
            "chordChanges",
            "defaultStrings",
            "highStringOne",
            "maximumFretStretch",
            "minimumFret",
            "noteToFretFunction",
            "predefinedDiagramTable",
            "stringTunings",
            "tablatureFormat",
        ]
    );
}
