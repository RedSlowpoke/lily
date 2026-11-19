//! `caesura-engraver.cc` (MS8 step 7, part 2, 367 lines): `Caesura_engraver`,
//! a class of layout's ([`CAESURA_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - `process_music` (`:94–124`): a `caesura-event` heard, `caesuraType` and
//!   `caesuraTypeTransform` read and the event's articulations' types
//!   listed; the grobs made from what they evaluate to.
//! - `acknowledge_bar_line` (`:126–139`): the first bar line that is no span
//!   bar, an observation that makes `process_acknowledged` (`:141–152`)
//!   evaluate the type again and remake the grobs if it, or the X parent
//!   they would take, changed.
//! - `evaluate_caesura_type` (`:154–197`): `caesuraType` with the user's
//!   articulations, through `caesuraTypeTransform` with the observations
//!   (`bar-line`); its `breath`, `scripts` and `articulations`, a change
//!   against the last evaluation's.
//! - `make_or_remake_grobs` (`:208–297`): the old grobs killed; a
//!   `BreathingSign` of the `breath` type (`set-breath-properties`); a
//!   `CaesuraScript` of each script type and of each of the event's
//!   articulations the types allow, made by `make_script_from_event`, X
//!   parented to the breathing sign or else the bar line.
//! - `stop_translation_timestep` (`:299–331`): the first script by
//!   `script_priority_less` (a stable sort) given an
//!   `outside-staff-priority` of 0; everything forgotten.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property,
};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::X;
use crate::grob;
use crate::script::script_priority_less;
use crate::script_engraver::make_script_from_event;

type Run<T> = Result<T, Exception>;

pub static CAESURA_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Caesura_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &["caesura-event"],
    acknowledges: &[("bar-line-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(CaesuraEngraver::default())),
    reads: &[
        "breathMarkDefinitions",
        "caesuraType",
        "caesuraTypeTransform",
        "scriptDefinitions",
    ],
    writes: &[],
};

/// The engraver's members, the grobs by handle and record. The evaluated
/// types are `None` while undefined (`SCM_UNDEFINED`).
#[derive(Clone, Default)]
struct CaesuraEngraver {
    caesura_ev: Option<Value>,
    breathing_sign: Option<(Value, u32)>,
    scripts: Vec<(Value, u32)>,
    bar_line: Option<Value>,
    x_parent: Option<Value>,
    observations_changed: bool,
    caesura_type: Value,
    caesura_type_transform: Value,
    conf_breathing_sign_type: Option<Value>,
    conf_script_types: Option<Value>,
    conf_artic_types: Option<Value>,
    user_artic_types: Value,
}

/// The elements of the list `l`, read while it is a pair.
fn elements(tx: &mut Tx<'_>, mut l: Value) -> Vec<Value> {
    let mut out = Vec::new();
    let heap = tx.heap();
    while let (Some(car), Some(cdr)) = (heap.car(l), heap.cdr(l)) {
        out.push(car);
        l = cdr;
    }
    out
}

/// `scm_assq_ref`: the value of `key` in `alist`, `#f` without one.
fn assq_ref(tx: &mut Tx<'_>, key: &str, alist: Value) -> Value {
    let heap = tx.heap();
    let key = heap.intern(key);
    heap.assq(key, alist)
        .and_then(|entry| heap.cdr(entry))
        .unwrap_or(Value::FALSE)
}

/// Whether `a` is `equal?` to `b`, an undefined `b` equal to nothing.
fn same(tx: &mut Tx<'_>, a: Value, b: Option<Value>) -> bool {
    b.is_some_and(|b| tx.heap().equal(a, b))
}

impl CaesuraEngraver {
    /// `choose_x_parent`: the breathing sign made, or else the bar line.
    fn choose_x_parent(&self) -> Option<Value> {
        self.breathing_sign.map(|b| b.0).or(self.bar_line)
    }

    /// `evaluate_caesura_type`: whether the types changed.
    fn evaluate_caesura_type(&mut self, tx: &mut Tx<'_>) -> Run<bool> {
        let depth = tx.heap().root_depth();
        let changed = self.evaluate_rooted(tx);
        tx.heap().unroot_to(depth);
        changed
    }

    fn evaluate_rooted(&mut self, tx: &mut Tx<'_>) -> Run<bool> {
        let heap = tx.heap();
        let articulations = heap.intern("articulations");
        // Add the user's articulations to the caesuraType value.
        let entry = heap.cons(articulations, self.user_artic_types);
        heap.root(entry);
        let mut props = heap.cons(entry, self.caesura_type);
        heap.root(props);

        // Pass caesuraType through the transform function, if it is set.
        if tx.heap().is_procedure(self.caesura_type_transform) {
            let obs = if self.bar_line.is_some() {
                let heap = tx.heap();
                let bar = heap.intern("bar-line");
                heap.list(&[bar])
            } else {
                Value::NULL
            };
            tx.heap().root(obs);
            let context = tx.t.arena.borrow().handle(tx.context);
            props = tx.apply(self.caesura_type_transform, &[context, props, obs])?;
            tx.heap().root(props);
        }

        let b_type = assq_ref(tx, "breath", props);
        let s_types = {
            let heap = tx.heap();
            let key = heap.intern("scripts");
            heap.assq(key, props)
                .and_then(|entry| heap.cdr(entry))
                .unwrap_or(Value::NULL)
        };
        let a_types = assq_ref(tx, "articulations", props);

        if same(tx, b_type, self.conf_breathing_sign_type)
            && same(tx, s_types, self.conf_script_types)
            && same(tx, a_types, self.conf_artic_types)
        {
            return Ok(false); // no change
        }

        self.conf_breathing_sign_type = Some(b_type);
        self.conf_script_types = Some(s_types);
        self.conf_artic_types = Some(a_types);
        Ok(true)
    }

    /// `make_or_remake_grobs`.
    fn make_or_remake_grobs(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.caesura_ev else {
            return Ok(());
        };
        let b_type = self.conf_breathing_sign_type.unwrap_or(Value::FALSE);
        let s_types = self.conf_script_types.unwrap_or(Value::NULL);
        let a_types = self.conf_artic_types.unwrap_or(Value::FALSE);

        // Discard the old grobs.
        self.x_parent = None;
        if let Some((sign, _)) = self.breathing_sign.take() {
            grob::suicide(tx.heap(), sign);
        }
        for (script, _) in self.scripts.drain(..) {
            grob::suicide(tx.host.heap(), script);
        }

        // Create new grobs.
        if b_type.tag() == Tag::Symbol {
            let id = tx.make_defined_grob("BreathingSign", Cause::Event(ev))?;
            let sign = tx.grob_handle(id);
            self.breathing_sign = Some((sign, id));
            let context = tx.t.arena.borrow().handle(tx.context);
            tx.call(
                "ly:breathing-sign::set-breath-properties",
                &[sign, context, b_type],
            )?;
        }

        self.x_parent = self.choose_x_parent();
        let mut scr_i = 0; // count scripts for make_script_from_event
        for s_type in elements(tx, s_types) {
            if s_type.tag() != Tag::Symbol {
                let written = tx.call("object->string", &[s_type])?;
                let written = tx
                    .heap()
                    .str(written)
                    .map(|s| alloc::string::String::from(s.as_str()));
                let msg = format!(
                    "caesura script type must be a symbol: {}",
                    written.unwrap_or_default()
                );
                tx.programming_error(&msg)?;
                continue;
            }
            let id = tx.make_defined_grob("CaesuraScript", Cause::Event(ev))?;
            let scr = tx.grob_handle(id);
            make_script_from_event(tx, scr, s_type, scr_i)?;
            if let Some(parent) = self.x_parent {
                grob::set_parent(tx.heap(), scr, parent, X);
            }
            self.scripts.push((scr, id));
            scr_i += 1;
        }

        let arts = event_property(tx.heap(), ev, "articulations");
        for art in elements(tx, arts) {
            if tx
                .heap()
                .foreign::<lily_engine_music::StreamEvent>(art)
                .is_none()
            {
                continue;
            }
            let a_type = event_property(tx.heap(), art, "articulation-type");
            // caesuraTypeTransform may have narrowed the set of acceptable
            // articulations. Check whether this one is allowed.
            if !elements(tx, a_types).contains(&a_type) {
                continue;
            }
            let id = tx.make_defined_grob("CaesuraScript", Cause::Event(art))?;
            let scr = tx.grob_handle(id);
            make_script_from_event(tx, scr, a_type, scr_i)?;
            if let Some(parent) = self.x_parent {
                grob::set_parent(tx.heap(), scr, parent, X);
            }
            // The event may override the default direction of the script.
            let d = event_property(tx.heap(), art, "direction");
            if matches!(d.as_fixnum(), Some(-1 | 1)) {
                grob::set_property(tx.heap(), scr, "direction", d);
            }
            self.scripts.push((scr, id));
            scr_i += 1;
        }
        Ok(())
    }
}

impl Work for CaesuraEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.breathing_sign.map(|b| b.1));
        let ids: Vec<u32> = self.scripts.iter().map(|s| s.1).collect();
        h.records(&ids);
        h.flags(&[
            self.observations_changed,
            self.conf_breathing_sign_type.is_some(),
            self.conf_script_types.is_some(),
            self.conf_artic_types.is_some(),
        ]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.caesura_ev, ev).map(drop)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.caesura_ev else {
            return Ok(());
        };
        self.caesura_type = tx.get("caesuraType");
        self.caesura_type_transform = tx.get("caesuraTypeTransform");

        // Form a symbol list describing the user-provided articulations.
        let arts = event_property(tx.heap(), ev, "articulations");
        let mut types = Vec::new();
        for art in elements(tx, arts) {
            if tx
                .heap()
                .foreign::<lily_engine_music::StreamEvent>(art)
                .is_none()
            {
                continue;
            }
            let a_type = event_property(tx.heap(), art, "articulation-type");
            if a_type.tag() == Tag::Symbol {
                types.push(a_type);
            }
        }
        self.user_artic_types = tx.heap().list(&types);

        if self.evaluate_caesura_type(tx)? {
            self.make_or_remake_grobs(tx)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if self.caesura_ev.is_none() || self.bar_line.is_some() {
            return Ok(());
        }
        let item = tx.grob_handle(grob);
        if !grob::has_interface(tx.heap(), item, "span-bar-interface") {
            self.bar_line = Some(item);
            self.observations_changed = true;
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.caesura_ev.is_none() {
            return Ok(());
        }
        let type_changed = self.observations_changed && self.evaluate_caesura_type(tx)?;
        self.observations_changed = false;
        if type_changed || self.x_parent != self.choose_x_parent() {
            self.make_or_remake_grobs(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Script_column will set the outside-staff-priority of every script
        // after the first, so we set the first, sorted as Script_column
        // sorts them (a stable sort).
        if !self.scripts.is_empty() {
            let mut ss: Vec<Value> = self.scripts.iter().map(|s| s.0).collect();
            for i in 1..ss.len() {
                let mut j = i;
                while j > 0 && script_priority_less(tx.host, ss[j], ss[j - 1])? {
                    ss.swap(j, j - 1);
                    j -= 1;
                }
            }
            let zero = tx.heap().integer(0);
            grob::set_property(tx.heap(), ss[0], "outside-staff-priority", zero);
        }
        *self = CaesuraEngraver::default();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.caesura_ev);
        out.extend(self.breathing_sign.map(|b| b.0));
        out.extend(self.scripts.iter().map(|s| s.0));
        out.extend(self.bar_line);
        out.extend(self.x_parent);
        out.extend([
            self.caesura_type,
            self.caesura_type_transform,
            self.user_artic_types,
        ]);
        out.extend(self.conf_breathing_sign_type);
        out.extend(self.conf_script_types);
        out.extend(self.conf_artic_types);
    }
}
