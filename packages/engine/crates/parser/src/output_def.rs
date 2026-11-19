//! Output blocks (02 §6's P5, part 2): `parser.yy`'s `header_block`,
//! `header_modification`, `score_block`, `book_block`, `bookpart_block`
//! and `output_def`, as the nested scopes `add_scope` and `remove_scope`
//! make of them, with the placement rules of their actions: what each block
//! takes, and the errors for what it does not (`\paper cannot be used in
//! \score`, `need \paper for paper block`, `Spurious expression in \score`,
//! `Missing music in \score`, `bad expression type`).
//!
//! An output definition is a copy of the one `get_paper`, `get_layout` or
//! `get_midi` (`lily-parser.cc`) finds: the top of a book's `$papers`, or
//! `$defaultpaper`, `$defaultlayout` or `$defaultmidi`, which a toplevel
//! output definition replaces. Its body is lexed in initial mode
//! (`push_initial_state`), its scope holds what it assigns, and an output
//! definition as its first item replaces it. A header is a module: a
//! toplevel one is a copy of `$defaultheader`, which it then replaces; a
//! book's, a part's and a score's are theirs; a header as a value is a new
//! module. Scores and books are values of their own here ([`Score`],
//! [`Book`]) with no more in them than these rules read, and output
//! definitions the music crate's (`OutputDef`, N3); the handlers LilyPond
//! hands scores and books to (`toplevel-score-handler`,
//! `book-music-handler`, …) are not called. In an output definition, a
//! context definition is bound to its name (`assign_context_def`), and
//! music is handed to `output-def-music-handler`, which modifies the
//! definitions it names.
//!
//! The defaults are those the init files define (`paper-defaults-init.ly`,
//! `declarations-init.ly`'s `\layout`, `midi-init.ly`); without them, an
//! output definition is a new one.
//! Each block recovers from a syntax error where its `error` production does:
//! an output definition's body and a book's or a part's at their next item
//! or `}`, a score's at `}`.

use crate::bison;
use crate::classify::Id;
use crate::context::{CONTEXT, DefOrMusic, NEWCONTEXT};
use crate::diag::{At, Severity};
use crate::lexer::{Mode, Tok, Token};
use crate::notes::starts_plain_music;
use crate::recover::{InScore, ScoreBody, Stepped, error_found};
use crate::syntax::{Assignment, Item, ItemKind, OutputKind};
use crate::{
    BOOK, BOOKPART, Env, HEADER, LAYOUT, LYRICMODE, MARKUP, MARKUPLIST, MIDI, NOTEMODE, PAPER,
    Parsed, Parser, SCORE, SyntaxError,
};
use alloc::vec::Vec;
use lily_engine_music::{Book, ContextDef, OutputDef, Score};
use lily_engine_scheme::{Foreign, Heap, Value};

const N: Mode = Mode::Notes;

/// The variable of each kind's default.
fn default_name(kind: OutputKind) -> &'static str {
    match kind {
        OutputKind::Paper => "$defaultpaper",
        OutputKind::Layout => "$defaultlayout",
        OutputKind::Midi => "$defaultmidi",
    }
}

/// `set_variable` of `output-def-kind` in an output definition's scope.
fn set_kind(env: &mut Env<'_>, scope: Value, kind: OutputKind) {
    let vm = env.vm();
    let symbol = vm.heap_mut().intern(kind.name());
    vm.module_define(scope, "output-def-kind", symbol);
}

/// Whether `value` is a module.
fn is_module(heap: &Heap, value: Value) -> bool {
    heap.foreign_object(value)
        .is_some_and(|f| f.type_name() == "module")
}

/// What `value` is to the blocks' actions.
enum Kind {
    OutputDef(Option<OutputKind>),
    ContextDef,
    Score,
    Module,
    Unspecified,
    Other,
}

impl Parser<'_, '_> {
    fn kind_of(&mut self, value: Value) -> Kind {
        if value == Value::UNSPECIFIED {
            return Kind::Unspecified;
        }
        let heap = self.env.vm().heap();
        if let Some(def) = heap.foreign::<OutputDef>(value) {
            let scope = def.scope;
            let vm = self.env.vm();
            let kind = vm.module_value(scope, "output-def-kind");
            let kind = kind.and_then(|k| vm.heap().name(k)).and_then(|name| {
                [OutputKind::Paper, OutputKind::Layout, OutputKind::Midi]
                    .into_iter()
                    .find(|k| k.name() == name)
            });
            return Kind::OutputDef(kind);
        }
        if heap.foreign::<ContextDef>(value).is_some() {
            Kind::ContextDef
        } else if heap.foreign::<Score>(value).is_some() {
            Kind::Score
        } else if is_module(heap, value) {
            Kind::Module
        } else {
            Kind::Other
        }
    }

    /// Wraps `object`, rooted.
    fn wrap<T: Foreign>(&mut self, object: T) -> Value {
        let value = self.env.vm().heap_mut().wrap(object);
        self.env.keep(value);
        value
    }

    /// A new module, rooted (`ly_make_module`).
    fn new_module(&mut self) -> Value {
        let module = self.env.make_module();
        self.env.keep(module);
        module
    }

    /// `get_header`: a copy of `$defaultheader` if it is a module, else a
    /// new module.
    fn get_header(&mut self) -> Value {
        let module = self.new_module();
        if let Some(header) = self.env.lookup("$defaultheader")
            && is_module(self.env.vm().heap(), header)
        {
            self.env.vm().module_copy(module, header);
        }
        module
    }

    /// `get_paper`, `get_layout` or `get_midi`: a copy of the top of
    /// `$papers` for a paper, if there is one, else of the kind's default,
    /// else a new output definition; of the kind it is copied for.
    fn get_output_def(&mut self, kind: OutputKind) -> Value {
        let mut from = None;
        if kind == OutputKind::Paper
            && let Some(papers) = self.env.lookup("$papers")
        {
            from = self.env.vm().heap().car(papers);
        }
        let from = from
            .filter(|&def| self.env.vm().heap().foreign::<OutputDef>(def).is_some())
            .or_else(|| self.env.lookup(default_name(kind)));
        let scope = self.new_module();
        if let Some(def) = from
            && let Some(old) = self
                .env
                .vm()
                .heap()
                .foreign::<OutputDef>(def)
                .map(|d| d.scope)
        {
            self.env.vm().module_copy(scope, old);
        }
        set_kind(&mut self.env, scope, kind);
        self.wrap(OutputDef::new(scope))
    }

    /// An output definition's scope.
    fn scope_of(&self, def: Value) -> Value {
        self.env
            .vm_ref()
            .heap()
            .foreign::<OutputDef>(def)
            .map_or(Value::FALSE, |d| d.scope)
    }

    /// `set_identifier` of a variable of LilyPond's own, in the innermost
    /// scope.
    fn set_variable(&mut self, name: &str, value: Value) {
        self.env.define(name, value);
    }

    /// A toplevel `\book`'s book, added to the file's `%toplevel-books`
    /// ([`crate::Parse::toplevel_books`]).
    pub(crate) fn collect_book(&mut self, book: Value) {
        let books = self.env.lookup("%toplevel-books").unwrap_or(Value::NULL);
        let books = match books.tag() {
            lily_engine_scheme::Tag::Pair => books,
            _ => Value::NULL,
        };
        let list = self.env.vm().heap_mut().cons(book, books);
        self.set_variable("%toplevel-books", list);
    }

    /// A toplevel `BOOK_IDENTIFIER`'s book: a book with a paper collected
    /// as a `\book`'s, else a part handed to `toplevel-bookpart-handler`.
    pub(crate) fn toplevel_book_identifier(&mut self, book: Value) {
        let heap = self.env.vm_ref().heap();
        let paper = heap.foreign::<Book>(book).map_or(Value::FALSE, |b| b.paper);
        if heap.foreign::<OutputDef>(paper).is_some() {
            self.collect_book(book);
        } else {
            self.hand("toplevel-bookpart-handler", None, book);
        }
    }

    /// `\paper`, `\layout` or `\midi` and the token's `kind`, just taken: an
    /// output definition, its scope added and its body lexed in initial
    /// mode until `}`; its syntax and its value. After a syntax error that
    /// its body does not recover from, the scope stays and the mode counts
    /// as pushed, as LilyPond leaves them, but the next tokens are lexed in
    /// the outer block's mode, the lookahead as it was lexed.
    pub(crate) fn output_def(&mut self, head: Token, kind: OutputKind) -> Parsed<(Item, Value)> {
        let mut def = self.get_output_def(kind);
        let scope = self.scope_of(def);
        self.env.add_scope(scope);
        self.pushed += 1;
        let outer = self.base;
        let result = self.output_def_body(&mut def);
        self.base = outer;
        if result.is_err() {
            self.relabel(outer);
        }
        let items = result?;
        self.env.remove_scope();
        self.pushed -= 1;
        let item = Item {
            span: self.span_from(head.span),
            kind: ItemKind::OutputDef(kind, items),
        };
        Ok((item, def))
    }

    /// `output_def_body '}'`: its items until `}`, and `output_def_body
    /// error`, which discards tokens until an item or `}`.
    fn output_def_body(&mut self, def: &mut Value) -> Parsed<Vec<Item>> {
        const I: Mode = Mode::Initial;
        self.base = I;
        self.expect(Tok::Char(b'{'), I, bison::OPEN_BRACE)?;
        let mut items = Vec::new();
        loop {
            let token = self.peek(I);
            let first = items.is_empty();
            let item = match token.kind {
                Tok::Char(b'}') => {
                    self.next(I);
                    return Ok(items);
                }
                Tok::String | Tok::Symbol => self.assignment().map(ItemKind::Assignment),
                Tok::ScmToken => {
                    self.next(I);
                    self.scheme_value(&token);
                    Ok(ItemKind::Scheme(token.span))
                }
                Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    self.embedded_scm_active().map(|(value, span)| {
                        match self.kind_of(value) {
                            // An output definition right after `{` replaces this one.
                            Kind::OutputDef(_) if first => {
                                *def = value;
                                self.env.remove_scope();
                                let scope = self.scope_of(value);
                                self.env.add_scope(scope);
                            }
                            Kind::ContextDef => self.assign_context_def(*def, value),
                            Kind::Unspecified => {}
                            _ if self.scheme_music(value, span, false).is_some() => {
                                self.output_def_music(*def, value);
                            }
                            _ => self.error(span, "bad expression type"),
                        }
                        ItemKind::Scheme(span)
                    })
                }
                // `music_or_context_def`, after the token that reduces the
                // mid-rule action that pushes notes mode, which its rule
                // pops: a context definition is bound to its name, music
                // handed to `output-def-music-handler`.
                kind if matches!(
                    kind,
                    Tok::Char(b'{')
                        | Tok::DoubleAngleOpen
                        | NOTEMODE
                        | LYRICMODE
                        | NEWCONTEXT
                        | CONTEXT
                        | crate::repeat::TEMPO
                        | Tok::Identifier(Id::Music | Id::MusicFunction)
                ) || crate::repeat::starts_basic(kind) =>
                {
                    self.pushed += 1;
                    let item = self.in_notes(|p| p.music_or_context_def(*def));
                    if item.is_ok() {
                        self.pushed -= 1;
                    }
                    item
                }
                // `music_assign`'s `music_property_def`.
                kind if crate::property::starts_property(kind) => {
                    self.pushed += 1;
                    let item = self.in_notes(|p| {
                        let start = p.peek(N);
                        let kind = p.music_property_def()?;
                        let span = p.span_from(start.span);
                        let music = crate::syntax::Music { span, kind };
                        let value = p.music_value(&music);
                        p.output_def_music(*def, value);
                        Ok(ItemKind::Music(music))
                    });
                    if item.is_ok() {
                        self.pushed -= 1;
                    }
                    item
                }
                _ => Err(self.syntax_error(&token, bison::MANY)),
            };
            match item {
                Ok(kind) => items.push(Item {
                    span: self.span_from(token.span),
                    kind,
                }),
                Err(SyntaxError) => {
                    if !self.resync(I, |k| k == Tok::Char(b'}') || starts_output_item(k)) {
                        return Err(SyntaxError);
                    }
                }
            }
        }
    }

    /// `music_or_context_def` in the output definition `def`: `\context {
    /// … }`, bound to its name, or music, handed to the handler.
    fn music_or_context_def(&mut self, def: Value) -> Parsed<ItemKind> {
        let token = self.peek(N);
        let music = if token.kind == crate::repeat::TEMPO {
            // `music_assign`'s `event_chord`.
            match self.music_embedded()? {
                Some(music) => music,
                None => return Err(self.syntax_error(&token, bison::MANY)),
            }
        } else if token.kind != CONTEXT {
            self.composite_music()?
        } else {
            let head = self.next(N);
            match self.context_def_or_music(head)? {
                DefOrMusic::Def(item, value) => {
                    self.assign_context_def(def, value);
                    return Ok(item.kind);
                }
                DefOrMusic::Music(music) => music,
            }
        };
        let value = self.music_value(&music);
        self.output_def_music(def, value);
        Ok(ItemKind::Music(music))
    }

    /// `assign_context_def`: the context definition `value` bound to its
    /// name in the output definition `def`'s scope.
    fn assign_context_def(&mut self, def: Value, value: Value) {
        let scope = self.scope_of(def);
        let heap = self.env.vm().heap();
        let Some(name) = heap.foreign::<ContextDef>(value).map(|d| d.name()) else {
            return;
        };
        let Some(name) = heap.name(name).map(alloc::string::String::from) else {
            return;
        };
        self.env.vm().module_define(scope, &name, value);
    }

    /// `output-def-music-handler` called with the output definition `def`
    /// and the music `value`.
    fn output_def_music(&mut self, def: Value, value: Value) {
        if let Some(handler) = self.procedure("output-def-music-handler") {
            self.call_scheme(handler, &[def, value]);
        }
    }

    /// `embedded_scm_active`: a `SCM_IDENTIFIER`, a lookup or a Scheme
    /// function's call, in the block's mode; the call's arguments in notes
    /// mode. Its value and span.
    fn embedded_scm_active(&mut self) -> Parsed<(Value, crate::Span)> {
        let start = self.peek(self.base);
        let value = match start.kind {
            Tok::Identifier(Id::Scm) => {
                self.next(self.base);
                self.env.value(start.value)
            }
            Tok::Identifier(Id::ScmFunction) => self.in_notes(|p| p.function_call())?.0,
            _ => self.lookup()?,
        };
        let span = crate::Span {
            start: start.loc.start,
            ..self.span_from(start.span)
        };
        Ok((value, span))
    }

    /// `lilypond_header` after `\header`, with `module` added as the
    /// innermost scope while its body is read; the body's fields.
    pub(crate) fn header_block(&mut self, module: Value) -> Parsed<Vec<Assignment>> {
        self.env.add_scope(module);
        let fields = self.header_body()?;
        self.env.remove_scope();
        Ok(fields)
    }

    /// `\header { … }`'s body after `\header`: assignments, `#` forms,
    /// evaluated and ignored, and `embedded_scm_active`: a module, whose
    /// bindings are copied into the header, or unspecified, or else `bad
    /// expression type`. A syntax error ends it, as `lilypond_header_body`
    /// has no `error` rule.
    fn header_body(&mut self) -> Parsed<Vec<Assignment>> {
        self.expect(Tok::Char(b'{'), self.base, bison::OPEN_BRACE)?;
        let mut fields = Vec::new();
        loop {
            let token = self.peek(self.base);
            match token.kind {
                Tok::Char(b'}') => {
                    self.next(self.base);
                    return Ok(fields);
                }
                Tok::String | Tok::Symbol => fields.push(self.assignment()?),
                Tok::ScmToken => {
                    self.next(self.base);
                    self.scheme_value(&token);
                }
                Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    let (value, span) = self.embedded_scm_active()?;
                    match self.kind_of(value) {
                        Kind::Module => {
                            let header = self.env.innermost();
                            self.env.vm().module_copy(header, value);
                        }
                        Kind::Unspecified => {}
                        _ => self.error(span, "bad expression type"),
                    }
                }
                _ => return Err(self.syntax_error(&token, bison::MANY)),
            }
        }
    }

    /// A toplevel `\header` after the keyword: a copy of `$defaultheader`
    /// that becomes `$defaultheader` (`header_block`).
    pub(crate) fn toplevel_header(&mut self) -> Parsed<Vec<Assignment>> {
        let module = self.get_header();
        let fields = self.header_block(module)?;
        self.set_variable("$defaultheader", module);
        Ok(fields)
    }

    /// What `embedded_scm_active` at toplevel does with `value` beyond
    /// text: an output definition becomes its kind's default, a module is
    /// copied into a copy of `$defaultheader` that replaces it; a score is
    /// for `toplevel-score-handler`. Whether it was any of these.
    pub(crate) fn toplevel_value(&mut self, value: Value) -> bool {
        match self.kind_of(value) {
            Kind::OutputDef(kind) => {
                if let Some(kind) = kind {
                    self.set_variable(default_name(kind), value);
                }
                true
            }
            Kind::Module => {
                let module = self.get_header();
                self.env.vm().module_copy(module, value);
                self.set_variable("$defaultheader", module);
                true
            }
            Kind::Score => {
                self.hand("toplevel-score-handler", None, value);
                true
            }
            Kind::ContextDef | Kind::Unspecified | Kind::Other => false,
        }
    }

    /// A toplevel output definition after its keyword: it becomes its
    /// kind's default.
    pub(crate) fn toplevel_output_def(
        &mut self,
        head: Token,
        kind: OutputKind,
    ) -> Parsed<ItemKind> {
        let (item, def) = self.output_def(head, kind)?;
        self.toplevel_value(def);
        Ok(item.kind)
    }

    /// A block as `identifier_init` or an argument takes it, at its keyword:
    /// a header (`header_modification`, a new module), a score, a book, a
    /// part or an output definition; its syntax and its value. A score's,
    /// a book's or a part's body is read in notes mode.
    pub(crate) fn block_value(&mut self) -> Parsed<(Item, Value)> {
        let head = self.next(self.base);
        let kind = match head.kind {
            PAPER => return self.output_def(head, OutputKind::Paper),
            LAYOUT => return self.output_def(head, OutputKind::Layout),
            MIDI => return self.output_def(head, OutputKind::Midi),
            HEADER => {
                let module = self.new_module();
                let fields = self.header_block(module)?;
                let item = Item {
                    span: self.span_from(head.span),
                    kind: ItemKind::Header(fields),
                };
                return Ok((item, module));
            }
            kind => kind,
        };
        self.in_notes(|p| match kind {
            SCORE => p.score_block(head),
            BOOK => p.book_block(head),
            _ => p.bookpart_block(head),
        })
    }

    /// `\score { … }` after `\score`: `score_items`, as its action places
    /// them. The first music, or a score as Scheme, is the score's, and any
    /// later music or value is `Spurious expression in \score`; a `\paper`
    /// cannot be in a score; headers are the score's header, a module given
    /// as Scheme is copied into it. Then `score_body error`, which discards
    /// tokens until `}`. An error inside an item recovers where the score
    /// is: `score_body error` has no state until the items are reduced.
    pub(crate) fn score_block(&mut self, head: Token) -> Parsed<(Item, Value)> {
        let toplevel = core::mem::take(&mut self.score_in_steps);
        self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
        self.score_body(ScoreBody {
            head,
            items: Vec::new(),
            score: None,
            header: Value::FALSE,
            defs: Vec::new(),
            toplevel,
        })
    }

    /// [`Parser::score_block`]'s items from where `body` stands: after its
    /// `{`, or after its music where the parse stopped in `scorify-music`
    /// (D272).
    pub(crate) fn score_body(&mut self, body: ScoreBody) -> Parsed<(Item, Value)> {
        let ScoreBody {
            head,
            mut items,
            mut score,
            mut header,
            mut defs,
            toplevel,
        } = body;
        loop {
            let token = self.peek(N);
            let kind = match token.kind {
                PAPER | LAYOUT | MIDI => {
                    self.next(N);
                    let kind = match token.kind {
                        PAPER => OutputKind::Paper,
                        LAYOUT => OutputKind::Layout,
                        _ => OutputKind::Midi,
                    };
                    let (item, def) = self.output_def(token, kind)?;
                    if self.score_output_def(def, item.span) {
                        defs.push(def);
                    }
                    items.push(item);
                    continue;
                }
                HEADER => {
                    self.next(N);
                    let module = match score {
                        Some(score) => self.score_header(score),
                        None => {
                            if header == Value::FALSE {
                                header = self.new_module();
                            }
                            header
                        }
                    };
                    let fields = self.header_block(module)?;
                    ItemKind::Header(fields)
                }
                Tok::ScmToken
                | Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    let (value, span, placed) = self.embedded_scm_value()?;
                    match self.kind_of(value) {
                        Kind::OutputDef(_) => {
                            if self.score_output_def(value, span) {
                                defs.push(value);
                            }
                        }
                        Kind::Score if score.is_none() => {
                            if header != Value::FALSE
                                && let Some(s) =
                                    self.env.vm().heap_mut().foreign_mut::<Score>(value)
                            {
                                s.header = header;
                            }
                            score = Some(value);
                        }
                        Kind::Module => {
                            let module = match score {
                                Some(score) => self.score_header(score),
                                None => {
                                    if header == Value::FALSE {
                                        header = self.new_module();
                                    }
                                    header
                                }
                            };
                            self.env.vm().module_copy(module, value);
                        }
                        Kind::Unspecified => {}
                        _ => match self.scheme_music(value, span, placed) {
                            Some(music) if score.is_none() => {
                                score = Some(self.scorify(&music, header));
                            }
                            _ => self.error(span, "Spurious expression in \\score"),
                        },
                    }
                    ItemKind::Scheme(span)
                }
                kind if starts_plain_music(kind) => {
                    let Some(music) = self.music_embedded()? else {
                        continue;
                    };
                    // `score_items score_item` is reduced with the next token lexed.
                    self.peek(N);
                    // A toplevel score's first item, in a compile that returns:
                    // `scorify-music` in steps of its own (B23, D272).
                    let first = toplevel && items.is_empty() && header == Value::FALSE;
                    if score.is_none() && first && defs.is_empty() && !error_found(&music) {
                        match self.scorify_in_steps(&music) {
                            Some(Stepped::Done(value)) => {
                                score = Some(self.scorified(value, header))
                            }
                            Some(Stepped::Paused(at)) => {
                                self.in_score = Some(InScore {
                                    body: ScoreBody {
                                        head,
                                        items,
                                        score,
                                        header,
                                        defs,
                                        toplevel,
                                    },
                                    token,
                                    music,
                                    at,
                                });
                                return Err(SyntaxError);
                            }
                            None => score = Some(self.scorify(&music, header)),
                        }
                    } else if score.is_none() {
                        score = Some(self.scorify(&music, header));
                    } else {
                        self.error(music.span, "Spurious expression in \\score");
                    }
                    ItemKind::Music(music)
                }
                _ => {
                    // Any other token reduces `score_items` to `score_body` first.
                    if score.is_none() {
                        self.diagnose(Severity::Error, At::Nowhere, "Missing music in \\score");
                        score = Some(self.wrap(Score::new(header)));
                    }
                    if token.kind != Tok::Char(b'}') {
                        self.syntax_error(&token, bison::SCORE_BODY);
                        if !self.resync(N, |k| k == Tok::Char(b'}')) {
                            return Err(SyntaxError);
                        }
                    }
                    self.next(N);
                    if let Some(s) =
                        score.and_then(|s| self.env.vm().heap_mut().foreign_mut::<Score>(s))
                    {
                        s.defs.extend(defs);
                    }
                    let item = Item {
                        span: self.span_from(head.span),
                        kind: ItemKind::Score(items),
                    };
                    return Ok((item, score.unwrap_or(Value::FALSE)));
                }
            };
            items.push(Item {
                span: self.span_from(token.span),
                kind,
            });
        }
    }

    /// A score's output definition, at `span`: a `\paper` is an error,
    /// and the score is not given it. Kept while the score is parsed.
    fn score_output_def(&mut self, def: Value, span: crate::Span) -> bool {
        if matches!(self.kind_of(def), Kind::OutputDef(Some(OutputKind::Paper))) {
            self.error(
                span,
                "\\paper cannot be used in \\score, use \\layout instead",
            );
            return false;
        }
        self.env.keep(def);
        true
    }

    /// A score's header, made a new module if it has none.
    fn score_header(&mut self, score: Value) -> Value {
        let header = self
            .env
            .vm()
            .heap()
            .foreign::<Score>(score)
            .map_or(Value::FALSE, |s| s.header);
        if is_module(self.env.vm().heap(), header) {
            return header;
        }
        let module = self.new_module();
        if let Some(s) = self.env.vm().heap_mut().foreign_mut::<Score>(score) {
            s.header = module;
        }
        module
    }

    /// `\book { … }` after `\book`: a book with a copy of `$defaultpaper`
    /// on a new `$papers` and a copy of `$defaultheader`, or the book of a
    /// `BOOK_IDENTIFIER` that comes first; its items until `}`.
    pub(crate) fn book_block(&mut self, head: Token) -> Parsed<(Item, Value)> {
        self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
        let book = if let Tok::Identifier(Id::Book) = self.peek(N).kind {
            let token = self.next(N);
            self.env.value(token.value)
        } else {
            // `init_papers`, then `push_paper` of the book's.
            self.set_variable("$papers", Value::NULL);
            let paper = self.get_output_def(OutputKind::Paper);
            let header = self.get_header();
            let book = self.wrap(Book::new(paper, header));
            let papers = self.env.vm().heap_mut().cons(paper, Value::NULL);
            self.env.keep(papers);
            self.set_variable("$papers", papers);
            book
        };
        self.set_variable("$current-book", book);
        let items = self.book_body(book, true)?;
        // `pop_paper`.
        if let Some(papers) = self.env.lookup("$papers")
            && let Some(rest) = self.env.vm().heap().cdr(papers)
        {
            self.set_variable("$papers", rest);
        }
        self.set_variable("$current-book", Value::FALSE);
        let item = Item {
            span: self.span_from(head.span),
            kind: ItemKind::Book(items),
        };
        Ok((item, book))
    }

    /// `\bookpart { … }` after `\bookpart`: a part with no paper and no
    /// header, or the book of a `BOOK_IDENTIFIER` that comes first; its
    /// items until `}`.
    pub(crate) fn bookpart_block(&mut self, head: Token) -> Parsed<(Item, Value)> {
        self.expect(Tok::Char(b'{'), N, bison::OPEN_BRACE)?;
        let book = if let Tok::Identifier(Id::Book) = self.peek(N).kind {
            let token = self.next(N);
            self.env.value(token.value)
        } else {
            self.wrap(Book::new(Value::FALSE, Value::FALSE))
        };
        self.set_variable("$current-bookpart", book);
        let items = self.book_body(book, false)?;
        self.set_variable("$current-bookpart", Value::FALSE);
        let item = Item {
            span: self.span_from(head.span),
            kind: ItemKind::BookPart(items),
        };
        Ok((item, book))
    }

    /// `book_body` (with `whole`) or `bookpart_body` after `{`: output
    /// definitions, of which only a `\paper` is the book's, its parts (a
    /// book's only), scores, music, markup, Scheme and headers, until `}`;
    /// and their `error` rule, which discards tokens until an item or `}`.
    fn book_body(&mut self, book: Value, whole: bool) -> Parsed<Vec<Item>> {
        let mut items = Vec::new();
        loop {
            let token = self.peek(N);
            let item = match token.kind {
                Tok::Char(b'}') => {
                    self.next(N);
                    return Ok(items);
                }
                PAPER | LAYOUT | MIDI => {
                    self.next(N);
                    let kind = match token.kind {
                        PAPER => OutputKind::Paper,
                        LAYOUT => OutputKind::Layout,
                        _ => OutputKind::Midi,
                    };
                    self.output_def(token, kind).map(|(item, def)| {
                        self.book_paper(book, def, item.span, whole);
                        item.kind
                    })
                }
                BOOKPART if whole => {
                    self.next(N);
                    self.bookpart_block(token).map(|(item, part)| {
                        self.hand("book-bookpart-handler", Some(book), part);
                        item.kind
                    })
                }
                SCORE => {
                    self.next(N);
                    self.score_block(token).map(|(item, score)| {
                        self.hand(book_handler(whole, "score"), Some(book), score);
                        item.kind
                    })
                }
                MARKUP => {
                    self.next(N);
                    self.full_markup().map(|markup| {
                        let text = self.one_text(markup);
                        self.hand(book_handler(whole, "text"), Some(book), text);
                        ItemKind::Markup
                    })
                }
                MARKUPLIST => {
                    self.next(N);
                    self.full_markup_list().map(|list| {
                        self.hand(book_handler(whole, "text"), Some(book), list);
                        ItemKind::Markup
                    })
                }
                HEADER => {
                    self.next(N);
                    let header = self.book_header(book);
                    self.header_block(header).map(ItemKind::Header)
                }
                Tok::ScmToken => {
                    self.next(N);
                    self.scheme_value(&token);
                    Ok(ItemKind::Scheme(token.span))
                }
                Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    self.embedded_scm_active().map(|(value, span)| {
                        if let Some(text) = self.text_of(value) {
                            self.hand(book_handler(whole, "text"), Some(book), text);
                            return ItemKind::Scheme(span);
                        }
                        match self.kind_of(value) {
                            Kind::OutputDef(_) => self.book_paper(book, value, span, whole),
                            Kind::Module => {
                                let header = self.book_header(book);
                                self.env.vm().module_copy(header, value);
                            }
                            Kind::Score => {
                                self.hand(book_handler(whole, "score"), Some(book), value)
                            }
                            Kind::Unspecified => {}
                            Kind::ContextDef | Kind::Other => {
                                self.error(span, "bad expression type")
                            }
                        }
                        ItemKind::Scheme(span)
                    })
                }
                kind if matches!(
                    kind,
                    Tok::Char(b'{')
                        | Tok::DoubleAngleOpen
                        | NOTEMODE
                        | LYRICMODE
                        | NEWCONTEXT
                        | CONTEXT
                        | Tok::Identifier(Id::Music | Id::MusicFunction)
                ) || crate::repeat::starts_basic(kind) =>
                {
                    self.composite_music().map(|music| {
                        // `book_body composite_music` is reduced with the next token lexed.
                        self.peek(N);
                        self.hand_music(book_handler(whole, "music"), Some(book), &music);
                        ItemKind::Music(music)
                    })
                }
                _ => Err(self.syntax_error(&token, bison::MANY)),
            };
            match item {
                Ok(kind) => items.push(Item {
                    span: self.span_from(token.span),
                    kind,
                }),
                Err(SyntaxError) => {
                    let follows =
                        |k| k == Tok::Char(b'}') || starts_book_item(k) && (whole || k != BOOKPART);
                    if !self.resync(N, follows) {
                        return Err(SyntaxError);
                    }
                }
            }
        }
    }

    /// `paper_block` in a book, at `span`: a `\paper` becomes the book's,
    /// and for a whole book the top of `$papers` (`set_paper`); any other
    /// kind is `need \paper for paper block`.
    fn book_paper(&mut self, book: Value, def: Value, span: crate::Span, whole: bool) {
        if !matches!(self.kind_of(def), Kind::OutputDef(Some(OutputKind::Paper))) {
            self.error(span, "need \\paper for paper block");
            return;
        }
        if let Some(b) = self.env.vm().heap_mut().foreign_mut::<Book>(book) {
            b.paper = def;
        }
        if whole && let Some(papers) = self.env.lookup("$papers") {
            self.env.vm().heap_mut().set_car(papers, def);
        }
    }

    /// A book's header, made a new module if it has none.
    fn book_header(&mut self, book: Value) -> Value {
        let header = self
            .env
            .vm()
            .heap()
            .foreign::<Book>(book)
            .map_or(Value::FALSE, |b| b.header);
        if is_module(self.env.vm().heap(), header) {
            return header;
        }
        let module = self.new_module();
        if let Some(b) = self.env.vm().heap_mut().foreign_mut::<Book>(book) {
            b.header = module;
        }
        module
    }

    /// Parses with `f` in notes mode from the lookahead on, in a block
    /// lexed in another mode, and goes back to it. The lookahead keeps the
    /// kind it was lexed with, as Bison's does when the lexer's state
    /// changes after it was read.
    pub(crate) fn in_notes<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let base = self.base;
        if base == N {
            return f(self);
        }
        self.modes.push(N);
        self.relabel(N);
        self.base = N;
        let result = f(self);
        self.base = base;
        self.modes.pop();
        self.relabel(base);
        result
    }

    /// Gives the lookahead the mode a token asked for in `mode` is lexed in
    /// now, as after a mid-rule action that pushes a mode: the lookahead
    /// was lexed before it, and lexes the same in both.
    pub(crate) fn relabel(&mut self, mode: Mode) {
        if let Some((token, _)) = self.ahead {
            self.ahead = Some((token, self.lex_mode(mode)));
        }
    }
}

/// The handler of a book's (`whole`) or a part's `what`, music, text or
/// score.
fn book_handler(whole: bool, what: &str) -> &'static str {
    match (whole, what) {
        (true, "music") => "book-music-handler",
        (true, "text") => "book-text-handler",
        (true, _) => "book-score-handler",
        (false, "music") => "bookpart-music-handler",
        (false, "text") => "bookpart-text-handler",
        (false, _) => "bookpart-score-handler",
    }
}

/// Whether `kind` starts an item of an output definition's body: what can
/// follow `output_def_body error` in P5's grammar.
fn starts_output_item(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::String
            | Tok::Symbol
            | Tok::ScmToken
            | Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | CONTEXT
            | Tok::Identifier(
                Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction | Id::Music | Id::MusicFunction
            )
            | crate::repeat::TEMPO
    ) || crate::property::starts_property(kind)
        || crate::repeat::starts_basic(kind)
}

/// Whether `kind` starts an item of a book's body: what can follow
/// `book_body error` in P5's grammar.
fn starts_book_item(kind: Tok) -> bool {
    matches!(
        kind,
        PAPER
            | LAYOUT
            | MIDI
            | BOOKPART
            | SCORE
            | MARKUP
            | MARKUPLIST
            | HEADER
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | CONTEXT
            | Tok::ScmToken
            | Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | Tok::Identifier(
                Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction | Id::Music | Id::MusicFunction
            )
    ) || crate::repeat::starts_basic(kind)
}
