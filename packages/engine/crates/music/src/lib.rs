//! The engine's music model (chapter 04). N1 so far (§5, §13): LilyPond's
//! values with their Scheme face, the `ly:` functions of `moment-scheme.cc`,
//! `duration-scheme.cc`, `pitch-scheme.cc` and `scale.cc`, worded as
//! LilyPond 2.26.0 words their errors.
//!
//! - [`Rational`] and [`Moment`] (`rational.rs`): LilyPond's rationals as it
//!   holds them, infinities and not-a-number included, with checked
//!   arithmetic (D61).
//! - [`Duration`] (`duration.rs`): a power of two, dots and a factor.
//! - [`Pitch`] and [`Scale`] (`pitch.rs`): pitches in a scale, transposition
//!   and intervals.
//! - [`Music`] and [`MusicFunction`] (`music.rs`): music, live or frozen
//!   into a [`Tree`] (`frozen.rs`, N2), behind D65's face, with `equal?`,
//!   its `origin` an [`Input`] (`input.rs`), and music functions; lengths
//!   and starts (`length.rs`); the 131 music types in [`TYPES`]
//!   (`types.rs`), generated from `define-music-types.scm` at build time,
//!   and the C++ callbacks they name that the engine does not have yet as
//!   [`PLACEHOLDERS`] (D67); the parser's constructors, [`Maker`]
//!   (`constructors.rs`, D69).
//! - The C++ callbacks of layout that `lily.scm`'s init data names, as
//!   named placeholders (`grob.rs`, D67).
//! - [`ContextMod`] (`context_mod.rs`): context modifications, as `\with`
//!   gives them (P6).
//! - [`ContextDef`] (`context_def.rs`, N3's definitions, D73): immutable
//!   context definitions, derived ones cached by their base and mods; and
//!   [`OutputDef`] (`output_def.rs`), the output definitions they are found
//!   in.
//! - [`Score`] and [`Book`] (`book.rs`): what the parser and the init's
//!   book handlers make and collect.
//! - N3's instances: contexts, dispatchers and override stacks in a
//!   translation's [`Arena`] (`context.rs`, `dispatcher.rs`,
//!   `override.rs`, `event_class.rs`, D83).
//! - N4: the iterators ([`Iteration`], `iterator.rs`) and the global loop
//!   ([`run_translator`], `global.rs`), with the translation trace's
//!   recorder ([`Trace`], `trace.rs`).
//! - N5 (D79): translator classes and groups, listeners, acknowledgers
//!   and the timestep protocol (`translator.rs`), `Timing_translator`
//!   (`timing.rs`) and `Mark_tracking_translator` (`marks.rs`), N6's
//!   `Grace_engraver` (`grace.rs`), N7's
//!   `Repeat_acknowledge_engraver` (`volta.rs`), and the grob and
//!   audio records translators make
//!   ([`Records`], `records.rs`); translators written in Scheme
//!   (`scheme_translator.rs`); D108's slot, in which layout keeps the
//!   rest of each record's grob ([`GrobSlot`]); and the playback map's performers
//!   (`playback.rs`), which a \midi translation ([`run_performance`])
//!   writes `timing.ly`'s JSON with.
//! - MS5's MIDI (chapter 07, M2–M6): the audio elements performers make
//!   (`audio.rs`), the performers (`performers.rs`, `Dynamic_performer`'s
//!   `dynamic_performer.rs`) and the `Performance`
//!   a `\midi` translation leaves (`performance.rs`), whose items are
//!   turned into events (`midi_item.rs`) that `crates/smf` writes.
//!
//! Each is a foreign object of the engine's Scheme. [`install`] defines their
//! `ly:` functions in a VM's `(lily)`, with `lily.scm`'s
//! `default-global-scale`, which a new pitch takes its scale from, and its
//! `ly:default-scale` and `ly:set-default-scale`, and
//! `define-music-types.scm`'s `make-music`, whose table
//! [`define_music_types`] makes of [`TYPES`]. A VM's image holds them, and
//! [`EXTENSION`] is what restoring it needs of this crate.
#![no_std]
extern crate alloc;

mod audio;
mod book;
mod checkpoint;
mod constructors;
mod context;
mod context_def;
mod context_mod;
mod control_performers;
mod dispatcher;
mod duration;
mod dynamic_performer;
mod event_class;
mod frozen;
mod global;
mod grace;
mod grob;
mod input;
mod iterator;
mod length;
mod lyrics;
mod mark_performers;
mod marks;
mod melisma;
mod midi_item;
mod music;
mod operators;
mod output_def;
#[path = "override.rs"]
mod override_;
mod partition;
mod performance;
mod performers;
mod pitch;
mod playback;
mod rational;
mod records;
mod relative;
mod replay;
mod resume;
pub mod returns;
mod scheme_translator;
mod timing;
mod trace;
mod translator;
mod transpose;
mod types;
mod unpure;
mod volta;
mod warn;

use alloc::boxed::Box;
use alloc::string::String;

use lily_engine_scheme::{
    Cx, Exception, Extension, Foreign, Heap, Loader, Num, Prim, REST, Thrown, Value, Vm,
};

pub use audio::{
    Audio, AudioNote, AudioSpanDynamic, AudioStaff, AudioText, moment_to_real, moment_to_ticks,
};
pub use book::{
    Book, Books, Score, add_bookpart, add_scores_to_bookpart, embedded_layout, paper_book,
    scaled_layout, set_parent as set_book_parent, toplevel_book,
};
pub use checkpoint::{
    Change, Checkpoint, GrobDigest, Kept, ShapeNode, Step, changed, compare, verify, verify_all,
};
pub use constructors::{Maker, Warning, add_post_events, set_origin, takes_voice_name};
pub use context::{
    Arena, Context, ContextHandle, CtxId, Dir, GLOBAL, Host, Translation,
    apply_property_operations, check_removal, create_context, create_unique_context, diagnose,
    dispatch, find_create_context, get_default_interpreter, get_user_accessible_interpreter,
    grob_wrote, path_to_acceptable_context, quiet, recorded, send,
    set_property as set_context_property, unquiet, unset_property as unset_context_property,
};
pub use context_def::{ContextDef, Translator, derive_context_def, make_context_def};
pub use context_mod::{
    ContextMod, add_context_mod, context_mods, copy_context_mod, make_context_mod,
};
pub use dispatcher::{
    DispId, Dispatchers, Handler, Listener, StreamEvent, clone_event, event_property, ly_deep_copy,
    make_event, music_event, set_event_property,
};
pub use duration::Duration;
pub use event_class::{ClassSet, EVENT_CLASSES, EventClasses, class_index};
pub use frozen::{Node, Prop, Tree, first_difference, freeze, thaw};
pub use global::{
    Going, Translated, iterate, run_embedded, run_performance, run_translator, run_translator_with,
    start_resuming,
};
pub use input::{Input, Moved};
pub use iterator::{IterId, Iteration};
pub use length::CALLBACKS;
pub use marks::{current_performance_mark, current_rehearsal_mark};
pub use melisma::melisma_busy;
pub use music::{
    Music, MusicFunction, Prob, deep_copy, frozen_node, is_frozen, is_music_type, make_prob,
    music_name, music_property, mutable_properties, prob_get, prob_set, push_property,
    set_mutable_properties, set_property,
};
pub use output_def::{
    OutputDef, find_context_def, host_lookup_variable, lookup_variable, normalize, root,
    scale_output_def, set_parent,
};
pub use override_::{
    GrobProperties, matched_pop, pop, push, temporary_override, temporary_revert, updated,
};
pub use partition::{PartRun, Parts};
pub use performance::{Performance, push_header, take_written};
pub use pitch::{Pitch, Scale};
pub use rational::{Moment, Rational};
pub use records::{
    AudioRecord, Cause, GrobRecord, GrobSlot, InterfaceSet, Interfaces, Origin, Records,
};
pub use relative::CALLBACKS as RELATIVE_CALLBACKS;
pub use replay::{Op, Replayed, resolve as resolve_replayed, set_replay, set_replay_partitions};
pub use resume::{Resumable, set_check as set_check_resume};
pub use scheme_translator::{clone_grob, grob_cause, grob_of};
pub use timing::assign_event_once;
pub use trace::{Trace, grob_columns};
pub use translator::{
    ACKNOWLEDGE_AUDIO, CLASSES, FINALIZE, Held, INITIALIZE, Kind, PRE_PROCESS, PROCESS,
    PROCESS_ACKNOWLEDGED, SCORE_ENGRAVER, START, STOP, TranslatorClass, TranslatorHandle, Tx, Work,
    WorkClone, connect_global, translator_handle,
};
pub use types::{MusicType, PLACEHOLDERS, Placeholder, TYPES, define_music_types, kind};
pub use unpure::{parts_of as unpure_pure_parts, unpure_of};

type R = Result<Value, Thrown>;

/// Defines the `ly:` functions of moments, durations, pitches and scales in
/// `vm`'s `(lily)`, and the default scale.
pub fn install(vm: &mut Vm) -> Result<(), Exception> {
    vm.set_operators(operators::operator);
    vm.define_primitives("lily", PRIMS, PRELUDE)?;
    vm.define_primitives("lily", grob::PRIMS, "")?;
    vm.define_primitives("lily", CONTEXT_PRIMS, "")?;
    vm.define_primitives("lily", BOOK_PRIMS, "")?;
    vm.define_primitives("lily", INSTANCE_PRIMS, context::PRELUDE)?;
    vm.define_primitives("lily", TRANSLATOR_PRIMS, translator::PRELUDE)?;
    vm.define_primitives("lily", PERFORMANCE_PRIMS, "")?;
    vm.define_primitives("lily", &[], scheme_translator::PRELUDE)?;
    vm.define_primitives("lily", &[], length::PRELUDE)?;
    vm.define_primitives("lily", &[], relative::PRELUDE)?;
    vm.define_primitives("lily", &[], warn::PRELUDE)?;
    vm.define_primitives("lily", &[], unpure::PRELUDE)?;
    vm.add_host_handlers(&global::HOSTS);
    vm.define_primitives("lily", &[], global::PRELUDE)
}

/// What [`lily_engine_scheme::Vm::restore`] needs of a VM that [`install`]
/// was given: the primitives, and the loaders of its types.
pub static EXTENSION: Extension = Extension {
    prims: &[
        PRIMS,
        grob::PRIMS,
        CONTEXT_PRIMS,
        BOOK_PRIMS,
        INSTANCE_PRIMS,
        TRANSLATOR_PRIMS,
        PERFORMANCE_PRIMS,
    ],
    operators: Some(operators::operator),
    hosts: &global::HOSTS,
    loaders: &[
        Loader {
            name: "Moment",
            load: |r| Some(Box::new(Moment::load(r)?)),
        },
        Loader {
            name: "Duration",
            load: |r| Some(Box::new(Duration::load(r)?)),
        },
        Loader {
            name: "Scale",
            load: |r| Some(Box::new(Scale::load(r)?)),
        },
        Loader {
            name: "Pitch",
            load: |r| Some(Box::new(Pitch::load(r)?)),
        },
        Loader {
            name: "Music",
            load: |r| Some(Box::new(Music::load(r)?)),
        },
        Loader {
            name: "Music_tree",
            load: |r| Some(Box::new(Tree::load(r)?)),
        },
        Loader {
            name: "Input",
            load: |r| Some(Box::new(Input::load(r)?)),
        },
        Loader {
            name: "Unpure_pure_container",
            load: |r| Some(Box::new(unpure::UnpurePure::load(r)?)),
        },
        Loader {
            name: "Context_mod",
            load: |r| Some(Box::new(context_mod::ContextMod::load(r)?)),
        },
        Loader {
            name: "Context_def",
            load: |r| Some(Box::new(context_def::ContextDef::load(r)?)),
        },
        Loader {
            name: "Output_def",
            load: |r| Some(Box::new(output_def::OutputDef::load(r)?)),
        },
        Loader {
            name: "Music_function",
            load: |r| Some(Box::new(MusicFunction::load(r)?)),
        },
    ],
};

/// `lily.scm`'s default scale, `ly:music-property`'s setter, and
/// `define-music-types.scm`'s `make-music` over `%music-properties`, the
/// table [`define_music_types`] sets. A scale that is not one and a music
/// type that is unknown are `ly:error`s, as in LilyPond.
const PRELUDE: &str = r#"
(define-session default-global-scale (ly:make-scale #(0 1 2 5/2 7/2 9/2 11/2)))
(define-public (ly:default-scale)
  "Get the global default scale."
  default-global-scale)
(define-public (ly:set-default-scale scale)
  (if (ly:note-scale? scale)
      (set! default-global-scale scale)
      (ly:error (G_ "note scale expected: ~S") scale)))
;; `Music_function::call`: a music function called from Scheme, its
;; arguments matched to its signature as the parser matches them, an
;; optional one that fails, or `*unspecified*`, and those after it given
;; their defaults; music in a default or the result at `(*location*)`.
;; The errors are `ly-syntax-constructors.scm`'s (part 2's file), which
;; LilyPond keeps in that module, `(lily)` exporting neither (OPEN F).
(define (%music-function-at arg clone?)
  (if (ly:music? arg)
      (let ((m (if clone? (ly:music-deep-copy arg) arg))
            (loc (*location*)))
        (if (ly:input-location? loc) (ly:music-set-property! m 'origin loc))
        m)
      arg))
(define (music-function-call-error fun m)
  (let* ((sigcar (car (ly:music-function-signature fun)))
         (pred? (if (pair? sigcar) (car sigcar) sigcar)))
    (ly:parser-error
     (format #f (G_ "~a function cannot return ~a")
             (type-name pred?)
             (value->lily-string m))
     (*location*))
    (and (pair? sigcar)
         (if (ly:music? (cdr sigcar))
             (ly:music-deep-copy (cdr sigcar) (*location*))
             (cdr sigcar)))))
(define (argument-error n pred arg)
  (ly:parser-error
   (format #f
           (G_ "wrong type for argument ~a.  Expecting ~a, found ~s")
           n (type-name pred) (music->make-music arg))
   (*location*)))
(define (%music-function-call self . rest)
  (let ((result (car (ly:music-function-signature self))))
    (define (defaults sig args)
      (if (and (pair? sig) (pair? (car sig)))
          (defaults (cdr sig) (cons (%music-function-at (cdar sig) #t) args))
          (values sig args)))
    (let loop ((rest rest) (sig (cdr (ly:music-function-signature self))) (args '()))
      (if (and (pair? rest) (pair? sig))
          (let ((arg (car rest)) (pred (car sig)))
            (cond ((not (pair? pred))
                   (if (pred arg)
                       (loop (cdr rest) (cdr sig) (cons arg args))
                       (begin
                         (argument-error (1+ (length args)) pred arg)
                         (%music-function-at (if (pair? result) (cdr result) #f) #t))))
                  (((car pred) arg) (loop (cdr rest) (cdr sig) (cons arg args)))
                  (else
                   (call-with-values (lambda () (defaults sig args))
                     (lambda (sig args)
                       (loop (if (unspecified? arg) (cdr rest) rest) sig args))))))
          (call-with-values (lambda () (defaults sig args))
            (lambda (sig args)
              (if (or (pair? rest) (pair? sig))
                  (scm-error 'wrong-number-of-args #f
                             "Wrong number of arguments to ~A" (list self) #f))
              (let ((res (apply (ly:music-function-extract self) (reverse args)))
                    (pred (if (pair? result) (car result) result)))
                (if (pred res)
                    (%music-function-at res #f)
                    (music-function-call-error self res)))))))))
;; `ly:set-origin!`'s default is `(*location*)`.
(define-public (ly:set-origin! m . origin)
  (%music-set-origin! m (if (pair? origin) (car origin) (*location*))))
;; `LY_DEFINE_WITH_SETTER`: `(set! (ly:music-property m 'key) value)`, and
;; `ly:prob-property`'s.
(define ly:prob-property
  (make-procedure-with-setter ly:prob-property ly:prob-set-property!))
(define ly:music-property
  (make-procedure-with-setter ly:music-property ly:music-set-property!))
(define-public %music-properties '())
(define-public (make-music name . music-properties)
  "Create a music object of given name, and set its properties
according to @code{music-properties}, a list of alternating property symbols
and values."
  (if (not (symbol? name))
      (ly:error (G_ "symbol expected: ~S") name))
  (let ((props (assq-ref %music-properties name)))
    (if (not (pair? props))
        (ly:error (G_ "cannot find music object: ~S") name))
    (let ((m (ly:make-music props)))
      (define (alist-set-props lst)
        (for-each (lambda (e)
                    (ly:music-set-property! m (car e) (cdr e)))
                  (reverse lst)))
      (define (set-props mus-props)
        (if (pair? mus-props)
            (let ((e (car mus-props))
                  (mus-props (cdr mus-props)))
              (cond ((symbol? e)
                     (ly:music-set-property! m e (car mus-props))
                     (set-props (cdr mus-props)))
                    ((ly:music? e)
                     (alist-set-props (ly:music-mutable-properties e))
                     (set-props mus-props))
                    ((or (pair? e) (null? e))
                     (alist-set-props e)
                     (set-props mus-props))
                    (else
                     (ly:error (G_ "bad make-music argument: ~S") e))))))
      (set-props music-properties)
      m)))
"#;

const fn p(name: &'static str, min: u8, max: u8, f: lily_engine_scheme::PrimFn) -> Prim {
    Prim { name, min, max, f }
}

/// The `i`th of [`CALLBACKS`] as a primitive.
macro_rules! cb {
    ($i:literal) => {
        p(length::CALLBACKS[$i], 1, 1, |cx, args| {
            length::callback(cx, $i, args[0])
        })
    };
}

/// The `i`th of [`RELATIVE_CALLBACKS`] as a primitive.
macro_rules! rel {
    ($i:literal) => {
        p(relative::CALLBACKS[$i], 2, 2, |cx, args| {
            relative::callback(cx, $i, args)
        })
    };
}

/// The `i`th of [`PLACEHOLDERS`] as a primitive.
macro_rules! ph {
    ($i:literal) => {
        types::placeholder_prim($i, |cx, _| types::unimplemented(cx, $i))
    };
}

/// The primitives of scores and books (MS2 step 16).
static BOOK_PRIMS: &[Prim] = &[
    p("ly:make-score", 1, 1, book::ly_make_score),
    p("ly:score?", 1, 1, book::is_score),
    p("ly:score-music", 1, 1, book::ly_score_music),
    p("ly:score-header", 1, 1, book::ly_score_header),
    p("ly:score-set-header!", 2, 2, book::ly_score_set_header),
    p("ly:score-error?", 1, 1, book::ly_score_error),
    p("ly:score-output-defs", 1, 1, book::ly_score_output_defs),
    p(
        "ly:score-add-output-def!",
        2,
        2,
        book::ly_score_add_output_def,
    ),
    p("ly:book?", 1, 1, book::is_book),
    p("ly:book-add-score!", 2, 2, book::ly_book_add_score),
    p("ly:book-add-bookpart!", 2, 2, book::ly_book_add_bookpart),
    p("ly:book-scores", 1, 1, book::ly_book_scores),
    p("ly:book-book-parts", 1, 1, book::ly_book_book_parts),
    p("ly:make-book-part", 1, 1, book::ly_make_book_part),
];

/// MS5's performances (`performance.rs`): `performance-scheme.cc`'s.
static PERFORMANCE_PRIMS: &[Prim] = &[
    p(
        "ly:performance-headers",
        1,
        1,
        performance::ly_performance_headers,
    ),
    p(
        "ly:performance-write",
        3,
        3,
        performance::ly_performance_write,
    ),
];

/// N5's translators (`translator.rs`): `ly:translator?`,
/// `ly:translator-context`, and the `%` ones of the creators of the
/// classes, which `translator::PRELUDE`'s `ly:translator-name`,
/// `ly:translator-description` and `ly:get-all-translators` read; and what
/// Scheme engravers make grobs with (`scheme_translator.rs`).
static TRANSLATOR_PRIMS: &[Prim] = &[
    p("ly:translator?", 1, 1, translator::is_translator),
    p(
        "ly:translator-context",
        1,
        1,
        translator::ly_translator_context,
    ),
    p("%translator-creators", 0, 0, translator::creators),
    p("ly:grob?", 1, 1, scheme_translator::is_grob),
    p(
        "ly:engraver-make-grob",
        3,
        3,
        scheme_translator::ly_engraver_make_grob,
    ),
    p(
        "ly:engraver-make-item",
        3,
        3,
        scheme_translator::ly_engraver_make_item,
    ),
    p(
        "ly:engraver-announce-end-grob",
        3,
        3,
        scheme_translator::ly_engraver_announce_end_grob,
    ),
    p(
        "ly:grob-property",
        2,
        3,
        scheme_translator::ly_grob_property,
    ),
    p(
        "ly:grob-set-property!",
        3,
        3,
        scheme_translator::ly_grob_set_property,
    ),
    p("%class-creator-name", 1, 1, translator::creator_name),
    p(
        "%class-creator-description",
        1,
        1,
        translator::creator_description,
    ),
];

/// N3's instances (`context.rs`, `override.rs`, `dispatcher.rs`): the
/// `ly:context-*` primitives, those of dispatchers (`ly:broadcast` among
/// them, D85), grob properties and stream events, `ly:set-middle-C!`, and
/// the `%` ones that `context::PRELUDE`'s checking procedures call.
static INSTANCE_PRIMS: &[Prim] = &[
    p("ly:context?", 1, 1, context::is_context),
    p(
        "ly:context-schedule-moment",
        2,
        2,
        global::ly_context_schedule_moment,
    ),
    p("ly:context-alias?", 2, 2, context::ly_context_alias),
    p(
        "ly:context-current-moment",
        1,
        1,
        context::ly_context_current_moment,
    ),
    p("ly:context-id", 1, 1, context::ly_context_id),
    p("ly:context-name", 1, 1, context::ly_context_name),
    p("ly:context-parent", 1, 1, context::ly_context_parent),
    p("ly:context-children", 1, 1, context::ly_context_children),
    p("ly:context-find", 2, 2, context::ly_context_find),
    p(
        "ly:context-event-source",
        1,
        1,
        context::ly_context_event_source,
    ),
    p(
        "ly:context-events-below",
        1,
        1,
        context::ly_context_events_below,
    ),
    p(
        "ly:context-output-def",
        1,
        1,
        context::ly_context_output_def,
    ),
    p("ly:context-property", 2, 7, context::ly_context_property),
    p(
        "ly:context-property-where-defined",
        2,
        3,
        context::ly_context_property_where_defined,
    ),
    p(
        "ly:context-grob-definition",
        2,
        2,
        override_::ly_context_grob_definition,
    ),
    p(
        "ly:context-matched-pop-property",
        3,
        3,
        override_::ly_context_matched_pop_property,
    ),
    p(
        "%context-set-property!",
        3,
        3,
        context::context_set_property,
    ),
    p(
        "%context-unset-property",
        2,
        2,
        context::context_unset_property,
    ),
    p("%context-check", 2, 2, context::context_check),
    p(
        "%context-grob-create!",
        2,
        2,
        override_::context_grob_create,
    ),
    p(
        "%context-push-property!",
        4,
        4,
        override_::context_push_property,
    ),
    p(
        "%context-pop-property!",
        3,
        3,
        override_::context_pop_property,
    ),
    p("ly:grob-properties?", 1, 1, override_::is_grob_properties),
    p(
        "ly:make-grob-properties",
        1,
        1,
        override_::ly_make_grob_properties,
    ),
    p("ly:dispatcher?", 1, 1, context::is_dispatcher),
    p("ly:add-listener", 2, 12, context::ly_add_listener),
    p("ly:broadcast", 2, 2, context::ly_broadcast),
    p("ly:set-middle-C!", 1, 1, context::ly_set_middle_c),
    p(
        "ly:connect-dispatchers",
        2,
        2,
        context::ly_connect_dispatchers,
    ),
    p(
        "ly:disconnect-dispatchers",
        2,
        2,
        context::ly_disconnect_dispatchers,
    ),
    p(
        "ly:listened-event-types",
        1,
        1,
        context::ly_listened_event_types,
    ),
    p(
        "ly:listened-event-class?",
        2,
        2,
        context::ly_listened_event_class,
    ),
    p("ly:stream-event?", 1, 1, dispatcher::is_stream_event),
    p(
        "ly:make-stream-event",
        1,
        2,
        dispatcher::ly_make_stream_event,
    ),
    p("ly:event-property", 2, 3, dispatcher::ly_event_property),
    p(
        "ly:event-set-property!",
        3,
        3,
        dispatcher::ly_event_set_property,
    ),
];

/// The primitives of context definitions and the output definitions that
/// hold them (N3), and `ly:context-mod-apply!`.
static CONTEXT_PRIMS: &[Prim] = &[
    p("ly:context-def?", 1, 1, context_def::is_context_def),
    p(
        "ly:context-def-lookup",
        2,
        3,
        context_def::ly_context_def_lookup,
    ),
    p(
        "ly:context-def-modify",
        2,
        2,
        context_def::ly_context_def_modify,
    ),
    p(
        "ly:context-mod-apply!",
        2,
        2,
        context_mod::ly_context_mod_apply,
    ),
    p("ly:output-def?", 1, 1, output_def::is_output_def),
    p("ly:output-def-clone", 1, 1, output_def::ly_output_def_clone),
    p("ly:make-output-def", 0, 0, output_def::ly_make_output_def),
    p(
        "ly:output-def-lookup",
        2,
        3,
        output_def::ly_output_def_lookup,
    ),
    p(
        "ly:output-def-parent",
        1,
        2,
        output_def::ly_output_def_parent,
    ),
    p("ly:output-def-scope", 1, 1, output_def::ly_output_def_scope),
    p(
        "ly:output-def-set-variable!",
        3,
        3,
        output_def::ly_output_def_set_variable,
    ),
    p("ly:paper-get-number", 2, 2, output_def::ly_paper_get_number),
    p(
        "ly:output-find-context-def",
        1,
        2,
        output_def::ly_output_find_context_def,
    ),
    p(
        "ly:output-description",
        1,
        1,
        output_def::ly_output_description,
    ),
];

/// The primitives, in the order a VM numbers them.
static PRIMS: &[Prim] = &[
    p("ly:make-moment", 1, 4, rational::make_moment),
    p("ly:moment?", 1, 1, rational::is_moment),
    p("ly:moment-add", 2, 2, rational::moment_add),
    p("ly:moment-sub", 2, 2, rational::moment_sub),
    p("ly:moment-mul", 2, 2, rational::moment_mul),
    p("ly:moment-div", 2, 2, rational::moment_div),
    p("ly:moment-main", 1, 1, rational::moment_main),
    p("ly:moment-grace", 1, 1, rational::moment_grace),
    p(
        "ly:moment-main-numerator",
        1,
        1,
        rational::moment_main_numerator,
    ),
    p(
        "ly:moment-main-denominator",
        1,
        1,
        rational::moment_main_denominator,
    ),
    p(
        "ly:moment-grace-numerator",
        1,
        1,
        rational::moment_grace_numerator,
    ),
    p(
        "ly:moment-grace-denominator",
        1,
        1,
        rational::moment_grace_denominator,
    ),
    p("ly:moment<?", 2, 2, rational::moment_less),
    p("ly:make-duration", 1, 4, duration::make_duration),
    p("ly:duration?", 1, 1, duration::is_duration),
    p("ly:duration<?", 2, 2, duration::duration_less),
    p("ly:duration::less?", 2, 2, duration::less),
    p("ly:number->duration", 1, 1, duration::number_to_duration),
    p("ly:duration-log", 1, 1, duration::duration_log),
    p("ly:duration-dot-count", 1, 1, duration::duration_dot_count),
    p("ly:intlog2", 1, 1, duration::ly_intlog2),
    p("ly:duration->moment", 1, 1, duration::duration_to_moment),
    p("ly:duration->number", 1, 1, duration::duration_to_number),
    p("ly:duration->string", 1, 1, duration::duration_to_string),
    p("ly:duration-factor", 1, 1, duration::duration_factor),
    p("ly:duration-scale", 1, 1, duration::duration_scale),
    p("ly:duration-compress", 2, 2, duration::duration_compress),
    p("ly:make-pitch", 2, 3, pitch::make_pitch),
    p("ly:pitch?", 1, 1, pitch::is_pitch),
    p("ly:pitch-transpose", 2, 2, pitch::pitch_transpose),
    p("ly:pitch-negate", 1, 1, pitch::pitch_negate),
    p("ly:pitch-steps", 1, 1, pitch::pitch_steps),
    p("ly:pitch-octave", 1, 1, pitch::pitch_octave),
    p("ly:pitch-alteration", 1, 1, pitch::pitch_alteration),
    p("ly:pitch-notename", 1, 1, pitch::pitch_notename),
    p("ly:pitch-tones", 1, 1, pitch::pitch_tones),
    p("ly:pitch-quartertones", 1, 1, pitch::pitch_quartertones),
    p("ly:pitch-semitones", 1, 1, pitch::pitch_semitones),
    p("ly:pitch<?", 2, 2, pitch::pitch_less),
    p("ly:pitch-diff", 2, 2, pitch::pitch_diff),
    p("ly:make-scale", 1, 1, pitch::make_scale),
    p("ly:note-scale?", 1, 1, pitch::is_scale),
    p("ly:make-music", 1, 1, music::make_music),
    p("ly:music?", 1, 1, music::is_music),
    p("ly:event?", 1, 1, music::is_event),
    p("ly:music-property", 2, 3, music::ly_music_property),
    p("ly:music-set-property!", 3, 3, music::music_set_property),
    p(
        "ly:music-mutable-properties",
        1,
        1,
        music::music_mutable_properties,
    ),
    p("ly:music-deep-copy", 1, 2, music::music_deep_copy),
    p("ly:make-music-function", 2, 2, music::make_music_function),
    p("ly:music-function?", 1, 1, music::is_music_function),
    p(
        "ly:music-function-signature",
        1,
        1,
        music::music_function_signature,
    ),
    p(
        "ly:music-function-extract",
        1,
        1,
        music::music_function_extract,
    ),
    p("ly:music-length", 1, 1, length::music_length),
    p("ly:music-start", 1, 1, length::music_start),
    p("ly:input-location?", 1, 1, input::is_input),
    p(
        "ly:input-file-line-char-column",
        1,
        1,
        input::file_line_char_column,
    ),
    p("%diagnostic-mark", 0, 0, warn::mark),
    p("ly:number->string", 1, 1, warn::number_to_string),
    p(
        "ly:translate-cpp-warning-scheme",
        1,
        1,
        warn::translate_cpp_warning_scheme,
    ),
    p("%music-freeze!", 1, 1, music::music_freeze),
    p("%music-frozen?", 1, 1, music::music_frozen),
    p("%music-children", 2, 2, length::music_children),
    p(
        "%music-immutable-property",
        2,
        2,
        length::music_immutable_property,
    ),
    // The lengths' and starts' callbacks, in `length::CALLBACKS`' order.
    cb!(0),
    cb!(1),
    cb!(2),
    cb!(3),
    cb!(4),
    cb!(5),
    cb!(6),
    cb!(7),
    cb!(8),
    cb!(9),
    cb!(10),
    p("ly:music-transpose", 2, 2, transpose::music_transpose),
    p(
        "ly:transpose-key-alist",
        2,
        2,
        transpose::transpose_key_alist_prim,
    ),
    p(
        "ly:make-music-relative!",
        2,
        2,
        relative::make_music_relative,
    ),
    p("%music-relative-pitch!", 2, 2, relative::relative_pitch),
    p("%music-set-origin!", 2, 2, music::music_set_origin),
    p(
        "ly:music-duration-length",
        1,
        1,
        music::music_duration_length,
    ),
    p(
        "ly:music-duration-compress",
        2,
        2,
        music::music_duration_compress,
    ),
    p("ly:music-list?", 1, 1, music::is_music_list),
    p("ly:make-prob", 2, REST, music::make_prob_p),
    p("ly:prob?", 1, 1, music::is_prob),
    p("ly:prob-property", 2, 3, music::prob_property),
    p("ly:prob-set-property!", 2, 3, music::prob_set_property),
    p("ly:prob-property?", 2, 2, music::prob_property_p),
    p(
        "ly:prob-mutable-properties",
        1,
        1,
        music::prob_mutable_properties,
    ),
    p(
        "ly:prob-immutable-properties",
        1,
        1,
        music::prob_immutable_properties,
    ),
    p("ly:make-unpure-pure-container", 1, 2, unpure::make),
    p("ly:unpure-pure-container?", 1, 1, unpure::is_container),
    p(
        "ly:unpure-pure-container-unpure-part",
        1,
        1,
        unpure::unpure_part,
    ),
    p(
        "ly:unpure-pure-container-pure-part",
        1,
        1,
        unpure::pure_part,
    ),
    p("%unpure-pure-unchanging?", 1, 1, unpure::is_unchanging),
    p("ly:bezier-extract", 3, 3, grob::bezier_extract),
    p(
        "ly:make-context-mod",
        0,
        1,
        context_mod::ly_make_context_mod,
    ),
    p("ly:context-mod?", 1, 1, context_mod::is_context_mod),
    p("ly:add-context-mod", 2, 2, context_mod::ly_add_context_mod),
    p(
        "ly:get-context-mods",
        1,
        1,
        context_mod::ly_get_context_mods,
    ),
    // The relative callbacks, in `relative::CALLBACKS`' order.
    rel!(0),
    rel!(1),
    rel!(2),
    rel!(3),
    rel!(4),
    // D67's placeholders, in `types::PLACEHOLDERS`' order.
    ph!(0),
    ph!(1),
    ph!(2),
    ph!(3),
    ph!(4),
    ph!(5),
    ph!(6),
    ph!(7),
    ph!(8),
    ph!(9),
    ph!(10),
    ph!(11),
    ph!(12),
    ph!(13),
    ph!(14),
    ph!(15),
    ph!(16),
    ph!(17),
    ph!(18),
    ph!(19),
    ph!(20),
    ph!(21),
    ph!(22),
    ph!(23),
    ph!(24),
];

// --- LilyPond's conversions (lily-guile.cc) ------------------------------------------

/// `is_scm<Rational>`: an exact real, or an infinity.
fn is_rational(heap: &Heap, v: Value) -> bool {
    match heap.num(v) {
        Some(Num::Int(_) | Num::Ratio(..)) => true,
        Some(Num::Real(x)) => x.is_infinite(),
        _ => false,
    }
}

/// `ly_is_list`: a proper list.
fn is_list(heap: &Heap, v: Value) -> bool {
    // Floyd's: a cycle is no list.
    let (mut slow, mut fast) = (v, v);
    loop {
        for _ in 0..2 {
            match heap.cdr(fast) {
                Some(next) => fast = next,
                None => return fast == Value::NULL,
            }
        }
        slow = heap.cdr(slow).unwrap_or(Value::NULL);
        if slow == fast {
            return false;
        }
    }
}

/// `scm_is_true (scm_positive_p (v))` of a real number.
fn is_positive(heap: &Heap, v: Value) -> bool {
    match heap.num(v) {
        Some(Num::Int(n) | Num::Ratio(n, _)) => n.signum() > 0,
        Some(Num::Real(x)) => x > 0.0,
        _ => false,
    }
}

/// `LY_ASSERT_TYPE`: `Wrong type argument in position N (expecting what)`.
fn check(
    cx: &mut Cx,
    ok: bool,
    subr: &str,
    position: usize,
    v: Value,
    what: &str,
) -> Result<(), Thrown> {
    if ok {
        Ok(())
    } else {
        Err(cx.wrong_type(subr, position, v, what))
    }
}

/// `LY_ASSERT_TYPE (scm_is_integer, …)`: `2` and `2.0`.
fn check_integer(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(), Thrown> {
    let ok = cx.heap().is_integer(v);
    check(cx, ok, subr, position, v, "integer")
}

/// `LY_ASSERT_TYPE (is_scm<Rational>, …)`, then `from_scm<Rational>`.
fn rational_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Rational, Thrown> {
    let ok = is_rational(cx.heap(), v);
    check(cx, ok, subr, position, v, "rational")?;
    to_rational(cx, v)
}

/// `scm_to_int64`: an exact integer in range, Guile's errors otherwise.
fn to_i64(cx: &mut Cx, v: Value) -> Result<i64, Thrown> {
    match cx.heap().to_i64(v) {
        Some(n) => Ok(n),
        None if cx.heap().is_exact_integer(v) => {
            Err(cx.out_of_bounds(i64::MIN, i64::MAX as u64, v))
        }
        None => Err(cx.wrong_type_vm(None, v, "exact integer")),
    }
}

/// `scm_to_int`, LilyPond's `from_scm<int>`.
fn to_i32(cx: &mut Cx, v: Value) -> Result<i32, Thrown> {
    let n = to_i64(cx, v)?;
    i32::try_from(n).map_err(|_| cx.out_of_bounds(i32::MIN as i64, i32::MAX as u64, v))
}

/// `from_scm<Rational>`: ±∞ from an infinity, else the numerator and the
/// denominator as `int64_t`s.
fn to_rational(cx: &mut Cx, v: Value) -> Result<Rational, Thrown> {
    if let Some(x) = cx.heap().as_flonum(v).filter(|x| x.is_infinite()) {
        return Ok(if x > 0.0 {
            Rational::INFINITY
        } else {
            -Rational::INFINITY
        });
    }
    let (n, d) = match cx.heap().as_ratio(v) {
        Some((n, d)) => (to_i64(cx, n)?, to_i64(cx, d)?),
        None => (to_i64(cx, v)?, 1),
    };
    Ok(Rational::new(n, d))
}

/// `to_scm (Rational)`: `±inf.0` for an infinity, else the numerator
/// divided by the denominator, Guile's overflow for NaN's `0/0`.
fn rational(cx: &mut Cx, r: Rational) -> R {
    if r.is_inf() {
        let x = if r.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        return Ok(cx.heap_mut().flonum(x));
    }
    let n = cx.heap_mut().integer(r.numerator());
    let d = cx.heap_mut().integer(r.denominator());
    cx.heap_mut()
        .div(n, d)
        .map_err(|e| cx.num_error("/", e, &[n, d]))
}

/// `is_scm<Rational>` and `from_scm<Rational>` on a heap: `None` for a value
/// that is none, or whose parts do not fit.
pub(crate) fn rational_of(heap: &Heap, v: Value) -> Option<Rational> {
    if !is_rational(heap, v) {
        return None;
    }
    if let Some(x) = heap.as_flonum(v) {
        return Some(if x > 0.0 {
            Rational::INFINITY
        } else {
            -Rational::INFINITY
        });
    }
    Some(match heap.as_ratio(v) {
        Some((n, d)) => Rational::new(heap.to_i64(n)?, heap.to_i64(d)?),
        None => Rational::new(heap.to_i64(v)?, 1),
    })
}

/// `to_scm (Rational)` on a heap, `#f` for NaN.
pub(crate) fn rational_value(heap: &mut Heap, r: Rational) -> Value {
    if r.is_inf() {
        let x = if r.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        return heap.flonum(x);
    }
    let n = heap.integer(r.numerator());
    let d = heap.integer(r.denominator());
    heap.div(n, d).unwrap_or(Value::FALSE)
}

/// A checked result that did not fit, where LilyPond's would wrap (D61).
#[cold]
fn overflow(cx: &mut Cx, subr: &str) -> Thrown {
    cx.error("numerical-overflow", Some(subr), "Numerical overflow", &[])
}

/// `LY_ASSERT_SMOB`: the foreign object of type `T`, whose type `name` is.
fn arg<T: Foreign + Copy>(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
    name: &str,
) -> Result<T, Thrown> {
    match cx.heap().foreign::<T>(v) {
        Some(&x) => Ok(x),
        None => Err(cx.wrong_type(subr, position, v, name)),
    }
}

fn is<T: Foreign>(cx: &mut Cx, v: Value) -> R {
    Ok(Value::bool(cx.heap().foreign::<T>(v).is_some()))
}

/// A string of what `write` writes.
fn string(cx: &mut Cx, f: impl FnOnce(&mut String)) -> Value {
    let mut out = String::new();
    f(&mut out);
    cx.heap_mut().string(out)
}
