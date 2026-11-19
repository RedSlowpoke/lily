//! `hyphen-engraver.cc` (MS4 step 15): `Hyphen_engraver`, a class of
//! layout's ([`HYPHEN_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - `process_music` (`:83–93`): a `LyricHyphen` for `--`, a
//!   `VowelTransition` for `\vowelTransition`.
//! - `acknowledge_lyric_syllable` (`:95–108`): a `LyricSpace` of the
//!   syllable where neither was heard, and the syllable the right bound of
//!   the one waiting for it, whose end is announced.
//! - `stop_translation_timestep` (`:110–155`): the timestep's spanner
//!   bound on its left to the last syllable, or, with none, LilyPond's
//!   warning and killed; one still waiting killed, with LilyPond's warning
//!   unless it is a `LyricSpace`; the timestep's spanner waiting.
//! - `finalize` (`:174–198`): those left completed by their heads
//!   (`completize_hyphen`, `extender_engraver.rs`' `completize_extender`
//!   of the same text), else killed, with LilyPond's warnings.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
};
use lily_engine_scheme::{Exception, Value};

use crate::extender_engraver::completize_extender as completize_hyphen;
use crate::grob;
use crate::item;
use crate::spanner;
use crate::tie_engraver::in_event_class;

type Run<T> = Result<T, Exception>;

pub static HYPHEN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Hyphen_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["hyphen-event", "vowel-transition-event"],
    acknowledges: &[("lyric-syllable-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(HyphenEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The engraver's state, its grobs by their records and handles. In a
/// given time step, we expect not to have both a hyphen and a vowel
/// transition; with neither, a `LyricSpace` puts a constraint on the
/// minimum distance between lyric words through spacing rods.
#[derive(Clone, Default)]
struct HyphenEngraver {
    ev: Option<Value>,
    finished_ev: Option<Value>,
    syllable: Option<(u32, Value)>,
    last_syllable: Option<Value>,
    /// A LyricHyphen or VowelTransition or LyricSpace, reset at every
    /// time step.
    hyphen: Option<(u32, Value)>,
    /// A previously created LyricHyphen or ... awaiting completion.
    /// Forgotten about as soon as it finds a right bound.
    finished_hyphen: Option<(u32, Value)>,
}

impl Work for HyphenEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for x in [self.syllable, self.hyphen, self.finished_hyphen] {
            h.maybe_record(x.map(|x| x.0));
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.ev, ev)?;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.ev {
            let name = if in_event_class(tx.heap(), ev, "vowel-transition-event") {
                "VowelTransition"
            } else {
                "LyricHyphen"
            };
            let id = tx.make_defined_grob(name, Cause::Event(ev))?;
            self.hyphen = Some((id, tx.grob_handle(id)));
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let syllable = tx.grob_handle(grob);
        if !item::is_item(tx.heap(), syllable) {
            return Ok(());
        }
        self.syllable = Some((grob, syllable));
        if self.hyphen.is_none() {
            let id = tx.make_defined_grob("LyricSpace", Cause::Grob(grob))?;
            self.hyphen = Some((id, tx.grob_handle(id)));
        }
        if let Some((id, finished)) = self.finished_hyphen.take() {
            spanner::set_bound(tx.host, finished, 1, syllable)?;
            tx.announce_end_grob(id);
            self.finished_ev = None;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((_, syllable)) = self.syllable {
            self.last_syllable = Some(syllable);
        }
        if let Some((_, hyphen)) = self.hyphen {
            if let Some(last) = self.last_syllable {
                spanner::set_bound(tx.host, hyphen, -1, last)?;
            } else {
                grob::warning(
                    tx.host,
                    hyphen,
                    "hyphen or vowel transition has no syllable to attach to on its left; \
                     removing it",
                )?;
                grob::suicide(tx.heap(), hyphen);
            }
        }

        if let (Some((_, finished)), Some(_)) = (self.finished_hyphen, self.hyphen) {
            // When we reach this, a hyphen is pending completion, and
            // another hyphen was created in the time step, conflicting with
            // it.  The one pending completion may be an automatically
            // created LyricSpace, in which case it is just removed.  This
            // happens with "some _ -- words".  Otherwise, there are
            // extraneous hyphens in the input (e.g., "some \vowelTransition
            // _ -- words") and we should warn.
            if !grob::has_interface(tx.heap(), finished, "lyric-space-interface") {
                grob::warning(
                    tx.host,
                    finished,
                    "this hyphen or vowel transition was overridden by a later one",
                )?;
            }
            grob::suicide(tx.heap(), finished);
        }

        if self.hyphen.is_some() {
            self.finished_hyphen = self.hyphen;
            self.finished_ev = self.ev;
        }

        self.hyphen = None;
        self.ev = None;
        self.syllable = None;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((_, hyphen)) = self.hyphen.take() {
            completize_hyphen(tx.host, hyphen)?;
            if spanner::get_bound(tx.host, hyphen, 1)?.is_none() {
                grob::warning(tx.host, hyphen, "removing unterminated hyphen")?;
                grob::suicide(tx.heap(), hyphen);
            }
        }

        if let Some((_, finished)) = self.finished_hyphen.take() {
            completize_hyphen(tx.host, finished)?;
            if spanner::get_bound(tx.host, finished, 1)?.is_none() {
                if self.finished_ev.is_some() {
                    grob::warning(tx.host, finished, "unterminated hyphen; removing")?;
                }
                grob::suicide(tx.heap(), finished);
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.ev);
        out.extend(self.finished_ev);
        out.extend(self.syllable.map(|(_, g)| g));
        out.extend(self.last_syllable);
        out.extend(self.hyphen.map(|(_, g)| g));
        out.extend(self.finished_hyphen.map(|(_, g)| g));
    }
}
