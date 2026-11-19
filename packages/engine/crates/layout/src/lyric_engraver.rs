//! `lyric-engraver.cc` (MS4 step 15): `Lyric_engraver`, a class of
//! layout's ([`LYRIC_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D97, D108), and the two functions the
//! lyric engravers share, [`get_voice_to_lyrics`] and
//! [`get_current_note_head`].
//!
//! - `process_music` (`:62–83`): a `LyricText` of the syllable heard, but
//!   for `_` (a `" "`), which aligns the last syllable by
//!   `lyricMelismaAlignment`, as a melisma of the voice the lyrics follow
//!   does where `ignoreMelismata` is not set.
//! - `stop_translation_timestep` (`:162–188`): the syllable's X parent
//!   that of the voice's note head of now, aligned by
//!   `lyricMelismaAlignment` where a melisma holds the voice
//!   (`melisma_busy`, the music crate's).
//!
//! The syllable's text is `lyric-text::print`'s, the image's, through the
//! text path of W5 (W6's rest is MS4 step 19).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, ContextHandle, CtxId, Dir, Kind, Moment, PROCESS, Rational, STOP, TranslatorClass, Tx,
    Work, assign_event_once, event_property, grob_of, melisma_busy,
};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::clef_engraver::is_true;
use crate::geometry::X;
use crate::grob;
use crate::item;
use crate::tie_engraver::{event_cause, event_length};

type Run<T> = Result<T, Exception>;

pub static LYRIC_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Lyric_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    // `process_music` asks the voice's `melismaBusy` every timestep.
    always: true,
    hears: &["lyric-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(LyricEngraver::default())),
    reads: &["ignoreMelismata", "lyricMelismaAlignment", "searchForVoice"],
    writes: &[],
};

/// `event_`, `text_` and `last_text_`, the grobs by their handles.
#[derive(Clone, Default)]
struct LyricEngraver {
    event: Option<Value>,
    text: Option<Value>,
    last_text: Option<Value>,
}

/// `get_voice_to_lyrics` (`:85–124`): the voice the lyrics of `tx`'s
/// context follow: their `associatedVoiceContext` while it cannot be
/// removed, else the context of the type `associatedVoiceType` near them
/// named `associatedVoice`, or, with `searchForVoice`, named as the lyrics
/// are up to their last `-`; else any of that type near them.
pub(crate) fn get_voice_to_lyrics(tx: &mut Tx<'_>) -> Option<CtxId> {
    let search_for_voice = is_true(tx.get("searchForVoice"));

    let avc = tx.get("associatedVoiceContext");
    if let Some(c) = tx.heap().foreign::<ContextHandle>(avc).map(|h| h.id)
        && !tx.t.arena.borrow().is_removable(c)
    {
        return Some(c);
    }

    let voice_name = tx.get("associatedVoice");
    let mut nm = String::from(tx.t.arena.borrow().id(tx.context));

    if let Some(s) = tx.heap().str(voice_name) {
        nm = String::from(s.as_str());
    } else if nm.is_empty() || !search_for_voice {
        return None;
    } else if let Some(idx) = nm.rfind('-') {
        nm.truncate(idx);
    }

    let voice_type = tx.get("associatedVoiceType");
    if voice_type.tag() != Tag::Symbol {
        return None;
    }

    let (t, c) = (tx.t, tx.context);
    let heap = tx.host.heap();
    let a = t.arena.borrow();
    a.find_context(heap, c, Dir::Center, voice_type, &nm)
        .or_else(|| a.find_context(heap, c, Dir::Center, voice_type, ""))
}

/// `get_current_note_head` (`:126–160`): the first of the voice's
/// `busyGrobs` that is a note head item ending where its note, begun now,
/// ends; LilyPond's programming error for an entry that is not a moment
/// and a grob.
pub(crate) fn get_current_note_head(tx: &mut Tx<'_>, voice: CtxId) -> Run<Option<Value>> {
    let now = tx.now();
    let sym = tx.heap().intern("busyGrobs");
    let mut s = tx.t.arena.borrow().get(voice, sym);
    while let (Some(entry), Some(rest)) = (tx.heap().car(s), tx.heap().cdr(s)) {
        s = rest;
        let heap = tx.heap();
        let g = heap.cdr(entry).filter(|&g| grob_of(heap, g).is_some());
        let end_mom = heap
            .car(entry)
            .and_then(|m| heap.foreign::<Moment>(m).copied());
        let (Some(end_mom), Some(g)) = (end_mom, g) else {
            tx.programming_error("busyGrobs invalid")?;
            continue;
        };

        // It's a bit irritating that we just have the length and
        // duration of the Grob.
        let Some(cause) = event_cause(tx, g)? else {
            continue;
        };
        let len = event_length(tx.heap(), cause, now);
        let end_from_now = now
            .checked_add(len)
            .unwrap_or(Moment::new(Rational::INFINITY, Rational::ZERO));
        // We cannot actually include more than a single grace note
        // using busyGrobs on ungraced lyrics since a grob ending on
        // grace time will just have disappeared from busyGrobs by the
        // time our ungraced lyrics appear.  At best we may catch a
        // single grace note.
        if end_mom.compare(end_from_now).is_eq()
            && item::is_item(tx.heap(), g)
            && grob::has_interface(tx.heap(), g, "note-head-interface")
        {
            return Ok(Some(g));
        }
    }

    Ok(None)
}

/// Whether a melisma holds `voice` and the lyrics do not ignore it.
fn held_by_melisma(tx: &mut Tx<'_>, voice: CtxId) -> bool {
    let t = tx.t;
    let busy = melisma_busy(tx.host.heap(), &t.arena.borrow(), voice);
    busy && !is_true(tx.get("ignoreMelismata"))
}

impl Work for LyricEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.event, ev)?;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.event {
            let text = event_property(tx.heap(), ev, "text");
            let space = tx.heap().string(" ");

            if tx.heap().equal(text, space) {
                if let Some(last) = self.last_text {
                    let align = tx.get("lyricMelismaAlignment");
                    grob::set_property(tx.heap(), last, "self-alignment-X", align);
                }
            } else {
                let id = tx.make_defined_grob("LyricText", Cause::Event(ev))?;
                self.text = Some(tx.grob_handle(id));
            }
        }

        let voice = get_voice_to_lyrics(tx);
        if let (Some(last), Some(voice)) = (self.last_text, voice) {
            let sym = tx.heap().intern("melismaBusy");
            let busy = tx.t.arena.borrow().get(voice, sym);
            if is_true(busy) && !is_true(tx.get("ignoreMelismata")) {
                let align = tx.get("lyricMelismaAlignment");
                grob::set_property(tx.heap(), last, "self-alignment-X", align);
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(text) = self.text {
            if let Some(voice) = get_voice_to_lyrics(tx)
                && let Some(head) = get_current_note_head(tx, voice)?
            {
                if let Some(p) = grob::get_parent(tx.heap(), head, X) {
                    grob::set_parent(tx.heap(), text, p, X);
                }
                if held_by_melisma(tx, voice) {
                    let align = tx.get("lyricMelismaAlignment");
                    grob::set_property(tx.heap(), text, "self-alignment-X", align);
                }
            }

            self.last_text = Some(text);
            self.text = None;
        }
        self.event = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
        out.extend(self.text);
        out.extend(self.last_text);
    }
}
