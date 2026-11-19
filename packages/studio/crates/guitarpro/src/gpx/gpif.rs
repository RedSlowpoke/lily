//! `score.gpif` into the model (README §3.3–§3.8, §4). The XML's last six
//! lists are flat tables whose items name one another by id: a master bar
//! its bars (one a track), a bar its four voices, a voice its beats, a beat
//! its rhythm and notes. Ids are shared freely, so each is resolved where it
//! is used and the item copied; a budget on the beats and notes copied keeps
//! a file that shares one voice everywhere from growing without bound.
//!
//! Where GPX counts differently from the model, this converts: strings from
//! the lowest (0) to the highest (1), `Tuning` reversed, tempos to quarter
//! notes. Drum notes are `Element` and `Variation` of Guitar Pro 6's drum
//! kit, which has no table to General MIDI yet (D166); they are left out,
//! and their beats become rests that keep the time.
use std::collections::HashMap;
use std::str::FromStr;

use roxmltree::{Document, Node};

use crate::Error;
use crate::model::*;

/// How many beats and notes one file may copy into the model: far more
/// than a long song with many tracks has.
const MAX_ITEMS: usize = 2_000_000;
/// A guitar's strings, highest first: the tuning of a fretted track that
/// gives none.
const STANDARD_TUNING: [u8; 6] = [64, 59, 55, 50, 45, 40];
/// The most strings a track may have.
const MAX_STRINGS: usize = 32;
/// Ticks of a whole note for a tempo's place in its bar.
const TEMPO_TICKS: f64 = 1920.0;

fn bad(what: impl std::fmt::Display) -> Error {
    Error::Container(format!("its score.gpif {what}"))
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|c| c.has_tag_name(name))
}

fn elements<'a, 'i>(node: Node<'a, 'i>, name: &str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |c| c.has_tag_name(name))
}

/// An element's text, its CDATA included.
fn text(node: Node) -> String {
    node.children()
        .filter(Node::is_text)
        .filter_map(|c| c.text())
        .collect()
}

/// A child's text, trimmed.
fn field(node: Node, name: &str) -> Option<String> {
    child(node, name).map(|c| text(c).trim().to_owned())
}

/// `Properties/Property[@name]`, as notes, beats and tracks hold them.
fn property<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    child(node, "Properties")?
        .children()
        .find(|c| c.has_tag_name("Property") && c.attribute("name") == Some(name))
}

/// A property's value: the text of its first element (`<Fret>3</Fret>`,
/// `<Flags>2</Flags>`, `<Float>50</Float>`), trimmed.
fn property_value(node: Node, name: &str) -> Option<String> {
    let value = property(node, name)?.children().find(Node::is_element)?;
    Some(text(value).trim().to_owned())
}

fn number<T: FromStr>(text: &str, what: impl std::fmt::Display) -> Result<T, Error> {
    text.trim()
        .parse()
        .map_err(|_| bad(format!("has {what} {:?}", text.trim())))
}

/// A list of ids, `0 -1 -1 -1`.
fn ids(text: &str, what: &str) -> Result<Vec<i64>, Error> {
    text.split_whitespace().map(|t| number(t, what)).collect()
}

/// An item of a table by its id.
fn table<'a, 'i>(
    root: Node<'a, 'i>,
    list: &str,
    item: &str,
) -> Result<HashMap<i64, Node<'a, 'i>>, Error> {
    let mut map = HashMap::new();
    for node in child(root, list)
        .into_iter()
        .flat_map(|l| elements(l, item))
    {
        let id = node
            .attribute("id")
            .ok_or_else(|| bad(format!("has a {item} without an id")))?;
        map.insert(number(id, format!("a {item} id"))?, node);
    }
    Ok(map)
}

/// Reads a `score.gpif` into a song.
pub fn parse(xml: &str) -> Result<Song, Error> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let doc = Document::parse(xml).map_err(|e| bad(format!("is not well-formed XML ({e})")))?;
    let root = doc.root_element();
    if !root.has_tag_name("GPIF") {
        return Err(bad(format!(
            "begins with <{}>, not <GPIF>",
            root.tag_name().name()
        )));
    }
    let revision = field(root, "GPRevision").unwrap_or_default();
    let version = if revision.is_empty() {
        "GPIF".to_owned()
    } else {
        format!("GPIF {revision}")
    };

    let heads = child(root, "Tracks")
        .into_iter()
        .flat_map(|t| elements(t, "Track"))
        .map(track)
        .collect::<Result<Vec<_>, _>>()?;

    let mut master_bars: Vec<MasterBar> = Vec::new();
    let mut bar_ids = Vec::new();
    for (i, node) in child(root, "MasterBars")
        .into_iter()
        .flat_map(|m| elements(m, "MasterBar"))
        .enumerate()
    {
        let (bar, ids) = master_bar(node, i, master_bars.last())?;
        master_bars.push(bar);
        bar_ids.push(ids);
    }
    let (tempo, tempo_name, tempo_changes) = tempos(root, &master_bars)?;

    let mut reader = Reader {
        bars: table(root, "Bars", "Bar")?,
        voices: table(root, "Voices", "Voice")?,
        beats: table(root, "Beats", "Beat")?,
        notes: table(root, "Notes", "Note")?,
        rhythms: table(root, "Rhythms", "Rhythm")?,
        spent: 0,
        costs: HashMap::new(),
    };
    let mut tracks = Vec::with_capacity(heads.len());
    for (t, head) in heads.iter().enumerate() {
        let mut bars = Vec::with_capacity(bar_ids.len());
        for (m, ids) in bar_ids.iter().enumerate() {
            let id = *ids
                .get(t)
                .ok_or_else(|| bad(format!("gives master bar {m} no bar for track {t}")))?;
            bars.push(if id < 0 {
                Bar::default()
            } else {
                reader.bar(head, id)?
            });
        }
        tracks.push(Track {
            bars,
            ..head.track.clone()
        });
    }

    Ok(Song {
        version,
        info: info(child(root, "Score")),
        tempo,
        tempo_name,
        tempo_changes,
        master_bars,
        tracks,
    })
}

fn info(score: Option<Node>) -> SongInfo {
    let get = |name| score.and_then(|s| field(s, name)).unwrap_or_default();
    let mut words = get("Words");
    let music = get("Music");
    if words.is_empty() && music.is_empty() {
        words = get("WordsAndMusic");
    }
    let mut notices: Vec<String> = get("Notices")
        .lines()
        .map(|l| l.trim_end().to_owned())
        .collect();
    while notices.last().is_some_and(String::is_empty) {
        notices.pop();
    }
    SongInfo {
        title: get("Title"),
        subtitle: get("SubTitle"),
        artist: get("Artist"),
        album: get("Album"),
        words,
        music,
        copyright: get("Copyright"),
        transcriber: get("Tabber"),
        instructions: get("Instructions"),
        notices,
    }
}

/// A master bar and the ids of its tracks' bars. A meter or key it does
/// not give is `previous`'s.
fn master_bar(
    node: Node,
    index: usize,
    previous: Option<&MasterBar>,
) -> Result<(MasterBar, Vec<i64>), Error> {
    let mut bar = MasterBar::default();
    if let Some(previous) = previous {
        bar.time = previous.time;
        bar.key = previous.key;
    }
    if let Some(time) = field(node, "Time") {
        let what = || bad(format!("gives master bar {index} the time {time:?}"));
        let (num, den) = time.split_once('/').ok_or_else(what)?;
        let numerator: u8 = num.trim().parse().map_err(|_| what())?;
        let denominator: u8 = den.trim().parse().map_err(|_| what())?;
        if numerator == 0 || !matches!(denominator, 1 | 2 | 4 | 8 | 16 | 32) {
            return Err(what());
        }
        bar.time = TimeSignature {
            numerator,
            denominator,
        };
    }
    if let Some(key) = child(node, "Key") {
        if let Some(count) = field(key, "AccidentalCount") {
            let fifths: i8 = number(&count, format!("master bar {index}'s key"))?;
            if !(-7..=7).contains(&fifths) {
                return Err(bad(format!(
                    "gives master bar {index} a key of {fifths} accidentals"
                )));
            }
            bar.key.fifths = fifths;
        }
        if let Some(mode) = field(key, "Mode") {
            bar.key.minor = mode.eq_ignore_ascii_case("minor");
        }
    }
    if let Some(repeat) = child(node, "Repeat") {
        bar.repeat_start = repeat.attribute("start") == Some("true");
        if repeat.attribute("end") == Some("true") {
            let count: u8 = repeat
                .attribute("count")
                .map_or(Ok(2), |c| number(c, "a repeat count"))?;
            // A repeat is played twice at least; GPX's count is the plays.
            bar.repeat_end = Some(count.max(2));
        }
    }
    if let Some(endings) = field(node, "AlternateEndings") {
        let mut endings: Vec<u8> = endings
            .split_whitespace()
            .map(|e| number(e, "an alternate ending"))
            .collect::<Result<_, _>>()?;
        endings.retain(|&e| e > 0);
        endings.sort_unstable();
        endings.dedup();
        bar.alternate_endings = endings;
    }
    if let Some(section) = child(node, "Section") {
        let text = field(section, "Text").unwrap_or_default();
        let letter = field(section, "Letter").unwrap_or_default();
        let marker = if text.is_empty() { letter } else { text };
        bar.marker = Some(marker).filter(|m| !m.is_empty());
    }
    if let Some(directions) = child(node, "Directions") {
        bar.directions = directions
            .children()
            .filter(|c| c.has_tag_name("Target") || c.has_tag_name("Jump"))
            .map(|c| direction(text(c).trim()))
            .collect();
    }
    if let Some(feel) = field(node, "TripletFeel") {
        bar.triplet_feel = if feel.ends_with("8th") {
            TripletFeel::Eighth
        } else if feel.ends_with("16th") {
            TripletFeel::Sixteenth
        } else {
            TripletFeel::None
        };
    }
    bar.double_bar = child(node, "DoubleBar").is_some_and(|d| text(d).trim() != "false");
    let bars = ids(
        &field(node, "Bars").unwrap_or_default(),
        "a master bar's bar id",
    )?;
    Ok((bar, bars))
}

/// GPX's name for a target or a jump, which spells *dal segno* `DaSegno`.
fn direction(name: &str) -> Direction {
    match name {
        "Segno" => Direction::Segno,
        "SegnoSegno" => Direction::SegnoSegno,
        "Coda" => Direction::Coda,
        "DoubleCoda" => Direction::DoubleCoda,
        "Fine" => Direction::Fine,
        "DaCapo" => Direction::DaCapo,
        "DaCapoAlCoda" => Direction::DaCapoAlCoda,
        "DaCapoAlDoubleCoda" => Direction::DaCapoAlDoubleCoda,
        "DaCapoAlFine" => Direction::DaCapoAlFine,
        "DaSegno" | "DalSegno" => Direction::DalSegno,
        "DaSegnoAlCoda" | "DalSegnoAlCoda" => Direction::DalSegnoAlCoda,
        "DaSegnoAlDoubleCoda" | "DalSegnoAlDoubleCoda" => Direction::DalSegnoAlDoubleCoda,
        "DaSegnoAlFine" | "DalSegnoAlFine" => Direction::DalSegnoAlFine,
        "DaSegnoSegno" | "DalSegnoSegno" => Direction::DalSegnoSegno,
        "DaSegnoSegnoAlCoda" | "DalSegnoSegnoAlCoda" => Direction::DalSegnoSegnoAlCoda,
        "DaSegnoSegnoAlDoubleCoda" | "DalSegnoSegnoAlDoubleCoda" => {
            Direction::DalSegnoSegnoAlDoubleCoda
        }
        "DaSegnoSegnoAlFine" | "DalSegnoSegnoAlFine" => Direction::DalSegnoSegnoAlFine,
        "DaCoda" => Direction::DaCoda,
        "DaDoubleCoda" => Direction::DaDoubleCoda,
        other => Direction::Other(other.to_owned()),
    }
}

/// The master track's tempo automations: the one at the very start is the
/// song's tempo, the rest its changes. `Value` is the tempo and its beat's
/// unit (1 an eighth, 2 a quarter, 3 a dotted quarter, 4 a half, 5 a
/// dotted half), `Position` a fraction of the bar.
fn tempos(root: Node, bars: &[MasterBar]) -> Result<(f64, String, Vec<TempoChange>), Error> {
    let automations = child(root, "MasterTrack")
        .and_then(|m| child(m, "Automations"))
        .into_iter()
        .flat_map(|a| elements(a, "Automation"))
        .filter(|a| field(*a, "Type").as_deref() == Some("Tempo"));
    let mut found = Vec::new();
    for automation in automations {
        let bar: usize = number(
            &field(automation, "Bar").unwrap_or_else(|| "0".into()),
            "a tempo's bar",
        )?;
        let Some(master) = bars.get(bar) else {
            continue;
        };
        let position: f64 = number(
            &field(automation, "Position").unwrap_or_else(|| "0".into()),
            "a tempo's position",
        )?;
        let value = field(automation, "Value").unwrap_or_default();
        let mut parts = value.split_whitespace();
        let bpm: f64 = number(parts.next().unwrap_or_default(), "a tempo")?;
        let unit: u8 = parts
            .next()
            .map_or(Ok(2), |u| number(u, "a tempo's unit"))?;
        let quarters = bpm
            * match unit {
                1 => 0.5,
                3 => 1.5,
                4 => 2.0,
                5 => 3.0,
                _ => 1.0,
            };
        if !quarters.is_finite() || quarters <= 0.0 {
            return Err(bad(format!("has a tempo of {value:?}")));
        }
        let length = master.length();
        let ticks = (position.clamp(0.0, 1.0) * f64::from(length.num) / f64::from(length.den)
            * TEMPO_TICKS)
            .round() as u32;
        let change = TempoChange {
            bar,
            offset: Fraction::new(ticks, TEMPO_TICKS as u32),
            bpm: quarters,
        };
        found.push((change, field(automation, "Text").unwrap_or_default()));
    }
    found.sort_by_key(|(a, _)| (a.bar, a.offset));
    let mut found = found.into_iter().peekable();
    let (tempo, name) = match found.next_if(|(c, _)| c.bar == 0 && c.offset == Fraction::ZERO) {
        Some((first, name)) => (first.bpm, name),
        None => (120.0, String::new()),
    };
    Ok((tempo, name, found.map(|(c, _)| c).collect()))
}

/// A track as its `Track` element gives it, before its bars; and what its
/// bars need: its string count and its chord diagrams by id.
struct TrackHead {
    track: Track,
    strings: usize,
    diagrams: HashMap<i64, Chord>,
}

fn track(node: Node) -> Result<TrackHead, Error> {
    let name = field(node, "Name").unwrap_or_default();
    let instrument = child(node, "Instrument")
        .and_then(|i| i.attribute("ref"))
        .unwrap_or_default()
        .to_owned();
    let mut midi = MidiChannel::default();
    let mut percussion = false;
    if let Some(gm) = child(node, "GeneralMidi") {
        percussion = gm.attribute("table") == Some("Percussion");
        let read = |key: &str, default: u8| -> Result<u8, Error> {
            field(gm, key).map_or(Ok(default), |v| {
                number(&v, format!("track {name:?}'s {key}"))
            })
        };
        midi.program = read("Program", midi.program)?;
        midi.port = read("Port", midi.port)?;
        midi.channel = read("PrimaryChannel", midi.channel)?;
        midi.effect_channel = read("SecondaryChannel", midi.effect_channel)?;
    }
    // The channel strip's twelfth and thirteenth parameters are its pan and
    // volume, 0 to 1, as the strip's own automations of them say.
    if let Some(parameters) = child(node, "RSE")
        .and_then(|r| child(r, "ChannelStrip"))
        .and_then(|c| field(c, "Parameters"))
    {
        let values: Vec<f64> = parameters
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        let scale = |v: f64| (v.clamp(0.0, 1.0) * 127.0).round() as u8;
        if let [.., pan, volume, _, _, _] = values[..]
            && values.len() == 16
        {
            midi.pan = scale(pan);
            midi.volume = scale(volume);
        }
    }
    let kind = if percussion || midi.channel == 9 {
        TrackKind::Drums
    } else if matches!(midi.program, 24..=39 | 104..=107) || fretted(&instrument) {
        TrackKind::Strings
    } else {
        TrackKind::Other
    };
    let mut tuning: Vec<u8> = match property_value(node, "Tuning") {
        Some(pitches) => pitches
            .split_whitespace()
            .map(|p| number::<u8>(p, format!("track {name:?}'s tuning")))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    if tuning.len() > MAX_STRINGS || tuning.iter().any(|&p| p > 127) {
        return Err(bad(format!("gives track {name:?} the tuning {tuning:?}")));
    }
    tuning.reverse();
    if tuning.is_empty() && kind != TrackKind::Drums {
        tuning = STANDARD_TUNING.to_vec();
    }
    let capo = property_value(node, "CapoFret")
        .map_or(Ok(0), |c| number(&c, format!("track {name:?}'s capo")))?;
    let lyrics = child(node, "Lyrics")
        .into_iter()
        .flat_map(|l| elements(l, "Line"))
        .filter_map(|line| {
            let text = field(line, "Text").unwrap_or_default();
            let start_bar = field(line, "Offset")?.parse().ok()?;
            (!text.is_empty()).then_some(LyricLine { start_bar, text })
        })
        .collect();
    let diagrams = diagrams(node);
    Ok(TrackHead {
        strings: tuning.len(),
        track: Track {
            short_name: field(node, "ShortName").unwrap_or_default(),
            name,
            kind,
            instrument,
            tuning,
            capo,
            frets: 24,
            midi,
            lyrics,
            bars: Vec::new(),
        },
        diagrams,
    })
}

/// A GPX instrument that is fretted (README §5.1): `e-gtr6`, `s-gtr6`,
/// `e-bass4`, a banjo or a ukulele.
fn fretted(instrument: &str) -> bool {
    let i = instrument.to_ascii_lowercase();
    ["gtr", "bass", "bnj", "banjo", "ukl", "uke", "ukulele"]
        .iter()
        .any(|k| i.contains(k))
}

/// The track's `DiagramCollection`: each item's name, and its frets with
/// string 0 the lowest, turned to the model's first string first.
fn diagrams(node: Node) -> HashMap<i64, Chord> {
    let mut out = HashMap::new();
    let items = property(node, "DiagramCollection")
        .and_then(|p| child(p, "Items"))
        .into_iter()
        .flat_map(|i| elements(i, "Item"));
    for item in items {
        let Some(id) = item.attribute("id").and_then(|i| i.trim().parse().ok()) else {
            continue;
        };
        let mut chord = Chord {
            name: item.attribute("name").unwrap_or_default().trim().to_owned(),
            frets: Vec::new(),
        };
        if let Some(diagram) = child(item, "Diagram") {
            let count: usize = diagram
                .attribute("stringCount")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0)
                .min(MAX_STRINGS);
            let base: u8 = diagram
                .attribute("baseFret")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0);
            chord.frets = vec![None; count];
            for fret in elements(diagram, "Fret") {
                let string: Option<usize> = fret.attribute("string").and_then(|s| s.parse().ok());
                let at: Option<u8> = fret.attribute("fret").and_then(|f| f.parse().ok());
                if let (Some(string), Some(at)) = (string, at)
                    && string < count
                {
                    chord.frets[count - 1 - string] = Some(base.saturating_add(at));
                }
            }
        }
        out.insert(id, chord);
    }
    out
}

/// The id tables, and what has been copied out of them so far.
struct Reader<'a, 'i> {
    bars: HashMap<i64, Node<'a, 'i>>,
    voices: HashMap<i64, Node<'a, 'i>>,
    beats: HashMap<i64, Node<'a, 'i>>,
    notes: HashMap<i64, Node<'a, 'i>>,
    rhythms: HashMap<i64, Node<'a, 'i>>,
    spent: usize,
    /// What copying a beat costs, itself and its notes, by id.
    costs: HashMap<i64, usize>,
}

fn get<'a, 'i>(
    table: &HashMap<i64, Node<'a, 'i>>,
    kind: &str,
    id: i64,
) -> Result<Node<'a, 'i>, Error> {
    table
        .get(&id)
        .copied()
        .ok_or_else(|| bad(format!("names {kind} {id}, which it does not have")))
}

/// A beat as the voice reads it: the beat, and whether it is a grace beat,
/// on the beat (`Some(true)`) or before it.
struct BeatRead {
    beat: Beat,
    grace: Option<bool>,
}

impl Reader<'_, '_> {
    /// Counts `items` against the budget before they are copied, so a file
    /// past it fails before the copying starts.
    fn spend(&mut self, items: usize) -> Result<(), Error> {
        self.spent = self.spent.saturating_add(items);
        if self.spent > MAX_ITEMS {
            return Err(bad(format!(
                "repeats its beats and notes more than {MAX_ITEMS} times"
            )));
        }
        Ok(())
    }

    /// A track's bar: its voices in their places, a voice the bar leaves
    /// out (`-1`) empty when a later one is there.
    fn bar(&mut self, head: &TrackHead, id: i64) -> Result<Bar, Error> {
        let node = get(&self.bars, "bar", id)?;
        let mut voices = Vec::new();
        let list = ids(&field(node, "Voices").unwrap_or_default(), "a voice id")?;
        for (k, id) in list.into_iter().enumerate() {
            if id < 0 {
                continue;
            }
            voices.resize_with(k, Voice::default);
            voices.push(self.voice(head, id)?);
        }
        Ok(Bar { voices })
    }

    /// A voice's beats. A grace beat takes no time: its notes become the
    /// `Grace` of the next beat's notes on the same strings, and are dropped
    /// where the next beat has none there (README §4).
    fn voice(&mut self, head: &TrackHead, id: i64) -> Result<Voice, Error> {
        let node = get(&self.voices, "voice", id)?;
        let mut beats = Vec::new();
        let mut graces: Vec<(Note, u8, bool)> = Vec::new();
        let mut last_fret: HashMap<u8, u8> = HashMap::new();
        let list = ids(&field(node, "Beats").unwrap_or_default(), "a beat id")?;
        let mut cost = 0usize;
        for &id in &list {
            cost = cost.saturating_add(self.cost(id)?);
        }
        self.spend(cost)?;
        for id in list {
            let read = self.beat(head, id, &last_fret)?;
            if let Some(on_beat) = read.grace {
                let value = read.beat.duration.value;
                graces.extend(read.beat.notes.into_iter().map(|n| (n, value, on_beat)));
                continue;
            }
            let mut beat = read.beat;
            for note in &mut beat.notes {
                if let Some((g, value, on_beat)) =
                    graces.iter().find(|(g, ..)| g.string == note.string)
                {
                    note.grace = Some(Grace {
                        fret: g.fret,
                        value: *value,
                        dynamic: g.dynamic,
                        transition: if g.slide.is_some() {
                            GraceTransition::Slide
                        } else if g.bend.is_some() {
                            GraceTransition::Bend
                        } else if g.hammer {
                            GraceTransition::Hammer
                        } else {
                            GraceTransition::None
                        },
                        on_beat: *on_beat,
                        dead: g.dead,
                    });
                }
                last_fret.insert(note.string, note.fret);
            }
            graces.clear();
            beats.push(beat);
        }
        Ok(Voice { beats })
    }

    /// A beat and its notes, as `spend` counts them.
    fn cost(&mut self, id: i64) -> Result<usize, Error> {
        if let Some(&cost) = self.costs.get(&id) {
            return Ok(cost);
        }
        let node = get(&self.beats, "beat", id)?;
        let notes = field(node, "Notes").map_or(0, |n| n.split_whitespace().count());
        self.costs.insert(id, 1 + notes);
        Ok(1 + notes)
    }

    fn beat(
        &self,
        head: &TrackHead,
        id: i64,
        last_fret: &HashMap<u8, u8>,
    ) -> Result<BeatRead, Error> {
        let node = get(&self.beats, "beat", id)?;
        let rhythm = child(node, "Rhythm")
            .and_then(|r| r.attribute("ref"))
            .ok_or_else(|| bad(format!("gives beat {id} no rhythm")))?;
        let duration = rhythm_of(get(
            &self.rhythms,
            "rhythm",
            number(rhythm, "a rhythm id")?,
        )?)?;
        let dynamic = match field(node, "Dynamic").as_deref() {
            Some("PPP") => Dynamic::Ppp,
            Some("PP") => Dynamic::Pp,
            Some("P") => Dynamic::P,
            Some("MP") => Dynamic::Mp,
            Some("MF") => Dynamic::Mf,
            Some("FF") => Dynamic::Ff,
            Some("FFF") => Dynamic::Fff,
            _ => Dynamic::F,
        };
        let mut beat = Beat {
            duration,
            ..Beat::default()
        };
        let mut tapped = false;
        if let Some(list) = field(node, "Notes") {
            for id in ids(&list, "a note id")? {
                let note = get(&self.notes, "note", id)?;
                if let Some((note, tap)) = note_of(head, note, id, dynamic, last_fret)? {
                    tapped |= tap;
                    beat.notes.push(note);
                }
            }
        }
        if beat.notes.is_empty() {
            beat.status = BeatStatus::Rest;
        }
        beat.text = field(node, "FreeText").filter(|t| !t.is_empty());
        beat.chord = field(node, "Chord")
            .and_then(|c| c.parse().ok())
            .and_then(|c| head.diagrams.get(&c).cloned());
        let direction = |name| match property_value(node, name).as_deref() {
            Some("Up") => Some(StrokeDirection::Up),
            Some("Down") => Some(StrokeDirection::Down),
            _ => None,
        };
        let effects = &mut beat.effects;
        effects.stroke = direction("Brush").map(|direction| Stroke {
            direction,
            value: 0,
        });
        effects.pick_stroke = direction("PickStroke");
        effects.rasgueado = property(node, "Rasgueado").is_some();
        effects.slap = if tapped {
            Some(Slap::Tap)
        } else if property(node, "Slapped").is_some() {
            Some(Slap::Slap)
        } else if property(node, "Popped").is_some() {
            Some(Slap::Pop)
        } else {
            None
        };
        effects.tremolo_bar = bend(node, "WhammyBar", "WhammyBar");
        effects.fade_in = field(node, "Fadding").as_deref() == Some("FadeIn");
        effects.wide_vibrato = field(node, "Vibrato").as_deref() == Some("Wide");
        let tremolo = field(node, "Tremolo")
            .and_then(|t| t.rsplit_once('/').and_then(|(_, v)| v.parse::<u8>().ok()))
            .filter(|v| matches!(*v, 8 | 16 | 32));
        for note in &mut beat.notes {
            note.tremolo_picking = tremolo;
        }
        let grace = match field(node, "GraceNotes").as_deref() {
            Some("OnBeat") => Some(true),
            Some("BeforeBeat") => Some(false),
            _ => None,
        };
        Ok(BeatRead { beat, grace })
    }
}

fn rhythm_of(node: Node) -> Result<Duration, Error> {
    let value = match field(node, "NoteValue").as_deref() {
        Some("Whole") => 1,
        Some("Half") => 2,
        Some("Quarter") => 4,
        Some("Eighth") => 8,
        Some("16th") => 16,
        Some("32nd") => 32,
        Some("64th") => 64,
        Some("128th") => 128,
        other => return Err(bad(format!("has a note value of {other:?}"))),
    };
    let dots: u8 = child(node, "AugmentationDot")
        .and_then(|d| d.attribute("count"))
        .map_or(Ok(0), |c| number(c, "a dot count"))?;
    if dots > 4 {
        return Err(bad(format!("gives a rhythm {dots} dots")));
    }
    let tuplet = child(node, "PrimaryTuplet").and_then(|t| {
        let notes: u8 = t.attribute("num")?.trim().parse().ok()?;
        let space: u8 = t.attribute("den")?.trim().parse().ok()?;
        (notes > 0 && space > 0 && notes != space).then_some(Tuplet { notes, space })
    });
    Ok(Duration {
        value,
        dots,
        tuplet,
    })
}

/// A note on a fretted track, and whether it is tapped; `None` for a drum
/// note (D166's gap) or one with no string.
fn note_of(
    head: &TrackHead,
    node: Node,
    id: i64,
    dynamic: Dynamic,
    last_fret: &HashMap<u8, u8>,
) -> Result<Option<(Note, bool)>, Error> {
    if head.track.kind == TrackKind::Drums {
        return Ok(None);
    }
    let Some(string) = property_value(node, "String") else {
        return Ok(None);
    };
    let from_lowest: usize = number(&string, format!("note {id}'s string"))?;
    if from_lowest >= head.strings {
        return Err(bad(format!(
            "puts note {id} on string {from_lowest} of a {}-string track",
            head.strings
        )));
    }
    let string = (head.strings - from_lowest) as u8;
    let tie = child(node, "Tie").is_some_and(|t| t.attribute("destination") == Some("true"));
    let fret = match property_value(node, "Fret") {
        Some(fret) => number(&fret, format!("note {id}'s fret"))?,
        None if tie => last_fret.get(&string).copied().unwrap_or(0),
        None => return Err(bad(format!("gives note {id} no fret"))),
    };
    let accent: u32 = field(node, "Accent")
        .and_then(|a| a.parse().ok())
        .unwrap_or(0);
    let slide = property_value(node, "Slide")
        .and_then(|f| f.parse::<u32>().ok())
        .and_then(|flags| {
            [
                Slide::Shift,
                Slide::Legato,
                Slide::OutDownwards,
                Slide::OutUpwards,
                Slide::IntoFromBelow,
                Slide::IntoFromAbove,
            ]
            .into_iter()
            .enumerate()
            .find(|(bit, _)| flags & (1 << bit) != 0)
            .map(|(_, slide)| slide)
        });
    let harmonic_fret = property_value(node, "HarmonicFret")
        .and_then(|f| f.parse::<f32>().ok())
        .map_or(fret.saturating_add(12), |f| {
            f.round().clamp(0.0, 255.0) as u8
        });
    let harmonic = match property_value(node, "HarmonicType").as_deref() {
        Some("Natural") => Some(Harmonic::Natural),
        Some("Artificial") => Some(Harmonic::Artificial),
        Some("Pinch") => Some(Harmonic::Pinch),
        Some("Tap" | "Tapped") => Some(Harmonic::Tapped {
            fret: harmonic_fret,
        }),
        Some("Semi") => Some(Harmonic::Semi),
        Some("Feedback") => Some(Harmonic::Feedback),
        _ => None,
    };
    let trill = field(node, "Trill")
        .and_then(|t| t.parse().ok())
        .map(|fret| Trill { fret, value: 16 });
    let note = Note {
        string,
        fret,
        tie,
        dead: property(node, "Muted").is_some(),
        ghost: child(node, "AntiAccent").is_some(),
        accent: if accent & 4 != 0 {
            Accent::Heavy
        } else if accent & 8 != 0 {
            Accent::Normal
        } else {
            Accent::None
        },
        dynamic,
        staccato: accent & 1 != 0,
        palm_mute: property(node, "PalmMuted").is_some(),
        let_ring: child(node, "LetRing").is_some(),
        hammer: property(node, "HopoOrigin").is_some(),
        slide,
        bend: bend(node, "Bended", "Bend"),
        harmonic,
        vibrato: child(node, "Vibrato").is_some(),
        trill,
        tremolo_picking: None,
        grace: None,
    };
    Ok(Some((note, property(node, "Tapped").is_some())))
}

/// A bend's or the whammy bar's curve when `enabled` is set: its origin,
/// middle and destination values (50 to the semitone) at their offsets (0
/// to 100 across the note).
fn bend(node: Node, enabled: &str, prefix: &str) -> Option<Bend> {
    property(node, enabled)?;
    let value = |name: &str| {
        property_value(node, &format!("{prefix}{name}")).and_then(|v| v.parse::<f32>().ok())
    };
    let origin = value("OriginValue").unwrap_or(0.0);
    let middle = value("MiddleValue");
    let destination = value("DestinationValue").unwrap_or(middle.unwrap_or(origin));
    let mut points = vec![(value("OriginOffset").unwrap_or(0.0), origin)];
    if let Some(middle) = middle {
        let first = value("MiddleOffset1").unwrap_or(50.0);
        points.push((first, middle));
        if let Some(second) = value("MiddleOffset2").filter(|s| *s != first) {
            points.push((second, middle));
        }
    }
    points.push((value("DestinationOffset").unwrap_or(100.0), destination));
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(Bend {
        points: points
            .into_iter()
            .map(|(offset, value)| BendPoint {
                position: (offset / 100.0).clamp(0.0, 1.0),
                semitones: value / 50.0,
            })
            .collect(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small `score.gpif`: a guitar with a capo and a chord, a drum
    /// track, two master bars, and items shared between them.
    pub(crate) const FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<GPIF>
  <GPRevision>9934</GPRevision>
  <Score>
    <Title><![CDATA[ Song ]]></Title>
    <Artist><![CDATA[Band]]></Artist>
    <Words><![CDATA[]]></Words>
    <WordsAndMusic><![CDATA[Someone]]></WordsAndMusic>
    <Tabber><![CDATA[Tabber]]></Tabber>
    <Notices><![CDATA[one
two
]]></Notices>
  </Score>
  <MasterTrack>
    <Tracks>0 1</Tracks>
    <Automations>
      <Automation><Type>Tempo</Type><Bar>1</Bar><Position>0.5</Position><Value>60 4</Value></Automation>
      <Automation><Type>DSPParam_00</Type><Bar>0</Bar><Position>0</Position><Value>0.5</Value></Automation>
      <Automation><Type>Tempo</Type><Bar>0</Bar><Position>0</Position><Value>90 2</Value><Text>Slow</Text></Automation>
    </Automations>
  </MasterTrack>
  <Tracks>
    <Track id="0">
      <Name><![CDATA[Guitar ]]></Name>
      <ShortName><![CDATA[gtr]]></ShortName>
      <Instrument ref="s-gtr6" />
      <RSE><ChannelStrip><Parameters>0 0 0 0 0 0 0 0 0 0 0 0.5 0.795 0 0 0</Parameters></ChannelStrip></RSE>
      <GeneralMidi table="Instrument">
        <Program>25</Program><Port>0</Port><PrimaryChannel>2</PrimaryChannel><SecondaryChannel>3</SecondaryChannel>
      </GeneralMidi>
      <Lyrics><Line><Text><![CDATA[la la]]></Text><Offset>1</Offset></Line><Line><Text><![CDATA[]]></Text><Offset>0</Offset></Line></Lyrics>
      <Properties>
        <Property name="Tuning"><Pitches>40 45 50 55 59 64</Pitches></Property>
        <Property name="CapoFret"><Fret>2</Fret></Property>
        <Property name="DiagramCollection"><Items>
          <Item id="0" name="F"><Diagram stringCount="6" fretCount="5" baseFret="0">
            <Fret string="0" fret="1"/><Fret string="1" fret="3"/><Fret string="5" fret="1"/>
          </Diagram></Item>
        </Items></Property>
      </Properties>
    </Track>
    <Track id="1">
      <Name><![CDATA[Drums]]></Name>
      <Instrument ref="drmkt" />
      <GeneralMidi table="Percussion"><Program>0</Program><PrimaryChannel>9</PrimaryChannel></GeneralMidi>
    </Track>
  </Tracks>
  <MasterBars>
    <MasterBar>
      <Key><AccidentalCount>-1</AccidentalCount><Mode>Major</Mode></Key>
      <Time>4/4</Time>
      <Repeat start="true" end="false" count="0" />
      <Section><Letter><![CDATA[A]]></Letter><Text><![CDATA[Intro]]></Text></Section>
      <Directions><Target>Segno</Target><Jump>DaSegnoAlCoda</Jump></Directions>
      <Bars>0 1</Bars>
    </MasterBar>
    <MasterBar>
      <Key><AccidentalCount>2</AccidentalCount><Mode>Minor</Mode></Key>
      <Time>3/4</Time>
      <Repeat start="false" end="true" count="3" />
      <AlternateEndings>2 1</AlternateEndings>
      <TripletFeel>Triplet8th</TripletFeel>
      <DoubleBar/>
      <Bars>2 3</Bars>
    </MasterBar>
  </MasterBars>
  <Bars>
    <Bar id="0"><Clef>G2</Clef><Voices>0 -1 1 -1</Voices></Bar>
    <Bar id="1"><Clef>Neutral</Clef><Voices>2 -1 -1 -1</Voices></Bar>
    <Bar id="2"><Clef>G2</Clef><Voices>3 -1 -1 -1</Voices></Bar>
    <Bar id="3"><Clef>Neutral</Clef><Voices>-1 -1 -1 -1</Voices></Bar>
  </Bars>
  <Voices>
    <Voice id="0"><Beats>0 1 2</Beats></Voice>
    <Voice id="1"><Beats>3</Beats></Voice>
    <Voice id="2"><Beats>4</Beats></Voice>
    <Voice id="3"><Beats>5 5 5 6</Beats></Voice>
  </Voices>
  <Beats>
    <Beat id="0"><Dynamic>MF</Dynamic><Rhythm ref="0" /><Chord><![CDATA[0]]></Chord><Notes>0 1</Notes>
      <Properties><Property name="Brush"><Direction>Down</Direction></Property></Properties></Beat>
    <Beat id="1"><Dynamic>P</Dynamic><GraceNotes>BeforeBeat</GraceNotes><Rhythm ref="3" /><Notes>2</Notes></Beat>
    <Beat id="2"><Dynamic>MF</Dynamic><Rhythm ref="0" /><Notes>3 1</Notes><Tremolo>1/16</Tremolo><FreeText><![CDATA[rit.]]></FreeText></Beat>
    <Beat id="3"><Rhythm ref="4" /></Beat>
    <Beat id="4"><Rhythm ref="4" /><Notes>4</Notes></Beat>
    <Beat id="5"><Rhythm ref="1" /><Notes>5</Notes></Beat>
    <Beat id="6"><Rhythm ref="2" /><Notes>6</Notes></Beat>
  </Beats>
  <Notes>
    <Note id="0"><Properties>
      <Property name="String"><String>0</String></Property><Property name="Fret"><Fret>1</Fret></Property>
    </Properties></Note>
    <Note id="1"><LetRing /><Properties>
      <Property name="String"><String>5</String></Property><Property name="Fret"><Fret>1</Fret></Property>
    </Properties></Note>
    <Note id="2"><Properties>
      <Property name="String"><String>0</String></Property><Property name="Fret"><Fret>0</Fret></Property>
      <Property name="HopoOrigin"><Enable /></Property>
    </Properties></Note>
    <Note id="3"><Tie origin="false" destination="true" /><Properties>
      <Property name="String"><String>0</String></Property>
      <Property name="Bended"><Enable /></Property>
      <Property name="BendOriginValue"><Float>0</Float></Property>
      <Property name="BendMiddleValue"><Float>100</Float></Property>
      <Property name="BendDestinationValue"><Float>100</Float></Property>
    </Properties></Note>
    <Note id="4"><Properties>
      <Property name="Element"><Element>0</Element></Property><Property name="Variation"><Variation>0</Variation></Property>
    </Properties></Note>
    <Note id="5"><Accent>9</Accent><AntiAccent>Normal</AntiAccent><Properties>
      <Property name="String"><String>2</String></Property><Property name="Fret"><Fret>3</Fret></Property>
      <Property name="Slide"><Flags>2</Flags></Property>
      <Property name="PalmMuted"><Enable /></Property>
    </Properties></Note>
    <Note id="6"><Vibrato>Slight</Vibrato><Trill>5</Trill><Properties>
      <Property name="String"><String>4</String></Property><Property name="Fret"><Fret>7</Fret></Property>
      <Property name="HarmonicType"><HType>Artificial</HType></Property>
      <Property name="Muted"><Enable /></Property>
    </Properties></Note>
  </Notes>
  <Rhythms>
    <Rhythm id="0"><NoteValue>Half</NoteValue></Rhythm>
    <Rhythm id="1"><NoteValue>Quarter</NoteValue><PrimaryTuplet num="3" den="2" /></Rhythm>
    <Rhythm id="2"><NoteValue>Quarter</NoteValue></Rhythm>
    <Rhythm id="3"><NoteValue>16th</NoteValue></Rhythm>
    <Rhythm id="4"><NoteValue>Whole</NoteValue></Rhythm>
    <Rhythm id="5"><NoteValue>Eighth</NoteValue><AugmentationDot count="2" /></Rhythm>
  </Rhythms>
</GPIF>"#;

    #[test]
    fn reads_the_score_and_master_track() {
        let song = parse(FIXTURE).unwrap();
        assert_eq!(song.version, "GPIF 9934");
        assert_eq!(song.info.title, "Song");
        assert_eq!(song.info.artist, "Band");
        assert_eq!(song.info.words, "Someone");
        assert_eq!(song.info.transcriber, "Tabber");
        assert_eq!(song.info.notices, ["one", "two"]);
        assert_eq!(song.tempo, 90.0);
        assert_eq!(song.tempo_name, "Slow");
        // 60 halves a minute, halfway through a bar of 3/4.
        assert_eq!(
            song.tempo_changes,
            [TempoChange {
                bar: 1,
                offset: Fraction::new(3, 8),
                bpm: 120.0
            }]
        );
    }

    #[test]
    fn reads_master_bars() {
        let song = parse(FIXTURE).unwrap();
        let [first, second] = &song.master_bars[..] else {
            panic!("{:?}", song.master_bars);
        };
        assert_eq!(first.time, TimeSignature::default());
        assert_eq!(
            first.key,
            KeySignature {
                fifths: -1,
                minor: false
            }
        );
        assert!(first.repeat_start);
        assert_eq!(first.repeat_end, None);
        assert_eq!(first.marker.as_deref(), Some("Intro"));
        assert_eq!(
            first.directions,
            [Direction::Segno, Direction::DalSegnoAlCoda]
        );
        assert_eq!(
            second.time,
            TimeSignature {
                numerator: 3,
                denominator: 4
            }
        );
        assert_eq!(
            second.key,
            KeySignature {
                fifths: 2,
                minor: true
            }
        );
        assert_eq!(second.repeat_end, Some(3));
        assert_eq!(second.alternate_endings, [1, 2]);
        assert_eq!(second.triplet_feel, TripletFeel::Eighth);
        assert!(second.double_bar);
        assert!(!first.double_bar);
    }

    #[test]
    fn reads_tracks() {
        let song = parse(FIXTURE).unwrap();
        let guitar = &song.tracks[0];
        assert_eq!(guitar.name, "Guitar");
        assert_eq!(guitar.short_name, "gtr");
        assert_eq!(guitar.instrument, "s-gtr6");
        assert_eq!(guitar.kind, TrackKind::Strings);
        assert_eq!(guitar.tuning, [64, 59, 55, 50, 45, 40]);
        assert_eq!(guitar.capo, 2);
        assert_eq!(guitar.midi.program, 25);
        assert_eq!(guitar.midi.channel, 2);
        assert_eq!(guitar.midi.effect_channel, 3);
        assert_eq!((guitar.midi.volume, guitar.midi.pan), (101, 64));
        assert_eq!(
            guitar.lyrics,
            [LyricLine {
                start_bar: 1,
                text: "la la".into()
            }]
        );
        let drums = &song.tracks[1];
        assert_eq!(drums.kind, TrackKind::Drums);
        assert!(drums.tuning.is_empty());
        assert_eq!(drums.midi.channel, 9);
    }

    #[test]
    fn reads_beats_and_notes_from_the_lowest_string_up() {
        let song = parse(FIXTURE).unwrap();
        let bar = &song.tracks[0].bars[0];
        // Voice 1 is left out, voice 2 is there: three voices, the second
        // empty.
        assert_eq!(bar.voices.len(), 3);
        assert!(bar.voices[1].beats.is_empty());
        assert_eq!(bar.voices[2].beats[0].status, BeatStatus::Rest);
        let beats = &bar.voices[0].beats;
        // The grace beat is folded into the beat after it.
        assert_eq!(beats.len(), 2);
        let chord = &beats[0];
        assert_eq!(chord.duration.value, 2);
        assert_eq!(
            chord.chord,
            Some(Chord {
                name: "F".into(),
                frets: vec![Some(1), None, None, None, Some(3), Some(1)]
            })
        );
        assert_eq!(
            chord.effects.stroke,
            Some(Stroke {
                direction: StrokeDirection::Down,
                value: 0
            })
        );
        // String 0, the lowest, is the model's sixth; 5 is its first.
        let strings: Vec<_> = chord.notes.iter().map(|n| (n.string, n.fret)).collect();
        assert_eq!(strings, [(6, 1), (1, 1)]);
        assert_eq!(chord.notes[0].dynamic, Dynamic::Mf);
        assert!(chord.notes[1].let_ring);
        assert_eq!(song.tracks[0].pitch(&chord.notes[0]), Some(43));

        let held = &beats[1];
        assert_eq!(held.text.as_deref(), Some("rit."));
        let low = &held.notes[0];
        assert_eq!((low.string, low.fret, low.tie), (6, 1, true));
        assert_eq!(low.tremolo_picking, Some(16));
        assert_eq!(
            low.grace,
            Some(Grace {
                fret: 0,
                value: 16,
                dynamic: Dynamic::P,
                transition: GraceTransition::Hammer,
                on_beat: false,
                dead: false,
            })
        );
        let bend = low.bend.as_ref().unwrap();
        assert_eq!(bend.peak(), 2.0);
        assert_eq!(bend.points.len(), 3);
        assert_eq!(bend.points[1].position, 0.5);
        // Note 1 is shared by beats 0 and 2, and copied into both, each
        // with its beat's effects.
        let (a, b) = (&chord.notes[1], &held.notes[1]);
        assert_eq!(
            (a.string, a.fret, a.let_ring),
            (b.string, b.fret, b.let_ring)
        );
        assert_eq!((a.tremolo_picking, b.tremolo_picking), (None, Some(16)));
        assert_eq!(b.grace, None);
    }

    #[test]
    fn reads_rhythms_and_note_effects() {
        let song = parse(FIXTURE).unwrap();
        let beats = &song.tracks[0].bars[1].voices[0].beats;
        assert_eq!(beats.len(), 4);
        let total = beats
            .iter()
            .fold(Fraction::ZERO, |sum, b| sum + b.duration.length());
        assert_eq!(total, song.master_bars[1].length());
        assert_eq!(
            beats[0].duration.tuplet,
            Some(Tuplet { notes: 3, space: 2 })
        );
        let slid = &beats[0].notes[0];
        assert_eq!(slid.string, 4);
        assert_eq!(slid.slide, Some(Slide::Legato));
        assert_eq!(slid.accent, Accent::Normal);
        assert!(slid.staccato && slid.ghost && slid.palm_mute);
        let last = &beats[3].notes[0];
        assert_eq!(last.string, 2);
        assert!(last.vibrato && last.dead);
        assert_eq!(last.harmonic, Some(Harmonic::Artificial));
        assert_eq!(last.trill, Some(Trill { fret: 5, value: 16 }));
    }

    #[test]
    fn leaves_drum_notes_out() {
        let song = parse(FIXTURE).unwrap();
        let drums = &song.tracks[1];
        let beat = &drums.bars[0].voices[0].beats[0];
        assert_eq!(beat.status, BeatStatus::Rest);
        assert!(beat.notes.is_empty());
        assert_eq!(beat.duration.length(), Fraction::new(1, 1));
        assert!(drums.bars[1].voices.is_empty());
    }

    #[test]
    fn a_broken_score_is_an_error() {
        let broken = |from: &str, to: &str| {
            assert!(FIXTURE.contains(from), "{from}");
            let result = parse(&FIXTURE.replace(from, to));
            assert!(
                matches!(result, Err(Error::Container(_))),
                "{to}: {result:?}"
            );
        };
        broken(r#"<Rhythm ref="2" />"#, r#"<Rhythm ref="9" />"#);
        broken("<Beats>5 5 5 6</Beats>", "<Beats>5 x</Beats>");
        broken("<String>4</String>", "<String>6</String>");
        broken("<Time>3/4</Time>", "<Time>3/5</Time>");
        broken(
            "<NoteValue>Whole</NoteValue>",
            "<NoteValue>Breve</NoteValue>",
        );
        broken("<Bars>2 3</Bars>", "<Bars>2</Bars>");
        broken(
            "<AccidentalCount>2</AccidentalCount>",
            "<AccidentalCount>9</AccidentalCount>",
        );
        broken("</GPIF>", "");
        broken("<GPIF>", "<Other>");
        assert!(parse("").is_err());
    }

    #[test]
    fn shared_items_are_copied_within_a_budget() {
        // One voice of 2000 beats, each of 1000 notes: two million notes
        // from a few kilobytes.
        let beats = vec!["0"; 2000].join(" ");
        let notes = vec!["0"; 1000].join(" ");
        let xml = format!(
            r#"<GPIF><Tracks><Track id="0"><GeneralMidi><Program>25</Program></GeneralMidi></Track></Tracks>
            <MasterBars><MasterBar><Time>4/4</Time><Bars>0</Bars></MasterBar></MasterBars>
            <Bars><Bar id="0"><Voices>0</Voices></Bar></Bars>
            <Voices><Voice id="0"><Beats>{beats}</Beats></Voice></Voices>
            <Beats><Beat id="0"><Rhythm ref="0"/><Notes>{notes}</Notes></Beat></Beats>
            <Notes><Note id="0"><Properties><Property name="String"><String>0</String></Property>
              <Property name="Fret"><Fret>0</Fret></Property></Properties></Note></Notes>
            <Rhythms><Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm></Rhythms></GPIF>"#
        );
        assert!(matches!(parse(&xml), Err(Error::Container(_))));
        // A track that gives no tuning plays as a guitar's.
        let small = xml.replace(&beats, "0 0 0 0").replace(&notes, "0");
        let song = parse(&small).unwrap();
        assert_eq!(song.tracks[0].tuning, STANDARD_TUNING);
        assert_eq!(song.tracks[0].bars[0].voices[0].beats.len(), 4);
        assert_eq!(song.tempo, 120.0);
    }
}
