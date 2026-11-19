//! Property operations (02 §6's P6, part 2, D72): `parser.yy`'s
//! `music_property_def`, `property_operation`, `context_prop_spec`,
//! `grob_prop_spec`, `grob_prop_path`, `simple_revert_context`,
//! `revert_arg` and `scalar`, with the `symbol_list_part` and
//! `symbol_list_rev` they read their paths with.
//!
//! In music, `\set`, `\unset`, `\override` and `\revert` make
//! `PropertySet`, `PropertyUnset`, `OverrideProperty` and `RevertProperty`
//! in `ContextSpeccedMusic` (`property-set` and the others), built and
//! frozen with the rest of the music where it is handed on
//! (`function.rs`). A path whose context is left out begins with a grob:
//! a symbol with the object property `is-grob?`, which `define-grobs.scm`
//! gives each grob's name; its context is then `Bottom`. `property-set`
//! and `property-override` are syntax functions, which check their
//! arguments' types as a music function's call does.
//!
//! In `\with { … }` and `\context { … }`, `name = value`, `\unset`,
//! `\override` and `\revert` are context mods (`property_operation`):
//! `(assign name value)`, `(unset name)`, `(push grob value path…)` and
//! `(pop grob path…)`.
//!
//! `\revert`'s path, and `\override`'s, may be split after the grob with
//! no `.` (`\revert Stem #'color`), which is `deprecated: missing `.' in
//! property path`. `simple_revert_context` pushes back what it read after
//! the context as an `SCM_IDENTIFIER` ([`crate::lexer::Lexer::push_extra`]),
//! which the token trace shows as LilyPond's does. The `BACKUP` and
//! `SCM_ARG` or `SYMBOL_LIST` tokens that `revert_arg` pushes are not
//! lexed, and not here: the loop of [`Parser::revert_arg`] stands for them.

use crate::bison;
use crate::classify::Id;
use crate::context::{CHANGE, CONTEXT, DefOrMusic, NEWCONTEXT, WITH};
use crate::lexer::{Mode, Tok, keyword};
use crate::markup::FullMarkup;
use crate::scheme::scheme_keys;
use crate::syntax::{Key, MusicKind, Property, PropertyOp};
use crate::{LYRICMODE, MARKUP, MARKUPLIST, NOTEMODE, Parsed, Parser, Span};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::Value;

pub(crate) const SET: Tok = Tok::Keyword(keyword("set"));
pub(crate) const UNSET: Tok = Tok::Keyword(keyword("unset"));
pub(crate) const OVERRIDE: Tok = Tok::Keyword(keyword("override"));
pub(crate) const REVERT: Tok = Tok::Keyword(keyword("revert"));
const N: Mode = Mode::Notes;

/// Whether `kind` starts `music_property_def`.
pub(crate) fn starts_property(kind: Tok) -> bool {
    matches!(kind, SET | UNSET | OVERRIDE | REVERT)
}

/// Whether `kind` starts `symbol_list_part`.
fn starts_part(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Symbol | Tok::String | Tok::Unsigned | Tok::ScmToken | Tok::Identifier(Id::Scm)
    )
}

/// Whether `kind` starts `scalar`'s `music_assign` or `pitch_or_music`.
fn starts_music(kind: Tok) -> bool {
    starts_property(kind)
        || matches!(
            kind,
            Tok::Char(b'{')
                | Tok::DoubleAngleOpen
                | NOTEMODE
                | LYRICMODE
                | NEWCONTEXT
                | CHANGE
                | Tok::AngleOpen
                | Tok::NotenamePitch
                | Tok::RestName
                | Tok::ChordRepetition
                | Tok::MultiMeasureRest
                | Tok::Identifier(Id::Music | Id::MusicFunction | Id::Pitch)
        )
}

/// A path as `property_path_dot_warning` writes it: its keys joined by `.`.
fn dotted(keys: &[Key]) -> String {
    let mut out = String::new();
    for (i, key) in keys.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        match key {
            Key::Symbol(name) => out.push_str(name),
            Key::Index(n) => out.push_str(&alloc::format!("{n}")),
        }
    }
    out
}

impl Parser<'_, '_> {
    /// `music_property_def`, at its keyword.
    pub(crate) fn music_property_def(&mut self) -> Parsed<MusicKind> {
        match self.property_def_etc(false)? {
            Ok(kind) => Ok(kind),
            Err(_) => unreachable!("a property operation is cut short only with `etc`"),
        }
    }

    /// `music_property_def`, or with `etc`, `\set` and `\override` cut
    /// short after `=` (`partial_function`): their call,
    /// `(property-set property context)` or `(property-override path
    /// context)`, `(#f)` for a wrong path, and those of a
    /// `partial_function` after it.
    pub(crate) fn property_def_etc(&mut self, etc: bool) -> Parsed<Result<MusicKind, Vec<Value>>> {
        let head = self.next(N);
        let (op, target, value) = match head.kind {
            SET | OVERRIDE => {
                let (op, target, name) = match head.kind {
                    SET => (PropertyOp::Set, self.context_prop_spec()?, "property-set"),
                    _ => (
                        PropertyOp::Override,
                        self.grob_prop_path()?,
                        "property-override",
                    ),
                };
                self.expect(Tok::Char(b'='), N, bison::EQUALS)?;
                let call = match target {
                    Some((context, second)) => {
                        let function = self.constructor(name).unwrap_or(Value::FALSE);
                        let list = self.env.vm().heap_mut().list(&[function, second, context]);
                        self.env.keep(list);
                        list
                    }
                    None => Value::FALSE,
                };
                let value = match self.partial_after(etc, call)? {
                    None => self.scalar()?,
                    Some(Err(calls)) => return Ok(Err(calls)),
                    Some(Ok(music)) => {
                        let value = self.music_value(&music);
                        self.env.keep(value);
                        value
                    }
                };
                (op, target, Some(value))
            }
            UNSET => (PropertyOp::Unset, self.context_prop_spec()?, None),
            _ => {
                let context = self.simple_revert_context()?;
                let (path, _) = self.revert_arg()?;
                let path = self.symbols(&path);
                (PropertyOp::Revert, Some((context, path)), None)
            }
        };
        // The syntax functions' signatures: `(symbol? symbol? scheme?)` and
        // `(symbol? symbol-list? scheme?)`, checked at the whole call.
        let span = self.span_from(head.span);
        let second = match op {
            PropertyOp::Set => Some("symbol?"),
            PropertyOp::Override => Some("symbol-list?"),
            _ => None,
        };
        let target = match (target, second) {
            (Some((context, path)), Some(second)) => {
                let checks = [("symbol?", context), (second, path)];
                self.signature_holds(span, &checks)
                    .then_some((context, path))
            }
            (target, _) => target,
        };
        let target = target.map(|(context, path)| (self.env.keep(context), self.env.keep(path)));
        let value = value.map(|v| self.env.keep(v));
        Ok(Ok(MusicKind::Property(Property { op, target, value })))
    }

    /// Whether each of `args` holds of its predicate, else `wrong type for
    /// argument` at `at` for the first that does not (`Music_function::call`).
    fn signature_holds(&mut self, at: Span, args: &[(&str, Value)]) -> bool {
        for (n, &(name, value)) in args.iter().enumerate() {
            let Some(pred) = self.procedure(name) else {
                continue;
            };
            if !self.holds(pred, value) {
                self.argument_error(at, n + 1, pred, value);
                return false;
            }
        }
        true
    }

    /// `property_operation` in `\with { … }` or `\context { … }`, at
    /// `\unset`, `\override` or `\revert`: its mod, None when the path was
    /// wrong.
    pub(crate) fn property_operation(&mut self) -> Parsed<Option<Value>> {
        let head = self.next(N);
        let heap_list = |p: &mut Self, items: &[Value]| {
            let list = p.env.vm().heap_mut().list(items);
            p.env.keep(list);
            list
        };
        match head.kind {
            UNSET => {
                let symbol = self.symbol()?;
                let symbol = self.env.value(symbol);
                let unset = self.env.vm().heap_mut().intern("unset");
                Ok(Some(heap_list(self, &[unset, symbol])))
            }
            OVERRIDE => {
                let (path, at) = self.revert_arg()?;
                self.expect(Tok::Char(b'='), N, bison::EQUALS)?;
                let value = self.scalar()?;
                if path.len() < 2 {
                    self.error(at, "bad grob property path");
                    return Ok(None);
                }
                let mut items = Vec::with_capacity(path.len() + 2);
                items.push(self.env.vm().heap_mut().intern("push"));
                items.push(self.scm_key(&path[0]));
                items.push(value);
                for key in &path[1..] {
                    items.push(self.scm_key(key));
                }
                Ok(Some(heap_list(self, &items)))
            }
            _ => {
                let (path, _) = self.revert_arg()?;
                let mut items = Vec::with_capacity(path.len() + 1);
                items.push(self.env.vm().heap_mut().intern("pop"));
                for key in &path {
                    items.push(self.scm_key(key));
                }
                Ok(Some(heap_list(self, &items)))
            }
        }
    }

    /// `property_operation`'s `symbol '=' scalar` after its symbol:
    /// `(assign symbol value)`.
    pub(crate) fn assign_mod(&mut self, symbol: Value) -> Parsed<Value> {
        self.env.keep(symbol);
        self.expect(Tok::Char(b'='), N, bison::EQUALS)?;
        let value = self.scalar()?;
        let heap = self.env.vm().heap_mut();
        let assign = heap.intern("assign");
        let list = heap.list(&[assign, symbol, value]);
        self.env.keep(list);
        Ok(list)
    }

    /// The list of `keys`' symbols and indices, kept.
    fn symbols(&mut self, keys: &[Key]) -> Value {
        let items: Vec<Value> = keys.iter().map(|k| self.scm_key(k)).collect();
        let list = self.env.vm().heap_mut().list(&items);
        self.env.keep(list);
        list
    }

    /// Whether `symbol` names a grob: its object property `is-grob?`.
    fn is_grob(&mut self, symbol: Value) -> bool {
        let Some(property) = self.procedure("object-property") else {
            return false;
        };
        let key = self.env.vm().heap_mut().intern("is-grob?");
        self.call_scheme(property, &[symbol, key])
            .is_some_and(|v| v != Value::FALSE)
    }

    /// `Bottom`, the context of a path that leaves it out.
    fn bottom(&mut self) -> Value {
        self.env.vm().heap_mut().intern("Bottom")
    }

    /// `symbol_list_part`, read with no token after it: a word's symbol, a
    /// string's, an `UNSIGNED`'s index, or the keys of Scheme's value (`not
    /// a key` and none for a value that has none); and its place.
    fn symbol_list_part(&mut self) -> Parsed<(Vec<Key>, Span)> {
        let token = self.peek(N);
        if !starts_part(token.kind) {
            return Err(self.syntax_error(&token, bison::MANY));
        }
        self.next(N);
        let span = Span {
            start: token.loc.start,
            ..self.span_from(token.span)
        };
        let keys = match token.kind {
            Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                let value = match token.kind {
                    Tok::ScmToken => self.scheme_value(&token),
                    _ => self.env.value(token.value),
                };
                match scheme_keys(self.env.vm().heap(), value) {
                    Some(keys) => keys,
                    None => {
                        self.error(span, "not a key");
                        Vec::new()
                    }
                }
            }
            Tok::Symbol => match self.word_keys(&token) {
                Some(keys) => keys,
                None => {
                    self.error(span, "not a key");
                    Vec::new()
                }
            },
            _ => alloc::vec![self.key(&token)],
        };
        Ok((keys, span))
    }

    /// `symbol_list_rev`, in order: parts separated by `.` or `,`; and its
    /// place.
    fn symbol_list_rev(&mut self) -> Parsed<(Vec<Key>, Span)> {
        let first = self.peek(N);
        let keys = self.property_path()?;
        let span = Span {
            start: first.loc.start,
            ..self.span_from(first.span)
        };
        Ok((keys, span))
    }

    /// `context_prop_spec`: a property, in `Bottom`, or a context and a
    /// property; else `bad context property path` and None.
    fn context_prop_spec(&mut self) -> Parsed<Option<(Value, Value)>> {
        let (keys, at) = self.symbol_list_rev()?;
        Ok(match &keys[..] {
            [property] => {
                let context = self.bottom();
                Some((context, self.scm_key(property)))
            }
            [context, property] => Some((self.scm_key(context), self.scm_key(property))),
            _ => {
                self.error(at, "bad context property path");
                None
            }
        })
    }

    /// `grob_prop_path`: a context, a grob and its property's path, the
    /// context `Bottom` if the path begins with a grob; or a context and
    /// a grob, and the path after them with no `.` between
    /// (`grob_prop_spec property_path`, deprecated). Else `bad grob
    /// property path` and None. The context and the rest, a list.
    fn grob_prop_path(&mut self) -> Parsed<Option<(Value, Value)>> {
        let (spec, at) = self.symbol_list_rev()?;
        let mut path: Vec<Value> = spec.iter().map(|k| self.scm_key(k)).collect();
        if let Some(&first) = path.first()
            && self.is_grob(first)
        {
            path.insert(0, self.bottom());
        }
        if starts_part(self.peek(N).kind) {
            let (rest, rest_at) = self.symbol_list_rev()?;
            if path.len() != 2 || rest.is_empty() {
                self.error(at, "bad grob property path");
                return Ok(None);
            }
            let mut whole = spec;
            whole.extend(rest.iter().cloned());
            self.dot_warning(rest_at, &whole);
            path.extend(rest.iter().map(|k| self.scm_key(k)));
        } else if path.len() < 3 {
            self.error(at, "bad grob property path");
            return Ok(None);
        }
        let list = self.env.vm().heap_mut().list(&path[1..]);
        self.env.keep(list);
        Ok(Some((path[0], list)))
    }

    /// `property_path_dot_warning` at `at`.
    fn dot_warning(&mut self, at: Span, path: &[Key]) {
        if path.is_empty() {
            return;
        }
        let message = alloc::format!("deprecated: missing `.' in property path {}", dotted(path));
        self.diagnose(
            crate::diag::Severity::Warning,
            crate::diag::At::Span(at),
            message,
        );
    }

    /// `simple_revert_context`: the first part of `\revert`'s path, read
    /// with no token after it. Its context is `Bottom` if it is empty or
    /// begins with a grob, and the part is pushed back; else its first
    /// key, and the rest is pushed back. Pushed back as an `SCM_IDENTIFIER`
    /// of their list, which `revert_arg` begins with.
    fn simple_revert_context(&mut self) -> Parsed<Value> {
        let (keys, _) = self.symbol_list_part()?;
        let values: Vec<Value> = keys.iter().map(|k| self.scm_key(k)).collect();
        let (context, rest) = match values.first() {
            Some(&first) if !self.is_grob(first) => (first, &values[1..]),
            _ => (self.bottom(), &values[..]),
        };
        let list = self.env.vm().heap_mut().list(rest);
        let index = self.env.keep(list);
        self.lexer.push_extra(Tok::Identifier(Id::Scm), index);
        Ok(context)
    }

    /// `revert_arg`: parts until there are two keys, each after `.`, `,`
    /// or nothing (deprecated), then more after `.` or `,`
    /// (`symbol_list_arg`). Its keys and its place.
    fn revert_arg(&mut self) -> Parsed<(Vec<Key>, Span)> {
        let first = self.peek(N);
        let (mut keys, _) = self.symbol_list_part()?;
        // `revert_arg_backup`'s `SCM_ARG`, while there are fewer than two.
        while keys.len() < 2 {
            let token = self.peek(N);
            match token.kind {
                Tok::Char(b'.' | b',') => {
                    self.next(N);
                    keys.extend(self.symbol_list_part()?.0);
                }
                kind if starts_part(kind) => {
                    let (more, at) = self.symbol_list_part()?;
                    keys.extend(more);
                    self.dot_warning(at, &keys);
                }
                _ => return Err(self.syntax_error(&token, bison::MANY)),
            }
        }
        // `SYMBOL_LIST`, and `symbol_list_arg`'s parts after it.
        if matches!(self.peek(N).kind, Tok::Char(b'.' | b',')) {
            self.next(N);
            keys.extend(self.property_path()?);
        }
        let span = Span {
            start: first.loc.start,
            ..self.span_from(first.span)
        };
        Ok((keys, span))
    }

    /// `scalar`: Scheme, music, a pitch, a number with its unit, a negative
    /// one, a string or a word, or a symbol list (`a.b`). Its value, kept.
    pub(crate) fn scalar(&mut self) -> Parsed<Value> {
        let token = self.peek(N);
        let value = match token.kind {
            Tok::ScmToken
            | Tok::ScmArg
            | Tok::Fraction
            | Tok::Identifier(Id::Lookup | Id::Module | Id::ScmFunction) => self.scheme_arg()?,
            Tok::Identifier(Id::Scm) => {
                self.next(N);
                self.env.value(token.value)
            }
            WITH => {
                let with = self.context_modification()?;
                self.env.value(with.value)
            }
            CONTEXT => {
                let head = self.next(N);
                match self.context_def_or_music(head)? {
                    DefOrMusic::Def(_, value) => value,
                    DefOrMusic::Music(music) => self.music_value(&music),
                }
            }
            kind if starts_music(kind) => match self.music_embedded()? {
                Some(crate::syntax::Music {
                    kind: MusicKind::Note(note),
                    ..
                }) if crate::assign::bare_pitch(&note) => self.pitch_value(note.pitch),
                Some(music) => self.music_value(&music),
                None => Value::UNSPECIFIED,
            },
            // `symbol_list_part_bare`'s `UNSIGNED` before `.` or `,`: a
            // number list `2,2,3` (P12), else `bare_number`.
            Tok::Unsigned => {
                self.next(N);
                if matches!(self.peek(N).kind, Tok::Char(b'.' | b',')) {
                    self.next(N);
                    let mut keys = alloc::vec![self.key(&token)];
                    keys.extend(self.property_path()?);
                    self.symbols(&keys)
                } else {
                    let value = self.number_value(&token);
                    self.bare_number(value)
                }
            }
            Tok::Real | Tok::Identifier(Id::Number) => self.scalar_number()?,
            Tok::Char(b'-') => {
                self.next(N);
                let number = self.peek(N);
                if !matches!(
                    number.kind,
                    Tok::Unsigned | Tok::Real | Tok::Identifier(Id::Number)
                ) {
                    return Err(self.syntax_error(&number, bison::BARE_NUMBER));
                }
                let value = self.scalar_number()?;
                self.env.vm().heap_mut().negate(value).unwrap_or(value)
            }
            // `string`'s `full_markup`, and `embedded_scm_arg`'s
            // `partial_markup` and `full_markup_list`.
            MARKUP => {
                self.next(N);
                match self.markup_mode_etc(true)? {
                    FullMarkup::Word(_, value) | FullMarkup::Markup(value) => value,
                    FullMarkup::Partial(commands) => self.partial_markup(commands, token.span),
                }
            }
            MARKUPLIST => {
                self.next(N);
                self.full_markup_list()?
            }
            Tok::String | Tok::Symbol => {
                self.next(N);
                let first = self.key(&token);
                if matches!(self.peek(N).kind, Tok::Char(b'.' | b',')) {
                    // `symbol_list_part_bare '.' property_path`.
                    self.next(N);
                    let mut keys = alloc::vec![first];
                    keys.extend(self.property_path()?);
                    self.symbols(&keys)
                } else {
                    let text = match first {
                        Key::Symbol(text) => text,
                        Key::Index(_) => String::new(),
                    };
                    self.env.vm().heap_mut().string(text.as_str())
                }
            }
            _ => return Err(self.syntax_error(&token, bison::ARGUMENT)),
        };
        self.env.keep(value);
        Ok(value)
    }

    /// `bare_number`, at an `UNSIGNED`, a `REAL` or a `NUMBER_IDENTIFIER`.
    fn scalar_number(&mut self) -> Parsed<Value> {
        let token = self.next(N);
        let value = self.number_value(&token);
        Ok(match token.kind {
            Tok::Identifier(_) => value,
            _ => self.bare_number(value),
        })
    }
}
