//! The LilyPond writer (D166): a score from the song model.
//!
//! - **The header** comes from `SongInfo`; the notices and instructions,
//!   which LilyPond's header has no place for, are comments above it.
//! - **One variable a track**, or one a voice for a track that has two or
//!   more (`leadVoiceOne`, `leadVoiceTwo`), named after the track and made
//!   unique (`fx`, `fxB`). Absolute pitches, a bar a line ending with its
//!   bar check and Guitar Pro's bar number (`|  % 12`). The first track's
//!   first voice also carries what belongs to the whole score: the tempo,
//!   section names, segni, codas and jumps.
//! - **Staves by kind:** a `Strings` track is a StaffGroup of a Staff in
//!   `treble_8` or `bass_8` (a bass when its highest string is below G3)
//!   and a TabStaff in the track's tuning and capo, the notes carrying their
//!   strings (`\6`) so that the tab shows Guitar Pro's frets. Drums are a
//!   DrumStaff in `\drummode`; any other track a Staff, its clef from where
//!   its notes lie.
//! - **Repeats** as `plan` reads them: `\repeat volta` and `\alternative`
//!   where Guitar Pro's marks say what LilyPond can, the bars unfolded in
//!   Guitar Pro's playing order where they do not.
//! - **Size:** a score whose first system is too tall for an A4 page, as
//!   ten tracks with tab are, is set at a smaller staff size, and after the
//!   first system a staff shows only where its track plays. A name longer
//!   than 12 characters takes two lines, or it would run past the page.
//! - **Playback:** a `\midi` with each track's General MIDI program as
//!   `midiInstrument`. The TabStaff plays nothing, or every note would sound
//!   twice. A score with repeats has a second `\score` that unfolds them for
//!   the MIDI (D145), the first keeping `\layout`.
//!
//! What a beat or note does is written where LilyPond writes it cleanly:
//! ties, slurs for hammer-ons, pull-offs and legato slides, `\glissando` for
//! slides, `\bendAfter` at a bend's peak, `\deadNote`, ghost notes in
//! parentheses, natural harmonics, accents, staccato, dynamics (not on
//! drums), `\prall` for vibrato, `\trill`, tremolo picking, grace notes,
//! pick strokes, strums as `\arpeggio`, and text for palm mute, let ring,
//! other harmonics, slap, chord names where they change and beat text.
//! A ghost note's softer dynamic is left to its parentheses. Left out: lyrics, the
//! tremolo bar, fade-in, rasgueado, chord diagrams, and the playing of
//! D.C. and D.S. jumps, which are written but not unfolded.
mod music;
mod names;
mod plan;

use crate::{Song, Track, TrackKind};
use music::{Kind, Part};

/// The LilyPond version the score is written for, as the studio's
/// templates are.
const VERSION: &str = "2.24.0";

/// The song as a LilyPond score, ready to save as a `.ly`.
pub fn write(song: &Song) -> String {
    let mut out = format!("\\version \"{VERSION}\"\n\n");
    out.push_str(&format!(
        "% Imported from Guitar Pro ({}) by Lily Studio.\n",
        names::comment(&song.version)
    ));
    let info = &song.info;
    if !info.instructions.trim().is_empty() {
        out.push_str(&format!(
            "% Instructions: {}\n",
            names::comment(&info.instructions)
        ));
    }
    for notice in info.notices.iter().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!("% {}\n", names::comment(notice)));
    }
    out.push('\n');
    // The first system shows every staff, and must fit on a page.
    let height: f64 = song
        .tracks
        .iter()
        .map(|track| staff_height(part(song, track).kind))
        .sum();
    let size = staff_size(height);
    if size < DEFAULT_STAFF_SIZE {
        out.push_str(&format!(
            "% So many staves that a system fits on a page only at a smaller size.\n\
             #(set-global-staff-size {size})\n\n"
        ));
    }
    out.push_str(&header(song));

    let plan = plan::plan(&song.master_bars);
    if plan.unfolded {
        out.push_str(
            "% Guitar Pro's repeat marks contradict one another here, so the bars are\n\
             % written out in the order Guitar Pro plays them, numbered as it numbers them.\n\n",
        );
    }
    let mut namer = names::Namer::default();
    let mut staves = Vec::new();
    for (t, track) in song.tracks.iter().enumerate() {
        let part = part(song, track);
        let base = namer.unique(&names::variable(&track.name));
        let vars: Vec<String> = if part.voices.len() == 1 {
            vec![format!("{base}Music")]
        } else {
            const ORDINALS: [&str; 4] = ["One", "Two", "Three", "Four"];
            (0..part.voices.len())
                .map(|k| format!("{base}Voice{}", ORDINALS[k.min(3)]))
                .collect()
        };
        let mode = if part.kind == Kind::Drums {
            "\\drummode "
        } else {
            ""
        };
        for (k, var) in vars.iter().enumerate() {
            let title = if track.name.trim().is_empty() {
                format!("% Track {}", t + 1)
            } else {
                format!("% Track {}: {}", t + 1, names::comment(&track.name))
            };
            let voice = if vars.len() > 1 {
                format!(", voice {}", k + 1)
            } else {
                String::new()
            };
            out.push_str(&format!("{title}{voice}\n{var} = {mode}{{\n"));
            out.push_str(&music::write_voice(&part, &plan, k, t == 0 && k == 0));
            out.push_str("}\n\n");
        }
        staves.push(staff(&part, &vars));
    }
    if staves.is_empty() || song.master_bars.is_empty() {
        return out;
    }
    let layout = if size < DEFAULT_STAFF_SIZE {
        "  \\layout {\n    \
         % After the first system, a staff shows only where its track plays.\n    \
         \\context { \\Staff \\RemoveEmptyStaves }\n    \
         \\context { \\TabStaff \\RemoveEmptyStaves }\n    \
         \\context { \\DrumStaff \\RemoveEmptyStaves }\n  }\n"
    } else {
        "  \\layout { }\n"
    };
    out.push_str("staves = <<\n");
    for s in &staves {
        out.push_str(s);
    }
    out.push_str(">>\n\n");
    let midi = "  \\midi {\n    \
                % The TabStaff shows the Staff's notes again: only the Staff plays them.\n    \
                \\context { \\TabStaff \\remove \"Staff_performer\" }\n  }\n";
    if plan.repeats {
        out.push_str(&format!("\\score {{\n  \\staves\n{layout}}}\n\n"));
        out.push_str("% The MIDI plays the repeats as often as Guitar Pro does.\n");
        out.push_str(&format!(
            "\\score {{\n  \\unfoldRepeats \\staves\n{midi}}}\n"
        ));
    } else {
        out.push_str(&format!("\\score {{\n  \\staves\n{layout}{midi}}}\n"));
    }
    out
}

/// LilyPond's staff size, in points.
const DEFAULT_STAFF_SIZE: u32 = 20;

/// How tall a track's staves are, a five-line staff with its spacing being
/// 1: a six-line TabStaff is half as tall again.
fn staff_height(kind: Kind) -> f64 {
    match kind {
        Kind::Strings => 2.5,
        Kind::Drums | Kind::Other => 1.0,
    }
}

/// The staff size at which a system of staves `height` tall fits on an A4
/// page: about 10 staff spaces a staff with its spacing, of the 160 the page
/// has at size 20 (`gp5-a.gp5`'s ten tracks, 20.5 high, overflow at 16 and
/// fit at 15), never below 11.
fn staff_size(height: f64) -> u32 {
    ((300.0 / height.max(1.0)).floor() as u32).clamp(11, DEFAULT_STAFF_SIZE)
}

fn header(song: &Song) -> String {
    let info = &song.info;
    let mut fields: Vec<(&str, String)> = Vec::new();
    let mut add = |name: &'static str, value: &str| {
        if !value.trim().is_empty() {
            fields.push((name, value.trim().to_string()));
        }
    };
    add("title", &info.title);
    add("subtitle", &info.subtitle);
    let composer = if info.music.trim().is_empty() {
        &info.artist
    } else {
        &info.music
    };
    let below: Vec<&str> = [
        Some(info.artist.as_str()).filter(|a| a.trim() != composer.trim()),
        Some(info.album.as_str()),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect();
    add("subsubtitle", &below.join(" — "));
    add("composer", composer);
    if info.words.trim() != composer.trim() {
        add("poet", &info.words);
    }
    if !info.transcriber.trim().is_empty() {
        add("arranger", &format!("Tab: {}", info.transcriber.trim()));
    }
    add("copyright", &info.copyright);
    if fields.is_empty() {
        return String::new();
    }
    let mut out = String::from("\\header {\n");
    for (name, value) in fields {
        out.push_str(&format!("  {name} = {}\n", names::string(&value)));
    }
    out.push_str("}\n\n");
    out
}

fn part<'a>(song: &'a Song, track: &'a Track) -> Part<'a> {
    let kind = match track.kind {
        TrackKind::Drums => Kind::Drums,
        TrackKind::Strings if !track.tuning.is_empty() => Kind::Strings,
        _ => Kind::Other,
    };
    let mut voices: Vec<usize> = Vec::new();
    for bar in &track.bars {
        for (v, voice) in bar.voices.iter().enumerate() {
            if !voice.is_empty() && !voices.contains(&v) {
                voices.push(v);
            }
        }
    }
    voices.sort_unstable();
    if voices.is_empty() {
        voices.push(0);
    }
    Part {
        song,
        track,
        kind,
        voices,
    }
}

/// The track's staff or staves, inside `staves`' `<< >>`.
fn staff(part: &Part, vars: &[String]) -> String {
    let track = part.track;
    let mut with: Vec<String> = Vec::new();
    let name = track.name.trim();
    let capo =
        (part.kind == Kind::Strings && track.capo > 0).then(|| format!("capo {}", track.capo));
    let lines = name_lines(name);
    match (lines.len(), &capo) {
        (1, None) => with.push(format!("instrumentName = {}", names::string(name))),
        (0, None) => {}
        (_, capo) => {
            let mut column: Vec<String> = lines.iter().map(|l| names::string(l)).collect();
            if let Some(capo) = capo {
                column.push(format!("\\small {}", names::string(capo)));
            }
            with.push(format!(
                "instrumentName = \\markup \\center-column {{ {} }}",
                column.join(" ")
            ));
        }
    }
    if !track.short_name.trim().is_empty() {
        with.push(format!(
            "shortInstrumentName = {}",
            names::string(&track.short_name)
        ));
    }
    let instrument = format!(
        "midiInstrument = {}",
        names::string(names::instrument(track.midi.program))
    );
    let voices = |context: &str, first: &str| -> String {
        if vars.len() == 1 {
            format!("\\new {context} {{ {first}\\{} }}", vars[0])
        } else {
            let mut s = String::from("<<\n");
            for (k, v) in vars.iter().enumerate() {
                let lead = if k == 0 { first } else { "" };
                s.push_str(&format!("        \\new {context} {{ {lead}\\{v} }}\n"));
            }
            s.push_str("      >>");
            s
        }
    };
    let block = |with: &[String], indent: &str| -> String {
        if with.is_empty() {
            String::new()
        } else {
            let mut s = String::from(" \\with {\n");
            for w in with {
                s.push_str(&format!("{indent}  {w}\n"));
            }
            s.push_str(&format!("{indent}}}"));
            s
        }
    };
    match part.kind {
        Kind::Strings => {
            // Open strings with the capo, lowest first, as \stringTuning
            // takes them.
            let open: Vec<String> = track
                .tuning
                .iter()
                .rev()
                .map(|p| names::pitch(p.saturating_add(track.capo).min(127), false))
                .collect();
            let bass = track.tuning.iter().max().is_some_and(|&p| p < 55);
            let clef = if bass { "bass_8" } else { "treble_8" };
            let staff_with = [instrument, "\\omit StringNumber".to_string()];
            format!(
                "  \\new StaffGroup{} <<\n    \\new Staff{} {}\n    \\new TabStaff{} {}\n  >>\n",
                block(&with, "  "),
                block(&staff_with, "    "),
                voices("Voice", &format!("\\clef \"{clef}\" ")),
                block(
                    &[format!(
                        "stringTunings = \\stringTuning <{}>",
                        open.join(" ")
                    )],
                    "    "
                ),
                voices("TabVoice", ""),
            )
        }
        Kind::Drums => format!(
            "  \\new DrumStaff{} {}\n",
            block(&with, "  "),
            voices("DrumVoice", "")
        ),
        Kind::Other => {
            with.push(instrument);
            let clef = if low(part) { "bass" } else { "treble" };
            format!(
                "  \\new Staff{} {}\n",
                block(&with, "  "),
                voices("Voice", &format!("\\clef \"{clef}\" "))
            )
        }
    }
}

/// The longest instrument name that fits in the first system's indent on
/// one line; a longer one, such as *Distortion Guitar*, would run past the
/// page's left edge.
const NAME_LINE: usize = 12;

/// A track's name in the lines it is printed in: one, or two split at the
/// space nearest its middle when it is longer than `NAME_LINE`.
fn name_lines(name: &str) -> Vec<&str> {
    if name.is_empty() {
        return Vec::new();
    }
    let length = name.chars().count();
    if length <= NAME_LINE {
        return vec![name];
    }
    let middle = name.len() / 2;
    let split = name
        .match_indices(' ')
        .map(|(i, _)| i)
        .min_by_key(|&i| i.abs_diff(middle));
    match split {
        Some(i) => vec![name[..i].trim_end(), name[i + 1..].trim_start()],
        None => vec![name],
    }
}

/// Whether a track's notes lie low enough for a bass clef: their mean pitch
/// is below G3 (55).
fn low(part: &Part) -> bool {
    let (mut sum, mut count) = (0u64, 0u64);
    for bar in &part.track.bars {
        for voice in &bar.voices {
            for beat in &voice.beats {
                for note in &beat.notes {
                    if let Some(p) = part.track.pitch(note) {
                        sum += u64::from(p);
                        count += 1;
                    }
                }
            }
        }
    }
    count > 0 && sum < 55 * count
}

#[cfg(test)]
mod tests;
