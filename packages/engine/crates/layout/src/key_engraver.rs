//! `key-engraver.cc` (MS3 step 17): `Key_engraver`, a class of layout's
//! ([`KEY_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! `initialize` (`:204–212`) sets `keyAlterations` and
//! `lastKeyAlterations` to `'()` and `tonic` to `Pitch ()` (`:207`, `:208`,
//! `:211`). A key change heard once (`listen_key_change`, `:124–130`) is
//! read (`read_event`, `:164–202`): its `pitch-alist` in
//! `keyAlterationOrder`'s order, what that order lacks after it with
//! LilyPond's warning, as `keyAlterations` (`:200`), and its `tonic`.
//! `process_music` (`:140–152`) makes the `KeySignature` where a break is
//! allowed, and a non-default one at a key change or where the
//! alterations changed; so does a clef acknowledged where
//! `createKeyOnClefChange` is set (`acknowledge_clef`, `:132–138`).
//! `create_key` (`:64–122`): its `c0-position` `middleCClefPosition`, a
//! `KeyCancellation` of what `lastKeyAlterations` had and the new key
//! drops or turns back, and its `alteration-alist` the key reversed; a
//! non-default one has `explicitKeySignatureVisibility` as its
//! `break-visibility`. `stop_translation_timestep` (`:154–162`) sets
//! `lastKeyAlterations` to `keyAlterations` (`:158`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, INITIALIZE, Kind, PROCESS, Pitch, Rational, STOP, TranslatorClass, Tx, Work,
    assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::clef_engraver::{break_allowed, is_true};
use crate::grob;
use crate::paper_column_engraver::rational;

type Run<T> = Result<T, Exception>;

pub static KEY_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Key_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | PROCESS | STOP,
    always: true,
    hears: &["key-change-event"],
    acknowledges: &[("clef-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(KeyEngraver::default())),
    reads: &[
        "createKeyOnClefChange",
        "explicitKeySignatureVisibility",
        "extraNatural",
        "forbidBreak",
        "forceBreak",
        "keyAlterationOrder",
        "keyAlterations",
        "lastKeyAlterations",
        "printKeyCancellation",
        "middleCClefPosition",
    ],
    writes: &["keyAlterations", "lastKeyAlterations", "tonic"],
};

/// `key_event_`, and `item_` and `cancellation_` by their handles.
#[derive(Clone, Default)]
struct KeyEngraver {
    key_event: Option<Value>,
    item: Option<Value>,
    cancellation: Option<Value>,
}

/// The items of a proper list, or of a list's proper part.
fn items(heap: &Heap, mut list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
    out
}

/// `ly_assoc`: the first entry of `alist` whose key is `key` (`equal?`).
fn assoc(heap: &Heap, key: Value, alist: Value) -> Option<Value> {
    items(heap, alist)
        .into_iter()
        .find(|&e| heap.car(e).is_some_and(|k| heap.equal(k, key)))
}

/// `from_scm<Rational>` of an entry's alteration, 0 if it is none.
fn alteration(heap: &Heap, entry: Value) -> Rational {
    let v = heap.cdr(entry).unwrap_or(Value::NULL);
    rational(heap, v, Rational::ZERO)
}

impl KeyEngraver {
    fn cause(&self) -> Cause {
        self.key_event.map_or(Cause::None, Cause::Event)
    }

    fn create_key(&mut self, tx: &mut Tx<'_>, is_default: bool) -> Run<()> {
        let item = match self.item {
            Some(item) => item,
            None => {
                let id = tx.make_defined_grob("KeySignature", self.cause())?;
                let item = tx.grob_handle(id);
                self.item = Some(item);

                // Use middleCClefPosition rather than middleCPosition,
                // because cue notes with a different clef will modify
                // middleCPosition. The Key signature, however, should
                // still be printed at the original position.
                let c0 = tx.get("middleCClefPosition");
                grob::set_property(tx.heap(), item, "c0-position", c0);

                let last = tx.get("lastKeyAlterations");
                let key = tx.get("keyAlterations");
                let print = is_true(tx.get("printKeyCancellation"));
                if (print || key == Value::NULL) && last != key {
                    let heap = tx.heap();
                    let mut restore = Vec::new();
                    for s in items(heap, last) {
                        let old_alter = alteration(heap, s);
                        let new_alter_pair = heap.car(s).and_then(|k| assoc(heap, k, key));
                        // `(new - old) * old < 0`: turned back towards
                        // natural or past it.
                        let turned = |new: Rational| {
                            let d = new.compare(old_alter) as i8;
                            let o = old_alter.compare(Rational::ZERO) as i8;
                            d * o < 0
                        };
                        if new_alter_pair.is_none_or(|p| turned(alteration(heap, p))) {
                            restore.push(s);
                        }
                    }
                    restore.reverse();

                    if !restore.is_empty() {
                        let id = tx.make_defined_grob("KeyCancellation", self.cause())?;
                        let cancellation = tx.grob_handle(id);
                        self.cancellation = Some(cancellation);
                        let heap = tx.heap();
                        let restore = heap.list(&restore);
                        grob::set_property(heap, cancellation, "alteration-alist", restore);
                        let c0 = tx.get("middleCClefPosition");
                        grob::set_property(tx.heap(), cancellation, "c0-position", c0);
                    }
                }

                let heap = tx.heap();
                let mut reversed = items(heap, key);
                reversed.reverse();
                let reversed = heap.list(&reversed);
                grob::set_property(heap, item, "alteration-alist", reversed);
                item
            }
        };

        if !is_default {
            let visibility = tx.get("explicitKeySignatureVisibility");
            let heap = tx.heap();
            grob::set_property(heap, item, "break-visibility", visibility);
            grob::set_property(heap, item, "non-default", Value::TRUE);
        }
        Ok(())
    }

    fn read_event(tx: &mut Tx<'_>, r: Value) -> Run<()> {
        let p = event_property(tx.heap(), r, "pitch-alist");
        if p.tag() != Tag::Pair {
            return Ok(());
        }

        let mut accs = Vec::new();

        let order = tx.get("keyAlterationOrder");
        let heap = tx.heap();
        let mut alist = items(heap, p);
        for s in items(heap, order) {
            if alist.is_empty() {
                break;
            }
            if let Some(&head) = alist.iter().find(|&&e| heap.equal(e, s)) {
                accs.push(head);
                alist.retain(|&e| !heap.equal(e, head));
            }
        }

        if !alist.is_empty() {
            let mut warn = false;
            for s in alist {
                if !alteration(heap, s).is_zero() {
                    warn = true;
                    accs.push(s);
                }
            }

            if warn {
                tx.event_warning(r, "Incomplete keyAlterationOrder for key signature")?;
            }
        }

        let accs = tx.heap().list(&accs);
        tx.set("keyAlterations", accs);
        let tonic = event_property(tx.heap(), r, "tonic");
        tx.set("tonic", tonic);
        Ok(())
    }
}

impl Work for KeyEngraver {
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("keyAlterations", Value::NULL);
        tx.set("lastKeyAlterations", Value::NULL);

        // `Pitch ()`, in `Lily::default_global_scale`.
        let scale = tx.host.lily("default-global-scale").unwrap_or(Value::FALSE);
        let p = tx.heap().wrap(Pitch::zero(scale));
        tx.set("tonic", p);
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        // do this only once, just to be on the safe side.
        if assign_event_once(tx, &mut self.key_event, ev)? {
            Self::read_event(tx, ev)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, _: u32) -> Run<()> {
        if is_true(tx.get("createKeyOnClefChange")) {
            self.create_key(tx, false)?;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Efficiency: don't create a KeySignature where it would not be
        // visible anyway.
        if break_allowed(tx) {
            self.create_key(tx, true)?;
        }

        let last = tx.get("lastKeyAlterations");
        if self.key_event.is_some() || last != tx.get("keyAlterations") {
            self.create_key(tx, false)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.item = None;
        let key = tx.get("keyAlterations");
        tx.set("lastKeyAlterations", key);
        self.cancellation = None;
        self.key_event = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.key_event);
        out.extend(self.item);
        out.extend(self.cancellation);
    }
}
