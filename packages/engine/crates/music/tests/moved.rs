//! N8's resume after an edit that moves bytes (MS6 step 7, D244): a place
//! in the text before the edit has its place in the text after it, with
//! the counts `Source::counts` gives (tabs to the next multiple of 8,
//! UTF-8's continuation bytes not counted); the links the old run wrote
//! and what it said moved with them, or refused where a message is on a
//! line the edit changed.

use std::rc::Rc;

use lily_engine_music::{Input, Moved};

const OLD: &str = "\\header { title = \"Sample\" }\n{ c4 d e f |\n\tg a b c }\n";

fn input(start: u32, end: u32, counts: [u32; 3]) -> Input {
    Input::new(0, start, end, Rc::from("dir/score.ly"), counts)
}

#[test]
fn the_same_text_is_no_edit() {
    assert!(Moved::new("dir/score.ly", OLD.as_bytes(), OLD.as_bytes()).is_none());
}

#[test]
fn a_place_before_the_edit_stays_and_one_after_it_moves_with_its_counts() {
    // `d ` inserted before `e` on line 2: `g` on line 3 moves two bytes, its
    // line and columns stay; `f` on line 2 moves two characters and columns.
    let new = OLD.replace("d e", "d d e");
    let m = Moved::new("dir/score.ly", OLD.as_bytes(), new.as_bytes()).unwrap();
    let c = OLD.find("c4").unwrap() as u32;
    let mut before = input(c, c + 2, [2, 2, 2]);
    m.input(&mut before);
    assert_eq!(
        (before.start, before.line, before.char, before.column),
        (c, 2, 2, 2)
    );
    let f = OLD.find("f |").unwrap() as u32;
    let mut on_its_line = input(f, f + 1, [2, 9, 9]);
    m.input(&mut on_its_line);
    assert_eq!(
        (
            on_its_line.start,
            on_its_line.line,
            on_its_line.char,
            on_its_line.column
        ),
        (f + 2, 2, 11, 11)
    );
    let g = OLD.find("g a").unwrap() as u32;
    let mut after = input(g, g + 1, [3, 1, 8]);
    m.input(&mut after);
    assert_eq!(
        (after.start, after.line, after.char, after.column),
        (g + 2, 3, 1, 8)
    );
    // Another file's places are not this one's.
    let mut other = Input::new(0, g, g + 1, Rc::from("other.ly"), [3, 1, 8]);
    m.input(&mut other);
    assert_eq!(other.start, g);
}

#[test]
fn a_line_inserted_moves_the_lines_after_it() {
    let new = OLD.replace("{ c4", "\n{ c4");
    let m = Moved::new("dir/score.ly", OLD.as_bytes(), new.as_bytes()).unwrap();
    let g = OLD.find("g a").unwrap() as u32;
    let mut after = input(g, g + 1, [3, 1, 8]);
    m.input(&mut after);
    assert_eq!((after.line, after.char, after.column), (4, 1, 8));
}

#[test]
fn links_move_as_the_trace_and_the_playback_map_write_them() {
    let new = OLD.replace("d e", "d d e");
    let m = Moved::new("dir/score.ly", OLD.as_bytes(), new.as_bytes()).unwrap();
    // The trace's (`file:line:char:column`, the file's name alone) and the
    // playback map's (the column from 1), and a place before the edit.
    let text = "NoteHead\tscore.ly:2:8:8\t1 0\n{\"href\":\"textedit:///dir/score.ly:3:1:9\"}\nscore.ly:2:2:2\n";
    assert_eq!(
        m.links(text),
        "NoteHead\tscore.ly:2:10:10\t1 0\n{\"href\":\"textedit:///dir/score.ly:3:1:9\"}\nscore.ly:2:2:2\n"
    );
    // Not a link: left as it is.
    assert_eq!(
        m.links("score.ly:x and score.ly:2:"),
        "score.ly:x and score.ly:2:"
    );
}

#[test]
fn what_was_said_moves_its_lines_or_refuses_one_the_edit_changed() {
    let new = OLD.replace("{ c4", "\n{ c4");
    let m = Moved::new("dir/score.ly", OLD.as_bytes(), new.as_bytes()).unwrap();
    // Line 1 is before the edit, line 3 after it.
    assert_eq!(
        m.said("dir/score.ly:1:3: warning: one\ndir/score.ly:3:2: warning: two\n")
            .as_deref(),
        Some("dir/score.ly:1:3: warning: one\ndir/score.ly:4:2: warning: two\n")
    );
    // A message on the edited line quotes it as it was.
    assert_eq!(m.said("dir/score.ly:2:1: warning: here\n"), None);
}

#[test]
fn an_edit_in_two_places_moves_what_is_between_them_with_the_first() {
    // A comment line added above and a note changed below (OPEN B31, D268):
    // `g` on line 3, between them, moves a line down with its bytes; a place
    // in the second change stays at its start; a message on line 3 moves.
    let new = OLD
        .replace("\\header", "% one more line\n\\header")
        .replace("b c }", "b d }");
    let m = Moved::new("dir/score.ly", OLD.as_bytes(), new.as_bytes()).unwrap();
    assert!(m.exact());
    let g = OLD.find("g a").unwrap() as u32;
    let mut between = input(g, g + 1, [3, 1, 8]);
    m.input(&mut between);
    assert_eq!(
        (between.start, between.line, between.char, between.column),
        (g + 16, 4, 1, 8)
    );
    let c = OLD.find("c }").unwrap() as u32;
    let mut changed = input(c, c + 1, [3, 7, 14]);
    m.input(&mut changed);
    assert_eq!((changed.start, changed.line), (c + 16, 4));
    assert_eq!(
        m.links("score.ly:2:4:4\nscore.ly:3:1:8\n"),
        "score.ly:3:4:4\nscore.ly:4:1:8\n"
    );
    assert_eq!(
        m.said("dir/score.ly:2:4: warning: two\n").as_deref(),
        Some("dir/score.ly:3:4: warning: two\n")
    );
    // The second change's line quotes it as it was.
    assert_eq!(m.said("dir/score.ly:3:7: warning: three\n"), None);
}

#[test]
fn an_edit_of_too_many_lines_in_places_is_not_followed() {
    // Every other line of many changed: the lines between are not looked for,
    // and a resume is refused (`exact`).
    let old: String = (0..600).map(|i| format!("c{i}\n")).collect();
    let new: String = (0..600)
        .map(|i| match i % 2 {
            0 => format!("d{i}\n"),
            _ => format!("c{i}\n"),
        })
        .collect();
    let m = Moved::new("dir/score.ly", old.as_bytes(), new.as_bytes()).unwrap();
    assert!(!m.exact());
    // A whole block pasted is followed: nothing of the old text is in it.
    let pasted = old.replace("c300\n", &"e\n".repeat(400));
    assert!(
        Moved::new("dir/score.ly", old.as_bytes(), pasted.as_bytes())
            .unwrap()
            .exact()
    );
}

#[test]
fn two_edits_on_one_line_move_what_is_between_them() {
    // A space taken out before `e'1` and one put in after it (OPEN B31): `e'1`
    // moves a byte back, its line the same (where its end goes between two
    // spaces either alignment gives).
    let old = "{ d'1  e'1 f'1 }\n";
    let new = "{ d'1 e'1  f'1 }\n";
    let m = Moved::new("dir/score.ly", old.as_bytes(), new.as_bytes()).unwrap();
    assert!(m.exact());
    let e = old.find("e'1").unwrap() as u32;
    let mut between = input(e, e + 3, [1, 7, 7]);
    m.input(&mut between);
    assert_eq!(
        (between.start, between.line, between.char, between.column),
        (e - 1, 1, 6, 6)
    );
    assert_eq!(m.links("score.ly:1:7:7\n"), "score.ly:1:6:6\n");
}
