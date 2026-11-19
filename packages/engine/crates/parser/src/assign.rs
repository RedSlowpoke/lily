//! Assignments and lookups (02 §6's P3): `parser.yy`'s `assignment`,
//! `identifier_init`, `property_path`, `symbol_list_part` and `lookup`, and
//! `set_identifier` (`lily-lexer.cc`), which defines a value into the
//! innermost scope, in a nested alist for `name.path = value`
//! (`nested-property.cc`).
//!
//! An assignment reads the token after its value before it defines, as
//! Bison does when it reduces `assignment` with that token as lookahead:
//! `x = { c }` then `\x` is an unknown command. After a value that is a
//! `SCM_IDENTIFIER` or a `FRACTION`, Bison reduces without looking, and the
//! next token is lexed after the definition. These are §5.4's table for
//! P3's values, each measured with LilyPond 2.26.0. P4's: a `#` form's
//! value is defined before the next token is lexed, a `$` form's by the
//! kind of its token, as a `\name`'s.
//!
//! In a property path, a part may be a `#` form or a `SCM_IDENTIFIER`
//! whose value is a key, a string or a list of them (`symbol_list_part`).
//!
//! P5's function calls as values (§5.4, measured with LilyPond 2.26.0): a
//! music or an event function's is read past, as music is; a Scheme
//! function's only when its last argument's reading looked at the token
//! after it (`x = \fun 5`, for a unit, but not `x = \fun #5`), which the
//! argument's own reading does here.
//!
//! P5's other values (§5.4, measured with LilyPond 2.26.0): a number
//! expression (`number.rs`) is defined after the token that ends it is
//! lexed; a block (`output_def.rs`) before the token after its `}`. In an
//! output definition the assignment's tokens are lexed in initial mode
//! ([`Parser::base`]); music, post-events and a call's arguments as a value
//! there are read in notes mode, where LilyPond lexes them in initial mode
//! too.
//!
//! P7's: `\markup …` and `\markuplist …` are defined before the token after
//! them is lexed, which is lexed in the block's mode, the markup's popped.
//!
//! P6's values: `\with`, and `\context { … }` as a block is, are defined
//! before the token after them is lexed; contexted music after it.
//! `\change Type = id` (`simple_music`'s `context_change`; MS2 step 36,
//! T25's `csh = \change Staff = "high"`) is defined before it, measured
//! with LilyPond 2.26.0, and so is `\override` or `\set` whose value is a
//! `#` form (MS8 step 10, OPEN F).

use crate::bison;
use crate::classify::Id;
use crate::context::{CHANGE, CONTEXT, DefOrMusic, NEWCONTEXT, WITH};
use crate::diag::{At, Severity};
use crate::lexer::{KEYWORDS, Mode, Tok, Token};
use crate::markup::FullMarkup;
use crate::partial::Init;
use crate::scheme::scheme_keys;
use crate::syntax::{Assignment, Key, Music, MusicKind, Note, PostEvent, Value};
use crate::{
    BOOK, BOOKPART, HEADER, LAYOUT, LYRICMODE, MARKUP, MARKUPLIST, MIDI, NOTEMODE, PAPER, Parsed,
    Parser, SCORE, Span,
};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Heap, Tag, Value as Scm, parse_number};

/// Whether a note is only its pitch: `pitch_or_music`'s action makes music
/// of it only for `!`, `?`, an octave check, a duration, `\rest` or
/// post-events.
pub(crate) fn bare_pitch(note: &Note) -> bool {
    !note.force_accidental
        && !note.cautionary
        && note.octave_check.is_none()
        && note.duration.span.is_none()
        && !note.rest
        && note.post.is_empty()
}

impl Parser<'_, '_> {
    /// `assignment`, at its name (`assignment_id`, a word or a string):
    /// `name = value` or `name.path = value`, its value defined as
    /// `set_identifier` defines it, into the innermost scope. Its tokens
    /// are lexed in the block's mode.
    pub(crate) fn assignment(&mut self) -> Parsed<Assignment> {
        let name = self.next(self.base);
        let text = match name.kind {
            Tok::String => String::from(self.lexer.string(&name)),
            _ => String::from_utf8_lossy(self.lexer.text_of(name.span)).into_owned(),
        };
        let mut path = Vec::new();
        if self.peek(self.base).kind == Tok::Char(b'.') {
            self.next(self.base);
            path = self.property_path()?;
            self.expect(Tok::Char(b'='), self.base, bison::EQUALS)?;
        } else {
            self.expect(Tok::Char(b'='), self.base, bison::ASSIGNMENT_ID)?;
        }
        let (value, scheme, peeks) = self.identifier_init()?;
        if peeks {
            self.peek(self.base);
        }
        self.set_identifier(&text, &path, scheme);
        Ok(Assignment {
            name: name.span,
            path,
            value,
        })
    }

    /// `identifier_init`: its syntax, the Scheme value it defines (rooted),
    /// and whether Bison reads the token after it before the assignment is
    /// reduced. Music, post-events and calls are read in notes mode
    /// ([`Parser::in_notes`]).
    pub(crate) fn identifier_init(&mut self) -> Parsed<(Value, Scm, bool)> {
        let token = self.peek(self.base);
        let (value, scheme) = match token.kind {
            Tok::String | Tok::Symbol | Tok::Unsigned => {
                self.next(self.base);
                let first = self.key(&token);
                if matches!(self.peek(self.base).kind, Tok::Char(b'.' | b',')) {
                    // `symbol_list_part_bare '.' property_path`.
                    self.next(self.base);
                    let mut keys = alloc::vec![first];
                    keys.extend(self.property_path()?);
                    let list = self.symbol_list(&keys);
                    (Value::SymbolList(keys), list)
                } else {
                    match first {
                        Key::Index(_) => {
                            let value = self.number_value(&token);
                            let value = self.bare_number(value);
                            let value = self.number_expression(value)?;
                            (Value::Number(self.span_from(token.span)), value)
                        }
                        Key::Symbol(text) => {
                            let string = self.env.vm().heap_mut().string(text.as_str());
                            (Value::String(text), string)
                        }
                    }
                }
            }
            Tok::Real | Tok::Identifier(Id::Number) => {
                let value = self.number_factor()?;
                let value = self.number_expression(value)?;
                (Value::Number(self.span_from(token.span)), value)
            }
            Tok::Char(b'-') => {
                self.next(self.base);
                let number = self.peek(self.base);
                if matches!(
                    number.kind,
                    Tok::Unsigned | Tok::Real | Tok::Identifier(Id::Number)
                ) {
                    // `'-' number_factor`.
                    let factor = self.number_factor()?;
                    let negated = self.env.vm().heap_mut().negate(factor).unwrap_or(factor);
                    self.env.keep(negated);
                    let value = self.number_expression(negated)?;
                    (Value::Number(self.span_from(token.span)), value)
                } else {
                    return self.partial_value(Some(token));
                }
            }
            Tok::Fraction => {
                self.next(self.base);
                let fraction = self.fraction(&token);
                return Ok((Value::Scheme(token.span), fraction, false));
            }
            Tok::Char(b'^' | b'_')
            | Tok::Identifier(Id::EventFunction | Id::MusicFunction | Id::ScmFunction) => {
                return self.partial_value(None);
            }
            kind if crate::partial::starts_partial(kind) => return self.partial_value(None),
            Tok::Char(b':') | Tok::EUnsigned | Tok::Identifier(Id::Event) => {
                self.in_notes(|p| {
                    // `post_event_cons` puts a `PostEvents` music's events first
                    // in it last first.
                    let mut events = p.post_event()?.unwrap_or_default();
                    events.reverse();
                    events.extend(p.post_events()?);
                    Ok(p.held_events(events))
                })?
            }
            Tok::Identifier(Id::Scm | Id::Book) => {
                self.next(self.base);
                let value = self.env.value(token.value);
                return Ok((Value::Scheme(token.span), value, false));
            }
            Tok::ScmToken => {
                self.next(self.base);
                let value = self.scheme_value(&token);
                return Ok((Value::Scheme(token.span), value, false));
            }
            Tok::Identifier(Id::Lookup | Id::Module) => {
                let value = self.lookup()?;
                (Value::Scheme(self.span_from(token.span)), value)
            }
            // `string`'s `full_markup`, `partial_markup`, and
            // `full_markup_list`: defined before the next token is lexed, in
            // the block's mode.
            MARKUP | MARKUPLIST => {
                self.next(self.base);
                let value = match token.kind {
                    MARKUP => match self.markup_mode_etc(true)? {
                        FullMarkup::Word(_, value) | FullMarkup::Markup(value) => value,
                        FullMarkup::Partial(commands) => self.partial_markup(commands, token.span),
                    },
                    _ => self.full_markup_list()?,
                };
                return Ok((Value::Scheme(self.span_from(token.span)), value, false));
            }
            HEADER | SCORE | BOOK | BOOKPART | PAPER | LAYOUT | MIDI => {
                let (item, value) = self.block_value()?;
                return Ok((Value::Block(alloc::boxed::Box::new(item)), value, false));
            }
            // `context_modification`, defined before the next token is lexed.
            WITH => {
                let with = self.in_notes(|p| p.context_modification())?;
                let value = self.env.value(with.value);
                return Ok((Value::With(with), value, false));
            }
            // `context_def_spec_block`, defined before the next token is
            // lexed, as a block is; or music.
            CONTEXT => {
                let found = self.in_notes(|p| {
                    let head = p.next(crate::args::N);
                    p.context_def_or_music(head)
                })?;
                match found {
                    DefOrMusic::Def(item, value) => {
                        return Ok((Value::Block(alloc::boxed::Box::new(item)), value, false));
                    }
                    DefOrMusic::Music(music) => self.in_notes(|p| p.held_music(music)),
                }
            }
            Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | Tok::Identifier(Id::Music) => self.in_notes(|p| {
                let music = p.composite_music()?;
                Ok(p.held_music(music))
            })?,
            kind if crate::repeat::starts_basic(kind) => self.in_notes(|p| {
                let music = p.composite_music()?;
                Ok(p.held_music(music))
            })?,
            // `music_assign`'s `context_change`, defined before the next
            // token is lexed.
            CHANGE => {
                let (value, scheme) = self.in_notes(|p| {
                    let start = p.peek(crate::args::N);
                    let kind = p.context_change()?;
                    let music = Music {
                        span: p.span_from(start.span),
                        kind,
                    };
                    Ok(p.held_music(music))
                })?;
                self.env.keep(scheme);
                return Ok((value, scheme, false));
            }
            // `music_assign`'s `music_property_def`: defined before the next
            // token is lexed where its value needed none to end, a `#`
            // form's (`x = \override Stem.color = #red` then `\x`, measured
            // with LilyPond 2.26.0; OPEN F), else after.
            kind if crate::property::starts_property(kind) => {
                let (value, scheme) = self.in_notes(|p| {
                    let start = p.peek(crate::args::N);
                    let kind = p.music_property_def()?;
                    let music = Music {
                        span: p.span_from(start.span),
                        kind,
                    };
                    Ok(p.held_music(music))
                })?;
                self.env.keep(scheme);
                return Ok((value, scheme, self.ahead.is_some()));
            }
            Tok::NotenamePitch
            | Tok::Identifier(Id::Pitch)
            | Tok::RestName
            | Tok::ChordRepetition
            | Tok::MultiMeasureRest
            | Tok::AngleOpen
            | crate::repeat::TEMPO => self.in_notes(|p| match p.music_embedded()? {
                Some(Music {
                    kind: MusicKind::Note(note),
                    ..
                }) if bare_pitch(&note) => {
                    let pitch = p.pitch_value(note.pitch);
                    Ok((Value::Pitch(note.pitch), pitch))
                }
                Some(music) => Ok(p.held_music(music)),
                None => Err(p.syntax_error(&token, bison::IDENTIFIER_INIT)),
            })?,
            _ => return Err(self.syntax_error(&token, bison::IDENTIFIER_INIT)),
        };
        self.env.keep(scheme);
        Ok((value, scheme, true))
    }

    /// `identifier_init` at a token that may start `partial_function`, or
    /// after a `-` that no number follows, `minus`: the value read in full,
    /// as the same tokens' rules read it, or `partial_function ETC`'s
    /// function (`partial.rs`), defined before the token after the `\etc`
    /// is lexed.
    pub(crate) fn partial_value(&mut self, minus: Option<Token>) -> Parsed<(Value, Scm, bool)> {
        // `\override` or `\set` read in full is defined before the next
        // token is lexed where its value needed none to end, a `#` form's
        // (OPEN F, MS8 step 10).
        let property = minus.is_none()
            && matches!(
                self.peek(self.base).kind,
                crate::property::SET | crate::property::OVERRIDE
            );
        let init = self.in_notes(|p| match minus {
            Some(minus) => p.partial_script_init(minus),
            None => p.partial_init(),
        })?;
        let (value, scheme) = match init {
            Init::Music(music) => {
                let (value, scheme) = self.in_notes(|p| p.held_music(music));
                self.env.keep(scheme);
                let peeks = !property || self.ahead.is_some();
                return Ok((value, scheme, peeks));
            }
            Init::Events(mut events) => self.in_notes(|p| {
                // `post_event_cons` puts a `PostEvents` music's events first
                // in it last first.
                events.reverse();
                events.extend(p.post_events()?);
                Ok(p.held_events(events))
            })?,
            Init::Scheme(value, span) => return Ok((Value::Scheme(span), value, false)),
            Init::Partial(value, span) => return Ok((Value::Scheme(span), value, false)),
        };
        self.env.keep(scheme);
        Ok((value, scheme, true))
    }

    /// The value of `\markup name = value` (`identifier_init`), with the
    /// token after it read where Bison reads it.
    pub(crate) fn markup_command_value(&mut self) -> Parsed<Scm> {
        let (_, scheme, peeks) = self.identifier_init()?;
        if peeks {
            self.peek(self.base);
        }
        Ok(scheme)
    }

    /// `post_event_nofinger post_events` after a `-` that no number follows:
    /// the `-` was taken, and the post-event goes on from the token after it.
    pub(crate) fn minus_post_events(&mut self, minus: Token) -> Parsed<(Value, Scm)> {
        let mut events = self.after_direction(minus)?;
        events.reverse();
        events.extend(self.post_events()?);
        Ok(self.held_events(events))
    }

    /// Music as its syntax, and the music it makes (N2), which is defined.
    fn held_music(&mut self, music: Music) -> (Value, Scm) {
        let value = self.music_value(&music);
        (Value::Music(music), value)
    }

    /// Post-events as their syntax, and their music: one event, or
    /// `PostEvents` music of several (`identifier_init`'s
    /// `post_event_nofinger post_events`).
    pub(crate) fn held_events(&mut self, events: Vec<PostEvent>) -> (Value, Scm) {
        let music = match &events[..] {
            [event] => self.event_value(event),
            _ => {
                let span = Span {
                    end: events.last().map_or(0, |e| e.span.end),
                    ..events[0].span
                };
                self.music_value(&Music {
                    span,
                    kind: MusicKind::PostEvents(events.clone()),
                })
            }
        };
        lily_engine_music::freeze(self.env.vm().heap_mut(), music);
        (Value::PostEvents(events), music)
    }

    /// An `UNSIGNED` or a `REAL` as Scheme reads it, negated after a `-`.
    pub(crate) fn number(&mut self, token: &Token, negative: bool) -> Scm {
        let digits = String::from_utf8_lossy(self.lexer.text_of(token.span));
        let text = if negative {
            alloc::format!("-{digits}")
        } else {
            digits.into_owned()
        };
        match parse_number(&text, 10) {
            Ok(Some(n)) => self.env.vm().heap_mut().number(n),
            _ => Scm::FALSE,
        }
    }

    /// A `FRACTION`'s value, `(n . d)` (`scan_fraction`).
    pub(crate) fn fraction(&mut self, token: &Token) -> Scm {
        let text = String::from_utf8_lossy(self.lexer.text_of(token.span)).into_owned();
        let (n, d) = text.split_once('/').unwrap_or((&text, "1"));
        let mut part = |digits: &str| match parse_number(digits, 10) {
            Ok(Some(n)) => self.env.vm().heap_mut().number(n),
            _ => Scm::FALSE,
        };
        let (n, d) = (part(n), part(d));
        self.env.vm().heap_mut().cons(n, d)
    }

    /// `symbol_list_element` or `symbol_list_part_bare` at a word, a string
    /// or an `UNSIGNED` that was just taken. A word is always one symbol:
    /// `try_word_variants` splits only at `.` and `,`, which no word has.
    pub(crate) fn key(&self, token: &Token) -> Key {
        match token.kind {
            Tok::String => Key::Symbol(String::from(self.lexer.string(token))),
            Tok::Unsigned => Key::Index(self.unsigned(token).unwrap_or(u64::MAX)),
            // A word of lyrics mode has its `_` made spaces.
            Tok::Symbol if self.lex_mode(Mode::Notes) == Mode::Lyrics => {
                Key::Symbol(String::from(self.lexer.string(token)))
            }
            _ => Key::Symbol(String::from_utf8_lossy(self.lexer.text_of(token.span)).into_owned()),
        }
    }

    /// `symbol_list_part_bare` of a `SYMBOL`: `try_word_variants` with
    /// `key-list?`, the symbols of the word split at `.` and `,`, which
    /// only a word of lyrics mode has; None (`not a key`) for a word that
    /// is no regular identifier.
    pub(crate) fn word_keys(&self, token: &Token) -> Option<Vec<Key>> {
        let Key::Symbol(text) = self.key(token) else {
            return None;
        };
        if !crate::args::is_regular_identifier(&text, true) {
            return None;
        }
        Some(
            text.split(['.', ','])
                .map(|s| Key::Symbol(String::from(s)))
                .collect(),
        )
    }

    /// `property_path`, `symbol_list_rev` in order: parts separated by `.`
    /// or `,`, each a word, a string, an `UNSIGNED`, or Scheme whose value
    /// is keys, else `not a key` and no key.
    pub(crate) fn property_path(&mut self) -> Parsed<Vec<Key>> {
        let mut keys = Vec::new();
        loop {
            let token = self.peek(self.base);
            match token.kind {
                Tok::Symbol => {
                    self.next(self.base);
                    match self.word_keys(&token) {
                        Some(found) => keys.extend(found),
                        None => self.error(token.loc, "not a key"),
                    }
                }
                Tok::String | Tok::Unsigned => {
                    self.next(self.base);
                    keys.push(self.key(&token));
                }
                Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                    self.next(self.base);
                    let value = if token.kind == Tok::ScmToken {
                        self.scheme_value(&token)
                    } else {
                        self.env.value(token.value)
                    };
                    match scheme_keys(self.env.vm().heap(), value) {
                        Some(found) => keys.extend(found),
                        None => self.error(token.loc, "not a key"),
                    }
                }
                _ => return Err(self.syntax_error(&token, bison::MANY)),
            }
            if !matches!(self.peek(self.base).kind, Tok::Char(b'.' | b',')) {
                return Ok(keys);
            }
            self.next(self.base);
        }
    }

    /// `lookup`, at a `LOOKUP_IDENTIFIER` or a `MODULE_IDENTIFIER`: its
    /// value, or the value its path leads to. A missing key in an alist is
    /// `'()` (`nested_property`'s fallback); in a module, `not found`.
    pub(crate) fn lookup(&mut self) -> Parsed<Scm> {
        let token = self.next(self.base);
        let mut value = self.env.value(token.value);
        if self.peek(self.base).kind != Tok::Char(b'.') {
            return Ok(value);
        }
        self.next(self.base);
        let keys = self.property_path()?;
        let mut rest = &keys[..];
        if token.kind == Tok::Identifier(Id::Module) {
            // Modules until a value that is none, or the path's end.
            while let [key, after @ ..] = rest {
                let found = match key {
                    Key::Symbol(name) => self.env.vm().module_value(value, name),
                    Key::Index(_) => None,
                };
                let Some(found) = found else {
                    let span = Span {
                        start: token.loc.start,
                        ..self.span_from(token.span)
                    };
                    self.error(span, "not found");
                    return Ok(Scm::UNDEFINED);
                };
                value = found;
                rest = after;
                let heap = self.env.vm().heap();
                let module = heap
                    .foreign_object(value)
                    .is_some_and(|f| f.type_name() == "module");
                if !module {
                    break;
                }
            }
        }
        let path: Vec<Scm> = rest.iter().map(|k| self.scm_key(k)).collect();
        let value = nested_property(self.env.vm().heap(), value, &path);
        self.env.keep(value);
        Ok(value)
    }

    pub(crate) fn scm_key(&mut self, key: &Key) -> Scm {
        let heap = self.env.vm().heap_mut();
        match key {
            Key::Symbol(name) => heap.intern(name),
            Key::Index(n) => match parse_number(&alloc::format!("{n}"), 10) {
                Ok(Some(n)) => heap.number(n),
                _ => Scm::FALSE,
            },
        }
    }

    /// A symbol list's value, rooted.
    fn symbol_list(&mut self, keys: &[Key]) -> Scm {
        let items: Vec<Scm> = keys.iter().map(|k| self.scm_key(k)).collect();
        let list = self.env.vm().heap_mut().list(&items);
        self.env.keep(list);
        list
    }

    /// `set_identifier`: `value` defined as `name` in the innermost scope,
    /// or for a path, into the alist `name` holds there, or a new one. A
    /// name that is a keyword is warned about, with no location.
    fn set_identifier(&mut self, name: &str, path: &[Key], value: Scm) {
        if KEYWORDS.contains(&name) {
            let message = alloc::format!("identifier name is a keyword: `{name}'");
            self.diagnose(Severity::Warning, At::Nowhere, message);
        }
        let mut value = value;
        if !path.is_empty() {
            let keys: Vec<Scm> = path.iter().map(|k| self.scm_key(k)).collect();
            let module = self.env.innermost();
            let previous = self.env.vm().module_value(module, name);
            let heap = self.env.vm().heap_mut();
            value = match previous {
                Some(alist) => nested_property_alist(heap, alist, &keys, value),
                None => nested_create_alist(heap, &keys, value),
            };
        }
        self.env.define(name, value);
    }
}

/// `assoc_tail` for keys, which are symbols and indices: the pair of
/// `alist` whose entry's key is `key`. An element that is no pair ends the
/// search, where LilyPond raises a Guile error.
fn assoc_tail(heap: &Heap, key: Scm, alist: Scm) -> Option<Scm> {
    let mut p = alist;
    while p.tag() == Tag::Pair {
        let entry = heap.car(p)?;
        if heap.eqv(heap.car(entry)?, key) {
            return Some(p);
        }
        p = heap.cdr(p)?;
    }
    None
}

/// `nested_create_alist`: `((k1 (k2 … (kn . value))))`.
fn nested_create_alist(heap: &mut Heap, keys: &[Scm], value: Scm) -> Scm {
    keys.iter().rev().fold(value, |inner, &key| {
        let entry = heap.cons(key, inner);
        heap.cons(entry, Scm::NULL)
    })
}

/// `nested_property_alist`: `alist` with `keys` set to `value`, the
/// replaced entry moved to the front, and the rest of `alist` shared.
fn nested_property_alist(heap: &mut Heap, alist: Scm, keys: &[Scm], value: Scm) -> Scm {
    let (&key, rest) = keys.split_first().expect("a path has a key");
    if rest.is_empty() {
        let entry = heap.cons(key, value);
        return heap.cons(entry, alist);
    }
    let Some(at) = assoc_tail(heap, key, alist) else {
        let inner = nested_create_alist(heap, rest, value);
        let entry = heap.cons(key, inner);
        return heap.cons(entry, alist);
    };
    let old = heap.car(at).and_then(|e| heap.cdr(e)).unwrap_or(Scm::NULL);
    let inner = nested_property_alist(heap, old, rest, value);
    let after = heap.cdr(at).unwrap_or(Scm::NULL);
    let tail = partial_list_copy(heap, alist, at, after);
    let entry = heap.cons(key, inner);
    heap.cons(entry, tail)
}

/// `partial_list_copy`: the elements of `list` before `end`, then `tail`.
fn partial_list_copy(heap: &mut Heap, list: Scm, end: Scm, tail: Scm) -> Scm {
    let mut items = Vec::new();
    let mut p = list;
    while p != end && p.tag() == Tag::Pair {
        items.push(heap.car(p).unwrap_or(Scm::NULL));
        p = heap.cdr(p).unwrap_or(Scm::NULL);
    }
    items
        .into_iter()
        .rev()
        .fold(tail, |rest, item| heap.cons(item, rest))
}

/// `nested_property` with `'()` as the fallback: the value at `keys` in
/// the nested alist `alist`.
fn nested_property(heap: &Heap, alist: Scm, keys: &[Scm]) -> Scm {
    let mut value = alist;
    for &key in keys {
        let Some(at) = assoc_tail(heap, key, value) else {
            return Scm::NULL;
        };
        value = heap.car(at).and_then(|e| heap.cdr(e)).unwrap_or(Scm::NULL);
    }
    value
}
