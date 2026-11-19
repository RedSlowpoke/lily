//! P11 part 1 (02 §2.4, §6; MS2 step 33, D92): `#{ … #}` templates run by
//! `parser-ly-from-scheme.scm` as it is. The reader makes a template the
//! call of `read-lily-expression-internal` with its text, file, line and
//! the closures of its `#` and `$` forms by byte (`LilyScan`), and that
//! calls `ly:parser-clone` and `ly:parse-string-expression`, which are the
//! parser's: in `(lily)` they are Scheme that checks its arguments as
//! `LY_ASSERT_TYPE` does and calls `%host-call`, and the interpreter calls
//! [`host`] with the whole VM, so that a parse runs inside a procedure.
//!
//! - **The parser** (`Lily_parser`, [`LilyParser`]) is a value: what a
//!   parse that runs Scheme publishes as LilyPond's `f_parser` fluid, in
//!   `(lily)`'s `%parser` ([`Env`] sets it while it lives), and what
//!   `ly:parser-clone` copies of it: its scopes, its pitch names and
//!   language, its default duration and tremolo, with the closures and the
//!   location it is given.
//! - **`ly:parse-string-expression`** parses the text as a source named by
//!   the file and numbered from the line ([`Source::with_line`]) after the
//!   synthetic `EMBEDDED_LILY`, as `embedded_lilypond`
//!   ([`Parser::embedded_lilypond`]): a `#` or `$` form at a byte of the
//!   closures is read, for its end, and its closure called in its place
//!   (`closures_` in `parse-scm.cc`); music is placed at the location
//!   (`override_input`). A template is so parsed again at each call.
//! - Its **messages** are placed in its own text and handed to the parse
//!   that runs it among its other messages (`%diagnostics`, kind `placed`),
//!   and its error makes `ly:parser-has-error?` of the clone true, for
//!   `error in #{ ... #}` at the call. Its **tokens**, when they are
//!   recorded, follow the outer parse's at the call, indented a level, as
//!   `-ddebug-parser` traces them.
//! - **`ly:parser-parse-string`** parses the text as a file of toplevel
//!   items, `<string>`, with the clone's scopes.

use crate::bison;
use crate::classify::{Env, Id};
use crate::context::{CHANGE, NEWCONTEXT};
use crate::diag::{Diagnostics, Handed, Message};
use crate::lexer::{Lexer, Mode, Tok};
use crate::pitch::NoteNames;
use crate::property::starts_property;
use crate::repeat::{TEMPO, starts_basic};
use crate::source::{Source, Sources};
use crate::syntax::{Duration, Music, MusicKind};
use crate::{LYRICMODE, NOTEMODE, Parser};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::{Exception, Foreign, Heap, Tracer, Value, Vm};

const N: Mode = Mode::Notes;

/// `Lily_parser`: what a parse publishes of itself for Scheme, and a clone
/// of it, which `ly:parse-string-expression` parses with.
pub(crate) struct LilyParser {
    /// `scopes_`, the file's module first.
    pub(crate) scopes: Vec<Value>,
    pub(crate) names: NoteNames,
    /// The language of the lexer's pitch names, if it has some.
    pub(crate) language: Option<String>,
    pub(crate) duration: Duration,
    pub(crate) tremolo: u32,
    /// The alist of byte offsets and closures `ly:parser-clone` was given.
    pub(crate) closures: Value,
    /// `override_input_`: an `Input`, or `#f`.
    pub(crate) location: Value,
    /// `error_level_`, of a clone that parsed.
    pub(crate) error: bool,
    /// `ly:parser-clear-error` of it ran, which the running parse takes
    /// after the Scheme that ran it returns (P12).
    pub(crate) cleared: bool,
    /// Whether its tokens are recorded.
    pub(crate) record: bool,
    /// The tokens of the parses it ran, indented, which it takes after the
    /// Scheme that ran them returns.
    pub(crate) tokens: Vec<String>,
    /// How many templates' parses run inside the one it clones.
    depth: u32,
}

/// How deep templates may run inside templates: past it, Guile's `Stack
/// overflow` (LilyPond's own stack ends in a crash).
const MAX_DEPTH: u32 = 64;

/// The number of a template's first source at depth 0, and how many each
/// depth has: a template's text and the strings it includes.
const FIRST_FILE: u16 = 0x8000;
const FILES: u16 = 0x100;

impl Foreign for LilyParser {
    fn type_name(&self) -> &'static str {
        "Lily_parser"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for &scope in &self.scopes {
            tracer.trace(scope);
        }
        tracer.trace(self.closures);
        tracer.trace(self.location);
    }
}

impl LilyParser {
    pub(crate) fn new(scopes: Vec<Value>) -> LilyParser {
        LilyParser {
            scopes,
            names: NoteNames::default(),
            language: None,
            duration: Duration::new(2, 0),
            tremolo: 8,
            closures: Value::NULL,
            location: Value::FALSE,
            error: false,
            cleared: false,
            record: false,
            tokens: Vec::new(),
            depth: 0,
        }
    }

    /// `Lily_parser (src, closures, location)`: a clone of `self`.
    fn clone_with(&self, closures: Value, location: Value) -> LilyParser {
        LilyParser {
            scopes: self.scopes.clone(),
            names: self.names.clone(),
            language: self.language.clone(),
            duration: self.duration,
            tremolo: self.tremolo,
            closures,
            location,
            error: false,
            cleared: false,
            record: self.record,
            tokens: Vec::new(),
            depth: self.depth + 1,
        }
    }
}

/// A message a template's parse placed, handed to the parse that ran it.
struct Placed(Message);

impl Foreign for Placed {
    fn type_name(&self) -> &'static str {
        "placed-message"
    }
}

/// The message of a `placed` entry of `%diagnostics`.
pub(crate) fn placed_message(heap: &Heap, value: Value) -> Option<Message> {
    heap.foreign::<Placed>(value).map(|p| p.0.clone())
}

/// `(lily)`'s `%parser`: the parser running, or `#f`.
pub(crate) const PARSER: &str = "%parser";

/// Makes `parser` `(lily)`'s `%parser` and gives what it was; `#f` when
/// `(lily)` has no `%parser` ([`DEFINITIONS`] were not evaluated).
pub(crate) fn install(vm: &mut Vm, parser: Value) -> Value {
    let Some(slot) = vm.slot("lily", PARSER) else {
        return Value::FALSE;
    };
    let previous = vm.slot_value(slot).unwrap_or(Value::FALSE);
    let lily = vm.slot_module(slot);
    // Each `Env` gives the one before back itself, inside an item or not:
    // an undo giving a template's parse's change back would leave a parser
    // that has gone since (MS6 step 3).
    vm.module_define_unlogged(lily, PARSER, parser);
    previous
}

/// The definitions in `(lily)` that reach the parser through `%host-call`,
/// evaluated with `ly:parser-include-string`'s ([`crate::define_include_string`]).
pub(crate) const DEFINITIONS: &str = r#"
(define %parser #f)
(define (%parser-arg subr parser position)
  (if (not (%host-call 'parser? parser))
      (if (= position 0)
          (scm-error 'wrong-type-arg subr "Wrong type argument (expecting ~A): ~S"
                     (list "Lily_parser" parser) (list parser))
          (scm-error 'wrong-type-arg subr
                     "Wrong type argument in position ~A (expecting ~A): ~S"
                     (list position "Lily_parser" parser) (list parser)))))
(define (%string-arg subr value position)
  (if (not (string? value))
      (scm-error 'wrong-type-arg subr
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list position "string" value) (list value))))
(define-public (ly:parser-clone . args)
  (let ((closures (if (pair? args) (car args) '()))
        (location (if (and (pair? args) (pair? (cdr args))) (cadr args) #f)))
    (%parser-arg "ly:parser-clone" %parser 0)
    (if (not (list? closures))
        (scm-error 'wrong-type-arg "ly:parser-clone"
                   "Wrong type argument in position ~A (expecting ~A): ~S"
                   (list 2 "list" closures) (list closures)))
    (%host-call 'parser-clone %parser closures location)))
(define-public (ly:parse-string-expression parser ly-code . rest)
  (%parser-arg "ly:parse-string-expression" parser 1)
  (%string-arg "ly:parse-string-expression" ly-code 2)
  (let ((filename (if (and (pair? rest) (string? (car rest))) (car rest) "<string>"))
        (line (if (and (pair? rest) (pair? (cdr rest)) (exact-integer? (cadr rest)))
                  (cadr rest) 0)))
    (%host-call 'parse-string parser ly-code filename line #t)))
(define-public (ly:parser-parse-string parser-smob ly-code)
  (%parser-arg "ly:parser-parse-string" parser-smob 1)
  (%string-arg "ly:parser-parse-string" ly-code 2)
  (%host-call 'parse-string parser-smob ly-code "<string>" 0 #f)
  *unspecified*)
(define-public (ly:parser-has-error? . parser)
  (let ((parser (if (pair? parser) (car parser) %parser)))
    (%parser-arg "ly:parser-has-error?" parser 1)
    (%host-call 'has-error parser)))
(define-public (ly:parser-clear-error . parser)
  (let ((parser (if (pair? parser) (car parser) %parser)))
    (%parser-arg "ly:parser-clear-error" parser 1)
    (%host-call 'clear-error parser)
    *unspecified*))
"#;

/// What `%host-call` does ([`Vm::set_host`]): `(%host-call what arg …)`,
/// its arguments checked by [`DEFINITIONS`].
pub(crate) fn host(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let what = args.first().and_then(|&a| vm.heap().name(a)).unwrap_or("");
    let arg = |i: usize| args.get(i).copied().unwrap_or(Value::FALSE);
    match what {
        "parser?" => Ok(Value::bool(
            vm.heap().foreign::<LilyParser>(arg(1)).is_some(),
        )),
        "parser-clone" => {
            let heap = vm.heap_mut();
            let Some(parser) = heap.foreign::<LilyParser>(arg(1)) else {
                return Ok(Value::FALSE);
            };
            let location = match heap.foreign::<lily_engine_music::Input>(arg(3)) {
                Some(_) => arg(3),
                None => Value::FALSE,
            };
            let clone = parser.clone_with(arg(2), location);
            Ok(heap.wrap(clone))
        }
        "parse-string" => {
            let code = vm.heap().str(arg(2)).map(|s| String::from(s.as_str()));
            let name = vm.heap().str(arg(3)).map(|s| String::from(s.as_str()));
            let line = arg(4).as_fixnum().map_or(0, |n| n.max(0) as usize);
            let (Some(code), Some(name)) = (code, name) else {
                return Ok(Value::UNSPECIFIED);
            };
            if vm
                .heap()
                .foreign::<LilyParser>(arg(1))
                .is_some_and(|p| p.depth > MAX_DEPTH)
            {
                return Err(vm.stack_overflow_error());
            }
            Ok(parse_string(
                vm,
                arg(1),
                code,
                &name,
                line,
                arg(5) != Value::FALSE,
            ))
        }
        "has-error" => Ok(Value::bool(
            vm.heap()
                .foreign::<LilyParser>(arg(1))
                .is_some_and(|p| p.error),
        )),
        "clear-error" => {
            if let Some(p) = vm.heap_mut().foreign_mut::<LilyParser>(arg(1)) {
                p.error = false;
                p.cleared = true;
            }
            Ok(Value::UNSPECIFIED)
        }
        _ => Ok(Value::UNSPECIFIED),
    }
}

/// Whether the clone `parser` was given a location, in whose file its
/// music is placed; else its own text, which its inputs then carry (D92).
fn detached_location(vm: &Vm, parser: Value) -> bool {
    vm.heap()
        .foreign::<LilyParser>(parser)
        .is_some_and(|p| p.location != Value::FALSE)
}

/// `parse_string_expression` (`expression`) or `parse_string` of `code`
/// with the clone `parser`: the value of `embedded_lilypond`, or the
/// unspecified. Its error is the clone's; its messages and tokens are the
/// running parse's ([`hand_on`]).
fn parse_string(
    vm: &mut Vm,
    parser: Value,
    code: String,
    name: &str,
    line: usize,
    expression: bool,
) -> Value {
    let Some(clone) = vm.heap().foreign::<LilyParser>(parser) else {
        return Value::UNSPECIFIED;
    };
    let names = clone.names.clone();
    let language = clone.language.clone();
    let depth = clone.depth.min(MAX_DEPTH + 1);
    let (duration, tremolo, record) = (clone.duration, clone.tremolo, clone.record);
    // Numbered apart from the files of the parses that run it, so that a
    // message at one of their `Input`s is placed by them.
    let mut sources = Sources::from(FIRST_FILE + depth as u16 * FILES);
    let mut source = Source::with_line(name, code.into_bytes(), line);
    if !detached_location(vm, parser) {
        source.detach();
    }
    let file = sources.add(source);
    let (value, tokens, included, failed, mut diagnostics) = {
        let mut lexer = Lexer::new(sources.get(file), file);
        if record {
            lexer.record_tokens();
        }
        if let Some(table) = language.as_deref().and_then(|l| names.language(l)) {
            lexer.set_pitch_names(table);
        }
        let env = Env::cloned(vm, parser, file);
        let mut p = Parser::new(lexer, &names, env);
        p.default_duration = duration;
        p.default_tremolo = tremolo;
        let value = match expression {
            true => p.embedded_lilypond(),
            false => {
                p.toplevel();
                Value::UNSPECIFIED
            }
        };
        let tokens = p.lexer.take_tokens();
        let included: Vec<Source> = p
            .lexer
            .included()
            .iter()
            .map(|src| Source::new(src.name(), src.text().to_vec()))
            .collect();
        let failed = p.lexer.error_level || p.lexer.diags.errors() > 0;
        let diagnostics = core::mem::take(&mut p.lexer.diags);
        // A file its `\include` must ask the host for stops the parse
        // running it too (D80).
        if p.lexer.waiting {
            p.env.vm().wait_for_host();
        }
        (value, tokens, included, failed, diagnostics)
    };
    // Rooted until it is the call's value.
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(value);
    for src in included {
        sources.add(src);
    }
    diagnostics.flush();
    if let Some(clone) = vm.heap_mut().foreign_mut::<LilyParser>(parser) {
        clone.error |= failed;
    }
    hand_on(vm, &diagnostics, &sources, tokens);
    vm.heap_mut().unroot_to(depth);
    value
}

/// The messages and tokens of a template's parse handed to the parse
/// running, `%parser`: each message in `%diagnostics`, placed as `#(placed
/// message "" mark)`, or at an `Input` of that parse's as a warning, an
/// error or a programming error; the tokens, indented, in its parser's.
fn hand_on(vm: &mut Vm, diagnostics: &Diagnostics, sources: &Sources, tokens: Option<Vec<String>>) {
    let handed = diagnostics.hand_on(sources);
    if !handed.is_empty()
        && let Some(cell) = vm
            .slot("lily", crate::scheme::DIAGNOSTICS)
            .and_then(|s| vm.slot_value(s))
    {
        let mark = vm.error_port_flushed() as i64;
        let heap = vm.heap_mut();
        let mark = heap.integer(mark);
        for handed in handed {
            let (kind, location, text) = match handed {
                Handed::Placed(message) => ("placed", heap.wrap(Placed(message)), String::new()),
                Handed::At(kind, input, text) => (kind, heap.wrap(input), text),
                Handed::Expect(text) => ("expect", Value::FALSE, text),
                Handed::Check => ("check", Value::FALSE, String::new()),
            };
            let kind = heap.intern(kind);
            let text = heap.string(&text);
            let entry = heap.vector(alloc::vec![kind, location, text, mark]);
            let list = heap.car(cell).unwrap_or(Value::NULL);
            let list = heap.cons(entry, list);
            heap.set_car(cell, list);
        }
    }
    if let Some(tokens) = tokens
        && let Some(running) = vm.slot("lily", PARSER).and_then(|s| vm.slot_value(s))
        && let Some(running) = vm.heap_mut().foreign_mut::<LilyParser>(running)
    {
        running
            .tokens
            .extend(tokens.into_iter().map(|t| alloc::format!("  {t}")));
    }
}

/// The closure of `closures`, an alist of byte offsets, at `byte`.
pub(crate) fn closure_at(heap: &Heap, closures: Value, byte: usize) -> Option<Value> {
    let mut at = closures;
    while let (Some(entry), Some(rest)) = (heap.car(at), heap.cdr(at)) {
        if heap.car(entry).and_then(|b| b.as_fixnum()) == Some(byte as i64) {
            return heap.cdr(entry).filter(|c| heap.is_procedure(*c));
        }
        at = rest;
    }
    None
}

/// Whether `kind` starts `music_embedded` as music, which a template of
/// several items is a list of.
fn starts_music(kind: Tok) -> bool {
    matches!(
        kind,
        Tok::Char(b'{')
            | Tok::DoubleAngleOpen
            | NOTEMODE
            | LYRICMODE
            | NEWCONTEXT
            | Tok::Identifier(Id::Music | Id::MusicFunction | Id::Pitch)
            | TEMPO
            | CHANGE
            | Tok::NotenamePitch
            | Tok::TonicnamePitch
            | Tok::DrumPitch
            | Tok::FigureOpen
            | Tok::RestName
            | Tok::ChordRepetition
            | Tok::MultiMeasureRest
            | Tok::AngleOpen
    ) || starts_basic(kind)
        || starts_property(kind)
}

/// Whether `kind` starts an item of `identifier_init_nonumber` that is no
/// music, or `embedded_lilypond_number`: what `identifier_init` takes.
fn starts_init(kind: Tok) -> bool {
    use crate::{BOOK, BOOKPART, HEADER, LAYOUT, MARKUP, MARKUPLIST, MIDI, PAPER, SCORE};
    matches!(
        kind,
        Tok::String
            | Tok::Symbol
            | Tok::Fraction
            | Tok::Char(b'^' | b'_' | b':')
            | Tok::EUnsigned
            | Tok::Identifier(
                Id::Event
                    | Id::EventFunction
                    | Id::Book
                    | Id::Lookup
                    | Id::Module
                    | Id::ScmFunction
            )
            | MARKUP
            | MARKUPLIST
            | HEADER
            | SCORE
            | BOOK
            | BOOKPART
            | PAPER
            | LAYOUT
            | MIDI
            | crate::context::WITH
            | crate::context::CONTEXT
    )
}

impl Parser<'_, '_> {
    /// `start_symbol: EMBEDDED_LILY embedded_lilypond`, notes mode pushed:
    /// nothing, which is `unspecified_music`; a duration, with post-events
    /// the note of it; music, one item (a pitch alone is a pitch) or a
    /// list of several, sequential music; or what `identifier_init_nonumber`
    /// takes besides, as an assignment's value. A syntax error leaves the
    /// unspecified, and the parse failed.
    pub(crate) fn embedded_lilypond(&mut self) -> Value {
        self.lexer.note_token("#{");
        self.embedded_body()
    }

    /// `embedded_lilypond`, after `EMBEDDED_LILY` or `INVALID`.
    fn embedded_body(&mut self) -> Value {
        let first = self.peek(N);
        let value = match first.kind {
            Tok::Eof => Ok(self.make_at("Music", first.span, &[])),
            // `INVALID embedded_lilypond`.
            Tok::Invalid => {
                self.next(N);
                self.lexer.error_level = true;
                return self.embedded_body();
            }
            Tok::Unsigned | Tok::Identifier(Id::Duration) => self.embedded_duration(),
            Tok::ScmToken | Tok::Identifier(Id::Scm) => self.embedded_scheme(),
            Tok::Real | Tok::Identifier(Id::Number) => self.embedded_number(),
            // `'-' embedded_lilypond_number`, or a post-event: an `UNSIGNED`
            // after `-` is a fingering, no number.
            Tok::Char(b'-') => {
                let minus = self.next(N);
                match self.peek(N).kind {
                    Tok::Real | Tok::Identifier(Id::Number) | Tok::Char(b'-') => {
                        self.embedded_number().map(|n| self.negated(n))
                    }
                    Tok::Unsigned => match self.unit_number() {
                        Some(n) => Ok(self.negated(n)),
                        None => self.minus_post_events(minus).map(|(_, value)| value),
                    },
                    // `identifier_init_nonumber`'s `'-' partial_function
                    // ETC` too, as an assignment reads it (D93, OPEN F).
                    _ => self.partial_value(Some(minus)).map(|(_, value, _)| value),
                }
            }
            // `identifier_init_nonumber`'s `partial_function ETC`, or music.
            Tok::Identifier(Id::MusicFunction)
            | crate::property::OVERRIDE
            | crate::property::SET
            | crate::repeat::REPEAT => match self.partial_init() {
                Ok(crate::partial::Init::Music(music)) => self.embedded_list(alloc::vec![music], 1),
                Ok(crate::partial::Init::Partial(value, _)) => Ok(value),
                Ok(_) => unreachable!("music or a call cut short"),
                Err(e) => Err(e),
            },
            kind if starts_music(kind) => self.embedded_list(Vec::new(), 0),
            kind if starts_init(kind) => self.identifier_init().map(|(_, value, _)| value),
            _ => Err(self.syntax_error(&first, bison::MANY)),
        };
        match value {
            Ok(value) => {
                // `start_symbol` was reduced, by default, with its value:
                // what follows aborts the parse, its lookahead discarded.
                let end = self.peek(N);
                if end.kind != Tok::Eof {
                    self.syntax_error(&end, bison::END_OF_INPUT);
                    self.lexer.error_level = true;
                }
                value
            }
            // `error`.
            Err(crate::SyntaxError) => {
                self.lexer.error_level = true;
                self.resync(N, |kind| kind == Tok::Eof);
                Value::UNSPECIFIED
            }
        }
    }

    /// `embedded_lilypond_number`: `-` and one, negated, or
    /// `bare_number_common`: a `REAL` and its unit, or a `NUMBER_IDENTIFIER`.
    fn embedded_number(&mut self) -> crate::Parsed<Value> {
        let token = self.peek(N);
        match token.kind {
            Tok::Char(b'-') => {
                self.next(N);
                let n = self.embedded_number()?;
                Ok(self.negated(n))
            }
            Tok::Real => {
                self.next(N);
                let value = self.number_value(&token);
                Ok(self.bare_number(value))
            }
            Tok::Identifier(Id::Number) => {
                self.next(N);
                Ok(self.number_value(&token))
            }
            Tok::Unsigned => match self.unit_number() {
                Some(n) => Ok(n),
                None => {
                    let token = self.peek(N);
                    Err(self.syntax_error(&token, bison::MANY))
                }
            },
            _ => Err(self.syntax_error(&token, bison::MANY)),
        }
    }

    /// `UNSIGNED NUMBER_IDENTIFIER`, a number and its unit, at an
    /// `UNSIGNED`: their product. None when no `NUMBER_IDENTIFIER`
    /// follows, the `UNSIGNED` put back, to be read again as a duration or
    /// a fingering.
    fn unit_number(&mut self) -> Option<Value> {
        let token = self.next(N);
        if self.peek(N).kind != Tok::Identifier(Id::Number) {
            self.put_back(&[token]);
            return None;
        }
        let value = self.number_value(&token);
        Some(self.bare_number(value))
    }

    /// `duration post_events`: with post-events, a note of the duration,
    /// which becomes the default; else the duration. Music after it makes
    /// it the first of a list.
    fn embedded_duration(&mut self) -> crate::Parsed<Value> {
        let first = self.peek(N);
        if first.kind == Tok::Unsigned
            && let Some(n) = self.unit_number()
        {
            return Ok(n);
        }
        let duration = self.duration()?;
        let post = self.post_events()?;
        let rhythm = Music {
            span: self.span_from(first.span),
            kind: MusicKind::Rhythm {
                duration,
                post: post.clone(),
            },
        };
        if self.peek(N).kind != Tok::Eof {
            self.default_duration = duration;
            return self.embedded_list(alloc::vec![rhythm], 1);
        }
        if post.is_empty() {
            return Ok(self.duration_value(duration));
        }
        self.default_duration = duration;
        Ok(self.music_value(&rhythm))
    }

    /// `embedded_scm` alone, its value as it is; before more, the first of
    /// a list (`music_embedded_backup`).
    fn embedded_scheme(&mut self) -> crate::Parsed<Value> {
        let (value, span, placed) = self.embedded_scm_value()?;
        if self.peek(N).kind == Tok::Eof {
            return Ok(value);
        }
        let mut musics = Vec::new();
        match self.scheme_music(value, span, placed) {
            Some(music) => musics.push(music),
            None if value != Value::UNSPECIFIED => self.diagnose(
                crate::diag::Severity::Warning,
                crate::diag::At::Span(span),
                "Ignoring non-music expression",
            ),
            None => {}
        }
        self.embedded_list(musics, 1)
    }

    /// Music to the end of input, after the `parsed` items of `musics`: one
    /// item alone is `identifier_init_nonumber`'s (a pitch alone is a
    /// pitch); more are `music_embedded music_embedded music_list`, which
    /// is what of them is music: one is itself, several sequential music,
    /// none `unspecified_music`, and post-events alone their `PostEvents`
    /// (`reverse_music_list`'s `compress`).
    fn embedded_list(&mut self, mut musics: Vec<Music>, parsed: usize) -> crate::Parsed<Value> {
        let start = musics.first().map(|m| m.span);
        let first = self.peek(N);
        let mut parsed = parsed;
        // `music_list`, to a token that starts no music; an element that is
        // a syntax error is `music_list error`'s.
        loop {
            let token = self.peek(N);
            if !crate::notes::starts_music(token.kind) {
                break;
            }
            parsed += 1;
            match self.music_embedded() {
                Ok(music) => musics.extend(music),
                Err(crate::SyntaxError) => {
                    if musics.last().is_none_or(|m| m.kind != MusicKind::Error) {
                        musics.push(Music {
                            span: token.span,
                            kind: MusicKind::Error,
                        });
                    }
                    let follows = |kind| kind == Tok::Eof || crate::notes::starts_music(kind);
                    if !self.resync(N, follows) {
                        break;
                    }
                }
            }
        }
        let span = self.span_from(start.unwrap_or(first.span));
        Ok(match &musics[..] {
            [] => self.make_at("Music", span, &[]),
            [one] => match &one.kind {
                MusicKind::Note(note) if parsed == 1 && crate::assign::bare_pitch(note) => {
                    self.pitch_value(note.pitch)
                }
                _ => self.music_value(one),
            },
            _ if musics
                .iter()
                .all(|m| matches!(m.kind, MusicKind::PostEvent(_) | MusicKind::PostEvents(_))) =>
            {
                let events = musics
                    .into_iter()
                    .flat_map(|m| match m.kind {
                        MusicKind::PostEvent(event) => alloc::vec![event],
                        MusicKind::PostEvents(events) => events,
                        _ => Vec::new(),
                    })
                    .collect();
                self.held_events(events).1
            }
            _ => self.music_value(&Music {
                span,
                kind: MusicKind::Sequential(musics),
            }),
        })
    }
}
