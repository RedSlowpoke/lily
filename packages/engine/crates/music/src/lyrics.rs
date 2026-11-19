//! The listeners of LilyPond's lyric combine iterator
//! (`lyric-combine-music-iterator.cc`), N7's second part (04 §13, MS2 step
//! 38). The iterator itself is a kind of `iterator.rs`' arena; what its
//! listeners need is here, in the translation's arena, since they are
//! called inside a dispatch, which cannot reach the iterators:
//!
//! - `set_busy`, on the music context's `events_below`, hears each
//!   `melodic-event` and keeps the latest moment it heard one;
//! - `forward_event`, beside it, sends each `structural-event` the voice
//!   hears (its repeats' and alternatives') on to the lyrics' context,
//!   which the repeat styler inside `\lyricsto` does not announce (D96);
//! - `check_new_context`, on Global's `events_below`, looks for the voice
//!   again at each `CreateContext`, so that a voice made in the timestep
//!   gets its syllable then.
//!
//! The voice is the context of `associatedVoice` (and
//! `associatedVoiceType`) of the lyrics' context, or of `\lyricsto`'s id
//! and type, found below Global. A melisma of the voice holds its
//! syllable back (`melisma.rs`, MS2 step 39) unless the lyrics'
//! `ignoreMelismata` is set.

use lily_engine_scheme::{Exception, Heap, Tag, Tracer, Value};

use crate::context::{Arena, CtxId, Dir, GLOBAL, Host, Translation, dispatch};
use crate::dispatcher::Listener;
use crate::melisma::melisma_busy;
use crate::{Moment, Rational};

type Run<T> = Result<T, Exception>;

/// What a lyric combine iterator's listener does with what it hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heard {
    /// `set_busy`.
    Busy,
    /// `forward_event`.
    Forward,
    /// `check_new_context`.
    NewContext,
}

/// A lyric combine iterator's state that its listeners share.
#[derive(Clone)]
pub(crate) struct Combine {
    /// `lyrics_context_` and `music_context_`.
    pub lyrics: Option<CtxId>,
    pub music: Option<CtxId>,
    /// `busy_moment_`: when the voice last heard a melodic event.
    pub busy: Moment,
    /// `lyricsto_voice_name_` and `lyricsto_voice_type_`.
    name: Value,
    kind: Value,
    /// Whether its lyrics' iterator is `ok`, as the iterator last found.
    pub lyric_ok: bool,
    /// Whether the iterator holds a context, which it lets go at `quit`.
    pub held: bool,
}

impl Combine {
    pub(crate) fn new(lyrics: Option<CtxId>, name: Value, kind: Value) -> Combine {
        Combine {
            lyrics,
            music: None,
            busy: Moment::new(-Rational::INFINITY, Rational::ZERO),
            name,
            kind,
            lyric_ok: false,
            held: true,
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.name);
        tracer.trace(self.kind);
    }
}

/// `find_voice`: the voice the lyrics follow, if it is not the one they
/// follow already.
pub(crate) fn find_voice(heap: &mut Heap, a: &Arena, n: u32) -> Option<CtxId> {
    let c = &a.combines[n as usize];
    let (mut name, mut kind) = (c.name, c.kind);
    if let Some(l) = c.lyrics {
        let running = a.get(l, heap.intern("associatedVoice"));
        if heap.str(running).is_some() {
            name = running;
            kind = a.get(l, heap.intern("associatedVoiceType"));
        }
    }
    let name = heap.str(name)?.as_str();
    if c.music.is_some_and(|m| a.id(m) == name) || kind.tag() != Tag::Symbol || !c.held {
        return None;
    }
    let name = alloc::string::String::from(name);
    a.find_context(heap, GLOBAL, Dir::Down, kind, &name)
}

/// `set_music_context`: the listeners moved from the voice followed till
/// now to `to`.
pub(crate) fn set_music_context(heap: &mut Heap, a: &mut Arena, n: u32, to: Option<CtxId>) {
    let melodic = heap.intern("melodic-event");
    let structural = heap.intern("structural-event");
    let (busy, forward) = (
        Listener::Combine(n, Heard::Busy),
        Listener::Combine(n, Heard::Forward),
    );
    if let Some(m) = a.combines[n as usize].music {
        let d = a.events_below(m);
        for (listener, class) in [(forward, structural), (busy, melodic)] {
            if let Some(class) = a.class_number(heap, class) {
                a.dispatchers.remove_listener(d, listener, class);
            }
        }
    }
    a.combines[n as usize].music = to;
    if let Some(m) = to {
        let d = a.events_below(m);
        a.add_listener(heap, d, busy, "melodic-event");
        // The repeats and alternatives the voice hears: their iterators
        // inside `\lyricsto` announce none of their own.
        a.add_listener(heap, d, forward, "structural-event");
    }
}

/// `find_voice`, and `set_music_context` of what it finds.
pub(crate) fn follow_voice(heap: &mut Heap, a: &mut Arena, n: u32) {
    if let Some(voice) = find_voice(heap, a, n) {
        set_music_context(heap, a, n, Some(voice));
    }
}

/// `start_new_syllable`: the voice has started a note now, and no melisma
/// holds the syllable back (`melisma_busy`), unless the lyrics'
/// `ignoreMelismata` is set.
pub(crate) fn start_new_syllable(heap: &mut Heap, a: &Arena, n: u32) -> bool {
    let c = &a.combines[n as usize];
    let (Some(lyrics), Some(music)) = (c.lyrics, c.music) else {
        return false;
    };
    if c.busy.compare(a.now()).is_lt() {
        return false;
    }
    let ignore = heap.intern("ignoreMelismata");
    a.get(lyrics, ignore) == Value::TRUE || !melisma_busy(heap, a, music)
}

/// What the listener `which` of the lyric combine iterator `n` does with
/// the event `ev`.
pub(crate) fn heard(
    host: &mut dyn Host,
    t: &Translation,
    n: u32,
    which: Heard,
    ev: Value,
) -> Run<()> {
    match which {
        Heard::Busy => {
            let mut a = t.arena.borrow_mut();
            let now = a.now();
            let c = &mut a.combines[n as usize];
            if c.music.is_some() && c.busy.compare(now).is_lt() {
                c.busy = now;
            }
            Ok(())
        }
        Heard::Forward => {
            let lyrics = t.arena.borrow().combines[n as usize].lyrics;
            match lyrics {
                Some(l) => {
                    let d = t.arena.borrow().event_source(l);
                    dispatch(host, t, d, ev)
                }
                None => Ok(()),
            }
        }
        Heard::NewContext => {
            let mut a = t.arena.borrow_mut();
            let c = &a.combines[n as usize];
            // `ok`: its pending moment is never finite, so `run_always`.
            let ok = c.lyric_ok && !c.music.is_some_and(|m| a.is_removable(m));
            if ok {
                follow_voice(host.heap(), &mut a, n);
            }
            Ok(())
        }
    }
}
