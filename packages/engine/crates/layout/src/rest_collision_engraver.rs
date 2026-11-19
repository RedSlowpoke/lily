//! `rest-collision-engraver.cc` (MS3 step 41): `Rest_collision_engraver`,
//! a class of layout's ([`REST_COLLISION_ENGRAVER`]) that the core passes
//! at its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! At `process_acknowledged` (`:54–92`) the X parents (items) of the
//! rhythmic heads in `busyGrobs` that end after now are a timestep's
//! columns: those of rests only if the rests start now (or have no
//! column yet), those of notes whenever they started. Once a timestep has
//! two columns and a rest among them, a `RestCollision`, with no cause,
//! takes them (`Rest_collision::add_column`). `stop_translation_timestep`
//! (`:94–98`) forgets it.
//!
//! LilyPond keeps the columns in a `std::unordered_set`, which libc++
//! walks newest first while its buckets are those of its first two
//! insertions; from the third, the order is one of their addresses'
//! hashes. The port takes each column once, in the reverse of
//! `busyGrobs`' order: LilyPond's for two columns, and for more where
//! their order matters (a warning's place, the order of rests of a
//! direction and equal shifts) the fixtures' (`rest-collisions.ly`).

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::X;
use crate::grob;
use crate::item;
use crate::note_column;
use crate::paper_column;
use crate::rest_collision;

type Run<T> = Result<T, Exception>;

pub static REST_COLLISION_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Rest_collision_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(RestCollisionEngraver::default())),
    reads: &["busyGrobs"],
    writes: &[],
};

/// `rest_collision_`, by its handle.
#[derive(Clone, Default)]
struct RestCollisionEngraver {
    rest_collision: Option<Value>,
}

impl Work for RestCollisionEngraver {
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut rest_count = 0;
        let mut columns: Vec<Value> = Vec::new();
        let now = tx.now();
        let mut s = tx.get("busyGrobs");
        while let (Some(entry), Some(next)) = (tx.heap().car(s), tx.heap().cdr(s)) {
            s = next;
            let heap = tx.heap();
            let (Some(m), Some(g)) = (heap.car(entry), heap.cdr(entry)) else {
                continue;
            };
            let Some(&m) = heap.foreign::<Moment>(m) else {
                continue;
            };
            if grob_of(heap, g).is_none()
                || !grob::has_interface(heap, g, "rhythmic-head-interface")
                || m.compare(now) != Ordering::Greater
            {
                continue;
            }
            let Some(column) = grob::get_parent(heap, g, X).filter(|&c| item::is_item(heap, c))
            else {
                continue;
            };

            // Only rests that start now; notes that started any time.
            let is_rest = grob::has_interface(heap, g, "rest-interface");
            let paper_column = item::get_column(heap, column);
            let counts = match paper_column {
                Some(pc) if is_rest => {
                    paper_column::when_mom(tx.host, pc)?.compare(now) == Ordering::Equal
                }
                _ => true,
            };
            if counts {
                if !columns.contains(&column) {
                    columns.push(column);
                }
                rest_count += usize::from(note_column::has_rests(tx.host, column)?);
            }
        }

        if self.rest_collision.is_none() && rest_count > 0 && columns.len() > 1 {
            let id = tx.make_defined_grob("RestCollision", Cause::None)?;
            let me = tx.grob_handle(id);
            self.rest_collision = Some(me);
            for &c in columns.iter().rev() {
                rest_collision::add_column(tx.host, me, c)?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.rest_collision = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.rest_collision);
    }
}
