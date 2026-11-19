//! `gp345` on files built here byte by byte, for the layouts no sample has
//! (README §1: chord diagrams, alternate endings, GP5 repeats and tempo
//! changes, trills, tremolo picking, slaps, pick strokes, capos, 7-string
//! tracks) and for damaged files.
use super::*;

/// A file being built, little-endian as GP writes.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn i8(&mut self, v: i8) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn i16(&mut self, v: i16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn raw(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
    fn zeros(&mut self, n: usize) -> &mut Self {
        self.0.resize(self.0.len() + n, 0);
        self
    }
    /// `ByteString(size)`.
    fn byte_string(&mut self, text: &[u8], size: usize) -> &mut Self {
        self.u8(text.len() as u8).raw(text).zeros(size - text.len())
    }
    /// `IntByteString`.
    fn ibs(&mut self, text: &[u8]) -> &mut Self {
        self.i32(text.len() as i32 + 1)
            .u8(text.len() as u8)
            .raw(text)
    }
    /// `IntString`.
    fn int_string(&mut self, text: &[u8]) -> &mut Self {
        self.i32(text.len() as i32).raw(text)
    }
}

/// What the file's head says; the rest is GP's defaults.
struct Head {
    major: u8,
    minor: u8,
    title: Vec<u8>,
    tempo: i32,
    /// Each channel's program.
    programs: [i32; 64],
    directions: [i16; 19],
    lyrics: (i32, Vec<(i32, &'static str)>),
}

impl Head {
    fn new(major: u8, minor: u8) -> Head {
        Head {
            major,
            minor,
            title: b"Song".to_vec(),
            tempo: 120,
            programs: [25; 64],
            directions: [-1; 19],
            lyrics: (0, Vec::new()),
        }
    }

    fn version(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
        }
    }

    fn v510(&self) -> bool {
        self.major == 5 && self.minor >= 10
    }

    /// The bytes up to and with the counts of bars and tracks.
    fn write(&self, bars: i32, tracks: i32) -> Bytes {
        let gp = self.major;
        let mut b = Bytes::default();
        let header = format!("FICHIER GUITAR PRO v{}.{:02}", self.major, self.minor);
        b.byte_string(header.as_bytes(), 30);
        b.ibs(&self.title);
        for _ in 0..(if gp == 5 { 8 } else { 7 }) {
            b.ibs(b"");
        }
        b.i32(1).ibs(b"notice");
        if gp < 5 {
            b.u8(0);
        }
        if gp >= 4 {
            b.i32(self.lyrics.0);
            for i in 0..5 {
                match self.lyrics.1.get(i) {
                    Some((bar, text)) => b.i32(*bar).int_string(text.as_bytes()),
                    None => b.i32(1).int_string(b""),
                };
            }
        }
        if self.v510() {
            b.i32(100).i32(0).zeros(11);
        }
        if gp == 5 {
            for v in [210, 297, 10, 10, 15, 10, 100] {
                b.i32(v);
            }
            b.i16(0x1FF);
            for _ in 0..10 {
                b.ibs(b"%TITLE%");
            }
            b.ibs(b"Moderate");
        }
        b.i32(self.tempo);
        if self.v510() {
            b.u8(0);
        }
        if gp == 3 {
            b.i32(0);
        } else {
            b.i8(0).zeros(4);
        }
        for program in self.programs {
            b.i32(program).raw(&[13, 8, 0, 0, 0, 0]).zeros(2);
        }
        if gp == 5 {
            for d in self.directions {
                b.i16(d);
            }
            b.i32(0);
        }
        b.i32(bars).i32(tracks);
        b
    }

    /// A master bar with `flags` and its data in the file's order, given as
    /// `data` for GP3/GP4 or GP5's fields before the beaming.
    fn bar(&self, b: &mut Bytes, index: usize, flags: u8, data: &[u8]) {
        if self.major == 5 && index > 0 {
            b.u8(0);
        }
        b.u8(flags).raw(data);
        if self.major == 5 {
            if flags & 0x03 != 0 {
                b.raw(&[2, 2, 2, 2]);
            }
            if flags & 0x10 == 0 {
                b.u8(0);
            }
            b.u8(0);
        }
    }

    /// A track of `tuning` on 1-based `channel`.
    #[allow(clippy::too_many_arguments)]
    fn track(
        &self,
        b: &mut Bytes,
        index: usize,
        flags: u8,
        name: &[u8],
        tuning: &[i32],
        channel: i32,
        capo: i32,
    ) {
        if self.major == 5 && (index == 0 || !self.v510()) {
            b.u8(0);
        }
        b.u8(flags).byte_string(name, 40).i32(tuning.len() as i32);
        for s in 0..7 {
            b.i32(tuning.get(s).copied().unwrap_or(0));
        }
        b.i32(1)
            .i32(channel)
            .i32(channel + 1)
            .i32(24)
            .i32(capo)
            .zeros(4);
        if self.major == 5 {
            b.i16(0).u8(0).u8(3).u8(0).zeros(24).i32(-1).i32(-1).i32(-1);
            if self.v510() {
                b.i32(-1).zeros(4).ibs(b"").ibs(b"");
            } else {
                b.i16(-1).u8(0);
            }
        }
    }

    /// The blank bytes after the tracks.
    fn after_tracks(&self, b: &mut Bytes) {
        if self.major == 5 {
            b.zeros(if self.v510() { 1 } else { 2 });
        }
    }

    /// A track's bar of `beats` (each written whole by `beat`); GP5 gets
    /// an empty second voice, and a line break unless `last`.
    fn measure(&self, b: &mut Bytes, beats: &[Vec<u8>], last: bool) {
        b.i32(beats.len() as i32);
        for beat in beats {
            b.raw(beat);
        }
        if self.major == 5 {
            b.i32(0);
            if !last {
                b.u8(0);
            }
        }
    }

    /// A beat: `flags` and `duration`, then `before` (the status when
    /// 0x40 is set, which goes before the duration, then the tuplet,
    /// chord, text, effects and mix table its flags ask for), then notes.
    fn beat(&self, flags: u8, duration: i8, before: &[u8], notes: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut b = Bytes::default();
        b.u8(flags);
        let before = match before.split_first() {
            Some((status, rest)) if flags & 0x40 != 0 => {
                b.u8(*status);
                rest
            }
            _ => before,
        };
        b.i8(duration).raw(before);
        let mask = notes.iter().fold(0u8, |m, (s, _)| m | (0x80 >> s));
        b.u8(mask);
        let mut notes: Vec<_> = notes.iter().collect();
        notes.sort_by_key(|(s, _)| *s);
        for (_, note) in notes {
            b.raw(note);
        }
        if self.major == 5 {
            b.i16(0);
        }
        b.0
    }

    /// A note at `fret`: `flags` beyond the type and fret (0x04, 0x40,
    /// 0x02 carry no data), and its effects when given.
    fn note(&self, kind: u8, fret: i8, flags: u8, effects: Option<&[u8]>) -> Vec<u8> {
        let mut b = Bytes::default();
        let flags = flags | 0x20 | if effects.is_some() { 0x08 } else { 0 };
        b.u8(flags).u8(kind).i8(fret);
        if self.major == 5 {
            b.u8(0);
        }
        if let Some(effects) = effects {
            b.raw(effects);
        }
        b.0
    }

    /// A song of one 4/4 bar and one standard guitar track, whose bar is
    /// `beats`.
    fn one_bar(&self, beats: &[Vec<u8>]) -> Vec<u8> {
        let mut b = self.write(1, 1);
        self.bar(&mut b, 0, 0x03, &[4, 4]);
        self.track(&mut b, 0, 0, b"Guitar", &[64, 59, 55, 50, 45, 40], 1, 0);
        self.after_tracks(&mut b);
        self.measure(&mut b, beats, true);
        if self.major < 5 {
            b.zeros(4);
        }
        b.0
    }

    fn read(&self, bytes: &[u8]) -> Song {
        let (song, end) = read_with_end(bytes, self.version()).unwrap();
        let trailer = if self.major < 5 { 4 } else { 0 };
        assert_eq!(
            end + trailer,
            bytes.len(),
            "the walk ends at the file's end"
        );
        song
    }
}

const ALL: [(u8, u8); 4] = [(3, 0), (4, 0), (5, 0), (5, 10)];

fn quarter_rest(head: &Head) -> Vec<u8> {
    head.beat(0x40, 0, &[2], &[])
}

#[test]
fn reads_a_minimal_file_of_each_layout() {
    for (major, minor) in ALL {
        let head = Head::new(major, minor);
        let notes = [(6, head.note(1, 3, 0, None)), (1, head.note(1, 0, 0, None))];
        let bytes = head.one_bar(&[
            head.beat(0, -1, &[], &notes),
            quarter_rest(&head),
            head.beat(0x01, 1, &[], &[(2, head.note(1, 1, 0, None))]),
            head.beat(0x40, 2, &[0], &[]),
        ]);
        let song = head.read(&bytes);
        assert_eq!(
            song.version,
            format!("FICHIER GUITAR PRO v{major}.{minor:02}")
        );
        assert_eq!(song.info.title, "Song");
        assert_eq!(song.info.notices, ["notice"]);
        assert_eq!(song.tempo, 120.0);
        assert_eq!(song.tempo_name, if major == 5 { "Moderate" } else { "" });
        let track = &song.tracks[0];
        assert_eq!(track.tuning, [64, 59, 55, 50, 45, 40]);
        assert_eq!(track.kind, TrackKind::Strings);
        assert_eq!((track.midi.channel, track.midi.effect_channel), (0, 1));
        assert_eq!((track.midi.volume, track.midi.pan), (104, 64));
        let bar = &track.bars[0];
        assert_eq!(bar.voices.len(), if major == 5 { 2 } else { 1 });
        let beats = &bar.voices[0].beats;
        let strings: Vec<_> = beats[0].notes.iter().map(|n| (n.string, n.fret)).collect();
        assert_eq!(strings, [(1, 0), (6, 3)]);
        assert_eq!(beats[0].duration.value, 2);
        assert_eq!(beats[1].status, BeatStatus::Rest);
        assert_eq!((beats[2].duration.value, beats[2].duration.dots), (8, 1));
        assert_eq!(beats[3].status, BeatStatus::Empty);
        assert_eq!(beats[3].duration.value, 16);
    }
}

#[test]
fn every_prefix_of_a_file_fails_cleanly() {
    for (major, minor) in ALL {
        let head = Head::new(major, minor);
        let note = head.note(1, 5, 0, None);
        let bytes = head.one_bar(&[head.beat(0, -2, &[], &[(3, note)])]);
        let end = bytes.len() - if major < 5 { 4 } else { 0 };
        for len in 0..end {
            match read(&bytes[..len], head.version()) {
                Err(Error::Truncated { offset, .. }) => assert!(offset <= len),
                other => panic!("v{major}.{minor}, {len} bytes: {other:?}"),
            }
        }
    }
}

#[test]
fn damaged_bytes_never_panic() {
    let head = Head::new(5, 10);
    let note = head.note(1, 5, 0, Some(&[0x01, 0x00, 1, 0, 0, 0, 0, 2, 0, 0, 0]));
    let bytes = head.one_bar(&[head.beat(0, 0, &[], &[(3, note)]), quarter_rest(&head)]);
    let mut state = 0x2545_F491u32;
    for _ in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut damaged = bytes.clone();
        let at = state as usize % damaged.len();
        damaged[at] = (state >> 8) as u8;
        let _ = read(&damaged, head.version());
    }
}

#[test]
fn refuses_impossible_values_with_their_offset() {
    let head = Head::new(4, 0);
    let mut bytes = head.one_bar(&[head.beat(0, 7, &[], &[])]);
    let at = bytes.len() - 4 - 2; // the duration, before the strings byte
    assert_eq!(bytes[at], 7);
    match read(&bytes, head.version()) {
        Err(Error::Invalid { offset, what }) => {
            assert_eq!(offset, at);
            assert!(what.contains("duration"), "{what}");
        }
        other => panic!("{other:?}"),
    }
    // A negative number of beats.
    let count = bytes.len() - 4 - 3 - 4;
    bytes[count..count + 4].copy_from_slice(&(-1i32).to_le_bytes());
    assert!(matches!(
        read(&bytes, head.version()),
        Err(Error::Invalid { offset, .. }) if offset == count
    ));
}

#[test]
fn reads_text_in_windows_1251() {
    let mut head = Head::new(3, 0);
    head.title = vec![0xCA, 0xF3, 0xEF, 0xEB, 0xE5, 0xF2];
    let song = head.read(&head.one_bar(&[head.beat(0, -2, &[], &[])]));
    assert_eq!(song.info.title, "Куплет");
}

#[test]
fn reads_repeats_and_alternate_endings() {
    // GP3: a repeat count of 2 is 3 plays; an ending by its number.
    let head = Head::new(3, 0);
    let mut b = head.write(3, 0);
    head.bar(&mut b, 0, 0x03 | 0x04, &[3, 4]);
    head.bar(&mut b, 1, 0x08 | 0x10, &[2, 1]);
    head.bar(&mut b, 2, 0x10 | 0x40 | 0x80, &[2, 0xFF, 1]);
    let song = head.read(&[b.0.as_slice(), &[0; 4]].concat());
    let bars = &song.master_bars;
    assert!(bars[0].repeat_start);
    assert_eq!(
        bars[1].time,
        TimeSignature {
            numerator: 3,
            denominator: 4
        }
    );
    assert_eq!(bars[1].repeat_end, Some(3));
    assert_eq!(bars[1].alternate_endings, [1]);
    assert_eq!(bars[2].alternate_endings, [2]);
    assert_eq!(
        bars[2].key,
        KeySignature {
            fifths: -1,
            minor: true
        }
    );
    assert!(bars[2].double_bar);

    // GP5: the plays as stored, endings as bits, the key before the ending.
    for minor in [0, 10] {
        let head = Head::new(5, minor);
        let mut b = head.write(2, 0);
        head.bar(&mut b, 0, 0x03 | 0x04, &[6, 8]);
        // repeat end, marker, key, then the endings' bits (1st and 3rd).
        let mut data = Bytes::default();
        data.i8(3)
            .ibs(b"Coda")
            .raw(&[255, 0, 0, 0])
            .i8(2)
            .i8(0)
            .u8(0b101);
        head.bar(&mut b, 1, 0x08 | 0x10 | 0x20 | 0x40, &data.0);
        head.after_tracks(&mut b);
        let song = head.read(&b.0);
        let bars = &song.master_bars;
        assert_eq!(
            bars[0].time,
            TimeSignature {
                numerator: 6,
                denominator: 8
            }
        );
        assert_eq!(bars[1].time, bars[0].time, "a meter not given is the last");
        assert_eq!(bars[1].repeat_end, Some(3));
        assert_eq!(bars[1].marker.as_deref(), Some("Coda"));
        assert_eq!(bars[1].key.fifths, 2);
        assert_eq!(bars[1].alternate_endings, [1, 3]);
    }
}

#[test]
fn reads_gp5_directions_and_triplet_feel() {
    let mut head = Head::new(5, 10);
    head.directions[2] = 1; // Segno at bar 1
    head.directions[10] = 2; // Da Segno al Coda at bar 2
    head.directions[0] = 2; // Coda at bar 2
    let mut b = head.write(2, 0);
    // A bar with an eighth-note triplet feel: flags, blank, feel.
    b.u8(0).u8(0).u8(1);
    b.u8(0).u8(0).u8(0).u8(2);
    head.after_tracks(&mut b);
    let song = head.read(&b.0);
    assert_eq!(song.master_bars[0].directions, [Direction::Segno]);
    assert_eq!(song.master_bars[0].triplet_feel, TripletFeel::Eighth);
    assert_eq!(
        song.master_bars[1].directions,
        [Direction::Coda, Direction::DalSegnoAlCoda]
    );
    assert_eq!(song.master_bars[1].triplet_feel, TripletFeel::Sixteenth);
}

/// A tempo-only mix table (README §2.17) for `head`'s layout.
fn tempo_mix(head: &Head, bpm: i32) -> Vec<u8> {
    let mut b = Bytes::default();
    b.i8(-1);
    if head.major == 5 {
        b.zeros(16);
    }
    b.raw(&[0xFF; 6]);
    if head.major == 5 {
        b.ibs(b"Fast");
    }
    b.i32(bpm).u8(0);
    if head.v510() {
        b.u8(0);
    }
    if head.major >= 4 {
        b.u8(1);
    }
    if head.major == 5 {
        b.i8(-1);
        if head.v510() {
            b.ibs(b"").ibs(b"");
        }
    }
    b.0
}

#[test]
fn reads_tempo_changes_at_their_beat() {
    for (major, minor) in ALL {
        let head = Head::new(major, minor);
        let triplet = 3i32.to_le_bytes();
        let bytes = head.one_bar(&[
            head.beat(0x10, 0, &tempo_mix(&head, 90), &[]),
            head.beat(0x20, 1, &triplet, &[]),
            head.beat(0x20, 1, &triplet, &[]),
            head.beat(
                0x20 | 0x10,
                1,
                &[triplet.as_slice(), &tempo_mix(&head, 140)].concat(),
                &[],
            ),
            head.beat(0, -1, &[], &[]),
        ]);
        let song = head.read(&bytes);
        assert_eq!(
            song.tempo, 90.0,
            "v{major}.{minor}: a change on the first beat is the start's"
        );
        assert_eq!(
            song.tempo_changes,
            [TempoChange {
                bar: 0,
                offset: Fraction::new(5, 12),
                bpm: 140.0
            }],
            "v{major}.{minor}"
        );
        let beats = &song.tracks[0].bars[0].voices[0].beats;
        assert_eq!(
            beats[1].duration.tuplet,
            Some(Tuplet { notes: 3, space: 2 })
        );
    }
}

#[test]
fn reads_chord_diagrams() {
    // The old form of GP3 and GP4: a name, a first fret, six frets.
    let mut old = Bytes::default();
    old.u8(0).ibs(b"C").i32(1);
    for f in [0, 1, 0, 2, 3, -1] {
        old.i32(f);
    }
    // GP3's new form.
    let mut gp3 = Bytes::default();
    gp3.u8(1)
        .u8(1)
        .zeros(3)
        .i32(0)
        .i32(0)
        .i32(0)
        .i32(-1)
        .i32(0)
        .u8(0);
    gp3.byte_string(b"Am", 20)
        .zeros(2)
        .i32(0)
        .i32(0)
        .i32(0)
        .i32(1);
    for f in [0, 1, 2, 2, 0, -1] {
        gp3.i32(f);
    }
    gp3.i32(0).zeros(24).zeros(7).zeros(1);
    // GP4 and GP5's new form, seven frets.
    let mut new = Bytes::default();
    new.u8(1)
        .u8(0)
        .zeros(3)
        .u8(7)
        .u8(0)
        .u8(0)
        .i32(-1)
        .i32(0)
        .u8(0);
    new.byte_string(b"G7", 21).zeros(4).i32(1);
    for f in [1, 0, 0, 0, 2, 3, -1] {
        new.i32(f);
    }
    new.zeros(32);
    for (major, minor, chord, name, frets) in [
        (
            3,
            0,
            &old,
            "C",
            vec![Some(0), Some(1), Some(0), Some(2), Some(3), None],
        ),
        (
            4,
            0,
            &old,
            "C",
            vec![Some(0), Some(1), Some(0), Some(2), Some(3), None],
        ),
        (
            3,
            0,
            &gp3,
            "Am",
            vec![Some(0), Some(1), Some(2), Some(2), Some(0), None],
        ),
        (
            4,
            0,
            &new,
            "G7",
            vec![Some(1), Some(0), Some(0), Some(0), Some(2), Some(3)],
        ),
        (
            5,
            0,
            &new,
            "G7",
            vec![Some(1), Some(0), Some(0), Some(0), Some(2), Some(3)],
        ),
        (
            5,
            10,
            &new,
            "G7",
            vec![Some(1), Some(0), Some(0), Some(0), Some(2), Some(3)],
        ),
    ] {
        let head = Head::new(major, minor);
        let bytes = head.one_bar(&[head.beat(0x02, -2, &chord.0, &[])]);
        let song = head.read(&bytes);
        let got = song.tracks[0].bars[0].voices[0].beats[0]
            .chord
            .clone()
            .unwrap();
        assert_eq!(got.name, name, "v{major}.{minor}");
        assert_eq!(got.frets, frets, "v{major}.{minor}: cut to the six strings");
    }
    // A name without a diagram.
    let head = Head::new(4, 0);
    let mut bare = Bytes::default();
    bare.u8(0).ibs(b"N.C.").i32(0);
    let song = head.read(&head.one_bar(&[head.beat(0x02, -2, &bare.0, &[])]));
    let chord = song.tracks[0].bars[0].voices[0].beats[0]
        .chord
        .clone()
        .unwrap();
    assert_eq!((chord.name.as_str(), chord.frets.len()), ("N.C.", 0));
}

#[test]
fn reads_a_seven_string_track_with_a_capo() {
    for (major, minor) in ALL {
        let mut head = Head::new(major, minor);
        head.programs[0] = 105; // banjo: fretted
        head.programs[2] = 1; // piano
        let mut b = head.write(1, 3);
        head.bar(&mut b, 0, 0x03, &[4, 4]);
        let seven = [64, 59, 55, 50, 45, 40, 35];
        head.track(&mut b, 0, 0, b"Seven", &seven, 1, 2);
        head.track(&mut b, 1, 0, b"Piano", &seven[..6], 3, 0);
        head.track(&mut b, 2, 0, b"Kit", &[0; 6], 10, 0);
        head.after_tracks(&mut b);
        let low = head.note(1, 3, 0, None);
        head.measure(&mut b, &[head.beat(0, -2, &[], &[(7, low)])], false);
        head.measure(&mut b, &[head.beat(0, -2, &[], &[])], false);
        head.measure(&mut b, &[head.beat(0, -2, &[], &[])], true);
        if major < 5 {
            b.zeros(4);
        }
        let song = head.read(&b.0);
        let track = &song.tracks[0];
        assert_eq!(track.tuning, [64u8, 59, 55, 50, 45, 40, 35]);
        assert_eq!(track.capo, 2);
        let note = &track.bars[0].voices[0].beats[0].notes[0];
        assert_eq!(note.string, 7);
        assert_eq!(track.pitch(note), Some(40), "tuning + capo + fret");
        let kinds: Vec<_> = song.tracks.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [TrackKind::Strings, TrackKind::Other, TrackKind::Drums]
        );
        assert_eq!(song.tracks[2].midi.channel, 9);
    }
}

#[test]
fn reads_note_effects_of_gp4_and_gp5() {
    for (major, minor) in [(4, 0), (5, 0), (5, 10)] {
        let head = Head::new(major, minor);
        let gp5 = major == 5;
        // Bend: kind, value, 3 points (0, 0) (30, 50) (60, 50).
        let mut bend = Bytes::default();
        bend.u8(1).i32(50).i32(3);
        for (p, v) in [(0, 0), (30, 50), (60, 50)] {
            bend.i32(p).i32(v).u8(0);
        }
        let mut first = Bytes::default();
        first
            .u8(0x01 | 0x02 | 0x08 | 0x10)
            .u8(0x01 | 0x02 | 0x04 | 0x20 | 0x40);
        first.raw(&bend.0);
        first.raw(&[5, 7, 1, 3]); // grace: fret, dynamic, slide, a 16th
        if gp5 {
            first.u8(0x03); // dead, on the beat
        }
        first.u8(3); // tremolo picking in 32nds
        first.raw(&[9, 2]); // trill with fret 9 in 32nds
        let mut second = Bytes::default();
        second.u8(0).u8(0x08 | 0x10);
        second.u8(if gp5 { 0x02 | 0x10 } else { 2 }); // legato slide
        if gp5 {
            second.u8(3).u8(17); // tapped at 17
        } else {
            second.u8(3); // tapped, 12 above the fret
        }
        let mut third = Bytes::default();
        third.u8(0).u8(0x10);
        if gp5 {
            third.u8(2).raw(&[0, 0, 1]); // artificial
        } else {
            third.u8(17);
        }
        let mut fourth = Bytes::default();
        fourth.u8(0).u8(0x08);
        fourth.u8(if gp5 { 0x20 } else { 0xFE }); // into from above
        let bytes = head.one_bar(&[head.beat(
            0,
            0,
            &[],
            &[
                (1, head.note(1, 3, 0x04 | 0x40, Some(&first.0))),
                (2, head.note(1, 5, 0x02, Some(&second.0))),
                (3, head.note(1, 7, 0, Some(&third.0))),
                (4, head.note(3, 0, 0, Some(&fourth.0))),
            ],
        )]);
        let song = head.read(&bytes);
        let notes = &song.tracks[0].bars[0].voices[0].beats[0].notes;
        let n = &notes[0];
        assert!(n.ghost && n.hammer && n.let_ring && n.staccato && n.palm_mute && n.vibrato);
        assert_eq!(n.accent, Accent::Normal);
        assert_eq!(n.bend.as_ref().unwrap().peak(), 2.0);
        assert_eq!(n.bend.as_ref().unwrap().points[1].position, 0.5);
        let grace = n.grace.unwrap();
        assert_eq!(
            (grace.fret, grace.value, grace.dynamic),
            (5, 16, Dynamic::Ff)
        );
        assert_eq!(grace.transition, GraceTransition::Slide);
        assert_eq!((grace.dead, grace.on_beat), (gp5, gp5));
        assert_eq!(n.tremolo_picking, Some(32));
        assert_eq!(n.trill, Some(Trill { fret: 9, value: 32 }));
        let n = &notes[1];
        assert_eq!(n.accent, if gp5 { Accent::Heavy } else { Accent::None });
        assert_eq!(n.slide, Some(Slide::Legato));
        assert_eq!(n.harmonic, Some(Harmonic::Tapped { fret: 17 }));
        assert_eq!(notes[2].harmonic, Some(Harmonic::Artificial));
        assert_eq!(notes[3].slide, Some(Slide::IntoFromAbove));
        assert!(notes[3].dead);
    }
}

#[test]
fn reads_beat_effects() {
    // GP4 and GP5: slap, tremolo bar, stroke, pick stroke, in that order.
    for (major, minor) in [(4, 0), (5, 0), (5, 10)] {
        let head = Head::new(major, minor);
        let mut fx = Bytes::default();
        fx.u8(0x02 | 0x10 | 0x20 | 0x40).u8(0x01 | 0x02 | 0x04);
        fx.u8(3); // pop
        fx.u8(6)
            .i32(-50)
            .i32(2)
            .i32(0)
            .i32(0)
            .u8(0)
            .i32(60)
            .i32(-50)
            .u8(0);
        // A down stroke in 32nds.
        fx.raw(if major == 5 { &[0, 3] } else { &[3, 0] });
        fx.u8(1); // pick stroke up
        let note = head.note(1, 0, 0, None);
        let song = head.read(&head.one_bar(&[head.beat(0x08, -2, &fx.0, &[(6, note)])]));
        let e = &song.tracks[0].bars[0].voices[0].beats[0].effects;
        assert!(e.wide_vibrato && e.fade_in && e.rasgueado);
        assert_eq!(e.slap, Some(Slap::Pop));
        assert_eq!(e.tremolo_bar.as_ref().unwrap().peak(), -2.0);
        assert_eq!(
            e.stroke,
            Some(Stroke {
                direction: StrokeDirection::Down,
                value: 32
            })
        );
        assert_eq!(e.pick_stroke, Some(StrokeDirection::Up));
    }
    // GP3: one flags byte; its harmonics and vibrato go to every note, and
    // 0x20 is a tremolo bar or a slap, with an int.
    let head = Head::new(3, 0);
    let notes = [
        (1, head.note(1, 12, 0, None)),
        (2, head.note(1, 12, 0, None)),
    ];
    let mut fx = Bytes::default();
    fx.u8(0x01 | 0x04 | 0x40).raw(&[0, 6]);
    let mut tap = Bytes::default();
    tap.u8(0x20).u8(1).i32(0);
    let mut dip = Bytes::default();
    dip.u8(0x20).u8(0).i32(-50);
    let song = head.read(&head.one_bar(&[
        head.beat(0x08, 0, &fx.0, &notes),
        head.beat(0x08, 0, &tap.0, &[]),
        head.beat(0x08, -1, &dip.0, &[]),
    ]));
    let beats = &song.tracks[0].bars[0].voices[0].beats;
    assert!(
        beats[0]
            .notes
            .iter()
            .all(|n| n.vibrato && n.harmonic == Some(Harmonic::Natural))
    );
    assert_eq!(
        beats[0].effects.stroke,
        Some(Stroke {
            direction: StrokeDirection::Up,
            value: 4
        })
    );
    assert_eq!(beats[1].effects.slap, Some(Slap::Tap));
    assert_eq!(beats[2].effects.tremolo_bar.as_ref().unwrap().peak(), -2.0);
}

#[test]
fn ties_take_the_fret_they_hold_across_bars() {
    let head = Head::new(5, 10);
    let mut b = head.write(2, 1);
    head.bar(&mut b, 0, 0x03, &[4, 4]);
    head.bar(&mut b, 1, 0, &[]);
    head.track(&mut b, 0, 0, b"Guitar", &[64, 59, 55, 50, 45, 40], 1, 0);
    head.after_tracks(&mut b);
    head.measure(
        &mut b,
        &[head.beat(0, -2, &[], &[(3, head.note(1, 9, 0, None))])],
        false,
    );
    head.measure(
        &mut b,
        &[head.beat(0, -2, &[], &[(3, head.note(2, 0, 0, None))])],
        true,
    );
    let song = head.read(&b.0);
    let tied = &song.tracks[0].bars[1].voices[0].beats[0].notes[0];
    assert!(tied.tie);
    assert_eq!(tied.fret, 9);
}

#[test]
fn gives_words_to_their_track() {
    let mut head = Head::new(4, 6);
    head.lyrics = (1, vec![(3, "Do-lo-rem\r\nip sum")]);
    let song = head.read(&head.one_bar(&[head.beat(0, -2, &[], &[])]));
    assert_eq!(
        song.tracks[0].lyrics,
        [LyricLine {
            start_bar: 2,
            text: "Do-lo-rem\nip sum".into()
        }]
    );
}
