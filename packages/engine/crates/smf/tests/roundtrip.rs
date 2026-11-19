//! The writer against LilyPond's `Midi_walker` (07 §4.2, M1): hand-made staves
//! whose bytes follow from midi-walker.cc and midi-chunk.cc, a merged unison
//! checked against LilyPond itself, and the 42 reference files read back and
//! written again, 39 byte for byte.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_smf::{CENTER, Error, Item, Staff, chunks, events, read, sort_tied_offs, write};

fn note(tick: i32, len: i32, key: u8) -> Item<'static> {
    Item::Note {
        tick,
        len,
        channel: 0,
        key,
        velocity: 90,
        bend: CENTER,
    }
}

fn staff(items: Vec<Item<'static>>, end: i32) -> Staff<'static> {
    Staff {
        items,
        start: 0,
        end,
        merge_unisons: false,
    }
}

/// The body of a one-track file written from `staff`.
fn track(staff: Staff) -> Vec<u8> {
    let mut out = Vec::new();
    write(&[staff], &mut out);
    out[22..].to_vec()
}

#[test]
fn a_file_is_a_header_and_a_track_per_staff() {
    let mut out = Vec::new();
    write(
        &[staff(
            vec![
                Item::Raw {
                    tick: 0,
                    bytes: b"\xff\x03\x01A",
                },
                note(0, 384, 60),
            ],
            384,
        )],
        &mut out,
    );
    let mut expected = b"MThd\0\0\0\x06\0\x01\0\x01\x01\x80MTrk\0\0\0\x12".to_vec();
    expected.extend(b"\0\xff\x03\x01A\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xff\x2f\0");
    assert_eq!(out, expected);
}

#[test]
fn the_end_of_track_waits_for_the_staffs_end_and_its_last_note() {
    assert_eq!(
        track(staff(vec![note(0, 384, 60)], 768)),
        b"\0\x90\x3c\x5a\x83\x00\x90\x3c\0\x83\x00\xff\x2f\0"
    );
    assert_eq!(
        track(staff(vec![note(0, 384, 60)], 0)),
        b"\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xff\x2f\0"
    );
}

#[test]
fn a_note_of_no_tick_is_started_and_stopped_at_its_tick() {
    // As LilyPond writes a note shorter than a tick (MS5 step 8's
    // fixtures/midi-walker.ly): its off, with no delta, before its on.
    assert_eq!(
        track(staff(vec![note(0, 0, 60), note(384, 384, 62)], 0)),
        b"\0\x90\x3c\0\0\x90\x3c\x5a\x83\x00\x90\x3e\x5a\x83\x00\x90\x3e\0\0\xff\x2f\0"
    );
}

#[test]
fn an_item_before_the_tick_of_the_one_ahead_goes_back_in_time() {
    // The items in the caller's order (their moments'), not sorted by tick: a
    // grace note at 362 after a note at 368 is written with no delta and
    // counted, and the next delta taken from 362.
    let mut out = Vec::new();
    let back = write(
        &[staff(
            vec![note(0, 368, 60), note(368, 15, 62), note(362, 27, 64)],
            0,
        )],
        &mut out,
    );
    assert_eq!(back, 1);
    assert_eq!(
        &out[22..],
        b"\0\x90\x3c\x5a\x82\x70\x90\x3c\0\0\x90\x3e\x5a\0\x90\x40\x5a\x15\x90\x3e\0\x06\x90\x40\0\0\xff\x2f\0"
    );
}

#[test]
fn a_unison_begun_together_is_merged_into_one_note() {
    let merged = b"\0\x90\x3c\x5a\x86\x00\x90\x3c\0\0\xff\x2f\0";
    // The longer one second: the first's off is taken over.
    assert_eq!(
        track(staff(vec![note(0, 384, 60), note(0, 768, 60)], 0)),
        merged
    );
    // The shorter one second: it is dropped.
    assert_eq!(
        track(staff(vec![note(0, 768, 60), note(0, 384, 60)], 0)),
        merged
    );
}

#[test]
fn a_unison_begun_later_ends_the_one_that_sounds() {
    // c 0–768, c again at 384 for 192: the first ends at 384 and the second
    // sounds to the later of the two ends.
    assert_eq!(
        track(staff(vec![note(0, 768, 60), note(384, 192, 60)], 0)),
        b"\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xff\x2f\0"
    );
    // With midiMergeUnisons it is merged instead.
    let mut merging = staff(vec![note(0, 384, 60), note(192, 384, 60)], 0);
    merging.merge_unisons = true;
    assert_eq!(
        track(merging),
        b"\0\x90\x3c\x5a\x84\x40\x90\x3c\0\0\xff\x2f\0"
    );
}

#[test]
fn an_event_with_no_delta_goes_before_the_notes_begun_at_its_tick() {
    let program = Item::Raw {
        tick: 0,
        bytes: b"\xc0\x30",
    };
    assert_eq!(
        track(staff(vec![note(0, 384, 60), program], 0)),
        b"\0\xc0\x30\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xff\x2f\0"
    );
    // Later in the track it takes the notes' delta, and they none.
    let program = Item::Raw {
        tick: 384,
        bytes: b"\xc0\x30",
    };
    assert_eq!(
        track(staff(
            vec![note(0, 384, 60), note(384, 384, 62), program],
            0
        )),
        b"\0\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xc0\x30\0\x90\x3e\x5a\x83\x00\x90\x3e\0\0\xff\x2f\0"
    );
}

#[test]
fn a_microtone_sets_the_wheel_with_its_note_and_resets_it_with_its_off() {
    let quarter_sharp = Item::Note {
        tick: 0,
        len: 384,
        channel: 1,
        key: 48,
        velocity: 90,
        bend: 0x2800,
    };
    let body = track(staff(vec![quarter_sharp], 0));
    assert_eq!(
        body,
        b"\0\xe1\x00\x50\0\x91\x30\x5a\x83\x00\x91\x30\0\0\xe1\x00\x40\0\xff\x2f\0"
    );
    let mut file = Vec::new();
    write(&[staff(vec![quarter_sharp], 0)], &mut file);
    assert_eq!(read(&file).unwrap()[0].items, [quarter_sharp]);
}

#[test]
fn a_grace_note_before_the_start_is_written_with_no_delta() {
    // Time never goes back: the note at -96 is written at the start, and its off 192 after it.
    assert_eq!(
        track(staff(vec![note(-96, 192, 60)], 0)),
        b"\0\x90\x3c\x5a\x81\x40\x90\x3c\0\0\xff\x2f\0"
    );
    // A performance that starts with graces starts the track there.
    let mut graces = staff(vec![note(0, 384, 60)], 0);
    graces.start = -96;
    assert_eq!(
        track(graces),
        b"\x60\x90\x3c\x5a\x83\x00\x90\x3c\0\0\xff\x2f\0"
    );
}

#[test]
fn a_file_reads_back_into_its_staves() {
    let name = Item::Raw {
        tick: 0,
        bytes: b"\xff\x03\x01A",
    };
    let staves = [staff(vec![name, note(0, 384, 60), note(192, 384, 64)], 960)];
    let mut file = Vec::new();
    write(&staves, &mut file);
    assert_eq!(read(&file).unwrap(), staves);
    let chunks = chunks(&file).unwrap();
    assert_eq!(
        chunks.iter().map(|c| &c.tag).collect::<Vec<_>>(),
        [b"MThd", b"MTrk"]
    );
    let ticks: Vec<u32> = events(chunks[1].body())
        .unwrap()
        .iter()
        .map(|e| e.tick)
        .collect();
    assert_eq!(ticks, [0, 0, 192, 384, 576, 960]);
}

#[test]
fn a_file_that_is_not_midi_or_is_cut_short_is_an_error() {
    assert_eq!(
        read(b"RIFF\0\0\0\x06\0\x01\0\x01\x01\x80"),
        Err(Error::NotMidi)
    );
    let mut file = Vec::new();
    write(&[staff(vec![note(0, 384, 60)], 0)], &mut file);
    assert_eq!(read(&file[..file.len() - 1]), Err(Error::Truncated));
    // An event without its status byte.
    let running = b"MThd\0\0\0\x06\0\x01\0\x01\x01\x80MTrk\0\0\0\x08\0\x90\x3c\x5a\x10\x3c\0\0";
    assert_eq!(read(running), Err(Error::RunningStatus));
}

#[test]
fn only_note_offs_at_one_tick_are_sorted() {
    let offs = |a: u8, b: u8| -> Vec<u8> {
        let mut file =
            b"MThd\0\0\0\x06\0\x01\0\x01\x01\x80MTrk\0\0\0\x0c\0\x90\0\0\0\x90\0\0\0\xff\x2f\0"
                .to_vec();
        (file[24], file[28]) = (a, b);
        file
    };
    assert_eq!(sort_tied_offs(&offs(0x43, 0x40)).unwrap(), offs(0x40, 0x43));
    // Not at one tick: left as it is.
    let mut apart = offs(0x43, 0x40);
    apart[26] = 1;
    assert_eq!(sort_tied_offs(&apart).unwrap(), apart);
}

/// `<< c'4 c'2 e'4 g'4 >>` in one voice: c'2 merges into c'4, which leaves
/// c'4's off in LilyPond's heap, ignored. That entry changes the heap's shape,
/// and g's off is popped before e's at 384. Read back from the file the merge
/// is gone, and the writer puts e's first: 07 §4.2's three mismatches.
const UNISON: [Item<'static>; 4] = [
    Item::Note {
        tick: 0,
        len: 384,
        channel: 0,
        key: 60,
        velocity: 90,
        bend: CENTER,
    },
    Item::Note {
        tick: 0,
        len: 768,
        channel: 0,
        key: 60,
        velocity: 90,
        bend: CENTER,
    },
    Item::Note {
        tick: 0,
        len: 384,
        channel: 0,
        key: 64,
        velocity: 90,
        bend: CENTER,
    },
    Item::Note {
        tick: 0,
        len: 384,
        channel: 0,
        key: 67,
        velocity: 90,
        bend: CENTER,
    },
];
const UNISON_TRACK: &[u8] = b"\0\x90\x3c\x5a\0\x90\x40\x5a\0\x90\x43\x5a\x83\x00\x90\x43\0\0\x90\x40\0\x83\x00\x90\x3c\0\0\xff\x2f\0";

#[test]
fn a_merged_unison_leaves_its_mark_on_the_order_of_note_offs() {
    let body = track(staff(UNISON.to_vec(), 0));
    assert_eq!(body, UNISON_TRACK);
    let mut file = Vec::new();
    write(&[staff(UNISON.to_vec(), 0)], &mut file);
    let mut again = Vec::new();
    write(&read(&file).unwrap(), &mut again);
    assert_ne!(again, file, "without the merged note, e's off comes first");
    assert_eq!(
        sort_tied_offs(&again).unwrap(),
        sort_tied_offs(&file).unwrap()
    );
}

/// The same staff as LilyPond writes it, when lilypond is installed.
#[test]
fn a_merged_unison_is_written_as_lilypond_writes_it() {
    if Command::new("lilypond").arg("--version").output().is_err() {
        eprintln!("skipped: lilypond is not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("lily-engine-smf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ly = dir.join("unison.ly");
    std::fs::write(&ly, "\\version \"2.26.0\"\n\\score { \\new Staff \\new Voice << c'4 c'2 e'4 g'4 >> \\midi { } }\n").unwrap();
    let status = Command::new("lilypond")
        .arg("-s")
        .arg("-o")
        .arg(dir.join("unison"))
        .arg(&ly)
        .status()
        .unwrap();
    assert!(status.success(), "lilypond failed on {}", ly.display());
    let theirs = std::fs::read(dir.join("unison.midi")).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    // LilyPond's own control track, and the staff from the voice's four notes.
    let mut staves = read(&theirs).unwrap();
    assert_eq!(staves.len(), 2);
    staves[1].items = UNISON.to_vec();
    let mut ours = Vec::new();
    write(&staves, &mut ours);
    assert_eq!(ours, theirs);
    assert!(theirs.ends_with(UNISON_TRACK));
}

fn corpus() -> Option<PathBuf> {
    let root = std::env::var_os("LILY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache/lily-engine")
        });
    let reference = root.join("ref");
    reference.is_dir().then_some(reference)
}

/// Every reference capture read back and written again: 39 of 42 byte for
/// byte, and the three whose merged unisons the file cannot tell equal once
/// the note-offs at one tick are sorted (07 §4.2).
#[test]
fn the_reference_files_are_written_again_39_of_42_byte_for_byte() {
    let Some(reference) = corpus() else {
        eprintln!("skipped: no captures in $LILY_CORPUS/ref (npm run corpus && npm run capture)");
        return;
    };
    // The corpus' scores, T01 to T30 and M01 to M12: not the captures of
    // tools/compare's own files (grob-calls).
    let score = |name: &str| {
        name.len() == 3
            && (name.starts_with('T') || name.starts_with('M'))
            && name[1..].bytes().all(|b| b.is_ascii_digit())
    };
    let mut ids: Vec<_> = std::fs::read_dir(&reference)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(score))
        .collect();
    ids.sort();
    let (mut same, mut ties) = (Vec::new(), Vec::new());
    for dir in &ids {
        let id = dir.file_name().unwrap().to_string_lossy().into_owned();
        for file in std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()) {
            if file.extension().is_none_or(|e| e != "midi") {
                continue;
            }
            let theirs = std::fs::read(&file).unwrap();
            let mut ours = Vec::new();
            write(&read(&theirs).unwrap(), &mut ours);
            if ours == theirs {
                same.push(id.clone());
            } else {
                assert_eq!(
                    sort_tied_offs(&ours).unwrap(),
                    sort_tied_offs(&theirs).unwrap(),
                    "{} differs",
                    file.display()
                );
                ties.push(id.clone());
            }
        }
    }
    assert_eq!(same.len() + ties.len(), 42, "the 42 captures");
    assert_eq!(ties, ["T24", "T25", "T27"]);
}
