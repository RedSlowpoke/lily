//! `note-head-line-engraver.cc` (MS3 step 52, 130 lines; in no part of the
//! split, ported here by the user's answer to the step's stop):
//! `Note_head_line_engraver`, a class of layout's
//! ([`NOTE_HEAD_LINE_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - Each rhythmic head acknowledged (`:57–68`) is the timestep's head;
//!   where the voice's `Staff` (`find_context_above`) is another than the
//!   last head's and `followVoice` is `#t`, a line follows from the last
//!   head.
//! - `process_acknowledged` (`:70–89`): that line, a `VoiceFollower`
//!   caused by the head, from the last head to it.
//! - `stop_translation_timestep` (`:91–98`): the head is the last head.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, CtxId, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work,
};
use lily_engine_scheme::{Exception, Value};

use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static NOTE_HEAD_LINE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Note_head_line_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[("rhythmic-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(NoteHeadLineEngraver::default())),
    reads: &["followVoice"],
    writes: &[],
};

/// `line_`, `head_` and `last_head_` by their handles (the head with its
/// record number), `last_staff_` and `follow_`.
#[derive(Clone, Default)]
struct NoteHeadLineEngraver {
    line: Option<Value>,
    last_staff: Option<CtxId>,
    follow: bool,
    head: Option<(Value, u32)>,
    last_head: Option<Value>,
}

/// `find_context_above (context (), 'Staff)`: the context or the first
/// above it with the alias.
fn staff_above(tx: &mut Tx<'_>) -> Option<CtxId> {
    let heap = tx.host.heap();
    let staff = heap.intern("Staff");
    let a = tx.t.arena.borrow();
    let mut at = Some(tx.context);
    while let Some(c) = at {
        if a.is_alias(heap, c, staff) {
            return Some(c);
        }
        at = a.parent(c);
    }
    None
}

impl Work for NoteHeadLineEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_context(self.last_staff);
        h.flags(&[self.follow]);
        h.maybe_record(self.head.map(|x| x.1));
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        self.head = Some((tx.grob_handle(grob), grob));
        let tr = staff_above(tx);
        if tr.is_some()
            && tr != self.last_staff
            && tx.get("followVoice") == Value::TRUE
            && self.last_head.is_some()
        {
            self.follow = true;
        }
        self.last_staff = tr;
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.line.is_none()
            && self.follow
            && let (Some(last_head), Some((head, id))) = (self.last_head, self.head)
        {
            /* TODO: Don't follow if there's a beam.

            We can't do beam-stuff here, since beam doesn't exist yet.
            Should probably store follow_ in line_, and suicide at some
            later point */
            let line_id = tx.make_defined_grob("VoiceFollower", Cause::Grob(id))?;
            let line = tx.grob_handle(line_id);
            self.line = Some(line);

            spanner::set_bound(tx.host, line, -1, last_head)?;
            spanner::set_bound(tx.host, line, 1, head)?;

            self.follow = false;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.line = None;
        if let Some((head, _)) = self.head.take() {
            self.last_head = Some(head);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.line);
        out.extend(self.head.map(|(v, _)| v));
        out.extend(self.last_head);
    }
}
