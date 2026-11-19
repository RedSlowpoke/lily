//! The order the score writes the bars in, with its repeats.
//!
//! Guitar Pro marks repeats bar by bar: a start, an end with its count of
//! plays, and the alternate endings each bar belongs to. The plan reads
//! them as LilyPond's `\repeat volta` and `\alternative`, then checks that
//! what LilyPond would play (`\unfoldRepeats`) is what Guitar Pro plays,
//! bar for bar. When it is not, the marks contradict one another (a start
//! with no end, a repeat inside a repeat, endings that skip a time through)
//! and the plan is the bars in the order Guitar Pro plays them: the repeats
//! unfolded, each bar numbered as Guitar Pro numbers it.
use crate::MasterBar;

/// One step of the plan, in the order the score is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Bar(BarStep),
    /// `\repeat volta count {`, its body starting at `first_bar`.
    RepeatOpen {
        count: u8,
        first_bar: usize,
    },
    RepeatClose,
    /// `\alternative {`.
    AlternativesOpen,
    /// `\volta 1,2 {`, starting at `first_bar`.
    AltOpen {
        endings: Vec<u8>,
        first_bar: usize,
    },
    AltClose,
    AlternativesClose,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarStep {
    /// The master bar, from 0.
    pub bar: usize,
    /// The bar written before this one is also the one played before it
    /// every time, so a tie, slur or glissando may run into this bar.
    pub joined: bool,
    /// Bars whose meter and key this bar follows: the one written before
    /// it, and, at the start of a later alternate ending, the end of the
    /// repeat's body. A `\time` or `\key` is written when it differs from
    /// any of them.
    pub after: Vec<usize>,
    /// Not the bar after the one written before it (an unfolded repeat):
    /// the score sets the bar number.
    pub renumber: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// The score has a `\repeat`, so its MIDI needs `\unfoldRepeats`.
    pub repeats: bool,
    /// The repeats could not be written as such and were unfolded.
    pub unfolded: bool,
}

impl Plan {
    pub fn bars(&self) -> impl Iterator<Item = &BarStep> {
        self.steps.iter().filter_map(|s| match s {
            Step::Bar(b) => Some(b),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    Bar(usize),
    Repeat {
        count: u8,
        body: Vec<usize>,
        alternatives: Vec<(Vec<u8>, Vec<usize>)>,
    },
}

pub fn plan(bars: &[MasterBar]) -> Plan {
    let played = played(bars);
    if let Some(items) = parse(bars).filter(|items| unfold(items) == played) {
        return from_items(&items);
    }
    let mut steps = Vec::new();
    let mut previous: Option<usize> = None;
    for &bar in &played {
        steps.push(Step::Bar(BarStep {
            bar,
            joined: previous.is_some(),
            after: previous.into_iter().collect(),
            renumber: previous.is_some_and(|p| p + 1 != bar),
        }));
        previous = Some(bar);
    }
    Plan {
        steps,
        repeats: false,
        unfolded: played.windows(2).any(|w| w[0] + 1 != w[1]),
    }
}

/// The bars in the order Guitar Pro plays them. A repeat with no start
/// mark goes back to the bar after the last repeat, or to the first bar; a
/// bar of alternate endings is played only on the times it names. A file
/// whose marks would loop for ever is cut off.
fn played(bars: &[MasterBar]) -> Vec<usize> {
    let limit = bars.len() * 16 + 16;
    let mut out = Vec::new();
    let (mut i, mut start, mut pass) = (0, 0, 1u8);
    let (mut jumped, mut in_endings) = (false, false);
    while i < bars.len() && out.len() < limit {
        let bar = &bars[i];
        // Past the last ending, or at a start mark, a new repeat begins; not
        // when a repeat has just gone back to its start.
        if !jumped && ((in_endings && bar.alternate_endings.is_empty()) || bar.repeat_start) {
            start = i;
            pass = 1;
        }
        in_endings = !bar.alternate_endings.is_empty();
        jumped = false;
        if !bar.alternate_endings.is_empty() && !bar.alternate_endings.contains(&pass) {
            i += 1;
            continue;
        }
        out.push(i);
        if let Some(count) = bar.repeat_end {
            if pass < count {
                pass += 1;
                i = start;
                jumped = true;
                continue;
            }
            pass = 1;
            start = i + 1;
        }
        i += 1;
    }
    out
}

/// The repeats as `\repeat volta` would write them, or `None` when the
/// marks cannot be read that way.
fn parse(bars: &[MasterBar]) -> Option<Vec<Item>> {
    let mut items = Vec::new();
    // Where the plain bars since the last repeat begin in `items`: a repeat
    // with no start mark takes them as the start of its body.
    let mut boundary = 0;
    let mut i = 0;
    while i < bars.len() {
        let bar = &bars[i];
        if !bar.repeat_start && bar.repeat_end.is_none() && bar.alternate_endings.is_empty() {
            items.push(Item::Bar(i));
            i += 1;
            continue;
        }
        let start = if bar.repeat_start {
            i
        } else {
            let first = match items.get(boundary) {
                Some(Item::Bar(b)) => *b,
                _ => i,
            };
            items.truncate(boundary);
            first
        };
        let (item, next) = match parse_repeat(bars, start) {
            Ok(found) => found,
            // A start mark before another start, with no end between: Guitar
            // Pro plays its bars once, as plain bars.
            Err(Some(next)) => {
                items.extend((start..next).map(Item::Bar));
                i = next;
                boundary = items.len();
                continue;
            }
            Err(None) => return None,
        };
        match item {
            Item::Repeat {
                count,
                body,
                alternatives,
            } if count < 2 && alternatives.is_empty() => {
                items.extend(body.into_iter().map(Item::Bar));
            }
            item => items.push(item),
        }
        i = next;
        boundary = items.len();
    }
    Some(items)
}

/// The repeat whose body starts at `start`, and the bar after it. `Err`
/// holds the next start mark when another repeat starts before this one
/// ends, and is `None` when the marks cannot be read as a repeat.
fn parse_repeat(bars: &[MasterBar], start: usize) -> Result<(Item, usize), Option<usize>> {
    let mut body = Vec::new();
    let mut j = start;
    loop {
        let bar = bars.get(j).ok_or(None)?;
        if j > start && bar.repeat_start {
            return Err(Some(j));
        }
        if !bar.alternate_endings.is_empty() {
            break;
        }
        body.push(j);
        j += 1;
        if let Some(count) = bar.repeat_end {
            let item = Item::Repeat {
                count,
                body,
                alternatives: Vec::new(),
            };
            return Ok((item, j));
        }
    }
    if body.is_empty() {
        return Err(None);
    }
    let mut alternatives: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();
    while let Some(bar) = bars.get(j) {
        if bar.alternate_endings.is_empty() || bar.repeat_start {
            break;
        }
        let endings = bar.alternate_endings.clone();
        let mut group = Vec::new();
        while let Some(bar) = bars.get(j) {
            if bar.alternate_endings != endings || bar.repeat_start {
                break;
            }
            group.push(j);
            j += 1;
            if bar.repeat_end.is_some() {
                break;
            }
        }
        alternatives.push((endings, group));
    }
    // The endings must take the times through in order, each once.
    let mut expected = 1u8;
    for (endings, _) in &alternatives {
        for &e in endings {
            if e != expected {
                return Err(None);
            }
            expected = expected.checked_add(1).ok_or(None)?;
        }
    }
    let count = expected - 1;
    if count < 2 {
        return Err(None);
    }
    let item = Item::Repeat {
        count,
        body,
        alternatives,
    };
    Ok((item, j))
}

/// The bars as LilyPond's `\unfoldRepeats` plays the items.
fn unfold(items: &[Item]) -> Vec<usize> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Bar(b) => out.push(*b),
            Item::Repeat {
                count,
                body,
                alternatives,
            } => {
                for pass in 1..=*count {
                    out.extend(body);
                    if let Some((_, bars)) = alternatives.iter().find(|(e, _)| e.contains(&pass)) {
                        out.extend(bars);
                    }
                }
            }
        }
    }
    out
}

fn from_items(items: &[Item]) -> Plan {
    let mut steps = Vec::new();
    let mut previous: Option<usize> = None;
    // Whether the next bar may join the one before it.
    let mut joins = false;
    let mut repeats = false;
    let push = |steps: &mut Vec<Step>, bar: usize, joined: bool, after: Vec<usize>| {
        steps.push(Step::Bar(BarStep {
            bar,
            joined,
            after,
            renumber: false,
        }));
    };
    for item in items {
        match item {
            Item::Bar(b) => {
                push(&mut steps, *b, joins, previous.into_iter().collect());
                previous = Some(*b);
                joins = true;
            }
            Item::Repeat {
                count,
                body,
                alternatives,
            } => {
                repeats = true;
                steps.push(Step::RepeatOpen {
                    count: *count,
                    first_bar: body[0],
                });
                for &b in body {
                    push(&mut steps, b, joins, previous.into_iter().collect());
                    previous = Some(b);
                    joins = true;
                }
                steps.push(Step::RepeatClose);
                let body_end = previous;
                if alternatives.is_empty() {
                    // The body's end goes back to its start, but for the last
                    // time.
                    joins = false;
                    continue;
                }
                steps.push(Step::AlternativesOpen);
                for (n, (endings, bars)) in alternatives.iter().enumerate() {
                    steps.push(Step::AltOpen {
                        endings: endings.clone(),
                        first_bar: bars[0],
                    });
                    for (k, &b) in bars.iter().enumerate() {
                        let mut after: Vec<usize> = previous.into_iter().collect();
                        let joined = if k == 0 && n > 0 {
                            after.extend(body_end.filter(|e| Some(*e) != previous));
                            false
                        } else {
                            true
                        };
                        push(&mut steps, b, joined, after);
                        previous = Some(b);
                    }
                    steps.push(Step::AltClose);
                }
                steps.push(Step::AlternativesClose);
                joins = true;
            }
        }
    }
    Plan {
        steps,
        repeats,
        unfolded: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(n: usize) -> Vec<MasterBar> {
        vec![MasterBar::default(); n]
    }

    fn order(plan: &Plan) -> Vec<usize> {
        plan.bars().map(|b| b.bar).collect()
    }

    #[test]
    fn plain_bars_have_no_repeats() {
        let plan = plan(&bars(3));
        assert_eq!(order(&plan), [0, 1, 2]);
        assert!(!plan.repeats && !plan.unfolded);
    }

    #[test]
    fn a_marked_repeat_with_two_endings() {
        let mut b = bars(5);
        b[1].repeat_start = true;
        b[2].alternate_endings = vec![1];
        b[2].repeat_end = Some(2);
        b[3].alternate_endings = vec![2];
        let plan = plan(&b);
        assert!(plan.repeats);
        assert_eq!(order(&plan), [0, 1, 2, 3, 4]);
        assert!(matches!(
            plan.steps[1],
            Step::RepeatOpen {
                count: 2,
                first_bar: 1
            }
        ));
        let Step::Bar(second) = &plan.steps[9] else {
            panic!("{:?}", plan.steps)
        };
        assert_eq!(second.bar, 3);
        assert!(!second.joined);
        assert_eq!(second.after, [2, 1]);
    }

    #[test]
    fn a_repeat_with_no_start_goes_back_to_the_last_repeat() {
        // As the .gpx sample: bars 0–6 repeat, then 7–14 with no start mark.
        let mut b = bars(16);
        b[0].repeat_start = true;
        b[6].repeat_end = Some(2);
        b[14].repeat_end = Some(2);
        let plan = plan(&b);
        assert!(plan.repeats && !plan.unfolded);
        let opens: Vec<_> = plan
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::RepeatOpen { first_bar, .. } => Some(*first_bar),
                _ => None,
            })
            .collect();
        assert_eq!(opens, [0, 7]);
    }

    #[test]
    fn endings_shared_by_two_times_through() {
        let mut b = bars(4);
        b[0].repeat_start = true;
        b[1].alternate_endings = vec![1, 2];
        b[1].repeat_end = Some(3);
        b[2].alternate_endings = vec![3];
        let plan = plan(&b);
        assert!(plan.repeats);
        assert!(plan.steps.contains(&Step::RepeatOpen {
            count: 3,
            first_bar: 0
        }));
        assert_eq!(played(&b), [0, 1, 0, 1, 0, 2, 3]);
    }

    #[test]
    fn a_start_before_another_start_is_a_plain_bar() {
        // Guitar Pro plays 0 1 2 1 2 3: the first start has no end of its own.
        let mut b = bars(4);
        b[0].repeat_start = true;
        b[1].repeat_start = true;
        b[2].repeat_end = Some(2);
        assert_eq!(played(&b), [0, 1, 2, 1, 2, 3]);
        let plan = plan(&b);
        assert!(plan.repeats && !plan.unfolded);
        assert_eq!(order(&plan), [0, 1, 2, 3]);
        assert!(matches!(
            plan.steps[1],
            Step::RepeatOpen { first_bar: 1, .. }
        ));
    }

    #[test]
    fn a_start_with_no_end_plays_once() {
        let mut b = bars(3);
        b[1].repeat_start = true;
        let plan = plan(&b);
        assert_eq!(order(&plan), [0, 1, 2]);
        assert!(!plan.repeats && !plan.unfolded);
    }

    #[test]
    fn endings_out_of_order_are_unfolded() {
        let mut b = bars(4);
        b[0].repeat_start = true;
        b[1].alternate_endings = vec![2];
        b[2].alternate_endings = vec![1];
        b[2].repeat_end = Some(2);
        assert_eq!(played(&b), [0, 2, 0, 1, 3]);
        let plan = plan(&b);
        assert!(!plan.repeats && plan.unfolded);
        assert_eq!(order(&plan), [0, 2, 0, 1, 3]);
        let renumbered: Vec<_> = plan.bars().filter(|b| b.renumber).map(|b| b.bar).collect();
        assert_eq!(renumbered, [2, 0, 3]);
    }
}
