//! Export MIDI (DECISIONS D54): the music the transport plays, written where
//! the user chooses in a save panel, whose checkbox leaves out the tracks of
//! the parts muted in the Parts fold (D45). Only whole tracks are left out:
//! the bytes of the others are lilypond's, unchanged.
//!
//! Export for GarageBand (D162) rewrites the events instead, by a plan the
//! renderer makes (`for_garageband`).
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The checkbox names the muted parts while there are at most this many.
const NAMED_PARTS: usize = 3;

/// The tracks of a Standard MIDI File, as midi.js indexes them: its `MTrk`
/// chunks in order. `bytes` without the tracks at `tracks`, and the header's
/// track count with them; any other chunk is kept.
pub fn without_tracks(bytes: &[u8], tracks: &[usize]) -> Result<Vec<u8>, String> {
    let invalid = |why: &str| format!("The MIDI could not be exported: {why}.");
    if bytes.len() < 14 || &bytes[0..4] != b"MThd" {
        return Err(invalid("it is not a MIDI file"));
    }
    let header_end = 8 + chunk_length(bytes, 0) as usize;
    if header_end < 14 || header_end > bytes.len() {
        return Err(invalid("its header is cut short"));
    }
    let mut out = bytes[..header_end].to_vec();
    let mut at = header_end;
    let mut track = 0;
    let mut kept = 0u16;
    while at < bytes.len() {
        if at + 8 > bytes.len() {
            return Err(invalid("it ends inside a chunk"));
        }
        let end = at + 8 + chunk_length(bytes, at) as usize;
        if end > bytes.len() {
            return Err(invalid("a chunk runs past its end"));
        }
        if &bytes[at..at + 4] == b"MTrk" {
            let leave_out = tracks.contains(&track);
            track += 1;
            if leave_out {
                at = end;
                continue;
            }
            kept += 1;
        }
        out.extend_from_slice(&bytes[at..end]);
        at = end;
    }
    if kept == 0 {
        return Err(invalid("it would have no tracks left"));
    }
    out[10..12].copy_from_slice(&kept.to_be_bytes());
    Ok(out)
}

/// What Export for GarageBand does to lilypond's MIDI (D162, README §6.3 of
/// `docs/research/garageband`), as the renderer works it out: it has the
/// parts, their names and instruments, `% BPM:` and bar 1, and this side only
/// edits events. Tracks are midi.js's indices, as for `without_tracks`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GarageBandPlan {
    /// The tracks to leave out: the words' tracks, which GarageBand makes
    /// into empty piano tracks (§4.3), and the muted parts behind D54's
    /// checkbox.
    #[serde(default)]
    pub drop: Vec<usize>,
    /// Each kept part: the track gets this name and program, and loses its own.
    #[serde(default)]
    pub parts: Vec<PlannedPart>,
    /// What every tempo, in µs a quarter, is multiplied by: atTempo's stretch
    /// for `% BPM:` (D155), the score's first tempo over `% BPM:`; 1 without.
    #[serde(default = "no_stretch")]
    pub tempo_ratio: f64,
    /// Ticks every sounding event moves later by, so the score's bar 1 is a
    /// bar of GarageBand's after a pickup: a bar less the pickup, 0 without one.
    #[serde(default)]
    pub lead_in: u32,
}

fn no_stretch() -> f64 {
    1.0
}

impl GarageBandPlan {
    /// The plan with `tracks` left out too, as D54's checkbox leaves out the
    /// muted parts once the save panel has answered: they join `drop`, and
    /// their parts are no longer named or given a program.
    pub fn without_parts(mut self, tracks: &[usize]) -> Self {
        self.parts.retain(|part| !tracks.contains(&part.track));
        self.drop.extend(tracks);
        self.drop.sort_unstable();
        self.drop.dedup();
        self
    }
}

/// A part of the plan: its track, the name `partNames` gives it, and the
/// General MIDI program it plays, 0 to 127 as in the file, from the Parts
/// fold or the written instrument.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedPart {
    pub track: usize,
    pub name: String,
    pub program: u8,
}

/// `bytes`, lilypond's MIDI, rewritten for GarageBand by `plan`, as
/// `docs/research/garageband/spike/make_import_bundle.py`'s `rewrite(...,
/// studio=True)` does:
///
/// - the tracks in `plan.drop` left out, and the control track's (the first
///   of a format-1 file) name and text events, which would be a track of
///   their own (§4.3);
/// - each part's track starts with a track-name event and a program change
///   on each channel it plays, in place of its own;
/// - every tempo event scaled by the ratio;
/// - every event moved later by the lead-in, except what sets up a track at
///   tick 0 (its tempo, meter, key, program): notes, words, markers and the
///   track's end move.
///
/// No volume or pan is written (no CC 7 or CC 10, §6.3); controllers the
/// score itself set stay. Running status is written out in full.
pub fn for_garageband(bytes: &[u8], plan: &GarageBandPlan) -> Result<Vec<u8>, String> {
    let invalid = |why: &str| format!("The MIDI could not be exported for GarageBand: {why}.");
    if !(plan.tempo_ratio.is_finite() && plan.tempo_ratio > 0.0) {
        return Err(invalid("its tempo is not a number above 0"));
    }
    if let Some(part) = plan.parts.iter().find(|part| part.program > 127) {
        return Err(invalid(&format!(
            "{} has no program {}",
            part.name, part.program
        )));
    }
    let trimmed = without_tracks(bytes, &plan.drop)?;
    let header_end = 8 + chunk_length(&trimmed, 0) as usize;
    let format = u16::from_be_bytes([trimmed[8], trimmed[9]]);
    // midi.js's indices into the file as it was, for the tracks kept.
    let mut kept = (0..).filter(|track| !plan.drop.contains(track));
    let mut out = trimmed[..header_end].to_vec();
    let mut at = header_end;
    while at < trimmed.len() {
        let end = at + 8 + chunk_length(&trimmed, at) as usize;
        if &trimmed[at..at + 4] != b"MTrk" {
            out.extend_from_slice(&trimmed[at..end]);
            at = end;
            continue;
        }
        let track = kept.next().expect("an index for every track");
        let events = read_events(&trimmed[at + 8..end]).map_err(|why| invalid(&why))?;
        let part = plan.parts.iter().find(|part| part.track == track);
        let rewritten = rewrite_track(events, part, format == 1 && track == 0, plan);
        let data = write_events(&rewritten).map_err(|why| invalid(&why))?;
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(&data);
        at = end;
    }
    Ok(out)
}

/// How long the first `lead_in` ticks of `bytes` last, in seconds, by its
/// tempo map: for the MIDI `for_garageband` wrote, the lead-in at the
/// rewritten first tempo, which every WAV of the export is shifted by
/// (README §8 stage 2). Ticks before the first tempo go at the first tempo,
/// as `make_import_bundle.py`'s `seconds_at` has it; a file with none goes at
/// MIDI's 120 a minute.
pub fn lead_in_seconds(bytes: &[u8], lead_in: u32) -> Result<f64, String> {
    let invalid = |why: &str| format!("The MIDI's lead-in could not be timed: {why}.");
    let trimmed = without_tracks(bytes, &[])?;
    let header_end = 8 + chunk_length(&trimmed, 0) as usize;
    let division = u16::from_be_bytes([trimmed[12], trimmed[13]]);
    if division == 0 || division & 0x8000 != 0 {
        return Err(invalid("it counts in frames, not quarters"));
    }
    let mut tempos = Vec::new();
    let mut at = header_end;
    while at < trimmed.len() {
        let end = at + 8 + chunk_length(&trimmed, at) as usize;
        if &trimmed[at..at + 4] == b"MTrk" {
            let events = read_events(&trimmed[at + 8..end]).map_err(|why| invalid(&why))?;
            tempos.extend(
                events
                    .iter()
                    .filter(|event| event.meta() == Some(TEMPO) && event.data.len() == 6)
                    .map(|event| {
                        let data = &event.data;
                        (
                            event.tick,
                            u32::from_be_bytes([0, data[3], data[4], data[5]]),
                        )
                    }),
            );
        }
        at = end;
    }
    tempos.sort_by_key(|&(tick, _)| tick);
    let lead_in = u64::from(lead_in);
    let (mut tick, mut tempo) = (0, tempos.first().map_or(500_000, |&(_, tempo)| tempo));
    let mut micros = 0.0;
    for &(at, next) in tempos.iter().filter(|&&(at, _)| at < lead_in) {
        micros += (at - tick) as f64 * f64::from(tempo);
        (tick, tempo) = (at, next);
    }
    micros += (lead_in - tick) as f64 * f64::from(tempo);
    Ok(micros / f64::from(division) / 1e6)
}

/// An event of a track at its tick from the start: its status byte and the
/// rest, running status undone.
#[derive(Clone, Debug, PartialEq)]
struct Event {
    tick: u64,
    data: Vec<u8>,
}

const META: u8 = 0xFF;
const TEXT: u8 = 0x01;
const COPYRIGHT: u8 = 0x02;
const TRACK_NAME: u8 = 0x03;
const LYRIC: u8 = 0x05;
const MARKER: u8 = 0x06;
const END_OF_TRACK: u8 = 0x2F;
const TEMPO: u8 = 0x51;
const PROGRAM: u8 = 0xC0;

impl Event {
    fn meta(&self) -> Option<u8> {
        (self.data[0] == META).then(|| self.data[1])
    }

    /// The channel of a channel message.
    fn channel(&self) -> Option<u8> {
        (0x80..0xF0)
            .contains(&self.data[0])
            .then(|| self.data[0] & 0x0F)
    }

    /// A channel message's kind, such as 0x90 for a note on.
    fn kind(&self) -> Option<u8> {
        self.channel().map(|_| self.data[0] & 0xF0)
    }

    /// What sounds, or marks a time in the music, and so takes the lead-in
    /// even at tick 0: notes, aftertouch, pitch bends, words, markers and the
    /// track's end.
    fn moves(&self) -> bool {
        matches!(self.meta(), Some(LYRIC | MARKER | END_OF_TRACK))
            || matches!(self.kind(), Some(0x80 | 0x90 | 0xA0 | 0xD0 | 0xE0))
    }
}

fn rewrite_track(
    events: Vec<Event>,
    part: Option<&PlannedPart>,
    control: bool,
    plan: &GarageBandPlan,
) -> Vec<Event> {
    let mut channels: Vec<u8> = events.iter().filter_map(Event::channel).collect();
    channels.sort_unstable();
    channels.dedup();
    let mut head = Vec::new();
    if let Some(part) = part {
        let name = part.name.as_bytes();
        let mut data = vec![META, TRACK_NAME];
        push_varlen(&mut data, name.len() as u64);
        data.extend_from_slice(name);
        head.push(Event { tick: 0, data });
        for &channel in &channels {
            head.push(Event {
                tick: 0,
                data: vec![PROGRAM | channel, part.program],
            });
        }
    }
    let mut setup = Vec::new();
    let mut rest = Vec::new();
    for mut event in events {
        let meta = event.meta();
        if control && matches!(meta, Some(TEXT | COPYRIGHT | TRACK_NAME)) {
            continue;
        }
        if part.is_some() && (meta == Some(TRACK_NAME) || event.kind() == Some(PROGRAM)) {
            continue;
        }
        if meta == Some(TEMPO) && event.data.len() == 6 {
            let tempo = u32::from_be_bytes([0, event.data[3], event.data[4], event.data[5]]);
            let scaled = (f64::from(tempo) * plan.tempo_ratio).round_ties_even();
            let scaled = scaled.clamp(1.0, f64::from(0xFF_FFFF_u32)) as u32;
            event.data[3..6].copy_from_slice(&scaled.to_be_bytes()[1..]);
        }
        if event.tick == 0 && !event.moves() {
            setup.push(event);
        } else {
            event.tick += u64::from(plan.lead_in);
            rest.push(event);
        }
    }
    head.extend(setup);
    head.extend(rest);
    head
}

/// The events of an `MTrk` chunk's data.
fn read_events(data: &[u8]) -> Result<Vec<Event>, String> {
    let cut = || "a track ends inside an event".to_string();
    let mut events = Vec::new();
    let (mut at, mut tick, mut running) = (0, 0u64, None);
    while at < data.len() {
        tick += read_varlen(data, &mut at).ok_or_else(cut)?;
        let status = match *data.get(at).ok_or_else(cut)? {
            byte if byte >= 0x80 => {
                at += 1;
                byte
            }
            _ => running.ok_or("an event has no status")?,
        };
        let body = at;
        match status {
            META | 0xF0 | 0xF7 => {
                if status == META {
                    at += 1; // its type
                }
                at += read_varlen(data, &mut at).ok_or_else(cut)? as usize;
                running = None;
            }
            0xF1..=0xFE => return Err(format!("it has a system message {status:#04x}")),
            _ => {
                at += if matches!(status & 0xF0, 0xC0 | 0xD0) {
                    1
                } else {
                    2
                };
                running = Some(status);
            }
        }
        if at > data.len() {
            return Err(cut());
        }
        let mut event = vec![status];
        event.extend_from_slice(&data[body..at]);
        events.push(Event { tick, data: event });
    }
    Ok(events)
}

/// `events` as an `MTrk` chunk's data, in their order, each with its status.
fn write_events(events: &[Event]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut last = 0;
    for event in events {
        let delta = event.tick - last;
        if delta > 0x0FFF_FFFF {
            return Err("an event is too far from the one before it".into());
        }
        push_varlen(&mut out, delta);
        out.extend_from_slice(&event.data);
        last = event.tick;
    }
    Ok(out)
}

fn read_varlen(data: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for _ in 0..4 {
        let byte = *data.get(*at)?;
        *at += 1;
        value = (value << 7) | u64::from(byte & 0x7F);
        if byte < 0x80 {
            return Some(value);
        }
    }
    None
}

fn push_varlen(out: &mut Vec<u8>, mut value: u64) {
    let mut bytes = vec![(value & 0x7F) as u8];
    value >>= 7;
    while value > 0 {
        bytes.push(0x80 | (value & 0x7F) as u8);
        value >>= 7;
    }
    out.extend(bytes.iter().rev());
}

fn chunk_length(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
}

/// The save panel's checkbox: "Leave out the muted part Tenor", or the parts
/// counted when there are more than a few.
pub fn checkbox_title(names: &[String]) -> String {
    match names {
        [name] => format!("Leave out the muted part {name}"),
        _ if names.len() <= NAMED_PARTS => {
            let (last, first) = names.split_last().expect("more than one name");
            format!("Leave out the muted parts {} and {last}", first.join(", "))
        }
        _ => format!("Leave out the {} muted parts", names.len()),
    }
}

/// The file the panel suggests: the score's name with `.mid`, beside it.
pub fn suggested_name(root_file: &Path) -> String {
    let stem = root_file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "score".into());
    format!("{stem}.mid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = tag.to_vec();
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(data);
        chunk
    }

    /// Format 1, `tracks` tracks of one byte each, which is their index; an unknown chunk after the first.
    fn midi(tracks: u8) -> Vec<u8> {
        let mut bytes = chunk(b"MThd", &[0, 1, 0, tracks, 1, 0x80]);
        for track in 0..tracks {
            bytes.extend(chunk(b"MTrk", &[track]));
            if track == 0 {
                bytes.extend(chunk(b"XFIH", &[9, 9]));
            }
        }
        bytes
    }

    #[test]
    fn leaves_out_the_tracks_and_counts_the_rest() {
        let out = without_tracks(&midi(4), &[1, 3]).unwrap();
        let mut expected = chunk(b"MThd", &[0, 1, 0, 2, 1, 0x80]);
        expected.extend(chunk(b"MTrk", &[0]));
        expected.extend(chunk(b"XFIH", &[9, 9]));
        expected.extend(chunk(b"MTrk", &[2]));
        assert_eq!(out, expected);
    }

    #[test]
    fn no_tracks_to_leave_out_is_the_same_file() {
        assert_eq!(without_tracks(&midi(3), &[]).unwrap(), midi(3));
        // A track the file does not have changes nothing.
        assert_eq!(without_tracks(&midi(3), &[7]).unwrap(), midi(3));
    }

    #[test]
    fn a_real_compile_keeps_its_bytes() {
        // lilypond's: a control track and three staves, at 14, 99, 288 and 448.
        let bytes = include_bytes!("../../../../vscode/test/fixtures/sample.midi");
        let out = without_tracks(bytes, &[2]).unwrap();
        assert_eq!(&out[10..12], &[0, 3]);
        assert_eq!(&out[12..288], &bytes[12..288]);
        assert_eq!(&out[288..], &bytes[448..]);
        assert_eq!(parse_tracks(&out), 3);
    }

    fn parse_tracks(bytes: &[u8]) -> usize {
        let mut at = 14;
        let mut tracks = 0;
        while at < bytes.len() {
            assert_eq!(&bytes[at..at + 4], b"MTrk");
            at += 8 + chunk_length(bytes, at) as usize;
            tracks += 1;
        }
        assert_eq!(at, bytes.len());
        tracks
    }

    #[test]
    fn refuses_what_is_not_midi() {
        assert!(without_tracks(b"RIFF....WAVEfmt ", &[]).is_err());
        let mut cut = midi(2);
        cut.truncate(cut.len() - 1);
        assert!(without_tracks(&cut, &[]).is_err());
        assert!(without_tracks(&midi(1), &[0]).is_err());
    }

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/garageband.midi");
    const EXPECTED: &[u8] = include_bytes!("../tests/fixtures/garageband.expected.mid");
    const PLAN: &str = include_str!("../tests/fixtures/garageband.plan.json");

    fn plan() -> GarageBandPlan {
        serde_json::from_str(PLAN).unwrap()
    }

    /// The header's format, track count and division, and each track's events.
    fn read(bytes: &[u8]) -> (u16, u16, u16, Vec<Vec<Event>>) {
        let word = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
        let mut tracks = Vec::new();
        let mut at = 14;
        while at < bytes.len() {
            assert_eq!(&bytes[at..at + 4], b"MTrk");
            let end = at + 8 + chunk_length(bytes, at) as usize;
            tracks.push(read_events(&bytes[at + 8..end]).unwrap());
            at = end;
        }
        assert_eq!(at, bytes.len());
        (word(8), word(10), word(12), tracks)
    }

    fn name_of(events: &[Event]) -> Option<String> {
        let event = events
            .iter()
            .find(|event| event.meta() == Some(TRACK_NAME))?;
        Some(String::from_utf8(event.data[3..].to_vec()).unwrap())
    }

    /// Notes on and off: tick, status and key.
    fn notes(events: &[Event]) -> Vec<(u64, u8, u8)> {
        events
            .iter()
            .filter(|event| matches!(event.kind(), Some(0x80 | 0x90)))
            .map(|event| (event.tick, event.data[0], event.data[1]))
            .collect()
    }

    fn midi_with(track: &[u8]) -> Vec<u8> {
        let mut bytes = chunk(b"MThd", &[0, 1, 0, 2, 1, 0x80]);
        bytes.extend(chunk(
            b"MTrk",
            &[0, 0xFF, 0x51, 3, 0x07, 0xA1, 0x20, 0, 0xFF, 0x2F, 0],
        ));
        bytes.extend(chunk(b"MTrk", track));
        bytes
    }

    #[test]
    fn rewrites_lilypond_midi_as_the_spike_does() {
        // garageband.midi is lilypond 2.26's MIDI of make_import_bundle.py's
        // score (a pickup, two tempos, words, a piano without midiInstrument
        // and drums), and garageband.expected.mid its rewrite of the same plan.
        assert_eq!(for_garageband(FIXTURE, &plan()).unwrap(), EXPECTED);
    }

    #[test]
    fn times_the_lead_in_at_the_rewritten_first_tempo() {
        // Two quarters (768 ticks at 384) at % BPM: 113.76, to the µs the
        // rewrite rounded the tempo to.
        let lead_in = lead_in_seconds(EXPECTED, plan().lead_in).unwrap();
        assert!((lead_in - 2.0 * 60.0 / 113.76).abs() < 1e-6, "{lead_in}");
        assert_eq!(lead_in_seconds(EXPECTED, 0).unwrap(), 0.0);
        // Ticks before the first tempo go at it; past a change, at the next.
        let tempo = |tick: u8, us: u32| {
            let us = us.to_be_bytes();
            [tick, 0xFF, 0x51, 3, us[1], us[2], us[3]]
        };
        let mut track = tempo(64, 1_000_000).to_vec();
        track.extend(tempo(64, 500_000));
        track.extend([0, 0xFF, 0x2F, 0]);
        let mut bytes = chunk(b"MThd", &[0, 0, 0, 1, 0, 128]);
        bytes.extend(chunk(b"MTrk", &track));
        assert_eq!(lead_in_seconds(&bytes, 256).unwrap(), 1.0 + 0.5);
        // No tempo: MIDI's 120 a minute. Frames: refused.
        let mut none = chunk(b"MThd", &[0, 0, 0, 1, 0, 128]);
        none.extend(chunk(b"MTrk", &[0, 0xFF, 0x2F, 0]));
        assert_eq!(lead_in_seconds(&none, 128).unwrap(), 0.5);
        let mut frames = chunk(b"MThd", &[0, 0, 0, 1, 0xE7, 40]);
        frames.extend(chunk(b"MTrk", &[0, 0xFF, 0x2F, 0]));
        assert!(lead_in_seconds(&frames, 128).is_err());
    }

    #[test]
    fn reads_back_each_rewritten_event() {
        let (_, _, division, before) = read(FIXTURE);
        let (format, count, _, after) = read(&for_garageband(FIXTURE, &plan()).unwrap());
        assert_eq!((format, count, division), (1, 7, 384));
        assert_eq!(after.len(), 7);

        // The control track: no title or texts; the meter at 0; the tempos
        // at % BPM: 113.76, the second keeping its proportion, after the lead-in.
        let control = &after[0];
        assert!(
            control
                .iter()
                .all(|event| !matches!(event.meta(), Some(1..=3)))
        );
        assert!(
            control
                .iter()
                .any(|event| event.tick == 0 && event.data == [0xFF, 0x58, 4, 3, 2, 24, 8])
        );
        let tempos: Vec<(u64, u32)> = control
            .iter()
            .filter(|event| event.meta() == Some(TEMPO))
            .map(|event| {
                (
                    event.tick,
                    u32::from_be_bytes([0, event.data[3], event.data[4], event.data[5]]),
                )
            })
            .collect();
        assert_eq!(tempos, [(0, 527_426), (4992 + 768, 703_234)]);
        let bpm = |us: u32| (6e9 / f64::from(us)).round() / 100.0;
        assert_eq!((bpm(tempos[0].1), bpm(tempos[1].1)), (113.76, 85.32));

        // The words' track (lilypond's 2) is left out: no lyric is left.
        assert!(
            after
                .iter()
                .flatten()
                .all(|event| event.meta() != Some(LYRIC))
        );
        let parts = [
            &before[1], &before[3], &before[4], &before[5], &before[6], &before[7],
        ];

        // Each part: its name first, then one program on its own channel,
        // in place of lilypond's (the piano's tracks had none, the drums 0).
        let names = [
            "Voice",
            "Violin",
            "Cello",
            "Piano · G clef",
            "Piano · F clef",
            "Drums",
        ];
        let programs = [52, 40, 42, 0, 0, 0];
        let channels = [0, 2, 3, 4, 5, 9];
        for (index, track) in after[1..].iter().enumerate() {
            assert_eq!(name_of(track).as_deref(), Some(names[index]));
            assert_eq!(track[0].meta(), Some(TRACK_NAME));
            let changes: Vec<&Event> = track
                .iter()
                .filter(|event| event.kind() == Some(PROGRAM))
                .collect();
            assert_eq!(changes.len(), 1, "{}", names[index]);
            assert_eq!(changes[0].tick, 0);
            assert_eq!(
                changes[0].data,
                [PROGRAM | channels[index], programs[index]]
            );
            assert_eq!(track[1], *changes[0]);
            // No volume or pan.
            assert!(
                track
                    .iter()
                    .all(|event| !(event.kind() == Some(0xB0) && matches!(event.data[1], 7 | 10)))
            );
            // Every note a lead-in later: a bar of 3/4 less the pickup's quarter.
            let moved: Vec<_> = notes(parts[index])
                .iter()
                .map(|&(tick, status, key)| (tick + 768, status, key))
                .collect();
            assert_eq!(notes(track), moved, "{}", names[index]);
            // What sets the track up stays at 0, and its end moves.
            assert_eq!(track.last().unwrap().meta(), Some(END_OF_TRACK));
            assert_eq!(
                track.last().unwrap().tick,
                parts[index].last().unwrap().tick + 768
            );
        }
        // The pickup sounds on beat 3 of GarageBand's bar 1, and the score's bar 1 is its bar 2.
        assert_eq!(notes(&after[1])[0], (768, 0x90, 74));
        assert!(
            after[1]
                .iter()
                .any(|event| event.tick == 0 && event.data == [0xFF, 0x59, 2, 1, 0])
        );
    }

    #[test]
    fn muted_parts_join_the_tracks_left_out() {
        let plan = GarageBandPlan {
            drop: vec![4],
            parts: vec![
                PlannedPart {
                    track: 1,
                    name: "Flute".into(),
                    program: 73,
                },
                PlannedPart {
                    track: 2,
                    name: "Piano".into(),
                    program: 0,
                },
            ],
            tempo_ratio: 1.0,
            lead_in: 0,
        };
        let without = plan.clone().without_parts(&[2, 4]);
        assert_eq!(without.drop, [2, 4]);
        assert_eq!(without.parts, plan.parts[..1]);
        assert_eq!(plan.clone().without_parts(&[]), plan);
    }

    #[test]
    fn an_empty_plan_changes_only_the_control_track() {
        let plan: GarageBandPlan = serde_json::from_str("{}").unwrap();
        assert_eq!(plan.tempo_ratio, 1.0);
        let (_, _, _, before) = read(FIXTURE);
        let (_, count, _, after) = read(&for_garageband(FIXTURE, &plan).unwrap());
        assert_eq!(count, 8);
        assert_eq!(after[1..], before[1..]);
        let kept: Vec<&Event> = before[0]
            .iter()
            .filter(|event| !matches!(event.meta(), Some(1..=3)))
            .collect();
        assert_eq!(after[0].iter().collect::<Vec<_>>(), kept);
    }

    #[test]
    fn running_status_is_written_out_and_each_channel_gets_its_program() {
        // Two notes on channel 0 by running status, one on channel 1, and a program of its own.
        let track = [
            0, 0xC0, 5, 0, 0x90, 60, 64, 0x60, 60, 0, 0, 0x91, 62, 64, 0x60, 0x81, 62, 0, 0, 0xFF,
            0x2F, 0,
        ];
        let plan = GarageBandPlan {
            drop: vec![],
            parts: vec![PlannedPart {
                track: 1,
                name: "Two".into(),
                program: 73,
            }],
            tempo_ratio: 0.5,
            lead_in: 0,
        };
        let (_, _, _, after) = read(&for_garageband(&midi_with(&track), &plan).unwrap());
        let data: Vec<&[u8]> = after[1].iter().map(|event| event.data.as_slice()).collect();
        assert_eq!(
            data,
            [
                &[0xFF, 0x03, 3, b'T', b'w', b'o'][..],
                &[0xC0, 73],
                &[0xC1, 73],
                &[0x90, 60, 64],
                &[0x90, 60, 0],
                &[0x91, 62, 64],
                &[0x81, 62, 0],
                &[0xFF, 0x2F, 0],
            ]
        );
        // 500 000 µs a quarter, halved.
        assert_eq!(after[0][0].data, [0xFF, 0x51, 3, 0x03, 0xD0, 0x90]);
    }

    #[test]
    fn refuses_a_plan_or_file_it_cannot_write() {
        let mut plan = plan();
        plan.tempo_ratio = 0.0;
        assert!(for_garageband(FIXTURE, &plan).is_err());
        let mut plan = self::plan();
        plan.parts[0].program = 128;
        assert!(for_garageband(FIXTURE, &plan).is_err());
        assert!(for_garageband(b"RIFF....WAVEfmt ", &self::plan()).is_err());
        // A data byte with no status before it, and a note cut short.
        assert!(for_garageband(&midi_with(&[0, 60, 64]), &self::plan()).is_err());
        assert!(for_garageband(&midi_with(&[0, 0x90, 60]), &self::plan()).is_err());
    }

    #[test]
    fn names_the_muted_parts() {
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        assert_eq!(
            checkbox_title(&names(&["Tenor"])),
            "Leave out the muted part Tenor"
        );
        assert_eq!(
            checkbox_title(&names(&["S.A", "Tenor", "Piano · F clef"])),
            "Leave out the muted parts S.A, Tenor and Piano · F clef"
        );
        assert_eq!(
            checkbox_title(&names(&["a", "b", "c", "d"])),
            "Leave out the 4 muted parts"
        );
    }

    #[test]
    fn suggests_the_score_name() {
        assert_eq!(suggested_name(Path::new("/music/ode.ly")), "ode.mid");
    }
}
