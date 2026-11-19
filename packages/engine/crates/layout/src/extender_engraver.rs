//! `extender-engraver.cc` (MS4 step 15): `Extender_engraver`, a class of
//! layout's ([`EXTENDER_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - The listeners (`:68–115`): the syllable, hyphen and extender heard;
//!   a `_` (`" "`) no syllable but the sign that a pending automatic
//!   extender is in a melisma, any other syllable the end of one that is
//!   not; a `CompletizeExtenderEvent` (the end of a `\lyricsto`'s lyrics)
//!   completing the pending extender.
//! - `process_music` (`:117–159`): a `LyricExtender` for `__`, or,
//!   with `autoExtenders`, for each syllable with no hyphen whose lyrics
//!   follow a voice, killed by the next syllable if no melisma came.
//! - `acknowledge_lyric_syllable` (`:161–174`): the syllable the new
//!   extender's left bound and the pending one's `next`, which completes
//!   it.
//! - `stop_translation_timestep` (`:176–228`): the voice's note head of
//!   now one of the extenders' `heads`; with none, the pending extender
//!   completed unless `extendersOverRests`; the new extender pending.
//! - `finalize` (`:249–268`): the pending extender killed if automatic
//!   with no melisma, else completed, LilyPond's `unterminated extender`
//!   if it has no right bound.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Host, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property, melisma_busy,
};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::grob;
use crate::grob_array::{add_grob, extract_grob_array};
use crate::item;
use crate::lyric_engraver::{get_current_note_head, get_voice_to_lyrics};
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static EXTENDER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Extender_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    // `stop_translation_timestep` asks the voice's melisma every timestep
    // an automatic extender is pending.
    always: true,
    hears: &[
        "lyric-event",
        "hyphen-event",
        "extender-event",
        "completize-extender-event",
    ],
    acknowledges: &[("lyric-syllable-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(ExtenderEngraver::default())),
    reads: &["autoExtenders", "extendersOverRests"],
    writes: &[],
};

/// The engraver's state, its extenders by their handles.
#[derive(Clone, Default)]
struct ExtenderEngraver {
    extender_ev: Option<Value>,
    lyric_ev: Option<Value>,
    hyphen_ev: Option<Value>,
    extender: Option<Value>,
    pending_extender: Option<Value>,
    pending_autoextender: bool,
    pending_autoextender_melisma_busy: bool,
}

/// `completize_extender` (`:230–247`): the extender's right bound its last
/// head if it has none; LilyPond's programming error for a head that is
/// no item.
pub(crate) fn completize_extender(host: &mut dyn Host, sp: Value) -> Run<()> {
    if spanner::get_bound(host, sp, 1)?.is_none() {
        let heads = extract_grob_array(host, sp, "heads")?;
        if let Some(&head) = heads.last() {
            // extract_item_set () would clean this up, but would be wasteful
            // given that we need to use only one of the elements
            if item::is_item(host.heap(), head) {
                spanner::set_bound(host, sp, 1, head)?;
            } else {
                grob::programming_error(host, head, "non-item among heads")?;
            }
        }
    }
    Ok(())
}

impl Work for ExtenderEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[
            self.pending_autoextender,
            self.pending_autoextender_melisma_busy,
        ]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            // listen_lyric
            0 => {
                // Do not use assign_event_once - we let the listener in
                // lyric-engraver.cc warn about conflicting events instead.
                let lyric = *self.lyric_ev.get_or_insert(ev);

                let text = event_property(tx.heap(), lyric, "text");
                let space = tx.heap().string(" ");
                if tx.heap().equal(text, space) {
                    // Don't register _ as a lyric starting an autoextender ...
                    self.lyric_ev = None;
                    // ... but note that we actually are inside a melisma now.
                    if self.pending_autoextender {
                        self.pending_autoextender_melisma_busy = true;
                    }
                } else if let Some(pending) = self.pending_extender
                    && self.pending_autoextender
                    && !self.pending_autoextender_melisma_busy
                {
                    grob::suicide(tx.heap(), pending);
                    self.pending_extender = None;
                }
            }
            // listen_hyphen: no assign_event_once, as listen_lyric.
            1 => {
                self.hyphen_ev.get_or_insert(ev);
            }
            2 => {
                assign_event_once(tx, &mut self.extender_ev, ev)?;
            }
            // listen_completize_extender: a CompletizeExtenderEvent is sent
            // at the end of each lyrics block to ensure any pending
            // extender can be correctly terminated if the lyrics end before
            // the associated voice.
            _ => {
                if let Some(pending) = self.pending_extender.take() {
                    completize_extender(tx.host, pending)?;
                }
            }
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut causing_event = None;

        if let Some(ev) = self.extender_ev {
            causing_event = Some(ev);
            self.pending_autoextender = false;
        } else if let Some(ev) = self.lyric_ev
            && self.hyphen_ev.is_none()
            && is_true(tx.get("autoExtenders"))
            && get_voice_to_lyrics(tx).is_some()
        {
            // We'd basically like to create an auto-extender only if the
            // voice is in a melisma.  But melismata indicated by _ in lyric
            // mode should also work, and these can be detected only when
            // the *next* lyric event comes along.  So we create an
            // auto-extender unconditionally and kill it off when the next
            // lyric event comes along if that isn't an underscore _.
            causing_event = Some(ev);
            self.pending_autoextender = true;
            // clean up pending_autoextender_melisma_busy_ from inherited
            // state; this boolean will be set to true, if necessary, in
            // stop_translation_timestep or even (in case of a Lyric melisma
            // _) in a future timestep.
            self.pending_autoextender_melisma_busy = false;
        }

        if let Some(ev) = causing_event {
            let id = tx.make_defined_grob("LyricExtender", Cause::Event(ev))?;
            let extender = tx.grob_handle(id);
            let auto = Value::bool(self.pending_autoextender);
            grob::set_property(tx.heap(), extender, "auto-generated", auto);
            self.extender = Some(extender);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);
        if !item::is_item(tx.heap(), item) {
            return Ok(());
        }
        if let Some(extender) = self.extender {
            spanner::set_bound(tx.host, extender, -1, item)?;
        }
        if let Some(pending) = self.pending_extender.take() {
            let next = tx.heap().intern("next");
            grob::set_object(tx.heap(), pending, next, item);
            completize_extender(tx.host, pending)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let voice = get_voice_to_lyrics(tx);
        if let Some(voice) = voice
            && self.pending_autoextender
        {
            // delay call to melisma_busy () until here since a melisma might
            // have started during timestep's processing
            let t = tx.t;
            self.pending_autoextender_melisma_busy = self.pending_autoextender_melisma_busy
                || melisma_busy(tx.host.heap(), &t.arena.borrow(), voice);
        }

        if self.extender.is_some() || self.pending_extender.is_some() {
            let h = match voice {
                Some(voice) => get_current_note_head(tx, voice)?,
                None => None,
            };
            if let Some(h) = h {
                let heads = tx.heap().intern("heads");
                for sp in [self.extender, self.pending_extender].into_iter().flatten() {
                    add_grob(tx.host, sp, heads, h)?;
                }
            } else if let Some(pending) = self.pending_extender
                && !is_true(tx.get("extendersOverRests"))
            {
                completize_extender(tx.host, pending)?;
                self.pending_extender = None;
            }
            if let Some(extender) = self.extender.take() {
                // We guarantee that after stop_translation_timestep,
                // extender_ is always null.
                self.pending_extender = Some(extender);
            }
        }

        self.extender_ev = None;
        self.lyric_ev = None;
        self.hyphen_ev = None;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(pending) = self.pending_extender.take() {
            if self.pending_autoextender && !self.pending_autoextender_melisma_busy {
                grob::suicide(tx.heap(), pending);
            } else {
                completize_extender(tx.host, pending)?;
                if spanner::get_bound(tx.host, pending, 1)?.is_none() {
                    grob::warning(tx.host, pending, "unterminated extender")?;
                }
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.extender_ev);
        out.extend(self.lyric_ev);
        out.extend(self.hyphen_ev);
        out.extend(self.extender);
        out.extend(self.pending_extender);
    }
}
