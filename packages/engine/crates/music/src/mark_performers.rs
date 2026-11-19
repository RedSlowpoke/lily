//! M5's text performers (07 §7, OPEN G2; MS5 step 4): `Lyric_performer`
//! (`lyric-performer.cc`, D97) and `Mark_performer`
//! (`mark-performer.cc`), whose items are `Audio_text`s made of a markup
//! as `Audio_text::markup_to_string` makes them.
//!
//! - A syllable is a lyric event (FF 05) in its Lyrics' track: its text, a
//!   string wrapped in `\tied-lyric` first as `lyric-text::print` wraps it,
//!   so that its tildes become underties, then `markup->string` of it.
//! - A mark is a marker event (FF 06), which the Score's control track
//!   takes: the text `Mark_engraver` would print of the rehearsal mark and
//!   of the performance mark (a segno, a coda, a section label) that
//!   `Mark_tracking_translator` chose (`marks.rs`). Its two getters are
//!   `mark-engraver.cc`'s, the engraver itself R9's (MS8's `RehearsalMark`).

use alloc::boxed::Box;
use alloc::string::String;

use lily_engine_scheme::{Exception, Tag, Value};

use crate::audio::{Audio, AudioText, LYRIC, MARKER};
use crate::dispatcher::{StreamEvent, event_property};
use crate::marks::{current_performance_mark, current_rehearsal_mark};
use crate::performers::performer;
use crate::records::Cause;
use crate::translator::{PROCESS, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

/// `Audio_text::markup_to_string`: `markup->string` of a markup, a string
/// as it is, and of anything else the empty string.
fn markup_to_string(tx: &mut Tx<'_>, markup: Value) -> Run<String> {
    // `Text_interface::is_markup`.
    let is_markup = match markup.tag() {
        Tag::String => true,
        Tag::Pair => tx.call("%markup?", &[markup])? != Value::FALSE,
        _ => false,
    };
    let text = if is_markup {
        tx.call("markup->string", &[markup])?
    } else {
        markup
    };
    Ok(tx
        .heap()
        .str(text)
        .map_or(String::new(), |s| String::from(s.as_str())))
}

/// `announce<Audio_text>` of `markup` as a text of `kind`.
fn announce_text(tx: &mut Tx<'_>, kind: u8, markup: Value, cause: Cause) -> Run<()> {
    let text = markup_to_string(tx, markup)?;
    tx.announce(Audio::Text(AudioText { kind, text }), cause);
    Ok(())
}

// --- Lyric_performer -------------------------------------------------------------

pub(crate) const LYRIC_PERFORMER: TranslatorClass = performer(
    "Lyric_performer",
    PROCESS | STOP,
    false,
    &["lyric-event"],
    || Box::new(LyricPerformer { event: None }),
    &[],
);

#[derive(Clone)]
struct LyricPerformer {
    event: Option<Value>,
}

impl Work for LyricPerformer {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.event.get_or_insert(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.event.take() else {
            return Ok(());
        };
        let mut text = event_property(tx.heap(), ev, "text");
        if text.tag() == Tag::String {
            text = tx.call("make-tied-lyric-markup", &[text])?;
        }
        if text != Value::NULL {
            let depth = tx.heap().root_depth();
            tx.heap().root(text);
            let done = announce_text(tx, LYRIC, text, Cause::Event(ev));
            tx.heap().unroot_to(depth);
            done?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.event = None;
        Ok(())
    }

    fn values(&self, out: &mut alloc::vec::Vec<Value>) {
        out.extend(self.event);
    }
}

// --- Mark_performer --------------------------------------------------------------

pub(crate) const MARK_PERFORMER: TranslatorClass = performer(
    "Mark_performer",
    PROCESS,
    true,
    &[],
    || Box::new(MarkPerformer),
    &["currentPerformanceMarkEvent", "currentRehearsalMarkEvent"],
);

#[derive(Clone)]
struct MarkPerformer;

impl Work for MarkPerformer {
    /// A marker of the rehearsal mark's text, then of the performance
    /// mark's, each that has one.
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for (property, get_text) in [
            (
                "currentRehearsalMarkEvent",
                current_rehearsal_mark as fn(&mut Tx<'_>, Value) -> Run<(&'static str, Value)>,
            ),
            ("currentPerformanceMarkEvent", current_performance_mark),
        ] {
            let ev = tx.get(property);
            if tx.heap().foreign::<StreamEvent>(ev).is_none() {
                continue;
            }
            let (_, text) = get_text(tx, ev)?;
            if text != Value::NULL {
                let depth = tx.heap().root_depth();
                tx.heap().root(text);
                let done = announce_text(tx, MARKER, text, Cause::Event(ev));
                tx.heap().unroot_to(depth);
                done?;
            }
        }
        Ok(())
    }
}
