//! `tweak-engraver.cc` (MS4 step 4, by the user's answer: no part of the
//! split ported it, as it makes no grobs): `Tweak_engraver`, a class of
//! layout's ([`TWEAK_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108). Each grob acknowledged gets the `tweaks` of the event
//! that caused it (`acknowledge_grob`, `:39–79`): a direct tweak and a
//! direct nested tweak of a grob caused by the event itself, a targeted
//! one (`Grob.property`) of a grob of that name caused by it through other
//! grobs too.

use alloc::boxed::Box;

use lily_engine_music::{Kind, TranslatorClass, Tx, Work, event_property};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::grob;
use crate::tie_engraver::event_cause;

type Run<T> = Result<T, Exception>;

pub static TWEAK_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tweak_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(TweakEngraver)),
    reads: &[],
    writes: &[],
};

#[derive(Clone)]
struct TweakEngraver;

/// `set_property (grob, sym, value)`: unchecked, as LilyPond's is without
/// its internal type checking.
fn set_property(tx: &mut Tx<'_>, g: Value, sym: Value, value: Value) {
    let heap = tx.heap();
    if let Some(name) = heap.name(sym).map(alloc::string::String::from) {
        grob::set_property(heap, g, &name, value);
    }
}

/// `set_nested_property (grob, path, value)`: `ly:grob-set-nested-property!`.
fn set_nested_property(tx: &mut Tx<'_>, g: Value, path: Value, value: Value) -> Run<()> {
    crate::call_lily(tx.host, "ly:grob-set-nested-property!", &[g, path, value])?;
    Ok(())
}

impl Work for TweakEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, id: u32) -> Run<()> {
        let g = tx.grob_handle(id);
        let mut ev = event_cause(tx, g)?;
        let direct = ev.is_some();
        if !direct {
            ev = grob::ultimate_event_cause(tx.heap(), g);
        }
        let Some(ev) = ev else {
            return Ok(());
        };
        // Each tweak conses an address and a value. The address has one of
        // the following forms:
        // symbol -> direct tweak
        // (grob . symbol) -> targeted tweak
        // (#t . symbol-path) -> direct nested tweak
        // (grob . symbol-path) -> targeted nested tweak
        let mut grobname = None;
        let mut s = event_property(tx.heap(), ev, "tweaks");
        while let (Some(tweak), Some(rest)) = (tx.heap().car(s), tx.heap().cdr(s)) {
            let heap = tx.heap();
            let (Some(address), Some(value)) = (heap.car(tweak), heap.cdr(tweak)) else {
                return Err(crate::wrong_type(heap, Some("car"), 1, tweak, "pair"));
            };
            if let (Some(target), Some(path)) = (heap.car(address), heap.cdr(address)) {
                if target.tag() == Tag::Symbol {
                    let name = match grobname {
                        Some(n) => n,
                        None => {
                            let n = grob::grob_name(heap, g);
                            let n = heap.intern(&n);
                            grobname = Some(n);
                            n
                        }
                    };
                    if target == name {
                        if path.tag() == Tag::Symbol {
                            set_property(tx, g, path, value);
                        } else {
                            set_nested_property(tx, g, path, value)?;
                        }
                    }
                } else if direct {
                    set_nested_property(tx, g, path, value)?;
                }
            } else if direct {
                set_property(tx, g, address, value);
            }
            s = rest;
        }
        Ok(())
    }
}
