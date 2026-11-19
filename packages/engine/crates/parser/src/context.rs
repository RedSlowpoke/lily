//! Contexts (02 §6's P6, part 1, D71): `parser.yy`'s `context_prefix`,
//! `contexted_basic_music`, `optional_id`, `optional_context_mods`,
//! `context_modification`, `context_mod_list`, `context_mod`,
//! `context_def_spec_block` and `context_change`, with `symbol` and
//! `simple_string`, which they read their names with.
//!
//! `\new` and `\context` make `ContextSpeccedMusic` of the music after them
//! (`context-create`, `context-find-or-create`), built and frozen with the
//! rest of the music where it is handed on (`function.rs`). `\with` makes a
//! context modification (the music crate's `ContextMod`) of its mods, each
//! `(consists "Foo")` and the like, and of the mods of what Scheme or music
//! gives in it; music is made one by `context-mod-music-handler`, which
//! `declarations-init.ly` defines as `lily-library.scm`'s
//! `context-mod-from-music`. The property operations of
//! a `\with` (`name = value`, `\unset`, `\override`, `\revert`) are
//! `property.rs`'s.
//!
//! `\context { … }` makes a context definition (N3, the music crate's
//! `ContextDef`) of its body's mods, added in order to the definition the
//! body begins with, if it does, or to a new one, and placed at the block;
//! an output definition binds it to its name (`output_def.rs`).
//!
//! Where `parser.yy` pushes the notes mode in a mid-rule action (`\with {`,
//! and music or a `\context` in an output definition), [`Parser::pushed`]
//! counts it until the rule pops it, so that a syntax error inside leaves
//! it pushed, and the end of input `Unfinished main input`, as LilyPond's.

use crate::bison;
use crate::classify::Id;
use crate::lexer::{KEYWORDS, Mode, Tok, Token, keyword};
use crate::syntax::{ContextMod, ContextMusic, Item, ItemKind, Music, MusicKind, With};
use crate::{LYRICMODE, NOTEMODE, Parsed, Parser, Span};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_music::{
    ContextDef, ContextMod as ContextModValue, Music as MusicValue, context_mods, make_context_def,
    make_context_mod,
};
use lily_engine_scheme::{Tag, Value};

pub(crate) const NEWCONTEXT: Tok = Tok::Keyword(keyword("new"));
pub(crate) const CONTEXT: Tok = Tok::Keyword(keyword("context"));
pub(crate) const WITH: Tok = Tok::Keyword(keyword("with"));
pub(crate) const CHANGE: Tok = Tok::Keyword(keyword("change"));
const N: Mode = Mode::Notes;

/// `context_def_mod`: the symbol of a mod's keyword.
fn def_mod(kind: Tok) -> Option<&'static str> {
    let Tok::Keyword(k) = kind else {
        return None;
    };
    Some(match KEYWORDS[k as usize] {
        "consists" => "consists",
        "remove" => "remove",
        "accepts" => "accepts",
        "defaultchild" => "default-child",
        "denies" => "denies",
        "alias" => "alias",
        "type" => "translator-type",
        "description" => "description",
        "name" => "context-name",
        _ => return None,
    })
}

/// Whether `kind` starts `context_mod_arg`'s `composite_music`.
fn starts_composite(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | CONTEXT
            | Tok::Identifier(Id::Music | Id::MusicFunction)
    ) || crate::repeat::starts_basic(kind)
}

/// `contextable_music`: `basic_music`, `pitch_as_music` or `event_chord`,
/// and `contexted_basic_music` again.
fn starts_contextable(kind: Tok) -> bool {
    starts_composite(kind)
        || matches!(
            kind,
            Tok::NotenamePitch
                | Tok::TonicnamePitch
                | Tok::DrumPitch
                | Tok::FigureOpen
                | Tok::Identifier(Id::Pitch)
                | Tok::RestName
                | Tok::ChordRepetition
                | Tok::MultiMeasureRest
                | Tok::AngleOpen
                | crate::repeat::TEMPO
        )
}

/// What the items of `\with { … }` or `\context { … }` gave so far.
#[derive(Default)]
struct Mods {
    /// The mods, in order.
    list: Vec<Value>,
    /// A context definition the body began with, which it modifies.
    def: Option<Value>,
    /// Whether `context_def_spec_body` has a definition yet: a mod or a
    /// value was read.
    started: bool,
}

impl Parser<'_, '_> {
    /// `contexted_basic_music` after its `\new` or `\context`, `head`, was
    /// taken: `context_prefix` (`symbol optional_id optional_context_mods`)
    /// and `contextable_music`, and `new_lyrics` after them (P8).
    pub(crate) fn contexted_music(&mut self, head: Token) -> Parsed<Music> {
        let context_type = self.symbol()?;
        let id = match self.peek(N).kind {
            Tok::Char(b'=') => {
                self.next(N);
                Some(self.simple_string()?)
            }
            _ => None,
        };
        let (with, ops) = self.optional_context_mods()?;
        let token = self.peek(N);
        if !starts_contextable(token.kind) {
            return Err(self.syntax_error(&token, bison::MANY));
        }
        // `basic_music` leaves an `\addlyrics` after it to the context's
        // music, a `contexted_basic_music` takes it itself.
        let music = match token.kind {
            NEWCONTEXT | CONTEXT => self.music_embedded()?,
            kind if starts_composite(kind) => Some(self.basic_composite()?),
            _ => self.music_embedded()?,
        };
        let Some(music) = music else {
            return Err(self.syntax_error(&token, bison::MANY));
        };
        let context = ContextMusic {
            new: head.kind == NEWCONTEXT,
            context_type,
            id,
            with,
            ops,
            music,
        };
        let music = Music {
            span: self.span_from(head.span),
            kind: MusicKind::Context(alloc::boxed::Box::new(context)),
        };
        self.new_lyrics(music)
    }

    /// `optional_context_mods`: the `\with`s there, and the index of their
    /// mods appended.
    pub(crate) fn optional_context_mods(&mut self) -> Parsed<(Vec<With>, u32)> {
        let mut with = Vec::new();
        let mut ops = Vec::new();
        while self.peek(N).kind == WITH {
            let w = self.context_modification()?;
            let value = self.env.value(w.value);
            let heap = self.env.vm().heap_mut();
            let mut p = context_mods(heap, value).unwrap_or(Value::NULL);
            while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                ops.push(item);
                p = rest;
            }
            with.push(w);
        }
        Ok((with, self.list_kept(&ops)))
    }

    /// A list of `items`, kept, and its index.
    pub(crate) fn list_kept(&mut self, items: &[Value]) -> u32 {
        let list = self.env.vm().heap_mut().list(items);
        self.env.keep(list)
    }

    /// A string value of `text`, kept, and its index.
    fn string_kept(&mut self, text: &str) -> u32 {
        let string = self.env.vm().heap_mut().string(text);
        self.env.keep(string)
    }

    /// A word's or a string's text.
    fn text(&self, token: &Token) -> String {
        match token.kind {
            Tok::String => String::from(self.lexer.string(token)),
            _ => String::from_utf8_lossy(self.lexer.text_of(token.span)).into_owned(),
        }
    }

    /// A `SCM_TOKEN`'s or a `SCM_IDENTIFIER`'s value, just taken
    /// (`embedded_scm_bare`), and its place.
    fn bare_scm(&mut self, token: &Token) -> (Value, Span) {
        let value = match token.kind {
            Tok::ScmToken => self.scheme_value(token),
            _ => self.env.value(token.value),
        };
        let span = Span {
            start: token.loc.start,
            ..self.span_from(token.span)
        };
        (value, span)
    }

    /// `symbol`: a string's symbol, a word's (`symbol expected` if it is no
    /// regular identifier), or a Scheme value's as `try_string_variants`
    /// takes it with `symbol?`, else `symbol expected` and an uninterned
    /// `undefined`. Its index.
    pub(crate) fn symbol(&mut self) -> Parsed<u32> {
        let (token, value) = self.string_or_symbol()?;
        Ok(self.symbol_of(&token, value))
    }

    /// The token that begins a `symbol` or a `simple_string`, read: a
    /// string, a word, or Scheme with its value (`embedded_scm_bare`,
    /// evaluated as Bison reduces it, before its lookahead).
    pub(crate) fn string_or_symbol(&mut self) -> Parsed<(Token, Option<(Value, Span)>)> {
        let token = self.peek(N);
        match token.kind {
            Tok::String | Tok::Symbol => {
                self.next(N);
                Ok((token, None))
            }
            Tok::ScmToken | Tok::Identifier(Id::Scm) => {
                self.next(N);
                let value = self.bare_scm(&token);
                Ok((token, Some(value)))
            }
            _ => Err(self.syntax_error(&token, bison::SYMBOL)),
        }
    }

    /// `symbol` of what [`Parser::string_or_symbol`] read. Its index.
    pub(crate) fn symbol_of(&mut self, token: &Token, value: Option<(Value, Span)>) -> u32 {
        let symbol = match value {
            None => {
                let text = self.text(token);
                if token.kind == Tok::Symbol && !crate::args::is_regular_identifier(&text, false) {
                    self.error(token.loc, "symbol expected");
                }
                self.env.vm().heap_mut().intern(&text)
            }
            Some((value, span)) => self.scheme_symbol(value, span),
        };
        self.env.keep(symbol)
    }

    /// `symbol`'s `embedded_scm_bare`: `value` as `try_string_variants`
    /// takes it with `symbol?`, else `symbol expected` at `span` and an
    /// uninterned `undefined`.
    fn scheme_symbol(&mut self, value: Value, span: Span) -> Value {
        let found = match self.procedure("symbol?") {
            Some(pred) => self.string_variants(pred, value),
            None => None,
        };
        match found {
            Some(symbol) => symbol,
            None => {
                self.error(span, "symbol expected");
                self.env.vm().heap_mut().uninterned("undefined")
            }
        }
    }

    /// `simple_string`: a string's or a word's text, or a Scheme value that
    /// is a string, else `simple string expected` and `""`. Its index.
    pub(crate) fn simple_string(&mut self) -> Parsed<u32> {
        let (token, value) = self.string_or_symbol()?;
        Ok(self.simple_string_of(&token, value))
    }

    /// `simple_string` of what [`Parser::string_or_symbol`] read. Its
    /// index.
    pub(crate) fn simple_string_of(&mut self, token: &Token, value: Option<(Value, Span)>) -> u32 {
        match value {
            None => {
                let text = self.text(token);
                self.string_kept(&text)
            }
            Some((value, _)) if value.tag() == Tag::String => self.env.keep(value),
            Some((_, span)) => {
                self.error(span, "simple string expected");
                self.string_kept("")
            }
        }
    }

    /// `context_change`, at `\change`: `\change Type = id`.
    pub(crate) fn context_change(&mut self) -> Parsed<MusicKind> {
        self.next(N);
        let context_type = self.symbol()?;
        self.expect(Tok::Char(b'='), N, bison::EQUALS)?;
        let id = self.simple_string()?;
        Ok(MusicKind::Change { context_type, id })
    }

    /// `context_modification`, at `\with`: `{ context_mod_list }`, its
    /// braces read in the notes mode its mid-rule action pushes, or
    /// `context_modification_arg`, a Scheme value or a `MUSIC_IDENTIFIER`.
    /// Music is made a context modification by the handler; anything else
    /// that is none is `not a context mod`, and an empty one, but for the
    /// unspecified value.
    pub(crate) fn context_modification(&mut self) -> Parsed<With> {
        let head = self.next(N);
        let token = self.peek(N);
        let (mods, value) = match token.kind {
            Tok::ScmToken
            | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                let (value, span, _) = self.embedded_scm_value()?;
                (Vec::new(), self.context_mod_arg(value, span))
            }
            Tok::Identifier(Id::Music) => {
                self.next(N);
                let value = self.env.value(token.value);
                (Vec::new(), self.context_mod_arg(value, token.loc))
            }
            _ => {
                // Any other token reduces the mid-rule action first.
                self.pushed += 1;
                self.modes.push(N);
                self.relabel(N);
                self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
                let mut mods = Mods::default();
                let items = self.mod_items(&mut mods, false)?;
                self.pushed -= 1;
                self.modes.pop();
                let value = make_context_mod(self.env.vm().heap_mut(), Value::NULL);
                for m in mods.list {
                    lily_engine_music::add_context_mod(self.env.vm().heap_mut(), value, m);
                }
                (items, value)
            }
        };
        Ok(With {
            span: self.span_from(head.span),
            mods,
            value: self.env.keep(value),
        })
    }

    /// `WITH context_modification_arg`'s action: `value`, a context
    /// modification, music made one, or an empty one.
    fn context_mod_arg(&mut self, value: Value, span: Span) -> Value {
        let value = self.mod_of_music(value);
        let heap = self.env.vm().heap_mut();
        if heap.foreign::<ContextModValue>(value).is_some() {
            return value;
        }
        if value != Value::UNSPECIFIED {
            self.error(span, "not a context mod");
        }
        let empty = make_context_mod(self.env.vm().heap_mut(), Value::NULL);
        self.env.keep(empty);
        empty
    }

    /// `context-mod-music-handler` of `value` if it is music, else `value`.
    fn mod_of_music(&mut self, value: Value) -> Value {
        if self.env.vm().heap().foreign::<MusicValue>(value).is_none() {
            return value;
        }
        self.procedure("context-mod-music-handler")
            .and_then(|handler| self.call_scheme(handler, &[value]))
            .unwrap_or(Value::UNSPECIFIED)
    }

    /// The items of `context_mod_list`, or of `context_def_spec_body` for
    /// a `def`, after `{` and to `}`: `context_mod`s, `\with`s in a
    /// definition, and `context_mod_arg`s, each Scheme value or music.
    fn mod_items(&mut self, mods: &mut Mods, def: bool) -> Parsed<Vec<ContextMod>> {
        let mut items = Vec::new();
        loop {
            let token = self.peek(N);
            let item = match token.kind {
                Tok::Char(b'}') => {
                    self.next(N);
                    return Ok(items);
                }
                kind if def_mod(kind).is_some() => {
                    self.next(N);
                    let name = def_mod(kind).unwrap_or_default();
                    let m = self.context_mod(&token, name)?;
                    mods.list.push(m);
                    mods.started = true;
                    ContextMod::Mod {
                        span: self.span_from(token.span),
                        name,
                    }
                }
                crate::property::UNSET | crate::property::OVERRIDE | crate::property::REVERT => {
                    if let Some(m) = self.property_operation()? {
                        mods.list.push(m);
                    }
                    mods.started = true;
                    ContextMod::Property(self.span_from(token.span))
                }
                // `property_operation`'s `symbol '=' scalar`.
                Tok::String | Tok::Symbol => {
                    let symbol = self.symbol()?;
                    let symbol = self.env.value(symbol);
                    let m = self.assign_mod(symbol)?;
                    mods.list.push(m);
                    mods.started = true;
                    ContextMod::Property(self.span_from(token.span))
                }
                WITH if def => {
                    let w = self.context_modification()?;
                    let value = self.env.value(w.value);
                    self.add_mods_of(mods, value);
                    mods.started = true;
                    ContextMod::With(w)
                }
                Tok::ScmToken
                | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    let (value, span, _) = self.embedded_scm_value()?;
                    let bare = matches!(token.kind, Tok::ScmToken | Tok::Identifier(Id::Scm));
                    if bare && self.peek(N).kind == Tok::Char(b'=') {
                        // `symbol '=' scalar`, its symbol `embedded_scm_bare`.
                        let symbol = self.scheme_symbol(value, span);
                        let m = self.assign_mod(symbol)?;
                        mods.list.push(m);
                        mods.started = true;
                        ContextMod::Property(self.span_from(token.span))
                    } else {
                        self.mod_value(mods, def, value, span);
                        ContextMod::Value(span)
                    }
                }
                kind if starts_composite(kind) => {
                    // `context_mod_arg`'s mid-rule action pushes the notes
                    // mode, which its rule pops.
                    self.pushed += 1;
                    self.modes.push(N);
                    self.relabel(N);
                    let music = self.composite_music()?;
                    self.pushed -= 1;
                    self.modes.pop();
                    let value = self.music_value(&music);
                    self.mod_value(mods, def, value, music.span);
                    ContextMod::Value(music.span)
                }
                _ => return Err(self.syntax_error(&token, bison::MANY)),
            };
            items.push(item);
        }
    }

    /// `context_mod`'s `context_def_mod` and its argument, after the
    /// keyword `head`: `(name "text")` for a string or a word, `(name
    /// value)` for Scheme, which only `\consists` and `\remove` take a
    /// non-string of; else the error, at the keyword, and `'()`.
    fn context_mod(&mut self, head: &Token, name: &'static str) -> Parsed<Value> {
        let token = self.peek(N);
        let value = match token.kind {
            Tok::String | Tok::Symbol => {
                self.next(N);
                let text = self.text(&token);
                self.env.vm().heap_mut().string(text.as_str())
            }
            Tok::ScmToken
            | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                let (value, _, _) = self.embedded_scm_value()?;
                if value.tag() != Tag::String && name != "consists" && name != "remove" {
                    self.error(
                        head.loc,
                        "only \\consists and \\remove take non-string argument.",
                    );
                    return Ok(Value::NULL);
                }
                value
            }
            _ => return Err(self.syntax_error(&token, bison::MANY)),
        };
        let heap = self.env.vm().heap_mut();
        let symbol = heap.intern(name);
        let m = heap.list(&[symbol, value]);
        self.env.keep(m);
        Ok(m)
    }

    /// A `context_mod_arg`'s action: nothing for the unspecified value; a
    /// context definition that a definition's body begins with becomes its
    /// definition; music is made a context modification, whose mods are
    /// added; anything else is `not a context mod`.
    fn mod_value(&mut self, mods: &mut Mods, def: bool, value: Value, span: Span) {
        if value == Value::UNSPECIFIED {
            return;
        }
        let is_def = self.env.vm().heap().foreign::<ContextDef>(value).is_some();
        if def && !mods.started && is_def {
            mods.def = Some(value);
            mods.started = true;
            return;
        }
        mods.started = true;
        let value = self.mod_of_music(value);
        if !self.add_mods_of(mods, value) && (def || value != Value::UNSPECIFIED) {
            self.error(span, "not a context mod");
        }
    }

    /// The mods of `value`, if it is a context modification, added to
    /// `mods`; whether it was one.
    fn add_mods_of(&mut self, mods: &mut Mods, value: Value) -> bool {
        let heap = self.env.vm().heap_mut();
        let Some(mut p) = context_mods(heap, value) else {
            return false;
        };
        while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            mods.list.push(item);
            p = rest;
        }
        true
    }

    /// `context_def_spec_block` after `\context`, `head`, at its `{`: a
    /// context definition of its body's mods, added to the one it began
    /// with or to a new one, placed at the block (`set_spot`). A mod of no
    /// known tag is LilyPond's programming error. Its syntax and its value.
    pub(crate) fn context_def(&mut self, head: Token) -> Parsed<(Item, Value)> {
        self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
        let mut mods = Mods::default();
        let items = self.mod_items(&mut mods, true)?;
        let span = self.span_from(head.span);
        let origin = self.origin(span);
        let heap = self.env.vm().heap_mut();
        let (def, unknown) = make_context_def(heap, mods.def, &mods.list, origin);
        self.env.keep(def);
        for _ in 0..unknown {
            self.lexer.diags.raw(String::from(
                "programming error: unknown context mod tag\ncontinuing, cross fingers\n",
            ));
        }
        let item = Item {
            span,
            kind: ItemKind::ContextDef(items),
        };
        Ok((item, def))
    }

    /// `\context` just taken where a definition or music may follow
    /// (`music_or_context_def`, a value, an argument): a definition at `{`,
    /// else contexted music. The definition's syntax and value, or the
    /// music's.
    pub(crate) fn context_def_or_music(&mut self, head: Token) -> Parsed<DefOrMusic> {
        if self.peek(N).kind == Tok::Char(b'{') {
            let (item, value) = self.context_def(head)?;
            return Ok(DefOrMusic::Def(item, value));
        }
        self.contexted_music(head).map(DefOrMusic::Music)
    }
}

/// What `\context` began: a definition or music.
pub(crate) enum DefOrMusic {
    Def(Item, Value),
    Music(Music),
}
