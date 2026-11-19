//! One voice of one track as LilyPond music: a bar a line, in the plan's
//! order and inside its repeats.
use super::names;
use super::plan::{Plan, Step};
use crate::{
    Accent, Beat, BeatStatus, Direction, Duration, Dynamic, Fraction, GraceTransition, Harmonic,
    MasterBar, Note, Slap, Slide, Song, StrokeDirection, Track, TripletFeel, Tuplet,
};

/// How a track is written, which is its `TrackKind` unless the track
/// cannot have it (a fretted track with no strings is a Staff alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Strings,
    Drums,
    Other,
}

/// A track as the score writes it.
pub struct Part<'a> {
    pub song: &'a Song,
    pub track: &'a Track,
    pub kind: Kind,
    /// The track's voices that have something in them, from 0; `[0]` when
    /// none has.
    pub voices: Vec<usize>,
}

/// The meter a master bar is written in: Guitar Pro's, or 4/4 where it
/// cannot be one.
pub fn meter(bar: &MasterBar) -> (u8, u8) {
    let t = bar.time;
    if t.numerator == 0 || !t.denominator.is_power_of_two() || t.denominator > 64 {
        (4, 4)
    } else {
        (t.numerator, t.denominator)
    }
}

fn bar_length(bar: &MasterBar) -> Fraction {
    let (n, d) = meter(bar);
    Fraction::new(n.into(), d.into())
}

fn frac(num: u64, den: u64) -> Fraction {
    let g = gcd(num, den.max(1));
    let (n, d) = (num / g, den.max(1) / g);
    Fraction::new(
        u32::try_from(n).unwrap_or(u32::MAX),
        u32::try_from(d).unwrap_or(1),
    )
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

fn sub(a: Fraction, b: Fraction) -> Fraction {
    let (an, ad, bn, bd) = (
        u64::from(a.num),
        u64::from(a.den),
        u64::from(b.num),
        u64::from(b.den),
    );
    frac((an * bd).saturating_sub(bn * ad), ad * bd)
}

fn mul(a: Fraction, b: Fraction) -> Fraction {
    frac(
        u64::from(a.num) * u64::from(b.num),
        u64::from(a.den) * u64::from(b.den),
    )
}

fn div(a: Fraction, b: Fraction) -> Fraction {
    frac(
        u64::from(a.num) * u64::from(b.den),
        u64::from(a.den) * u64::from(b.num).max(1),
    )
}

const ONE: Fraction = Fraction { num: 1, den: 1 };

/// A duration LilyPond can write: a tuplet only when it is one.
fn norm(d: Duration) -> Duration {
    let tuplet = d
        .tuplet
        .filter(|t| t.notes > 0 && t.space > 0 && t.notes != t.space);
    Duration { tuplet, ..d }
}

/// The length of `d` without its tuplet.
fn untupled(d: Duration) -> Fraction {
    Duration { tuplet: None, ..d }.length()
}

/// `d` as written (`8.`), times `scale`: a multiplier (`4*1/2`) where it
/// was cut short, or where its value is not one LilyPond has.
fn written(d: Duration, scale: Option<Fraction>) -> String {
    let valid = matches!(d.value, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128);
    let (mut text, base) = if valid {
        let mut t = d.value.to_string();
        for _ in 0..d.dots {
            t.push('.');
        }
        (t, untupled(d))
    } else {
        ("1".to_string(), ONE)
    };
    let factor = mul(div(untupled(d), base), scale.unwrap_or(ONE));
    if factor != ONE {
        text.push_str(&multiplier(factor));
    }
    text
}

fn multiplier(f: Fraction) -> String {
    if f.den == 1 {
        format!("*{}", f.num)
    } else {
        format!("*{}/{}", f.num, f.den)
    }
}

/// A length as a duration: `4`, `2.`, else `1*5/8`.
fn length_text(len: Fraction) -> String {
    let pow = |d: u32| d.is_power_of_two() && d <= 128;
    if len.num == 1 && pow(len.den) {
        len.den.to_string()
    } else if len.num == 3 && pow(len.den) && len.den >= 2 {
        format!("{}.", len.den / 2)
    } else {
        format!("1{}", multiplier(len))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    One,
    /// `\voiceOne` to `\voiceFour`, from 0.
    Voice(usize),
}

enum What<'a> {
    /// The notes sounding, with their pitches, lowest first.
    Notes(&'a Beat, Vec<(&'a Note, u8)>),
    /// A rest; `None` pads a voice shorter than its bar.
    Rest(Option<&'a Beat>),
    /// A spacer: an empty beat, or what pads a second voice.
    Skip,
    /// The whole bar's rest.
    BarRest,
    /// The whole bar's spacer.
    BarSkip,
}

struct Ev<'a> {
    what: What<'a>,
    start: Fraction,
    len: Fraction,
    /// The written duration, `None` for a pad or a whole bar.
    duration: Option<Duration>,
    scale: Option<Fraction>,
}

impl Ev<'_> {
    fn tuplet(&self) -> Option<Tuplet> {
        self.duration.and_then(|d| d.tuplet)
    }

    fn notes(&self) -> Option<(&Beat, &[(&Note, u8)])> {
        match &self.what {
            What::Notes(beat, notes) => Some((beat, notes)),
            _ => None,
        }
    }
}

struct BarEvents<'a> {
    evs: Vec<Ev<'a>>,
    mode: Mode,
    /// The voice ran past the bar's end and was cut there.
    cut: bool,
}

impl<'a> Part<'a> {
    fn prefers_flats(&self, bar: usize) -> bool {
        self.song.master_bars[bar].key.fifths < 0
    }

    /// The voice `k` (of `self.voices`) in master bar `bar`.
    fn bar_events(&self, bar: usize, k: usize) -> BarEvents<'a> {
        let master = &self.song.master_bars[bar];
        let len = bar_length(master);
        let track_bar = self.track.bars.get(bar);
        let voice_of = |k: usize| track_bar.and_then(|b| b.voices.get(self.voices[k]));
        let active: Vec<usize> = (0..self.voices.len())
            .filter(|&i| voice_of(i).is_some_and(|v| !v.is_empty()))
            .collect();
        let mode = match active.iter().position(|&a| a == k) {
            Some(p) if active.len() >= 2 => Mode::Voice(p.min(3)),
            _ => Mode::One,
        };
        let whole = |what| BarEvents {
            evs: vec![Ev {
                what,
                start: Fraction::ZERO,
                len,
                duration: None,
                scale: None,
            }],
            mode,
            cut: false,
        };
        let Some(voice) = voice_of(k).filter(|_| active.contains(&k)) else {
            return whole(if k == 0 && active.is_empty() {
                What::BarRest
            } else {
                What::BarSkip
            });
        };
        let mut evs = Vec::new();
        let mut pos = Fraction::ZERO;
        let mut cut = false;
        for beat in &voice.beats {
            if pos >= len {
                cut = true;
                break;
            }
            let duration = norm(beat.duration);
            let l = duration.length();
            let (scale, actual) = if pos + l > len {
                cut = true;
                let left = sub(len, pos);
                (Some(div(left, l)), left)
            } else {
                (None, l)
            };
            let what = match beat.status {
                BeatStatus::Empty => What::Skip,
                BeatStatus::Rest => What::Rest(Some(beat)),
                BeatStatus::Normal => {
                    let notes = self.sounding(beat);
                    if notes.is_empty() {
                        What::Rest(Some(beat))
                    } else {
                        What::Notes(beat, notes)
                    }
                }
            };
            evs.push(Ev {
                what,
                start: pos,
                len: actual,
                duration: Some(duration),
                scale,
            });
            pos = pos + actual;
        }
        if pos < len {
            // A voice shorter than its bar: rests in the first voice that
            // plays, spacers in the others.
            let what = if active.first() == Some(&k) {
                What::Rest(None)
            } else {
                What::Skip
            };
            evs.push(Ev {
                what,
                start: pos,
                len: sub(len, pos),
                duration: None,
                scale: None,
            });
        }
        let silent = evs.iter().all(|e| match &e.what {
            What::Notes(..) => false,
            What::Rest(Some(b)) => b.text.is_none() && b.chord.is_none(),
            _ => true,
        });
        if silent && active.len() == 1 {
            let mut whole = whole(What::BarRest);
            whole.cut = cut;
            return whole;
        }
        BarEvents { evs, mode, cut }
    }

    /// The beat's notes that sound, with their pitches, lowest first.
    fn sounding(&self, beat: &'a Beat) -> Vec<(&'a Note, u8)> {
        let mut notes: Vec<(&Note, u8)> = beat
            .notes
            .iter()
            .filter_map(|n| self.track.pitch(n).map(|p| (n, p)))
            .collect();
        notes.sort_by(|a, b| a.1.cmp(&b.1).then(b.0.string.cmp(&a.0.string)));
        if self.kind == Kind::Drums {
            notes.dedup_by_key(|n| n.1);
        }
        notes
    }

    /// The written name of a note at MIDI pitch `p`.
    fn name(&self, p: u8, flats: bool) -> String {
        match self.kind {
            Kind::Drums => names::drum(p).to_string(),
            _ => names::pitch(p, flats),
        }
    }
}

/// What a voice carries from beat to beat as it is written.
struct State {
    /// The duration LilyPond would take for a note written without one.
    last: Option<String>,
    dynamic: Dynamic,
    palm_mute: bool,
    let_ring: bool,
    /// The chord name printed last, so that a name is printed where it
    /// changes, not on each beat that carries it.
    chord: Option<String>,
    mode: Mode,
    feel: TripletFeel,
}

/// The music of voice `k` of `part`, the lines inside its braces.
/// `conductor` is the one voice that carries what belongs to the whole
/// score: tempo, section names, segni and codas, and bar numbers.
pub fn write_voice(part: &Part, plan: &Plan, k: usize, conductor: bool) -> String {
    let song = part.song;
    let bars: Vec<BarEvents> = plan.bars().map(|b| part.bar_events(b.bar, k)).collect();
    let steps: Vec<_> = plan.bars().collect();

    // For each event, whether the next one in the voice follows on from it:
    // both are notes, in the same bar or in bars played one after the other.
    let flat: Vec<(usize, usize)> = bars
        .iter()
        .enumerate()
        .flat_map(|(s, b)| (0..b.evs.len()).map(move |e| (s, e)))
        .collect();
    let ev = |(s, e): (usize, usize)| &bars[s].evs[e];
    let mut links: Vec<Vec<bool>> = bars.iter().map(|b| vec![false; b.evs.len()]).collect();
    for w in flat.windows(2) {
        let (a, b) = (w[0], w[1]);
        let joined = a.0 == b.0 || (b.0 == a.0 + 1 && steps[b.0].joined);
        links[a.0][a.1] = joined && ev(a).notes().is_some() && ev(b).notes().is_some();
    }
    let next_of = |s: usize, e: usize| -> Option<&Ev> {
        if e + 1 < bars[s].evs.len() {
            Some(&bars[s].evs[e + 1])
        } else {
            bars.get(s + 1).and_then(|b| b.evs.first())
        }
    };
    let prev_linked = |s: usize, e: usize| -> Option<(usize, usize)> {
        let p = if e > 0 {
            (s, e - 1)
        } else if s > 0 && !bars[s - 1].evs.is_empty() {
            (s - 1, bars[s - 1].evs.len() - 1)
        } else {
            return None;
        };
        links[p.0][p.1].then_some(p)
    };
    // A hammer-on, pull-off or legato slide that has a note to go to.
    let legato_out = |s: usize, e: usize| -> bool {
        links[s][e]
            && bars[s].evs[e].notes().is_some_and(|(_, notes)| {
                notes
                    .iter()
                    .any(|(n, _)| n.hammer || n.slide == Some(Slide::Legato))
            })
    };

    let mut state = State {
        last: None,
        dynamic: Dynamic::F,
        palm_mute: false,
        let_ring: false,
        chord: None,
        mode: Mode::One,
        feel: TripletFeel::None,
    };
    let mut tempo = song.tempo;
    let mut out = String::new();
    let mut depth = 1usize;
    let line = |out: &mut String, depth: usize, text: &str| {
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(text);
        out.push('\n');
    };
    let mut bar_index = 0usize;
    for (i, step) in plan.steps.iter().enumerate() {
        match step {
            Step::RepeatOpen { count, first_bar } => {
                line(&mut out, depth, &format!("\\repeat volta {count} {{"));
                depth += 1;
                if conductor {
                    let passes: Vec<String> = (2..=*count).map(|p| p.to_string()).collect();
                    line(
                        &mut out,
                        depth,
                        &format!(
                            "\\unfolded \\volta {} \\set Score.currentBarNumber = #{}",
                            passes.join(","),
                            first_bar + 1
                        ),
                    );
                }
            }
            Step::AlternativesOpen => {
                line(&mut out, depth, "\\alternative {");
                depth += 1;
            }
            Step::AltOpen { endings, first_bar } => {
                let list: Vec<String> = endings.iter().map(|e| e.to_string()).collect();
                line(&mut out, depth, &format!("\\volta {} {{", list.join(",")));
                depth += 1;
                if conductor {
                    line(
                        &mut out,
                        depth,
                        &format!(
                            "\\unfolded \\set Score.currentBarNumber = #{}",
                            first_bar + 1
                        ),
                    );
                }
            }
            Step::RepeatClose | Step::AltClose | Step::AlternativesClose => {
                depth -= 1;
                line(&mut out, depth, "}");
            }
            Step::Bar(step) => {
                let s = bar_index;
                bar_index += 1;
                let bar = &bars[s];
                let master = &song.master_bars[step.bar];
                let (num, den) = meter(master);
                let mut lines: Vec<String> = Vec::new();
                if bar.cut {
                    lines.push(format!(
                        "% Guitar Pro's bar {} runs past its {num}/{den}: the rest is left out.",
                        step.bar + 1
                    ));
                }
                if conductor && step.renumber {
                    lines.push(format!("\\set Score.currentBarNumber = #{}", step.bar + 1));
                }
                let first = s == 0;
                let differs = |f: &dyn Fn(&MasterBar) -> bool| {
                    first || step.after.iter().any(|&a| f(&song.master_bars[a]))
                };
                if differs(&|m| meter(m) != (num, den)) {
                    lines.push(format!("\\time {num}/{den}"));
                }
                if part.kind != Kind::Drums && differs(&|m| m.key != master.key) {
                    lines.push(format!(
                        "\\key {}",
                        names::key(master.key.fifths, master.key.minor)
                    ));
                }
                // Score-wide marks at the bar's start, and tempo changes
                // inside it, which go before the beat they fall in.
                let mut inline: Vec<(Fraction, String)> = Vec::new();
                let mut end: Vec<String> = Vec::new();
                let mut texts_first: Vec<String> = Vec::new();
                if conductor {
                    if first && song.tempo >= 1.0 {
                        let name = if song.tempo_name.trim().is_empty() {
                            String::new()
                        } else {
                            format!("{} ", names::string(&song.tempo_name))
                        };
                        lines.push(format!("\\tempo {name}4 = {}", song.tempo.round()));
                    }
                    for change in song.tempo_changes.iter().filter(|c| c.bar == step.bar) {
                        if change.bpm < 1.0 || change.bpm.round() == tempo.round() {
                            continue;
                        }
                        tempo = change.bpm;
                        let text = format!("\\tempo 4 = {}", change.bpm.round());
                        if change.offset == Fraction::ZERO {
                            lines.push(text);
                        } else {
                            inline.push((change.offset, text));
                        }
                    }
                    if let Some(marker) = master.marker.as_deref().filter(|m| !m.trim().is_empty())
                    {
                        lines.push(format!("\\sectionLabel {}", names::string(marker)));
                    }
                    for d in &master.directions {
                        match direction(d) {
                            Placed::Start(text) => lines.push(text),
                            Placed::End(text) => end.push(text),
                        }
                    }
                    if master.triplet_feel != state.feel {
                        state.feel = master.triplet_feel;
                        let feel = match master.triplet_feel {
                            TripletFeel::None => "straight",
                            TripletFeel::Eighth => "triplet feel 8ths",
                            TripletFeel::Sixteenth => "triplet feel 16ths",
                        };
                        texts_first.push(format!("^\\markup \\italic {}", names::string(feel)));
                    }
                }
                if bar.mode != state.mode {
                    state.mode = bar.mode;
                    lines.push(
                        match bar.mode {
                            Mode::One => "\\oneVoice",
                            Mode::Voice(0) => "\\voiceOne",
                            Mode::Voice(1) => "\\voiceTwo",
                            Mode::Voice(2) => "\\voiceThree",
                            Mode::Voice(_) => "\\voiceFour",
                        }
                        .to_string(),
                    );
                }

                // The bar's music.
                state.last = None;
                let flats = part.prefers_flats(step.bar);
                let mut items: Vec<String> = Vec::new();
                let mut e = 0;
                while e < bar.evs.len() {
                    let tuplet = bar.evs[e].tuplet();
                    // A tuplet takes the events with its ratio until they
                    // make a whole group.
                    let group_end = match tuplet {
                        Some(t) => tuplet_group(&bar.evs, e, t),
                        None => e + 1,
                    };
                    let mut group: Vec<String> = Vec::new();
                    for (g, ev) in bar.evs.iter().enumerate().take(group_end).skip(e) {
                        let mut text = String::new();
                        for (offset, t) in &inline {
                            let last = g + 1 == bar.evs.len();
                            if *offset >= ev.start && (*offset < ev.start + ev.len || last) {
                                text.push_str(t);
                                text.push(' ');
                            }
                        }
                        let mut extra = Vec::new();
                        if g == 0 {
                            extra.append(&mut texts_first);
                        }
                        let ctx = EvCtx {
                            part,
                            link: links[s][g],
                            next: next_of(s, g),
                            open_slur: legato_out(s, g)
                                && !prev_linked(s, g).is_some_and(|p| legato_out(p.0, p.1)),
                            close_slur: prev_linked(s, g).is_some_and(|p| legato_out(p.0, p.1))
                                && !legato_out(s, g),
                            flats,
                        };
                        text.push_str(&event(&ctx, ev, &mut state, extra));
                        group.push(text);
                    }
                    match tuplet {
                        Some(t) => items.push(format!(
                            "\\tuplet {}/{} {{ {} }}",
                            t.notes,
                            t.space,
                            group.join(" ")
                        )),
                        None => items.extend(group),
                    }
                    e = group_end;
                }
                items.extend(end);
                let last_step = !plan.steps[i + 1..]
                    .iter()
                    .any(|s| matches!(s, Step::Bar(_)));
                let next_is_bar = matches!(plan.steps.get(i + 1), Some(Step::Bar(_)));
                if last_step && plan.steps.get(i + 1) != Some(&Step::RepeatClose) {
                    items.push("\\bar \"|.\"".into());
                } else if master.double_bar && next_is_bar {
                    items.push("\\bar \"||\"".into());
                }
                lines.push(format!("{} |  % {}", items.join(" "), step.bar + 1));
                for l in &lines {
                    line(&mut out, depth, l);
                }
            }
        }
    }
    out
}

/// Where a direction is written: a sign at the bar's start, or a jump at
/// its end.
enum Placed {
    Start(String),
    End(String),
}

fn direction(d: &Direction) -> Placed {
    let jump = |t: &str| Placed::End(format!("\\jump {}", names::string(t)));
    match d {
        Direction::Segno => Placed::Start("\\segnoMark 1".into()),
        Direction::SegnoSegno => Placed::Start("\\segnoMark 2".into()),
        Direction::Coda => Placed::Start("\\codaMark 1".into()),
        Direction::DoubleCoda => Placed::Start("\\codaMark 2".into()),
        Direction::Fine => jump("Fine"),
        Direction::DaCapo => jump("D.C."),
        Direction::DaCapoAlCoda => jump("D.C. al Coda"),
        Direction::DaCapoAlDoubleCoda => jump("D.C. al Double Coda"),
        Direction::DaCapoAlFine => jump("D.C. al Fine"),
        Direction::DalSegno => jump("D.S."),
        Direction::DalSegnoAlCoda => jump("D.S. al Coda"),
        Direction::DalSegnoAlDoubleCoda => jump("D.S. al Double Coda"),
        Direction::DalSegnoAlFine => jump("D.S. al Fine"),
        Direction::DalSegnoSegno => jump("D.S.S."),
        Direction::DalSegnoSegnoAlCoda => jump("D.S.S. al Coda"),
        Direction::DalSegnoSegnoAlDoubleCoda => jump("D.S.S. al Double Coda"),
        Direction::DalSegnoSegnoAlFine => jump("D.S.S. al Fine"),
        Direction::DaCoda => jump("To Coda"),
        Direction::DaDoubleCoda => jump("To Double Coda"),
        Direction::Other(text) => jump(text),
    }
}

/// The end of the tuplet group starting at `from`: the events with the same
/// ratio, until their written lengths make whole beats of the tuplet (a
/// triplet of eighths, or a quarter and an eighth).
fn tuplet_group(evs: &[Ev], from: usize, t: Tuplet) -> usize {
    let mut written = Fraction::ZERO;
    let mut smallest: Option<Fraction> = None;
    let mut j = from;
    while j < evs.len() && evs[j].tuplet() == Some(t) {
        let d = evs[j].duration.unwrap_or_default();
        let w = mul(untupled(d), evs[j].scale.unwrap_or(ONE));
        written = written + w;
        smallest = Some(smallest.map_or(w, |s| s.min(w)));
        j += 1;
        let actual = mul(written, Fraction::new(t.space.into(), t.notes.into()));
        let enough = mul(smallest.unwrap_or(w), Fraction::new(t.notes.into(), 1));
        if written >= enough && actual.den.is_power_of_two() {
            break;
        }
    }
    j
}

struct EvCtx<'a, 'b> {
    part: &'b Part<'a>,
    /// The next event in the voice follows on from this one.
    link: bool,
    next: Option<&'b Ev<'a>>,
    open_slur: bool,
    close_slur: bool,
    flats: bool,
}

/// One event as LilyPond: a note or chord with what it does, a rest or a
/// spacer.
fn event(ctx: &EvCtx, ev: &Ev, state: &mut State, extra: Vec<String>) -> String {
    let duration = |state: &mut State, text: String, keep: bool| -> String {
        let out = if keep && state.last.as_deref() == Some(text.as_str()) {
            String::new()
        } else {
            text.clone()
        };
        state.last = keep.then_some(text);
        out
    };
    match &ev.what {
        What::BarRest => {
            state.last = None;
            format!("R{}{}", length_text(ev.len), extra.concat())
        }
        What::BarSkip => {
            state.last = None;
            format!("s{}{}", length_text(ev.len), extra.concat())
        }
        What::Skip => {
            let d = match ev.duration {
                Some(d) => {
                    let w = written(d, ev.scale);
                    let keep = ev.scale.is_none() && !w.contains('*');
                    duration(state, w, keep)
                }
                None => duration(state, length_text(ev.len), false),
            };
            format!("s{d}{}", extra.concat())
        }
        What::Rest(beat) => {
            let d = match ev.duration {
                Some(d) => {
                    let w = written(d, ev.scale);
                    let keep = ev.scale.is_none() && !w.contains('*');
                    duration(state, w, keep)
                }
                None => duration(state, length_text(ev.len), false),
            };
            let mut out = format!("r{d}");
            if let Some(beat) = beat {
                out.push_str(&beat_texts(beat, state));
            }
            out.push_str(&extra.concat());
            out
        }
        What::Notes(beat, notes) => notes_event(ctx, ev, beat, notes, state, extra),
    }
}

/// Text above the staff that any beat may have: its chord's name, where it
/// differs from the last one printed, and its
/// text.
fn beat_texts(beat: &Beat, state: &mut State) -> String {
    let mut out = String::new();
    if let Some(chord) = beat.chord.as_ref().filter(|c| !c.name.trim().is_empty())
        && state.chord.as_deref() != Some(chord.name.as_str())
    {
        out.push_str(&format!("^\\markup \\bold {}", names::string(&chord.name)));
        state.chord = Some(chord.name.clone());
    }
    if let Some(text) = beat.text.as_deref().filter(|t| !t.trim().is_empty()) {
        out.push_str(&format!("^{}", names::string(text)));
    }
    out
}

fn notes_event(
    ctx: &EvCtx,
    ev: &Ev,
    beat: &Beat,
    notes: &[(&Note, u8)],
    state: &mut State,
    extra: Vec<String>,
) -> String {
    let part = ctx.part;
    let strings = part.kind == Kind::Strings;
    let drums = part.kind == Kind::Drums;
    let mut out = String::new();

    // Grace notes, before the beat (slashed) or on it.
    let graces: Vec<(&Note, crate::Grace)> = notes
        .iter()
        .filter_map(|(n, _)| n.grace.map(|g| (*n, g)))
        .collect();
    let mut after_grace = false;
    if !graces.is_empty() {
        let mut els: Vec<String> = Vec::new();
        for (n, g) in &graces {
            let Some(p) = part.track.pitch(&Note {
                fret: g.fret,
                ..(*n).clone()
            }) else {
                continue;
            };
            let mut el = String::new();
            if g.dead && !drums {
                el.push_str("\\deadNote ");
            }
            el.push_str(&part.name(p, ctx.flats));
            if strings {
                el.push_str(&format!("\\{}", n.string));
            }
            els.push(el);
        }
        if !els.is_empty() {
            let value = graces
                .iter()
                .map(|(_, g)| g.value)
                .find(|v| matches!(v, 8 | 16 | 32 | 64))
                .unwrap_or(16);
            let on_beat = graces.iter().any(|(_, g)| g.on_beat);
            out.push_str(if on_beat {
                "\\grace "
            } else {
                "\\slashedGrace "
            });
            if els.len() == 1 {
                let el = &els[0];
                // The duration goes after the pitch, before the string.
                match el.rfind('\\').filter(|_| strings) {
                    Some(at) => out.push_str(&format!("{}{value}{}", &el[..at], &el[at..])),
                    None => out.push_str(&format!("{el}{value}")),
                }
            } else {
                out.push_str(&format!("<{}>{value}", els.join(" ")));
            }
            if !drums
                && graces
                    .iter()
                    .any(|(_, g)| g.transition == GraceTransition::Slide)
            {
                out.push_str("\\glissando");
            }
            out.push(' ');
            after_grace = true;
        }
    }

    // Which notes hold over into the next beat.
    let next_notes = ctx.next.filter(|_| ctx.link).and_then(|n| n.notes());
    let ties = |n: &Note, p: u8| -> bool {
        next_notes.is_some_and(|(_, next)| {
            next.iter()
                .any(|(m, q)| m.tie && *q == p && (drums || m.string == n.string))
        })
    };
    let harmonic = |n: &Note| !drums && n.harmonic == Some(Harmonic::Natural);
    let chord = notes.len() > 1 || notes.iter().any(|(n, _)| harmonic(n));

    let d = ev.duration.unwrap_or_default();
    // Tremolo picking: strokes shorter than the note.
    let tremolo = notes
        .iter()
        .filter_map(|(n, _)| n.tremolo_picking)
        .find(|v| matches!(v, 8 | 16 | 32 | 64) && *v > d.value && ev.scale.is_none())
        .map(|v| format!(":{v}"))
        .unwrap_or_default();
    let w = written(d, ev.scale);
    let explicit = after_grace || !tremolo.is_empty();
    let keep = ev.scale.is_none() && !w.contains('*') && tremolo.is_empty();
    let dur = if explicit {
        state.last = keep.then(|| w.clone());
        w.clone()
    } else {
        let text = if keep && state.last.as_deref() == Some(w.as_str()) {
            String::new()
        } else {
            w.clone()
        };
        state.last = keep.then(|| w.clone());
        text
    };

    let element = |n: &Note, p: u8, in_chord: bool| -> String {
        let mut el = String::new();
        if n.dead && !drums {
            el.push_str("\\deadNote ");
        }
        if n.ghost {
            el.push_str("\\parenthesize ");
        }
        el.push_str(&part.name(p, ctx.flats));
        if in_chord {
            if ties(n, p) {
                el.push('~');
            }
            if harmonic(n) {
                el.push_str("\\harmonic");
            }
            if strings {
                el.push_str(&format!("\\{}", n.string));
            }
        }
        el
    };
    if chord {
        let els: Vec<String> = notes.iter().map(|(n, p)| element(n, *p, true)).collect();
        out.push_str(&format!("<{}>{dur}{tremolo}", els.join(" ")));
    } else {
        let (n, p) = notes[0];
        out.push_str(&element(n, p, false));
        out.push_str(&dur);
        out.push_str(&tremolo);
        if ties(n, p) {
            out.push('~');
        }
        if strings {
            out.push_str(&format!("\\{}", n.string));
        }
    }

    // What the beat does.
    if ctx.open_slur {
        out.push('(');
    }
    if ctx.close_slur {
        out.push(')');
    }
    let any = |f: &dyn Fn(&Note) -> bool| notes.iter().any(|(n, _)| f(n));
    if !drums && ctx.link && any(&|n| matches!(n.slide, Some(Slide::Shift) | Some(Slide::Legato))) {
        out.push_str("\\glissando");
    }
    if chord && notes.len() > 1 && beat.effects.stroke.is_some() {
        out.push_str("\\arpeggio");
    }
    let accent = notes.iter().map(|(n, _)| n.accent).max_by_key(|a| match a {
        Accent::None => 0,
        Accent::Normal => 1,
        Accent::Heavy => 2,
    });
    match accent {
        Some(Accent::Normal) => out.push_str("->"),
        Some(Accent::Heavy) => out.push_str("-^"),
        _ => {}
    }
    if any(&|n| n.staccato) {
        out.push_str("-.");
    }
    if !drums {
        if beat.effects.wide_vibrato {
            out.push_str("\\prallprall");
        } else if any(&|n| n.vibrato) {
            out.push_str("\\prall");
        }
        if any(&|n| n.trill.is_some()) {
            out.push_str("\\trill");
        }
        match beat.effects.pick_stroke {
            Some(StrokeDirection::Down) => out.push_str("\\downbow"),
            Some(StrokeDirection::Up) => out.push_str("\\upbow"),
            None => {}
        }
        // A bend at its peak; else a slide out of the note as a fall or a
        // doit.
        let peak = notes
            .iter()
            .filter_map(|(n, _)| n.bend.as_ref().map(|b| b.peak()))
            .fold(0.0f32, |a, b| if b.abs() > a.abs() { b } else { a })
            .round() as i32;
        let out_slide = notes.iter().find_map(|(n, _)| match n.slide {
            Some(Slide::OutDownwards) => Some(-2),
            Some(Slide::OutUpwards) => Some(2),
            _ => None,
        });
        // A LilyPond number, not Scheme's `#+2`: Guile would read what
        // follows (`#+2\\mp`) as part of the token, a symbol.
        if let Some(amount) = Some(peak).filter(|p| *p != 0).or(out_slide) {
            out.push_str(&format!("\\bendAfter {amount}"));
        }
    }
    // A ghost note's softer dynamic is its parentheses: only the others
    // change the dynamic, or a run of f with ghost notes reads mf f mf f.
    let dynamic = notes
        .iter()
        .filter(|(n, _)| !n.ghost)
        .map(|(n, _)| n.dynamic)
        .max();
    if !drums
        && let Some(dynamic) = dynamic
        && dynamic != state.dynamic
    {
        state.dynamic = dynamic;
        out.push_str(&format!("\\{}", dynamic.name()));
    }
    out.push_str(&beat_texts(beat, state));
    if strings {
        let text = |t: &str| format!("^\\markup \\small {}", names::string(t));
        let palm = any(&|n| n.palm_mute);
        if palm && !state.palm_mute {
            out.push_str(&text("P.M."));
        }
        state.palm_mute = palm;
        let ring = any(&|n| n.let_ring);
        if ring && !state.let_ring {
            out.push_str(&text("let ring"));
        }
        state.let_ring = ring;
        let kind = notes.iter().find_map(|(n, _)| match n.harmonic {
            Some(Harmonic::Artificial) => Some("A.H."),
            Some(Harmonic::Tapped { .. }) => Some("T.H."),
            Some(Harmonic::Pinch) => Some("P.H."),
            Some(Harmonic::Semi) => Some("S.H."),
            Some(Harmonic::Feedback) => Some("Fdbk."),
            _ => None,
        });
        if let Some(kind) = kind {
            out.push_str(&text(kind));
        }
        if let Some(slap) = beat.effects.slap {
            out.push_str(&text(match slap {
                Slap::Tap => "T",
                Slap::Slap => "S",
                Slap::Pop => "P",
            }));
        }
    }
    out.push_str(&extra.concat());
    out
}
