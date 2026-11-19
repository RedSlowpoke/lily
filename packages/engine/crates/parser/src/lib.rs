//! The engine's LilyPond parser (docs/research/wasm-engine/02-parser.md §5),
//! built in the order of 02 §6's syntax list, P1 to P12.
//!
//! P1 so far: [`Source`]s with their line tables and [`Span`]s into them,
//! with line, character and column counted as `source-file.cc` counts them
//! (tabs to multiples of 8, UTF-8 characters once); the [`Lexer`]'s shared
//! rules (comments, strings and escapes, `\version`, the byte order mark and
//! the UTF-8 checks); [`Diagnostics`] in LilyPond's message and excerpt
//! format; and [`parse`], which parses one file into [`syntax`] and gives the
//! stderr LilyPond would print for it.
//!
//! P2: the notes core (`notes.rs`), parsed by recursive descent into
//! spanned syntax with the pitch names of [`NoteNames`]; syntax errors worded
//! in Bison's terms (`bison.rs`) and recovered from where LilyPond's `error`
//! productions recover (`recover.rs`). Toplevel items are music, `name =
//! value`, blocks, and `\markup` as a skeleton that P7 completes.
//!
//! P3: `\name` and shorthands classified by their values in the file's
//! module, which uses the interpreter's `(lily)` (`classify.rs`), and
//! assignments defined into it (`assign.rs`). [`parse`] takes the [`Vm`]
//! whose `(lily)` that is.
//!
//! P4: `#` and `$` forms read and run where LilyPond reads and runs them,
//! with Bison's timing (`scheme.rs`): at toplevel, in a header, as an
//! assignment's value, in property paths, in music and as a multiplier.
//!
//! P5, part 1: music, event and Scheme functions applied by their
//! signatures (`function.rs`), their arguments read as `parser.yy`'s
//! argument lists read them (`args.rs`), with rejected optional arguments
//! put back for the next parameter (`cursor.rs`).
//!
//! N2 (MS2 step 4): music made natively from the syntax (`function.rs`)
//! with the music crate's constructors, its `origin` at its span, where
//! LilyPond hands music on: to a function or a predicate, as a variable's
//! value, and as a score's music, which the book handlers collect
//! ([`Parse::toplevel_scores`], MS2 step 16).
//!
//! P5, part 2: `\score`, `\header`, `\paper`, `\layout`, `\midi`, `\book`
//! and `\bookpart` as nested scopes with LilyPond's placement rules
//! (`output_def.rs`), and number expressions with units (`number.rs`).
//!
//! P6, part 1 (`context.rs`): `\new`, `\context` and `\change` as
//! `ContextSpeccedMusic` and `ContextChange`, `\with` as a context
//! modification, and `\context { … }` in output definitions, its context
//! definition a stand-in until N3.
//!
//! P7 (`markup.rs`): markup mode, markup commands by their signatures,
//! markup lists, Scheme markup, `\markuplist`, `\score` and `\score-lines`
//! in markup, at toplevel, in books, in scripts, as arguments, values and
//! `scalar`s, and `\markup name = …`; and case 20's Guile abort
//! ([`Parse::guile_abort`]).
//!
//! The session (`session.rs`, D66): an init file parsed once with
//! [`parse_init`], its variables recorded as `session-save` records them,
//! and each file's module given them, as `session-replay` does: since MS2
//! step 16 by using the init file's module, and `declarations-init.ly`
//! whole, with the files it includes (`\include`, `lexer.rs`). Since step
//! 19 a score includes files too ([`parse_including`]), looked for where
//! LilyPond looks, and the parse stops for one the host must be asked for.
//!
//! P10 (`chord.rs`, `figure.rs`): chord, drum and figure mode, pushed by
//! their heads as the lexer's music mode ([`Parser::modes`]); chords made
//! by the `chords` part's `construct-chord-elements`, drum notes, and
//! figures.
//!
//! P8 (`notes.rs`, `lexer.rs`): lyrics mode, pushed as P10's modes are by
//! `\lyricmode` and `\lyrics`, and by `lyric_mode_music` after
//! `\addlyrics` and `\lyricsto`; its lists are notes mode's, their
//! syllables lyric events (`lyric_element_music`), and `\addlyrics` after
//! music is `add-lyrics`, made where Bison reduces it.
//!
//! P11, part 1 (`embedded.rs`, MS2 step 33, D92): `#{ … #}` templates, by
//! `parser-ly-from-scheme.scm`'s own Scheme: `ly:parser-clone` of the
//! parser running, which each parse publishes as `(lily)`'s `%parser`, and
//! `ly:parse-string-expression`, which the interpreter's `%host-call` runs
//! inside the procedure that calls it; and `ly:parser-parse-string`.
//!
//! P11, part 2 (`partial.rs`, MS2 step 34, D93): functions, property
//! operations, repeats, directions and markup cut short by `\etc`, made by
//! `ly-syntax-constructors.scm`'s own constructors; `\include` of a
//! `\name` and of a `#` or `$` form (`lexer.rs`). And P12's productions
//! that `syntax.ly` reaches: `DURATION_IDENTIFIER`s, a number and its unit
//! in a template, a music function's post-event after a direction, and a
//! music function, music or Scheme in a chord.
//!
//! MS6 step 3 (`reuse.rs`, 02 §5.7, D235): [`parse_again`] keeps a
//! [`Reuse`] of the parse's segments, the toplevel items between the
//! boundaries where it could start again, as items of the interpreter's
//! undo log, and parses a file after an edit from the first segment the
//! edit can change, those before it done again from what undoing them
//! kept. A template's `\include` reads the VM's files (D236).

#![no_std]
extern crate alloc;

mod args;
mod assign;
mod bison;
mod chord;
pub mod classify;
mod context;
mod cursor;
pub mod diag;
mod embedded;
mod figure;
pub mod function;
pub mod lexer;
mod markup;
mod notes;
mod number;
pub mod output_def;
mod partial;
pub mod pitch;
mod property;
mod recover;
mod repeat;
mod reuse;
mod scheme;
mod session;
pub mod source;
pub mod syntax;

pub use classify::{Env, Id, Include};
pub use diag::{At, Diagnostic, Diagnostics, Located, Message, Severity};
pub use lexer::{Lexer, Mode, Tok, Token, VersionSeen};
pub use pitch::{NoteNames, Pitch, PitchNames};
pub use reuse::{ParseStep, Parsing, Reuse, parse_again, parse_on};
pub use scheme::define_include_string;
pub use session::{end_session, parse_init};
pub use source::{Counts, Source, Sources, Span};
pub use syntax::{Item, ItemKind, Music, MusicKind, OutputKind};

use alloc::string::String;
use alloc::vec::Vec;
use bison::Expected;
use lexer::{LILYPOND_VERSION, keyword};
use lily_engine_scheme::Vm;
use syntax::Duration;

/// One file parsed.
pub struct Parse {
    pub sources: Sources,
    pub file: u16,
    pub diagnostics: Diagnostics,
    pub version: VersionSeen,
    /// The toplevel items, in the file's order.
    pub items: Vec<Item>,
    /// The file's module, in which its assignments defined.
    pub module: lily_engine_scheme::Value,
    /// `error_level_`: the file failed, with or without a message.
    pub error_level: bool,
    /// The tokens the parser read, for [`parse_tokens`]: one name each, as
    /// `-ddebug-parser` traces them (`lexer.rs`).
    pub tokens: Option<Vec<String>>,
    /// A Guile error escaped `ly:parse-file` and ended the run, whose text
    /// is the last diagnostic: Guile printed it, and `lily.scm` lists no
    /// failed files and warns of no missing `\version` (case 20, D64).
    pub guile_abort: bool,
    /// The parse stopped for a file the host must be asked for (`lexer.rs`):
    /// what it made is to be dropped, and the file parsed again once the
    /// host has answered.
    pub waiting: bool,
}

impl Parse {
    /// The scores the toplevel handlers collected in the file's
    /// `toplevel-scores` (`collect-scores-for-book`), in the file's order:
    /// the list holds the last first, as `init.ly` makes the file's book of
    /// it, and `Book::process` reverses it (MS2 step 40). Values of the
    /// parse's module, which the caller roots before Scheme runs again.
    pub fn toplevel_scores(&self, vm: &mut Vm) -> Vec<lily_engine_scheme::Value> {
        let mut scores = Vec::new();
        let mut at = vm
            .module_value(self.module, "toplevel-scores")
            .unwrap_or(lily_engine_scheme::Value::NULL);
        while let (Some(score), Some(next)) = (vm.heap().car(at), vm.heap().cdr(at)) {
            scores.push(score);
            at = next;
        }
        scores.reverse();
        scores
    }

    /// The parts the toplevel handler collected in the file's
    /// `toplevel-bookparts` (`collect-bookpart-for-book`), in the file's
    /// order: the toplevel `\bookpart` blocks, each after a part of the
    /// toplevel scores before it, if any. Values of the parse's module, as
    /// [`Parse::toplevel_scores`]'.
    pub fn toplevel_bookparts(&self, vm: &mut Vm) -> Vec<lily_engine_scheme::Value> {
        let mut parts = Vec::new();
        let mut at = vm
            .module_value(self.module, "toplevel-bookparts")
            .unwrap_or(lily_engine_scheme::Value::NULL);
        while let (Some(part), Some(next)) = (vm.heap().car(at), vm.heap().cdr(at)) {
            parts.push(part);
            at = next;
        }
        parts.reverse();
        parts
    }

    /// The books of the file's `\book` blocks, in the file's order: where
    /// LilyPond calls `toplevel-book-handler`, which prints them (layout's),
    /// the parser adds them to the file's `%toplevel-books` (MS2 step 20).
    /// Values of the parse's module, as [`Parse::toplevel_scores`]'.
    pub fn toplevel_books(&self, vm: &mut Vm) -> Vec<lily_engine_scheme::Value> {
        let mut books = Vec::new();
        let mut at = vm
            .module_value(self.module, "%toplevel-books")
            .unwrap_or(lily_engine_scheme::Value::NULL);
        while let (Some(book), Some(next)) = (vm.heap().car(at), vm.heap().cdr(at)) {
            books.push(book);
            at = next;
        }
        books.reverse();
        books
    }

    /// Whether LilyPond would list the file as failed: it had an error.
    pub fn failed(&self) -> bool {
        self.error_level || self.guile_abort || self.diagnostics.errors() > 0
    }

    /// What LilyPond prints for the file with `--loglevel=WARNING`, placed
    /// ([`diag::Message`]): its diagnostics, and `failed files:` if it failed
    /// and the run went on.
    pub fn messages(&self) -> Vec<diag::Message> {
        self.report().0
    }

    /// [`Parse::messages`], and the warnings `ly:expect-warning` expected
    /// that none of them met, the oldest first, for the translations to
    /// meet.
    pub fn report(&self) -> (Vec<diag::Message>, Vec<String>) {
        let (mut messages, expected) = self.diagnostics.report(&self.sources);
        if self.failed() && !self.guile_abort {
            messages.push(diag::Message::failed_files(&[self
                .sources
                .get(self.file)
                .name()]));
        }
        (messages, expected)
    }

    /// What a translation of the file's scores said since the last call
    /// (MS2 step 40): the music crate's warnings and what Scheme wrote to
    /// the error port, taken into `diags` as the parse takes them after a
    /// Scheme call, to be placed in this parse's files; a parser error at
    /// no location is at the root's end, where LilyPond's lexer is then.
    pub fn take_said(&self, vm: &mut Vm, diags: &mut Diagnostics) {
        let end = self.sources.get(self.file).text().len() as u32;
        let here = Span {
            file: self.file,
            start: end,
            end,
        };
        scheme::take_scheme_output(vm, diags, here);
    }

    /// What LilyPond prints to stderr for the file with `--loglevel=WARNING`.
    pub fn stderr(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for message in self.messages() {
            message.write(&mut out);
        }
        out
    }
}

/// Parses `text` as the main file `name`, which locations print, with the
/// pitch names of `names`' default language, in a new module of `vm` that
/// uses its `(lily)` ([`Env::new`]).
pub fn parse(name: &str, text: Vec<u8>, names: &NoteNames, vm: &mut Vm) -> Parse {
    parse_in(name, text, names, Env::new(vm), false, true)
}

/// [`parse`], with `\include` reading what `include` has of each file it
/// looks for; one the host must be asked for stops the parse
/// ([`Parse::waiting`]).
pub fn parse_including(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    vm: &mut Vm,
    include: Include<'_>,
) -> Parse {
    let mut env = Env::new(vm);
    env.include = Some(include);
    parse_in(name, text, names, env, false, true)
}

/// [`parse_including`], with the tokens the parser read recorded in
/// [`Parse::tokens`].
pub fn parse_tokens_including(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    vm: &mut Vm,
    include: Include<'_>,
) -> Parse {
    let mut env = Env::new(vm);
    env.include = Some(include);
    parse_in(name, text, names, env, true, true)
}

/// [`parse`], with the tokens the parser read recorded in [`Parse::tokens`].
pub fn parse_tokens(name: &str, text: Vec<u8>, names: &NoteNames, vm: &mut Vm) -> Parse {
    parse_in(name, text, names, Env::new(vm), true, true)
}

/// Parses `text` as `name` in the scopes of `env`, recording the tokens
/// read if `tokens`; as the main file if `main`, which `init.ly` warns
/// about if it has no `\version`.
fn parse_in(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    env: Env<'_>,
    tokens: bool,
    main: bool,
) -> Parse {
    match parse_with(name, text, names, env, tokens, main, None, None) {
        reuse::Ending::Ended(ended) => ended.0,
        reuse::Ending::Paused(_) => unreachable!("only a parse with segments stops"),
    }
}

/// [`parse_in`], in segments recorded for a [`Reuse`] when `reusing`
/// gives them (02 §5.7, `reuse.rs`): started at the last of their
/// boundaries with what was said, made and included before it, or from the
/// start. Then also the segments, and what was said, made and included at
/// the end. With `paused` too, the segments of a parse that stopped at their
/// last boundary, gone on with from there with what it had said, made and
/// included (D267). A parse with segments may stop at a boundary when the
/// step's time is spent ([`reuse::Ending::Paused`]).
#[allow(clippy::too_many_arguments)]
fn parse_with(
    name: &str,
    text: Vec<u8>,
    names: &NoteNames,
    env: Env<'_>,
    tokens: bool,
    main: bool,
    reusing: Option<reuse::Segments>,
    paused: Option<reuse::Resumed>,
) -> reuse::Ending {
    let mut sources = Sources::default();
    let file = sources.add(Source::new(name, text));
    let mut lexer = Lexer::new(sources.get(file), file);
    if tokens {
        lexer.record_tokens();
    }
    if let Some(table) = names.language(pitch::DEFAULT_LANGUAGE) {
        lexer.set_pitch_names(table);
    }
    let mut parser = Parser::new(lexer, names, env);
    let mut items = Vec::new();
    let mut in_score = None;
    match (reusing, paused) {
        (Some(segments), Some((diagnostics, made, included, stopped))) => {
            let at = stopped.as_ref().map(|s| &s.at);
            parser.resume_segments(segments, diagnostics, &included, at);
            items = made;
            in_score = stopped;
        }
        (Some(segments), None) => items = parser.start_segments(segments),
        (None, _) => {}
    }
    let stopped = match in_score {
        Some(stopped) => parser.go_on_score(stopped, &mut items),
        None => false,
    };
    if !stopped {
        parser.toplevel_into(&mut items);
    }
    if parser.paused() {
        if let Some(paused) = parser.pause_segments(items) {
            return reuse::Ending::Paused(alloc::boxed::Box::new(paused));
        }
        unreachable!("a parse stops only with its segments");
    }
    let ended = parser.end_segments(&items);
    let module = parser.env.module();
    let tokens = parser.lexer.take_tokens();
    let included: Vec<Source> = parser
        .lexer
        .included()
        .iter()
        .map(|src| Source::new(src.name(), src.text().to_vec()))
        .collect();
    let Parser {
        lexer:
            Lexer {
                diags: mut diagnostics,
                version,
                error_level,
                waiting,
                ..
            },
        guile_abort,
        ..
    } = parser;
    for src in included {
        sources.add(src);
    }
    // The end of init.ly, after the main file.
    if main && version == VersionSeen::No && !guile_abort {
        diagnostics.warning(
            At::Line(file, 1),
            alloc::format!(
                "no \\version statement found, please add\n\n\\version \"{LILYPOND_VERSION}\"\n\nfor future compatibility"
            ),
        );
    }
    diagnostics.flush();
    let parse = Parse {
        sources,
        file,
        diagnostics,
        version,
        items,
        module,
        error_level,
        tokens,
        guile_abort,
        waiting,
    };
    reuse::Ending::Ended(alloc::boxed::Box::new((parse, ended)))
}

pub(crate) const BOOK: Tok = Tok::Keyword(keyword("book"));
pub(crate) const BOOKPART: Tok = Tok::Keyword(keyword("bookpart"));
pub(crate) const HEADER: Tok = Tok::Keyword(keyword("header"));
pub(crate) const LAYOUT: Tok = Tok::Keyword(keyword("layout"));
pub(crate) const MARKUP: Tok = Tok::Keyword(keyword("markup"));
pub(crate) const MARKUPLIST: Tok = Tok::Keyword(keyword("markuplist"));
pub(crate) const MIDI: Tok = Tok::Keyword(keyword("midi"));
pub(crate) const PAPER: Tok = Tok::Keyword(keyword("paper"));
pub(crate) const SCORE: Tok = Tok::Keyword(keyword("score"));
pub(crate) const NOTEMODE: Tok = Tok::Keyword(keyword("notemode"));
pub(crate) const LYRICMODE: Tok = Tok::Keyword(keyword("lyricmode"));
pub use function::define_type_names;

/// Whether `kind` can follow `lilypond error` (the end of input included):
/// what starts a toplevel item in P2's grammar.
fn starts_item(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Eof
            | Tok::Invalid
            | Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | context::NEWCONTEXT
            | context::CONTEXT
            | HEADER
            | MARKUP
            | MARKUPLIST
            | SCORE
            | BOOK
            | BOOKPART
            | PAPER
            | LAYOUT
            | MIDI
            | Tok::Symbol
            | Tok::String
            | Tok::ScmToken
            | Tok::Identifier(
                Id::Music
                    | Id::Scm
                    | Id::Lookup
                    | Id::Module
                    | Id::MusicFunction
                    | Id::ScmFunction
                    | Id::Book
            )
    ) || repeat::starts_basic(kind)
}

/// A syntax error, reported (or not, while Bison would be recovering) at
/// the token that is still [`Parser::peek`]'s, unless Bison discarded it:
/// the production around it with an `error` rule recovers from it
/// (`recover.rs`).
pub(crate) struct SyntaxError;

pub(crate) type Parsed<T> = Result<T, SyntaxError>;

/// The parser's state: `Lily_parser`'s and its lexer's (02 §3.3).
pub(crate) struct Parser<'a, 'v> {
    lexer: Lexer<'a>,
    /// The pitch names of every language, which `\language` chooses from.
    names: &'a NoteNames,
    /// The scopes `\name` is looked up in and assignments define into.
    env: Env<'v>,
    /// Bison's `yyerrstatus`: after a syntax error, the tokens still to be
    /// shifted before the next one is reported.
    recovering: u8,
    /// Bison aborted the parse: a syntax error at the end of input, which
    /// only toplevel's `error` rule can take. Nothing more is reported.
    aborted: bool,
    /// A Guile error ended the run ([`Parse::guile_abort`]); the parse is
    /// aborted with it.
    guile_abort: bool,
    /// A syntax error in markup left the lexer in markup mode, which no
    /// rule pops: every token after it is lexed in markup mode.
    markup_stuck: bool,
    /// The lexer modes `\notemode` and `\markup` pushed and their rules
    /// have not popped yet, as a syntax error inside them leaves them; and
    /// the initial mode of an output definition's body.
    pushed: u32,
    /// The music modes pushed (P10): what [`Mode::Notes`] lexes in, the
    /// last pushed, notes when none is. `\chordmode` and the other heads
    /// push theirs, and so do the rules that push the notes mode, which
    /// pop them where those pop [`Parser::pushed`].
    modes: Vec<Mode>,
    /// The mode the current block's own tokens are lexed in: initial in an
    /// output definition's body, else notes. Music is read in notes mode.
    base: Mode,
    /// The lookahead token and the mode it was lexed in.
    ahead: Option<(Token, Mode)>,
    /// How many values were kept before the lookahead was lexed
    /// ([`Env::release`]).
    ahead_kept: usize,
    /// Where the last token taken is located to ([`Token::loc`]), where a
    /// node's span ends.
    last_end: u32,
    /// `default_duration_`: what a note without a duration takes.
    default_duration: Duration,
    /// `default_tremolo_type_`: what `:` alone takes.
    default_tremolo: u32,
    /// What was said before the lookahead was lexed.
    ahead_said: (usize, usize),
    /// Its segments, when it keeps a [`Reuse`] (`reuse.rs`).
    segments: Option<reuse::Segments>,
    /// The next `\score` block is a toplevel item, whose `scorify-music` may
    /// run in steps (B23, D272); and where one stopped in it.
    score_in_steps: bool,
    in_score: Option<recover::InScore>,
}

impl<'a, 'v> Parser<'a, 'v> {
    /// A parser of what `lexer` lexes, in the scopes of `env`, which
    /// publishes its pitch names and whether it records tokens to Scheme
    /// (`embedded.rs`).
    fn new(lexer: Lexer<'a>, names: &'a NoteNames, env: Env<'v>) -> Parser<'a, 'v> {
        let mut parser = Parser {
            lexer,
            names,
            env,
            recovering: 0,
            aborted: false,
            guile_abort: false,
            markup_stuck: false,
            pushed: 0,
            modes: Vec::new(),
            base: Mode::Notes,
            ahead: None,
            ahead_kept: 0,
            last_end: 0,
            default_duration: Duration::new(2, 0),
            default_tremolo: 8,
            ahead_said: (0, 0),
            segments: None,
            score_in_steps: false,
            in_score: None,
        };
        let record = parser.lexer.records_tokens();
        let language = parser.language().map(String::from);
        if let Some(p) = parser.env.lily_parser() {
            p.names = names.clone();
            p.record = record;
            p.language = language;
        }
        parser
    }

    /// The language of the lexer's pitch names.
    fn language(&self) -> Option<&'a str> {
        self.names.language_of(self.lexer.pitch_names()?)
    }

    /// What `ly:parser-clone` copies of the parser that runs Scheme now:
    /// its default duration and tremolo, and its pitch names' language;
    /// and whether it has an error, for `ly:parser-has-error?`.
    pub(crate) fn publish(&mut self) {
        let (duration, tremolo) = (self.default_duration, self.default_tremolo);
        let language = self.language();
        let error = self.lexer.error_level || self.lexer.diags.errors() > 0;
        if let Some(p) = self.env.lily_parser() {
            p.duration = duration;
            p.tremolo = tremolo;
            p.error = error;
            if p.language.as_deref() != language {
                p.language = language.map(String::from);
            }
        }
    }

    /// After Scheme the parser ran: the tokens of the templates it parsed,
    /// after the parser's own, and a clear of the parser's errors it asked.
    pub(crate) fn take_template_tokens(&mut self) {
        // `ly:parser-clear-error` of the running parse, which the Scheme
        // just run called: `clear_error` (P12).
        if let Some(p) = self.env.lily_parser()
            && core::mem::take(&mut p.cleared)
        {
            self.lexer.error_level = false;
            self.lexer.diags.clear_errors();
        }
        let tokens = self.env.template_tokens();
        if tokens.is_empty() {
            return;
        }
        // Bison read the lookahead, and not yet what it pushed.
        match self.ahead {
            Some(_) => self.lexer.insert_tokens(tokens),
            None => self.lexer.extend_tokens(tokens),
        }
    }
}

impl<'a> Parser<'a, '_> {
    /// The mode a token asked for in `mode` is lexed in: notes mode is the
    /// music mode pushed last ([`Parser::modes`]).
    pub(crate) fn lex_mode(&self, mode: Mode) -> Mode {
        match mode {
            _ if self.markup_stuck => Mode::Markup,
            Mode::Notes => self.modes.last().copied().unwrap_or(Mode::Notes),
            mode => mode,
        }
    }

    /// The next token, lexed in `mode` and left to be taken.
    fn peek(&mut self, mode: Mode) -> Token {
        let mode = self.lex_mode(mode);
        if let Some((token, lexed)) = self.ahead {
            debug_assert!(
                lexed == mode || token.kind == Tok::Eof || self.markup_stuck,
                "a token was looked at in another mode"
            );
            return token;
        }
        self.ahead_kept = self.env.kept();
        self.ahead_said = self.lexer.diags.said();
        let token = self.lexer.next_in(mode, &mut self.env);
        // A value pushed before the token was lexed, as further values of
        // a form are, was kept before it.
        if let Tok::Identifier(_) = token.kind {
            self.ahead_kept = self.ahead_kept.min(token.value as usize);
        }
        if token.kind == Tok::Eof {
            self.end_of_input(&token);
        }
        self.ahead = Some((token, mode));
        token
    }

    /// Takes the next token: Bison's shift.
    fn next(&mut self, mode: Mode) -> Token {
        let token = self.peek(mode);
        self.ahead = None;
        self.recovering = self.recovering.saturating_sub(1);
        // Bison's `@$` ends where the last token is located: a `#` form at
        // its `#`.
        self.last_end = token.loc.end;
        token
    }

    /// `here_input`: the last token the lexer read, the lookahead if there
    /// is one, else where the last token taken ends.
    pub(crate) fn here(&self) -> Span {
        match &self.ahead {
            Some((token, _)) => token.span,
            None => Span {
                file: self.lexer.file(),
                start: self.last_end,
                end: self.last_end,
            },
        }
    }

    /// Drops the next token, as Bison's recovery discards a lookahead.
    fn discard(&mut self) {
        self.ahead = None;
    }

    /// The span from `start`'s first byte to the last token taken.
    fn span_from(&self, start: Span) -> Span {
        Span {
            end: self.last_end.max(start.start),
            ..start
        }
    }

    /// `lilypond`: toplevel items, and `lilypond error`, which discards
    /// tokens until one can start an item.
    fn toplevel(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        self.toplevel_into(&mut items);
        items
    }

    /// [`Parser::toplevel`], its items after `items`, which a parse that
    /// keeps a [`Reuse`] takes the last parse's into (`reuse.rs`).
    fn toplevel_into(&mut self, items: &mut Vec<Item>) {
        while !self.aborted {
            self.segment_boundary(items);
            if self.paused() {
                return;
            }
            let token = self.peek(Mode::Notes);
            let kind = match token.kind {
                Tok::Eof => break,
                Tok::Invalid => {
                    self.next(Mode::Notes);
                    continue;
                }
                kind if matches!(
                    kind,
                    Tok::Char(b'{')
                        | Tok::DoubleAngleOpen
                        | NOTEMODE
                        | LYRICMODE
                        | context::NEWCONTEXT
                        | context::CONTEXT
                        | Tok::Identifier(Id::Music | Id::MusicFunction)
                ) || repeat::starts_basic(kind) =>
                {
                    self.composite_music().map(|music| {
                        // `toplevel_expression` is reduced with the next token lexed.
                        self.peek(Mode::Notes);
                        self.hand_music("toplevel-music-handler", None, &music);
                        ItemKind::Music(music)
                    })
                }
                HEADER => {
                    self.next(Mode::Notes);
                    self.toplevel_header().map(ItemKind::Header)
                }
                MARKUP => {
                    self.next(Mode::Notes);
                    self.toplevel_markup()
                }
                MARKUPLIST => {
                    self.next(Mode::Notes);
                    self.full_markup_list().map(|list| {
                        self.hand("toplevel-text-handler", None, list);
                        ItemKind::Markup
                    })
                }
                SCORE => {
                    self.next(Mode::Notes);
                    self.score_in_steps = true;
                    let block = self.score_block(token);
                    // Stopped in its `scorify-music` (D272): the parse goes on
                    // there in the next step.
                    if self.in_score.is_some() {
                        return;
                    }
                    self.score_ended(block)
                }
                BOOK => {
                    self.next(Mode::Notes);
                    self.book_block(token).map(|(item, book)| {
                        self.collect_book(book);
                        item.kind
                    })
                }
                BOOKPART => {
                    self.next(Mode::Notes);
                    self.bookpart_block(token).map(|(item, part)| {
                        self.hand("toplevel-bookpart-handler", None, part);
                        item.kind
                    })
                }
                PAPER => {
                    self.next(Mode::Notes);
                    self.toplevel_output_def(token, OutputKind::Paper)
                }
                LAYOUT => {
                    self.next(Mode::Notes);
                    self.toplevel_output_def(token, OutputKind::Layout)
                }
                MIDI => {
                    self.next(Mode::Notes);
                    self.toplevel_output_def(token, OutputKind::Midi)
                }
                // A book with a paper for `toplevel-book-handler`, which
                // collects it, else a part for `toplevel-bookpart-handler`.
                Tok::Identifier(Id::Book) => {
                    self.next(Mode::Notes);
                    let book = self.env.value(token.value);
                    self.toplevel_book_identifier(book);
                    Ok(ItemKind::Scheme(token.span))
                }
                Tok::Symbol | Tok::String => self.assignment().map(ItemKind::Assignment),
                Tok::Identifier(Id::Scm | Id::Lookup | Id::Module | Id::ScmFunction) => {
                    self.toplevel_scheme().map(ItemKind::Scheme)
                }
                // `toplevel_expression: SCM_TOKEN`, evaluated and ignored.
                Tok::ScmToken => {
                    self.next(Mode::Notes);
                    self.scheme_value(&token);
                    Ok(ItemKind::Scheme(token.span))
                }
                _ => Err(self.syntax_error(&token, bison::MANY)),
            };
            self.item_ended(items, token, kind);
        }
        self.segments_end();
    }

    /// A toplevel `\score` block parsed: its score handed to
    /// `toplevel-score-handler`.
    fn score_ended(
        &mut self,
        block: Parsed<(Item, lily_engine_scheme::Value)>,
    ) -> Parsed<ItemKind> {
        block.map(|(item, score)| {
            self.hand("toplevel-score-handler", None, score);
            item.kind
        })
    }

    /// A toplevel item that began with `token` ended as `kind`.
    fn item_ended(&mut self, items: &mut Vec<Item>, token: Token, kind: Parsed<ItemKind>) {
        match kind {
            Ok(kind) => items.push(Item {
                span: self.span_from(token.span),
                kind,
            }),
            Err(SyntaxError) => {
                self.resync(Mode::Notes, starts_item);
            }
        }
        self.segment_moved();
        // What the item kept is in the module or the scores' music now,
        // or nowhere: only the lookahead's values are carried on, and
        // those of the tokens pushed to come before the input's next.
        if !self.lexer.has_extra() {
            let kept = match self.ahead {
                Some(_) => self.ahead_kept,
                None => self.env.kept(),
            };
            self.env.release(kept);
        }
    }

    /// The toplevel `\score` stopped in its `scorify-music` (D272) gone on
    /// with, the lexer where its music ended: the call, then the block's
    /// items and the toplevel item's end. Whether it stopped again.
    fn go_on_score(&mut self, mut stopped: recover::InScore, items: &mut Vec<Item>) -> bool {
        let Ok(value) = self.scorify_on() else {
            self.in_score = Some(stopped);
            return true;
        };
        let body = &mut stopped.body;
        body.score = Some(self.scorified(value, body.header));
        let item = Item {
            span: self.span_from(stopped.token.span),
            kind: ItemKind::Music(stopped.music),
        };
        body.items.push(item);
        let head = stopped.body.head;
        let block = self.score_body(stopped.body);
        let kind = self.score_ended(block);
        self.item_ended(items, head, kind);
        false
    }

    /// `embedded_scm_active` at toplevel, a `SCM_IDENTIFIER` or a lookup:
    /// a string is text for `toplevel-text-handler`; a score, an output
    /// definition and a module are taken as `output_def.rs` has it; any
    /// other value but the unspecified is `bad expression type`.
    /// Bison reduces a `SCM_IDENTIFIER` with no lookahead, a lookup after
    /// the token that follows it.
    fn toplevel_scheme(&mut self) -> Parsed<Span> {
        let start = self.peek(Mode::Notes);
        let value = if start.kind == Tok::Identifier(Id::Scm) {
            self.next(Mode::Notes);
            self.env.value(start.value)
        } else if start.kind == Tok::Identifier(Id::ScmFunction) {
            self.function_call()?.0
        } else {
            let value = self.lookup()?;
            self.peek(Mode::Notes);
            value
        };
        let span = Span {
            start: start.loc.start,
            ..self.span_from(start.span)
        };
        if let Some(text) = self.text_of(value) {
            self.hand("toplevel-text-handler", None, text);
            return Ok(span);
        }
        let fine = value == lily_engine_scheme::Value::UNSPECIFIED || self.toplevel_value(value);
        if !fine {
            self.error(span, "bad expression type");
        }
        Ok(span)
    }

    /// `full_markup` at toplevel, for `toplevel-text-handler` in a list of
    /// its own, after `\markup`; or `markup_mode_word '=' identifier_init`,
    /// whose `=` is lexed after the mode is popped.
    fn toplevel_markup(&mut self) -> Parsed<ItemKind> {
        let (name, markup) = match self.markup_mode()? {
            markup::FullMarkup::Word(name, markup) => (name, markup),
            markup::FullMarkup::Markup(markup) => {
                let text = self.one_text(markup);
                self.hand("toplevel-text-handler", None, text);
                return Ok(ItemKind::Markup);
            }
            markup::FullMarkup::Partial(_) => unreachable!("markup is cut short only with `etc`"),
        };
        if self.peek(Mode::Notes).kind != Tok::Char(b'=') {
            let text = self.one_text(markup);
            self.hand("toplevel-text-handler", None, text);
            return Ok(ItemKind::Markup);
        }
        self.next(Mode::Notes);
        let first = self.peek(Mode::Notes);
        let value = self.markup_command_value()?;
        let at = Span {
            start: first.loc.start,
            ..self.span_from(first.span)
        };
        self.define_markup_command(&name, value, at);
        Ok(ItemKind::Markup)
    }

    /// Takes a `kind` token, or reports the one there, with what Bison
    /// lists as `expected` there.
    fn expect(&mut self, kind: Tok, mode: Mode, expected: Expected) -> Parsed<Token> {
        let token = self.peek(mode);
        if token.kind != kind {
            return Err(self.syntax_error(&token, expected));
        }
        Ok(self.next(mode))
    }
}
