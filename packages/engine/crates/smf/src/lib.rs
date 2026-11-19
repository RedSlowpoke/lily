//! The engine's Standard MIDI Files (chapter 07 §4.2, M1). A performance is a
//! list of staves, each a list of audio items in the order of their moments
//! (`Midi_walker`'s stable sort by column, the caller's); [`write`] turns it
//! into the file LilyPond 2.26 writes
//! (`midi-walker.cc`, `midi-item.cc`, `midi-chunk.cc`, `performance.cc`).
//! [`chunks`] and [`events`] read any file of LilyPond's or ours, and [`read`]
//! reads one back into the staves it was written from.
#![no_std]
extern crate alloc;
use alloc::vec::Vec;

/// Ticks a quarter note.
pub const DIVISION: u16 = 384;
/// The pitch wheel at rest: a note with this bend is written without one.
pub const CENTER: u16 = 0x2000;

/// What a performer hands to the writer, already in MIDI terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item<'a> {
    /// A note: start and length in ticks, written as note-on and, `len` later,
    /// note-on with velocity 0; one of 0 ticks too (a note of no length is
    /// the caller's to leave out, as `Midi_walker::process` does). `bend` is the
    /// pitch wheel of a microtone, set before the note-on and reset after its
    /// off; [`CENTER`] for none.
    Note {
        tick: i32,
        len: i32,
        channel: u8,
        key: u8,
        velocity: u8,
        bend: u16,
    },
    /// Any other event, written as is at `tick`: a meta event (`FF type len data`) or a channel message.
    Raw { tick: i32, bytes: &'a [u8] },
}

impl Item<'_> {
    pub fn tick(&self) -> i32 {
        match *self {
            Item::Note { tick, .. } | Item::Raw { tick, .. } => tick,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staff<'a> {
    pub items: Vec<Item<'a>>,
    /// Where the performance starts: below 0 when the score begins with grace notes.
    pub start: i32,
    /// Where the track ends: the score's end, plus `midiSkipOffset`.
    pub end: i32,
    /// `midiMergeUnisons`: a note of a key that sounds is merged into it even
    /// when it begins later.
    pub merge_unisons: bool,
}

/// The whole file: format 1, a track per staff, [`DIVISION`] ticks a quarter.
/// Returns how many times an item came before the tick of the one ahead of
/// it, each LilyPond's programming error "Going back in MIDI time.".
pub fn write(staves: &[Staff], out: &mut Vec<u8>) -> usize {
    out.extend_from_slice(b"MThd\0\0\0\x06\0\x01");
    out.extend_from_slice(&(staves.len() as u16).to_be_bytes());
    out.extend_from_slice(&DIVISION.to_be_bytes());
    let mut back = 0;
    for staff in staves {
        out.extend_from_slice(b"MTrk\0\0\0\0");
        let start = out.len();
        back += walk(staff, out);
        let length = (out.len() - start) as u32;
        out[start - 4..start].copy_from_slice(&length.to_be_bytes());
    }
    back
}

/// A note-off waiting in the queue: its tick, the note (index into the
/// staff's items), and whether a later note of the same key took it over.
#[derive(Clone, Copy)]
struct Off {
    tick: i32,
    note: usize,
    ignore: bool,
}

/// An event of the track before it is written: a note's on or off, or a raw event.
#[derive(Clone, Copy)]
enum Event<'a> {
    On(usize),
    Off(usize),
    Raw(&'a [u8]),
}

struct Walker<'s, 'a> {
    items: &'s [Item<'a>],
    merge_unisons: bool,
    last: i32,
    back: usize,
    queue: Vec<Off>,
    track: Vec<(u32, Event<'a>)>,
}

fn walk(staff: &Staff, out: &mut Vec<u8>) -> usize {
    let mut w = Walker {
        items: &staff.items,
        merge_unisons: staff.merge_unisons,
        last: staff.start,
        back: 0,
        queue: Vec::new(),
        track: Vec::new(),
    };
    for (i, &item) in staff.items.iter().enumerate() {
        w.stop_notes(item.tick());
        match item {
            Item::Note { tick, len, key, .. } => w.start_note(i, tick, tick + len, key),
            Item::Raw { tick, bytes } => w.output(tick, Event::Raw(bytes)),
        }
    }
    w.stop_notes(i32::MAX);
    for &(delta, event) in &w.track {
        variable(delta, out);
        match event {
            Event::On(i) => note(&staff.items[i], true, out),
            Event::Off(i) => note(&staff.items[i], false, out),
            Event::Raw(bytes) => out.extend_from_slice(bytes),
        }
    }
    // The end of track is appended as is, after everything at its tick.
    variable(staff.end.saturating_sub(w.last).max(0) as u32, out);
    out.extend_from_slice(b"\xff\x2f\x00");
    w.back
}

impl<'a> Walker<'_, 'a> {
    fn key(&self, note: usize) -> u8 {
        match self.items[note] {
            Item::Note { key, .. } => key,
            Item::Raw { .. } => 0,
        }
    }

    fn start_note(&mut self, index: usize, now: i32, mut stop: i32, key: u8) {
        // One key sounds once per track: a new note ends the one that sounds,
        // or, begun together with it, is merged into it.
        for i in 0..self.queue.len() {
            let queued = self.queue[i];
            if queued.ignore || self.key(queued.note) != key {
                continue;
            }
            if self.items[queued.note].tick() == now || self.merge_unisons {
                if queued.tick < stop {
                    self.queue[i].ignore = true;
                    heap::push(
                        &mut self.queue,
                        Off {
                            tick: stop,
                            note: queued.note,
                            ignore: false,
                        },
                    );
                }
                return;
            }
            stop = stop.max(queued.tick);
            self.output(now, Event::Off(queued.note));
            self.queue[i].ignore = true;
            break;
        }
        heap::push(
            &mut self.queue,
            Off {
                tick: stop,
                note: index,
                ignore: false,
            },
        );
        self.output(now, Event::On(index));
    }

    fn stop_notes(&mut self, until: i32) {
        while let Some(&first) = self.queue.first() {
            if first.tick > until {
                break;
            }
            heap::pop(&mut self.queue);
            if !first.ignore {
                self.output(first.tick, Event::Off(first.note));
            }
        }
    }

    /// `Midi_walker::output_event`: time never goes back (a grace note before
    /// what was written is written with no delta, and counted), but the next
    /// delta is taken from it.
    fn output(&mut self, now: i32, event: Event<'a>) {
        let delta = now.saturating_sub(self.last);
        if delta < 0 {
            self.back += 1;
        }
        let delta = delta.max(0) as u32;
        self.last = now;
        self.add(delta, event);
    }

    /// `Midi_track::add`: an event other than a note-on that comes with no delta
    /// goes before the note-ons already at its tick, so that a program change or
    /// a note-off takes effect before the notes begin.
    fn add(&mut self, delta: u32, event: Event<'a>) {
        let mut entry = (delta, event);
        let mut at = self.track.len();
        if delta == 0 && !matches!(event, Event::On(_)) {
            while at > 0 {
                let previous = &mut self.track[at - 1];
                if !matches!(previous.1, Event::On(_)) {
                    break;
                }
                if previous.0 != 0 {
                    entry.0 = previous.0;
                    previous.0 = 0;
                    at -= 1;
                    break;
                }
                at -= 1;
            }
        }
        self.track.insert(at, entry);
    }
}

/// A note's on or off: note-on with velocity 0 as the off, and a microtone's
/// pitch wheel set before the on and reset after the off, in the same event.
fn note(item: &Item, on: bool, out: &mut Vec<u8>) {
    let Item::Note {
        channel,
        key,
        velocity,
        bend,
        ..
    } = *item
    else {
        return;
    };
    let tuned = bend != CENTER;
    if on && tuned {
        out.extend_from_slice(&[0xe0 | channel, (bend & 0x7f) as u8, (bend >> 7) as u8, 0]);
    }
    out.extend_from_slice(&[0x90 | channel, key, if on { velocity } else { 0 }]);
    if !on && tuned {
        out.extend_from_slice(&[
            0,
            0xe0 | channel,
            (CENTER & 0x7f) as u8,
            (CENTER >> 7) as u8,
        ]);
    }
}

/// A MIDI variable-length number.
pub fn variable(mut value: u32, out: &mut Vec<u8>) {
    let mut bytes = [0u8; 5];
    let mut n = 0;
    loop {
        bytes[n] = (value & 0x7f) as u8 | if n > 0 { 0x80 } else { 0 };
        n += 1;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    out.extend(bytes[..n].iter().rev());
}

/// LilyPond's `PQueue` is `std::push_heap` and `std::pop_heap` with `greater`:
/// which of two note-offs at one tick comes first is decided by the C++
/// library's heap. This is libc++'s (LLVM 16 and later: Floyd's pop), which
/// Homebrew's LilyPond for macOS is built with; libstdc++ may order such ties
/// differently.
mod heap {
    use super::Off;
    use alloc::vec::Vec;

    /// `greater` as the heap's comparison: a min-heap of ticks.
    fn less(a: &Off, b: &Off) -> bool {
        a.tick > b.tick
    }

    /// libc++'s `__sift_up` of the last of `v[..len]`.
    fn sift_up(v: &mut [Off], mut len: usize) {
        if len <= 1 {
            return;
        }
        let mut last = len - 1;
        len = (len - 2) / 2;
        if less(&v[len], &v[last]) {
            let t = v[last];
            loop {
                v[last] = v[len];
                last = len;
                if len == 0 {
                    break;
                }
                len = (len - 1) / 2;
                if !less(&v[len], &t) {
                    break;
                }
            }
            v[last] = t;
        }
    }

    pub fn push(v: &mut Vec<Off>, off: Off) {
        v.push(off);
        let n = v.len();
        sift_up(v, n);
    }

    /// libc++'s `__pop_heap`: the first moves to the end, and is removed.
    pub fn pop(v: &mut Vec<Off>) {
        let len = v.len();
        #[cfg(feature = "classic")]
        if len > 1 {
            // Before LLVM 16: the last swapped to the root and sifted down.
            v.swap(0, len - 1);
            let n = len - 1;
            if n >= 2 {
                let mut start = 0;
                let mut child = 1;
                if child + 1 < n && less(&v[child], &v[child + 1]) {
                    child += 1;
                }
                if !less(&v[child], &v[start]) {
                    let top = v[start];
                    loop {
                        v[start] = v[child];
                        start = child;
                        if (n - 2) / 2 < child {
                            break;
                        }
                        child = 2 * child + 1;
                        if child + 1 < n && less(&v[child], &v[child + 1]) {
                            child += 1;
                        }
                        if less(&v[child], &top) {
                            break;
                        }
                    }
                    v[start] = top;
                }
            }
        }
        #[cfg(not(feature = "classic"))]
        if len > 1 {
            // Floyd: the hole goes down to a leaf along the smaller child, the
            // last fills it and goes up.
            let top = v[0];
            let mut hole = 0;
            let mut child = 0;
            loop {
                child = 2 * child + 1;
                if child + 1 < len && less(&v[child], &v[child + 1]) {
                    child += 1;
                }
                v[hole] = v[child];
                hole = child;
                if child > (len - 2) / 2 {
                    break;
                }
            }
            let last = len - 1;
            if hole == last {
                v[hole] = top;
            } else {
                v[hole] = v[last];
                v[last] = top;
                sift_up(&mut v[..], hole + 1);
            }
        }
        v.pop();
    }
}

// --- reading ---------------------------------------------------------------

/// Why a file could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// It does not begin with an `MThd` chunk of at least 6 bytes.
    NotMidi,
    /// A chunk or an event runs past the end.
    Truncated,
    /// An event without its status byte: neither LilyPond nor the engine writes one.
    RunningStatus,
}

/// A chunk of a file, its 8-byte header included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub tag: [u8; 4],
    pub bytes: &'a [u8],
}

impl<'a> Chunk<'a> {
    pub fn body(&self) -> &'a [u8] {
        &self.bytes[8..]
    }
}

/// The chunks of a file in order, `MThd` first.
pub fn chunks(bytes: &[u8]) -> Result<Vec<Chunk<'_>>, Error> {
    if bytes.len() < 14 || &bytes[..4] != b"MThd" || be32(bytes, 4) < 6 {
        return Err(Error::NotMidi);
    }
    let mut chunks = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if at + 8 > bytes.len() {
            return Err(Error::Truncated);
        }
        let end = at + 8 + be32(bytes, at + 4) as usize;
        if end > bytes.len() {
            return Err(Error::Truncated);
        }
        let tag = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        chunks.push(Chunk {
            tag,
            bytes: &bytes[at..end],
        });
        at = end;
    }
    Ok(chunks)
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// An event of a track: its tick from the track's start, and its bytes from the status on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackEvent<'a> {
    pub tick: u32,
    pub bytes: &'a [u8],
}

impl TrackEvent<'_> {
    /// A note-on with velocity 0, or a note-off.
    pub fn is_off(&self) -> bool {
        let b = self.bytes;
        (b[0] & 0xf0 == 0x90 && b[2] == 0) || b[0] & 0xf0 == 0x80
    }

    fn is_on(&self) -> bool {
        self.bytes[0] & 0xf0 == 0x90 && self.bytes[2] > 0
    }
}

/// The events of an `MTrk` chunk's body, in order.
pub fn events(body: &[u8]) -> Result<Vec<TrackEvent<'_>>, Error> {
    let mut at = 0;
    let mut tick = 0u32;
    let mut events = Vec::new();
    while at < body.len() {
        tick = tick.wrapping_add(read_variable(body, &mut at)?);
        let start = at;
        let status = *body.get(at).ok_or(Error::Truncated)?;
        if status < 0x80 {
            return Err(Error::RunningStatus);
        }
        at += 1;
        at += match status {
            0xff => {
                at += 1;
                read_variable(body, &mut at)? as usize
            }
            0xf0 | 0xf7 => read_variable(body, &mut at)? as usize,
            _ if matches!(status & 0xf0, 0xc0 | 0xd0) => 1,
            _ => 2,
        };
        if at > body.len() {
            return Err(Error::Truncated);
        }
        events.push(TrackEvent {
            tick,
            bytes: &body[start..at],
        });
    }
    Ok(events)
}

fn read_variable(bytes: &[u8], at: &mut usize) -> Result<u32, Error> {
    let mut value = 0u32;
    for _ in 0..4 {
        let c = *bytes.get(*at).ok_or(Error::Truncated)?;
        *at += 1;
        value = (value << 7) | u32::from(c & 0x7f);
        if c < 0x80 {
            return Ok(value);
        }
    }
    Err(Error::Truncated)
}

/// A file read back into the staves it was written from: each note-on paired
/// with the next off of its channel and key (a microtone's pitch wheel before
/// it and after its off folded into the note), every other event but the end
/// of track kept with its tick, in file order. Notes that LilyPond merged into
/// one come back as that one.
pub fn read(bytes: &[u8]) -> Result<Vec<Staff<'_>>, Error> {
    let mut staves = Vec::new();
    for chunk in chunks(bytes)?.iter().filter(|c| &c.tag == b"MTrk") {
        let events = events(chunk.body())?;
        let end = events
            .iter()
            .rev()
            .find(|e| e.bytes == b"\xff\x2f\x00")
            .map_or(0, |e| e.tick);
        let mut taken = alloc::vec![false; events.len()];
        let mut items = Vec::new();
        for (i, e) in events.iter().enumerate() {
            if taken[i] || e.bytes == b"\xff\x2f\x00" || e.is_off() {
                continue;
            }
            let at = e.tick as i32;
            if !e.is_on() {
                items.push(Item::Raw {
                    tick: at,
                    bytes: e.bytes,
                });
                continue;
            }
            let (channel, key) = (e.bytes[0] & 0x0f, e.bytes[1]);
            let off = (i + 1..events.len()).find(|&o| {
                events[o].is_off()
                    && events[o].bytes[0] & 0x0f == channel
                    && events[o].bytes[1] == key
            });
            let stop = off.map_or(end, |o| events[o].tick);
            let mut bend = CENTER;
            // LilyPond writes a microtone's wheel and its note-on as one event, the
            // wheel's reset and its off as another: the events just before and after.
            let before = i
                .checked_sub(1)
                .filter(|&b| !taken[b])
                .map(|b| events[b])
                .filter(|b| b.tick == e.tick && b.bytes.len() == 3);
            if let (Some(o), Some(b)) = (off, before)
                && b.bytes[0] == 0xe0 | channel
                && events.get(o + 1).is_some_and(|r| {
                    r.tick == events[o].tick
                        && r.bytes == [0xe0 | channel, (CENTER & 0x7f) as u8, (CENTER >> 7) as u8]
                })
            {
                items.pop();
                bend = u16::from(b.bytes[1]) | u16::from(b.bytes[2]) << 7;
                taken[o + 1] = true;
            }
            let velocity = e.bytes[2];
            items.push(Item::Note {
                tick: at,
                len: stop as i32 - at,
                channel,
                key,
                velocity,
                bend,
            });
        }
        staves.push(Staff {
            items,
            start: 0,
            end: end as i32,
            merge_unisons: false,
        });
    }
    Ok(staves)
}

/// The file with each run of note-offs at one tick in one order (by their
/// bytes): what is left of a difference from LilyPond's file once the heap's
/// ties are set aside (07 §4.3, C2b).
pub fn sort_tied_offs(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = bytes.to_vec();
    for chunk in chunks(bytes)?.iter().filter(|c| &c.tag == b"MTrk") {
        let events = events(chunk.body())?;
        let mut i = 0;
        while i < events.len() {
            let mut j = i + 1;
            if events[i].is_off() {
                while j < events.len() && events[j].is_off() && events[j].tick == events[i].tick {
                    j += 1;
                }
            }
            let mut run: Vec<&[u8]> = events[i..j].iter().map(|e| e.bytes).collect();
            run.sort();
            for (e, sorted) in events[i..j].iter().zip(run) {
                let at = e.bytes.as_ptr() as usize - bytes.as_ptr() as usize;
                out[at..at + sorted.len()].copy_from_slice(sorted);
            }
            i = j;
        }
    }
    Ok(out)
}
