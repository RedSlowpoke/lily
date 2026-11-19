//! The lexer (02 §2, §5.2): `lexer.ll`'s rules, written out with flex's
//! longest match and, on a tie, its first rule. A pull lexer: the parser
//! passes the mode of each token it asks for, so a mode switch always comes
//! before the next token is lexed.
//!
//! P1 and P2 so far: the rules every mode shares (white space and `\r`, the
//! UTF-8 byte order mark, `%` and `%{ %}` comments, strings and their escapes,
//! `\version`), `YYText_utf8`'s check of words, comments and strings, and the
//! rules that cut words, commands, numbers and special characters in the
//! notes, initial and markup modes; in notes mode, `r s R q` and the pitch
//! names of the current language (`scan_word`). `\sourcefilename` and
//! `\sourcefileline` (P12, MS8 step 10) rename the file its locations print
//! and number its lines; `\maininput`, which only init files may use, is
//! the engine's own start of the main file.
//!
//! P3: a `\name` that is no keyword, and a shorthand, are looked up in the
//! [`Scopes`] that [`Lexer::next_in`] is given and classified by their value
//! (`classify.rs`); unbound, they are an unknown command or an undefined
//! shorthand, returned as a string. [`Lexer::next`] has no scopes, as an
//! empty module.
//!
//! P4: `#` and `$` hand the text after them to the scopes' reader
//! (`scheme.rs`), which reads one form and gives where it ends. `#` is a
//! `SCM_TOKEN` that carries the form, for the grammar to evaluate; `$` is
//! evaluated at once and its value classified as a `\name`'s, an
//! unspecified value giving no token. Further values of either wait as extra
//! tokens, which come before the input's next (`push_extra_token`). Without
//! scopes, both are returned unread.
//!
//! P7: in markup mode, a `\name` is a markup command first, then a markup
//! list command, by the signature `markup-command-signature` gives
//! (`lookup_markup_command`), and so is a `$` form whose value is one.
//! The `markup?`, `markup-list?` and `scheme?` tokens it pushes are only
//! recorded: the parser reads the signature itself (`markup.rs`).
//!
//! The tokens Bison reads (MS2): with [`Lexer::record_tokens`], each token
//! the parser takes from the lexer is recorded by its name in Bison's
//! messages, as `-ddebug-parser` traces it (`tools/compare/tokens.mjs`): a
//! function's `EXPECT_SCM`, `EXPECT_OPTIONAL` and `EXPECT_NO_MORE_ARGS`
//! after it, as `scan_scm_id` pushes them, and a token the parser put back
//! (`MYBACKUP`, `cursor.rs`) marked `^` when it is read again. `REPARSE` and
//! `BACKUP`, which the parser pushes in LilyPond, are not recorded.
//!
//! `ly:parser-include-string` (P11, brought forward for T16): a string the
//! parser gives [`Lexer::include_string`] is lexed next, as
//! `<included string>`, and the input it interrupted goes on after its end,
//! which gives no token (`Includable_lexer::new_input`). Its source is
//! leaked, to be borrowed as long as the main file's; [`Lexer::included`]
//! gives them to [`crate::Parse::sources`]. P11's include stack owns them.
//!
//! `\include "file"` (MS2 step 16, brought forward from P11 for the init
//! files, and step 19 for scores): the file is looked for where LilyPond
//! looks (`source::include_candidates`), each name asked of the scopes
//! ([`Scopes::include_file`]), and the first they have is lexed next, as an
//! included string is, under that name. Scopes that give no files leave
//! `\include` an unknown command, as before. Since MS2 step 34 (P11) the
//! name may be a `\name` or a `#` or `$` form whose value is a string; one
//! that is none leaves the `incl` start condition pushed, as LilyPond's. A
//! file the host must be asked for first ([`Hosted::Ask`]), or one that
//! Scheme needed ([`Scopes::waiting`]), ends the input: the parse stops
//! ([`Lexer::waiting`]), to be made again once the host has answered.
//! Since MS7 step 7 (OPEN F, D263) the candidates after the first the host
//! must be asked for are asked at the same stop, up to one it has, so that
//! a file found in LilyPond's `ly/` costs one parse more, not one a
//! directory.
//!
//! P10: chord mode (`chords`), with the pitch names as `TONICNAME_PITCH`,
//! the words of `chordmodifiers` as `CHORD_MODIFIER`, its numbers without
//! `STRICTREAL` and its `- : / /+ ^`; figure mode (`figures`), with `< >`,
//! `_`, `\+`, `\!`, `\\`, `[ ]` and the alteration expressions of
//! figures, and no pitch names; and drum mode, which is notes mode with the
//! words of `drumPitchNames` as its only pitch names (`DRUM_PITCH`). Their
//! tables are looked up in the scopes when a word is lexed
//! ([`Scopes::chord_modifier`], [`Scopes::drum_pitch`]), where LilyPond
//! reads them when the mode is pushed.

use crate::classify::{Id, NoScopes, Scopes};
use crate::diag::{At, Diagnostics};
use crate::pitch::{Pitch, PitchNames};
use crate::source::{self, Source, Span};
use alloc::string::String;
use alloc::vec::Vec;
use lily_engine_scheme::Hosted;

/// The lexer's start conditions that P1 has rules for (02 §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Output definition bodies.
    Initial,
    /// The start of every file.
    Notes,
    Markup,
    /// `\lyricmode`'s and `\lyrics`', and the music of `\addlyrics` and
    /// `\lyricsto` (P8).
    Lyrics,
    /// `\chordmode`'s and `\chords`' (P10).
    Chords,
    /// `\figuremode`'s and `\figures`'.
    Figures,
    /// `\drummode`'s and `\drums`': notes mode with the drum pitch names.
    Drums,
}

/// The keywords of `lily-lexer.cc`'s table; [`Tok::Keyword`] is an index.
pub const KEYWORDS: [&str; 45] = [
    "accepts",
    "addlyrics",
    "alias",
    "alternative",
    "book",
    "bookpart",
    "change",
    "chordmode",
    "chords",
    "consists",
    "context",
    "default",
    "defaultchild",
    "denies",
    "description",
    "drummode",
    "drums",
    "etc",
    "figuremode",
    "figures",
    "header",
    "layout",
    "lyricmode",
    "lyrics",
    "lyricsto",
    "markup",
    "markuplist",
    "midi",
    "name",
    "new",
    "notemode",
    "override",
    "paper",
    "remove",
    "repeat",
    "rest",
    "revert",
    "score",
    "sequential",
    "set",
    "simultaneous",
    "tempo",
    "type",
    "unset",
    "with",
];

/// The index in [`KEYWORDS`] of `name`.
pub const fn keyword(name: &str) -> u8 {
    let mut i = 0;
    while i < KEYWORDS.len() {
        if KEYWORDS[i].len() == name.len() && bytes_eq(KEYWORDS[i].as_bytes(), name.as_bytes()) {
            return i as u8;
        }
        i += 1;
    }
    panic!("not a keyword")
}

const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// A token's kind: `parser.yy`'s token, by the name its messages use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Eof,
    Keyword(u8),
    /// `\score-lines`, a keyword in markup mode only.
    ScoreLines,
    /// A quoted string, or a name the lexer could not classify: its text is [`Lexer::string`].
    String,
    /// A bare word; in lyrics mode, a syllable, whose text is
    /// [`Lexer::string`].
    Symbol,
    /// `--` in lyrics mode (`HYPHEN`).
    Hyphen,
    /// `__` in lyrics mode (`EXTENDER`).
    Extender,
    /// A pitch name of the current language, in notes mode: its value is the
    /// [`Pitch`], read with [`Lexer::pitch`].
    NotenamePitch,
    /// `r` or `s`, in notes mode: its value is the letter.
    RestName,
    /// A pitch name in chord mode: its value is the [`Pitch`], as
    /// [`Tok::NotenamePitch`]'s.
    TonicnamePitch,
    /// A drum pitch name in drum mode: its value is the index of its drum
    /// type's symbol in the scopes.
    DrumPitch,
    /// A word of `chordmodifiers` in chord mode: its value is the index of
    /// its procedure in the scopes.
    ChordModifier,
    /// `:`, `-`, `^`, `/` and `/+` in chord mode.
    ChordColon,
    ChordMinus,
    ChordCaret,
    ChordSlash,
    ChordBass,
    /// `<`, `>` and `_` in figure mode.
    FigureOpen,
    FigureClose,
    FigureSpace,
    /// `\+`, `\!` and `\\` in figure mode.
    EPlus,
    EExclamation,
    EBackslash,
    /// `+`, `-` and `!` in figure mode, with white space before, among and
    /// after them, or in `[ ]`: its text is the token's span.
    FigureAlteration,
    /// `q`, in notes mode.
    ChordRepetition,
    /// `R`, in notes mode.
    MultiMeasureRest,
    Unsigned,
    /// `\1`, a string number.
    EUnsigned,
    Fraction,
    Real,
    /// A character the lexer returns as itself, `'.'`.
    Char(u8),
    AngleOpen,
    AngleClose,
    DoubleAngleOpen,
    DoubleAngleClose,
    /// `#` and the form after it: its value is the form's index in the
    /// scopes ([`crate::Env`]), or [`NO_FORM`] when none was read.
    ScmToken,
    /// `$`, unread: lexed without scopes.
    ScmImmediate,
    /// An argument's value put back for the next parameter (`SCM_ARG`,
    /// `cursor.rs`): its value is the index [`crate::Env::value`] takes.
    ScmArg,
    /// Bison's `BACKUP`, which only an expected set names: never lexed.
    Backup,
    /// A `\name` or a shorthand bound in the scopes, by its value's kind:
    /// its value is the index [`crate::Env::value`] takes.
    Identifier(Id),
    /// `\version` with a version that is no good.
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Tok,
    /// Its text, from its first byte to its last.
    pub span: Span,
    /// Where LilyPond locates it: flex's `lexloc_` when the token is
    /// returned, the last thing matched. A string's closing quote; for the
    /// end of input, whatever was matched last, white space included.
    pub loc: Span,
    /// For [`Tok::String`] and a syllable's [`Tok::Symbol`], its index in
    /// the lexer's strings; for
    /// [`Tok::NotenamePitch`] and [`Tok::RestName`], what they name; for
    /// [`Tok::Identifier`], its value's index in the scopes.
    pub value: u32,
}

/// The value of a [`Tok::ScmToken`] that holds no form: a read error, the
/// end of input after `#`, or no scopes to read with.
pub const NO_FORM: u32 = u32::MAX;

/// What `\version` left in the main file (init.ly's `version-seen`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionSeen {
    No,
    /// Found, but no version this LilyPond accepts.
    Invalid,
    Valid(String),
}

/// The LilyPond whose behaviour the parser reproduces (`(ly:version)`).
pub const LILYPOND_VERSION: &str = "2.26.0";
const VERSION_NUMBERS: [Ratio; 3] = [Ratio(2, 1), Ratio(26, 1), Ratio(0, 1)];

const SPECIAL: &[u8] = b"-+*/=<>{}!?_^',.:";
const ESCAPED: &[u8] = b"nt\\'\"";
const BOM: &[u8] = b"\xef\xbb\xbf";

fn is_letter(b: u8) -> bool {
    b.is_ascii_alphabetic() || b >= 0x80
}

fn is_white(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\t' | b'\x0c' | b'\r')
}

pub struct Lexer<'a> {
    src: &'a Source,
    file: u16,
    pos: usize,
    /// flex's `lexloc_`: what was matched last.
    last: (usize, usize),
    strings: Vec<String>,
    /// The table of the current language, the top of `pitchname_tab_stack_`.
    pitch_names: Option<&'a PitchNames>,
    pub diags: Diagnostics,
    pub version: VersionSeen,
    /// `error_level_`: the file failed, with or without a message.
    pub error_level: bool,
    /// `extra_tokens_`, the last to come first, each with whether the
    /// parser put it back.
    extra: Vec<(Token, bool)>,
    /// The names of the tokens read, when they are recorded.
    tokens: Option<Vec<String>>,
    /// The main file's index; the strings included are numbered after it.
    main: (&'a Source, u16),
    /// The strings included, by their index after the main file's: leaked,
    /// so that a parse started again can lex after them (`reuse.rs`).
    included: Vec<&'static Source>,
    /// The inputs an included string interrupted: source, index, position
    /// and what was matched last, the innermost last.
    outer: Vec<(&'a Source, u16, usize, (usize, usize))>,
    /// The input ended for a file the host must be asked for.
    pub waiting: bool,
    /// How many names of tokens the last token lexed pushed were recorded
    /// after its own.
    pushed_names: usize,
    /// The names a lookahead put back pushed, by its span, recorded again
    /// when it is read again, as Bison reads them after it.
    deferred: Vec<(Span, Vec<String>)>,
    /// The `incl` start condition, which an `\include` whose name was no
    /// string leaves pushed: only a name, white space and comments are
    /// lexed in it, and the input ends unfinished.
    incl: bool,
    /// The furthest byte of the main file it or the reader looked at: past
    /// a form whose read failed, the end, where the reader may have looked
    /// (`reuse.rs`'s boundaries).
    seen: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a Source, file: u16) -> Lexer<'a> {
        Lexer {
            src,
            file,
            pos: 0,
            last: (0, 0),
            strings: Vec::new(),
            pitch_names: None,
            diags: Diagnostics::default(),
            version: VersionSeen::No,
            error_level: false,
            extra: Vec::new(),
            tokens: None,
            main: (src, file),
            included: Vec::new(),
            outer: Vec::new(),
            waiting: false,
            pushed_names: 0,
            deferred: Vec::new(),
            incl: false,
            seen: 0,
        }
    }

    /// `include_string`: `text` is lexed next, before the rest of the
    /// current input, as `<included string>`.
    pub fn include_string(&mut self, text: String) {
        self.include_source("<included string>", text.into_bytes());
    }

    /// `new_input`: `text` is lexed next, before the rest of the current
    /// input, as the source `name`.
    fn include_source(&mut self, name: &str, text: Vec<u8>) {
        let src: &'static Source =
            alloc::boxed::Box::leak(alloc::boxed::Box::new(Source::new(name, text)));
        self.included.push(src);
        let file = self.main.1 + self.included.len() as u16;
        self.outer.push((self.src, self.file, self.pos, self.last));
        (self.src, self.file, self.pos, self.last) = (src, file, 0, (0, 0));
    }

    /// The strings included, in the order of their indexes after the main
    /// file's.
    pub fn included(&self) -> &[&'static Source] {
        &self.included
    }

    /// Where a parse can start again (02 §5.7, `reuse.rs`): the position
    /// and what was matched last, when the main file is being read with no
    /// string included in it unfinished, no token pushed, and nothing that
    /// ends the input waiting.
    pub(crate) fn boundary(&self) -> Option<(usize, (usize, usize))> {
        let clean = self.outer.is_empty()
            && self.file == self.main.1
            && self.extra.is_empty()
            && self.deferred.is_empty()
            && !self.incl
            && !self.waiting
            && self.tokens.is_none();
        clean.then_some((self.pos, self.last))
    }

    /// The last byte of the main file examined: past what it and the
    /// reader looked at, the byte after, which deciding where a token ends
    /// looks at (`reuse.rs`).
    pub(crate) fn examined(&self) -> usize {
        self.seen.max(self.pos) + 1
    }

    /// How many strings tokens carry: a braced block that adds none has
    /// none of their indexes in its music (`reuse.rs`).
    pub(crate) fn strings_len(&self) -> usize {
        self.strings.len()
    }

    /// Where it stands in the main file: its position and what was matched
    /// last, for [`Lexer::seek`].
    pub(crate) fn place(&self) -> (usize, (usize, usize)) {
        (self.pos, self.last)
    }

    /// Goes on at `pos` of the file it reads, `last` matched last: past a
    /// braced block taken from the last parse (`reuse.rs`).
    pub(crate) fn seek(&mut self, pos: usize, last: (usize, usize)) {
        (self.pos, self.last) = (pos, last);
        self.seen = self.seen.max(pos);
    }

    /// Starts lexing again at `pos` of the main file, `last` matched last,
    /// after the strings `included` of the parse it starts again.
    pub(crate) fn restart(
        &mut self,
        pos: usize,
        last: (usize, usize),
        included: &[&'static Source],
    ) {
        (self.pos, self.last) = (pos, last);
        self.seen = pos;
        self.included = included.to_vec();
    }

    /// The source of index `file`: the main file or a string included.
    /// The number of the source being read.
    pub fn file(&self) -> u16 {
        self.file
    }

    /// The span of `input` in the sources read: None when it is in no file
    /// of its number and name, as for music another parse made (D69).
    pub fn span_of(&self, input: &lily_engine_music::Input) -> Option<Span> {
        let src = match input.file.checked_sub(self.main.1)? {
            0 => self.main.0,
            i => *self.included.get(i as usize - 1)?,
        };
        src.is_named(&input.name).then_some(Span {
            file: input.file,
            start: input.start,
            end: input.end,
        })
    }

    pub fn source_of(&self, file: u16) -> &'a Source {
        match file.checked_sub(self.main.1 + 1) {
            Some(i) => self.included[i as usize],
            None => self.main.0,
        }
    }

    /// The source it lexes and its index.
    pub fn source(&self) -> (&'a Source, u16) {
        (self.src, self.file)
    }

    /// `push_extra_token`: a token of `kind` carrying `value`, located at
    /// what was lexed last, to come before the input's next.
    pub fn push_extra(&mut self, kind: Tok, value: u32) {
        let at = self.span(self.last);
        let token = Token {
            kind,
            span: at,
            loc: at,
            value,
        };
        self.extra.push((token, false));
    }

    /// `push_extra_token` of a token made already: it comes before the
    /// input's next, and before the tokens pushed earlier.
    pub(crate) fn push_token(&mut self, token: Token) {
        self.extra.push((token, true));
    }

    /// [`Lexer::push_token`] of the last token lexed, which the parser
    /// looked at and puts back: the names of what it pushed are recorded
    /// when it is read again, not before what is read until then.
    pub(crate) fn push_lookahead(&mut self, token: Token) {
        if let Some(tokens) = &mut self.tokens
            && self.pushed_names > 0
        {
            let names = tokens.split_off(tokens.len() - self.pushed_names.min(tokens.len()));
            self.deferred.push((token.span, names));
        }
        self.pushed_names = 0;
        self.push_token(token);
    }

    /// Whether tokens were pushed that are still to come: their values are
    /// kept already ([`crate::Env::release`]).
    pub(crate) fn has_extra(&self) -> bool {
        !self.extra.is_empty()
    }

    /// Records the tokens read from now on.
    pub fn record_tokens(&mut self) {
        self.tokens.get_or_insert_with(Vec::new);
    }

    /// The tokens read since [`Lexer::record_tokens`], one name each.
    pub fn take_tokens(&mut self) -> Option<Vec<String>> {
        self.tokens.take()
    }

    /// Whether it records the tokens it reads.
    pub(crate) fn records_tokens(&self) -> bool {
        self.tokens.is_some()
    }

    /// Records a token the parser reads that no input has, by its name:
    /// `EMBEDDED_LILY`'s `#{` (`embedded.rs`).
    pub(crate) fn note_token(&mut self, name: &str) {
        if let Some(tokens) = &mut self.tokens {
            tokens.push(name.into());
        }
    }

    /// Records the tokens another parse read, if it records any.
    pub(crate) fn extend_tokens(&mut self, more: Vec<String>) {
        if let Some(tokens) = &mut self.tokens {
            tokens.extend(more);
        }
    }

    /// Records the tokens another parse read while the last token lexed
    /// was not yet read by the parser: before the names of the tokens it
    /// pushed, which Bison reads after it (`scheme?`, `EXPECT_NO_MORE_ARGS`).
    pub(crate) fn insert_tokens(&mut self, more: Vec<String>) {
        if let Some(tokens) = &mut self.tokens {
            let at = tokens.len() - self.pushed_names.min(tokens.len());
            tokens.splice(at..at, more);
        }
    }

    /// The table of the current language.
    pub(crate) fn pitch_names(&self) -> Option<&'a PitchNames> {
        self.pitch_names
    }

    /// A string of [`Tok::String`] made for a token the lexer did not lex.
    pub(crate) fn add_string(&mut self, text: String) -> u32 {
        self.strings.push(text);
        (self.strings.len() - 1) as u32
    }

    /// The first name of a pitch's step and alteration in the current
    /// language.
    pub(crate) fn name_of_pitch(&self, step: u8, alteration: i8) -> Option<&'a [u8]> {
        self.pitch_names?.name_of(step, alteration)
    }

    /// The pitch a word names in the current language (`scan_word`).
    pub(crate) fn pitch_name(&self, name: &[u8]) -> Option<Pitch> {
        self.pitch_names?.get(name)
    }

    /// The text of a [`Tok::String`].
    pub fn string(&self, token: &Token) -> &str {
        &self.strings[token.value as usize]
    }

    /// The pitch of a [`Tok::NotenamePitch`].
    pub fn pitch(&self, token: &Token) -> Pitch {
        Pitch::unpack(token.value)
    }

    /// The bytes in `span`, of the main file or a string included.
    pub fn text_of(&self, span: Span) -> &'a [u8] {
        &self.source_of(span.file).text()[span.start as usize..span.end as usize]
    }

    /// Makes `names` the pitch names of notes mode; with none, every bare
    /// word but `r s R q` is a [`Tok::Symbol`] there.
    pub fn set_pitch_names(&mut self, names: &'a PitchNames) {
        self.pitch_names = Some(names);
    }

    fn text(&self) -> &'a [u8] {
        self.src.text()
    }

    fn span(&self, (start, end): (usize, usize)) -> Span {
        Span {
            file: self.file,
            start: start as u32,
            end: end as u32,
        }
    }

    /// flex's `YY_USER_ACTION`: the text from `start` to `end` is matched.
    fn matched(&mut self, start: usize, end: usize) {
        self.last = (start, end);
        self.pos = end;
        if self.file == self.main.1 {
            self.seen = self.seen.max(end);
        }
    }

    /// A form read to `end`, or, if its read failed, read where the reader
    /// may have looked: to the end of the text.
    fn read_to(&mut self, read: bool, end: usize) {
        if self.file == self.main.1 {
            let end = if read { end } else { self.text().len() };
            self.seen = self.seen.max(end);
        }
    }

    /// `LexerError`: at what was matched last.
    fn error(&mut self, message: impl Into<String>) {
        let at = At::Span(self.span(self.last));
        self.diags.error(at, message);
    }

    fn token(&self, kind: Tok, start: usize, value: u32) -> Token {
        Token {
            kind,
            span: self.span((start, self.pos)),
            loc: self.span(self.last),
            value,
        }
    }

    fn string_token(&mut self, start: usize, text: String) -> Token {
        self.strings.push(text);
        self.token(Tok::String, start, (self.strings.len() - 1) as u32)
    }

    /// The next token, lexed in `mode` with no scopes.
    pub fn next(&mut self, mode: Mode) -> Token {
        self.next_in(mode, &mut NoScopes)
    }

    /// The next token, lexed in `mode`, with names looked up in `scopes`.
    pub fn next_in(&mut self, mode: Mode, scopes: &mut dyn Scopes) -> Token {
        let (token, again) = match self.extra.pop() {
            Some(extra) => extra,
            None => (self.lex(mode, scopes), false),
        };
        if let Some(tokens) = &mut self.tokens {
            tokens.push(match again {
                true => alloc::format!("^{}", token.kind.display()),
                false => token.kind.display(),
            });
            let lexed = tokens.len();
            if again
                && let Some(at) = self
                    .deferred
                    .iter()
                    .position(|(span, _)| *span == token.span)
            {
                let (_, names) = self.deferred.remove(at);
                tokens.extend(names);
                self.pushed_names = tokens.len() - lexed;
                return token;
            }
            let function = matches!(
                token.kind,
                Tok::Identifier(Id::MusicFunction | Id::EventFunction | Id::ScmFunction)
            );
            if function
                && !again
                && let Some(optional) = scopes.parameters(token.value)
            {
                // `scan_scm_id` pushes them in the signature's order, so they
                // come out the last parameter's first.
                for optional in optional.into_iter().rev() {
                    if optional {
                        tokens.push("optional?".into());
                    }
                    tokens.push("scheme?".into());
                }
                tokens.push("EXPECT_NO_MORE_ARGS".into());
            }
            let markup = matches!(
                token.kind,
                Tok::Identifier(Id::MarkupFunction | Id::MarkupListFunction)
            );
            if markup && !again {
                // `push_markup_predicates`: the last parameter's first.
                for expect in scopes.markup_expects(token.value) {
                    tokens.push(expect.into());
                }
                tokens.push("EXPECT_NO_MORE_ARGS".into());
            }
            self.pushed_names = tokens.len() - lexed;
        }
        token
    }

    /// The next token of the input, extra tokens aside.
    fn lex(&mut self, mode: Mode, scopes: &mut dyn Scopes) -> Token {
        if self.waiting || scopes.waiting() {
            self.waiting = true;
            return self.token(Tok::Eof, self.pos, 0);
        }
        let text = self.text();
        loop {
            let start = self.pos;
            let Some(&b) = text.get(start) else {
                // The end of an included string goes back to what it interrupted.
                if let Some(outer) = self.outer.pop() {
                    (self.src, self.file, self.pos, self.last) = outer;
                    return self.lex(mode, scopes);
                }
                if core::mem::take(&mut self.incl) {
                    self.error("Unfinished main input");
                }
                return self.token(Tok::Eof, start, 0);
            };
            let next = text.get(start + 1).copied();
            if mode == Mode::Figures
                && let Some(end) = self.figure_alteration_end(start)
            {
                // `{FIG_ALT_EXPR}` is longer than `{WHITE}+`, or ties with
                // the special characters and comes first.
                self.matched(start, end);
                return self.token(Tok::FigureAlteration, start, 0);
            }
            if is_white(b) {
                let end = start
                    + text[start..]
                        .iter()
                        .position(|&c| !is_white(c))
                        .unwrap_or(text.len() - start);
                self.matched(start, end);
                continue;
            }
            if mode != Mode::Markup && text[start..].starts_with(BOM) {
                self.matched(start, start + 3);
                let c = self.src.counts(start as u32);
                if c.line != 1 || c.column != 0 {
                    let at = At::Span(self.span(self.last));
                    self.diags.warning(at, "stray UTF-8 BOM encountered");
                }
                continue;
            }
            match b {
                b'%' if next == Some(b'{') => {
                    self.matched(start, start + 2);
                    self.long_comment();
                    continue;
                }
                b'%' => {
                    match next {
                        Some(b'\n' | b'\r') => self.matched(start, start + 2),
                        None => self.matched(start, start + 1),
                        Some(_) => {
                            let eol = text[start..].iter().position(|&c| c == b'\n' || c == b'\r');
                            let end = eol.map_or(text.len(), |n| start + n + 1);
                            self.matched(start, end);
                            self.check_utf8(start, end);
                        }
                    }
                    continue;
                }
                _ if self.incl => {
                    if matches!(b, b'"' | b'\\' | b'#' | b'$')
                        && let Some(token) = self.include(mode, scopes)
                    {
                        return token;
                    }
                    // `<*>.`.
                    let end = start
                        + 1
                        + text[start + 1..]
                            .iter()
                            .take_while(|&&c| c & 0xc0 == 0x80)
                            .count();
                    self.matched(start, end);
                    self.check_utf8(start, end);
                    let shown = String::from_utf8_lossy(&text[start..end]).into_owned();
                    self.error(alloc::format!("invalid character: `{shown}'"));
                    return self.token(Tok::Char(b'%'), start, 0);
                }
                b'"' => {
                    self.matched(start, start + 1);
                    match self.quote(start, false, scopes) {
                        Some(token) => return token,
                        None => continue,
                    }
                }
                b'#' => {
                    self.matched(start, start + 1);
                    let Some((form, end)) =
                        scopes.read(self.src, self.file, start + 1, &mut self.diags)
                    else {
                        return self.token(Tok::ScmToken, start, NO_FORM);
                    };
                    // `skip_chars`: the form is passed, and the token is
                    // still located at `#`.
                    self.pos = end;
                    self.read_to(form.is_some(), end);
                    self.error_level |= form.is_none();
                    return self.token(Tok::ScmToken, start, form.unwrap_or(NO_FORM));
                }
                b'$' => {
                    self.matched(start, start + 1);
                    let Some((form, end)) =
                        scopes.read(self.src, self.file, start + 1, &mut self.diags)
                    else {
                        return self.token(Tok::ScmImmediate, start, 0);
                    };
                    self.pos = end;
                    self.read_to(form.is_some(), end);
                    let values =
                        scopes.immediate(self.src, self.file, form, start + 1, &mut self.diags);
                    // A template's tokens come before the `$` form's.
                    let templates = scopes.template_tokens();
                    self.extend_tokens(templates);
                    self.error_level |= values.is_none();
                    let values = values.unwrap_or_default();
                    for &(id, value) in values.iter().skip(1).rev().flatten() {
                        self.push_extra(Tok::Identifier(id), value);
                    }
                    let token = match values.first() {
                        // A procedure with a markup signature, in markup mode.
                        Some(&Some((Id::Scm, value))) if mode == Mode::Markup => {
                            let id = scopes.markup_function(value).unwrap_or(Id::Scm);
                            Some(self.token(Tok::Identifier(id), start, value))
                        }
                        Some(&Some((id, value))) => {
                            Some(self.token(Tok::Identifier(id), start, value))
                        }
                        _ => None,
                    };
                    let included = scopes.included_strings();
                    if included.is_empty() {
                        match token {
                            Some(token) => return token,
                            None => continue,
                        }
                    }
                    for text in included {
                        self.include_string(text);
                    }
                    return token.unwrap_or_else(|| self.lex(mode, scopes));
                }
                _ => {}
            }
            let token = match mode {
                Mode::Markup => self.markup(start, scopes),
                Mode::Lyrics => self.lyrics(start, scopes),
                _ => self.notes(start, mode, scopes),
            };
            if let Some(token) = token {
                return token;
            }
        }
    }

    /// The end of the word `{SYMBOL}` at `i`, letters with single `-` or `_`
    /// between them, or `i` if none starts there.
    fn symbol_end(&self, mut i: usize) -> usize {
        let text = self.text();
        if !text.get(i).is_some_and(|&b| is_letter(b)) {
            return i;
        }
        i += 1;
        loop {
            match text.get(i).copied() {
                Some(b) if is_letter(b) => i += 1,
                Some(b'-' | b'_') if text.get(i + 1).is_some_and(|&b| is_letter(b)) => i += 2,
                _ => return i,
            }
        }
    }

    fn digits_end(&self, i: usize) -> usize {
        i + self.text()[i..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    }

    /// The end of `{FIG_ALT_EXPR}` at `i`, white space, one of `+ - !`, and
    /// more of them and of white space; or of `\[{FIG_ALT_EXPR}\]`.
    fn figure_alteration_end(&self, i: usize) -> Option<usize> {
        let text = self.text();
        if text.get(i) == Some(&b'[') {
            let end = self.bare_alteration_end(i + 1)?;
            return (text.get(end) == Some(&b']')).then_some(end + 1);
        }
        self.bare_alteration_end(i)
    }

    /// The end of `{FIG_ALT_EXPR}` at `i`, which has no brackets.
    fn bare_alteration_end(&self, i: usize) -> Option<usize> {
        let text = self.text();
        let symbol = |b: u8| matches!(b, b'+' | b'-' | b'!');
        let first = i + text[i..].iter().take_while(|&&c| is_white(c)).count();
        if !text.get(first).is_some_and(|&b| symbol(b)) {
            return None;
        }
        Some(
            first
                + text[first..]
                    .iter()
                    .take_while(|&&c| symbol(c) || is_white(c))
                    .count(),
        )
    }

    /// The rules at a `\` that notes, initial, chord, figure, drum and
    /// lyrics mode share: `\version`, `\"…"` (a command's name quoted) and
    /// `{COMMAND}`, `\include` among them; None if none matches there.
    fn command(
        &mut self,
        start: usize,
        mode: Mode,
        scopes: &mut dyn Scopes,
    ) -> Option<Option<Token>> {
        let text = self.text();
        let command = self.symbol_end(start + 1);
        if text[start + 1..].starts_with(b"version") {
            let white = start
                + 8
                + text[start + 8..]
                    .iter()
                    .take_while(|&&c| is_white(c))
                    .count();
            if white >= command {
                self.matched(start, white);
                return Some(self.version());
            }
        }
        // `\\sourcefilename{WHITE}*` and `\\sourcefileline{WHITE}*` (P12).
        for (word, line) in [
            (&b"sourcefilename"[..], false),
            (&b"sourcefileline"[..], true),
        ] {
            if !text[start + 1..].starts_with(word) {
                continue;
            }
            let after = start + 1 + word.len();
            let white = after + text[after..].iter().take_while(|&&c| is_white(c)).count();
            if white >= command {
                self.matched(start, white);
                if line {
                    self.source_file_line();
                } else {
                    self.source_file_name(scopes);
                }
                return Some(None);
            }
        }
        if text.get(start + 1) == Some(&b'"') {
            self.matched(start, start + 2);
            return Some(self.quote(start, true, scopes));
        }
        if command > start + 1 {
            self.matched(start, command);
            self.check_utf8(start, command);
            let name = String::from_utf8_lossy(&text[start + 1..command]).into_owned();
            // `\\maininput`: the main file is begun by the engine itself, so
            // only an init file may begin it (P12).
            if name == "maininput" {
                self.error("\\maininput not allowed outside init files");
                return Some(None);
            }
            if name == "include"
                && let Some(token) = self.include(mode, scopes)
            {
                return Some(Some(token));
            }
            return Some(Some(self.escaped_word(start, name, scopes)));
        }
        None
    }

    /// The rules of notes, initial, chord, figure and drum mode, after the
    /// shared ones.
    fn notes(&mut self, start: usize, mode: Mode, scopes: &mut dyn Scopes) -> Option<Token> {
        let text = self.text();
        let b = text[start];
        let next = text.get(start + 1).copied();
        if b == b'\\' {
            if let Some(token) = self.command(start, mode, scopes) {
                return token;
            }
            if mode == Mode::Figures {
                let kind = match next {
                    Some(b'+') => Some(Tok::EPlus),
                    Some(b'!') => Some(Tok::EExclamation),
                    Some(b'\\') => Some(Tok::EBackslash),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.matched(start, start + 2);
                    return Some(self.token(kind, start, 0));
                }
            }
            let digits = self.digits_end(start + 1);
            if matches!(mode, Mode::Notes | Mode::Drums | Mode::Figures) && digits > start + 1 {
                self.matched(start, digits);
                return Some(self.token(Tok::EUnsigned, start, 0));
            }
            let end = if next.is_some_and(|c| c != b'\n') {
                start + 2
            } else {
                start + 1
            };
            return Some(self.shorthand(start, end, scopes));
        }
        if is_letter(b) {
            let end = self.symbol_end(start);
            self.matched(start, end);
            self.check_utf8(start, end);
            if mode == Mode::Initial {
                return Some(self.token(Tok::Symbol, start, 0));
            }
            // `{RESTNAME}`, `q` and `R` come before `{SYMBOL}`, and their
            // pseudo backup rules tie with `{SYMBOL}/[-_]`: one letter alone
            // is theirs.
            let (kind, value) = match &text[start..end] {
                b"r" | b"s" => (Tok::RestName, u32::from(b)),
                b"q" => (Tok::ChordRepetition, 0),
                b"R" => (Tok::MultiMeasureRest, 0),
                word => self.scan_word(word, mode, scopes),
            };
            return Some(self.token(kind, start, value));
        }
        if let Some(token) = self.number(start, mode) {
            return Some(token);
        }
        let (kind, len) = match (mode, b, next) {
            (Mode::Figures, b'<', _) => (Tok::FigureOpen, 1),
            (Mode::Figures, b'>', _) => (Tok::FigureClose, 1),
            (Mode::Figures, b'_', _) => (Tok::FigureSpace, 1),
            (Mode::Figures, b'[' | b']', _) => (Tok::Char(b), 1),
            (Mode::Chords, b'-', _) => (Tok::ChordMinus, 1),
            (Mode::Chords, b':', _) => (Tok::ChordColon, 1),
            (Mode::Chords, b'/', Some(b'+')) => (Tok::ChordBass, 2),
            (Mode::Chords, b'/', _) => (Tok::ChordSlash, 1),
            (Mode::Chords, b'^', _) => (Tok::ChordCaret, 1),
            _ => match (b, next) {
                (b'<', Some(b'<')) => (Tok::DoubleAngleOpen, 2),
                (b'>', Some(b'>')) => (Tok::DoubleAngleClose, 2),
                (b'<', _) => (Tok::AngleOpen, 1),
                (b'>', _) => (Tok::AngleClose, 1),
                _ if SPECIAL.contains(&b) => (Tok::Char(b), 1),
                _ => return Some(self.shorthand(start, start + 1, scopes)),
            },
        };
        self.matched(start, start + len);
        Some(self.token(kind, start, 0))
    }

    /// `scan_word` of a word in notes, chord, figure or drum mode: a pitch
    /// name of the table on top, in notes and chord mode the current
    /// language's, in drum mode `drumPitchNames`; in chord mode, else a
    /// chord modifier; else a [`Tok::Symbol`].
    fn scan_word(&mut self, word: &[u8], mode: Mode, scopes: &mut dyn Scopes) -> (Tok, u32) {
        let name = || String::from_utf8_lossy(word);
        match mode {
            Mode::Notes | Mode::Chords => {
                if let Some(pitch) = self.pitch_names.and_then(|names| names.get(word)) {
                    let kind = match mode {
                        Mode::Notes => Tok::NotenamePitch,
                        _ => Tok::TonicnamePitch,
                    };
                    return (kind, pitch.pack());
                }
                if mode == Mode::Chords
                    && let Some(value) = scopes.chord_modifier(&name())
                {
                    return (Tok::ChordModifier, value);
                }
            }
            Mode::Drums => {
                if let Some(value) = scopes.drum_pitch(&name()) {
                    return (Tok::DrumPitch, value);
                }
            }
            _ => {}
        }
        (Tok::Symbol, 0)
    }

    /// The rules of lyrics mode (P8), after the shared ones: numbers
    /// (`{FRACTION}`, `{STRICTREAL}`, `{UNSIGNED}`), `\version`, `\"…"`,
    /// commands with `\include`, shorthands, `* . = { }`, `<<` and `>>`
    /// alone, and a word of anything else up to white space, a digit or
    /// `$ # { } " \`, whose `_` are spaces (`lyric_fudge`), as a
    /// [`Tok::Symbol`] with that text; `__` and `--` are the extender and
    /// the hyphen. A word is any byte past ASCII too, so that non-ASCII
    /// letters are words as LilyPond's `[^…]` takes them, checked as UTF-8.
    fn lyrics(&mut self, start: usize, scopes: &mut dyn Scopes) -> Option<Token> {
        let text = self.text();
        let b = text[start];
        if b.is_ascii_digit()
            && let Some(token) = self.number(start, Mode::Notes)
        {
            return Some(token);
        }
        if b == b'\\' {
            if let Some(token) = self.command(start, Mode::Lyrics, scopes) {
                return token;
            }
            let end = if text.get(start + 1).is_some_and(|&c| c != b'\n') {
                start + 2
            } else {
                start + 1
            };
            return Some(self.shorthand(start, end, scopes));
        }
        if b == b'|' {
            return Some(self.shorthand(start, start + 1, scopes));
        }
        if matches!(b, b'*' | b'.' | b'=' | b'{' | b'}') {
            self.matched(start, start + 1);
            return Some(self.token(Tok::Char(b), start, 0));
        }
        let stops = |c: u8| {
            is_white(c)
                || c.is_ascii_digit()
                || matches!(c, b'$' | b'#' | b'{' | b'}' | b'"' | b'\\')
        };
        let end = start + 1 + text[start + 1..].iter().take_while(|&&c| !stops(c)).count();
        self.matched(start, end);
        self.check_utf8(start, end);
        // `\<\<` and `\>\>` tie with a word of two and come first.
        let kind = match &text[start..end] {
            b"__" => Tok::Extender,
            b"--" => Tok::Hyphen,
            b"<<" => Tok::DoubleAngleOpen,
            b">>" => Tok::DoubleAngleClose,
            word => {
                let word = String::from_utf8_lossy(word).replace('_', " ");
                let token = self.string_token(start, word);
                return Some(Token {
                    kind: Tok::Symbol,
                    ..token
                });
            }
        };
        Some(self.token(kind, start, 0))
    }

    /// `{FRACTION}`, `{STRICTREAL}` and `{UNSIGNED}` in notes mode, and
    /// all but `{STRICTREAL}` in chord mode;
    /// `{FRACTION}`, `{REAL}`, `-{UNSIGNED}` and `{UNSIGNED}` in initial mode.
    fn number(&mut self, start: usize, mode: Mode) -> Option<Token> {
        let text = self.text();
        let at = |i: usize| text.get(i).copied();
        let unsigned = self.digits_end(start);
        let mut best = (Tok::Unsigned, unsigned);
        if unsigned > start {
            if at(unsigned) == Some(b'/') {
                let den = self.digits_end(unsigned + 1);
                if den > unsigned + 1 {
                    best = (Tok::Fraction, den);
                }
            }
            // Chord mode has no `{STRICTREAL}`: `5.3` is two steps.
            if at(unsigned) == Some(b'.') && mode != Mode::Chords {
                let frac = self.digits_end(unsigned + 1);
                if mode == Mode::Initial || frac > unsigned + 1 {
                    best = (Tok::Real, frac);
                }
            }
        } else if mode == Mode::Initial {
            // -?{UNSIGNED}(\.{N}*)? and -?\.{N}+
            let sign = start + usize::from(at(start) == Some(b'-'));
            let int = self.digits_end(sign);
            if int > sign {
                best = (Tok::Real, int);
                if at(int) == Some(b'.') {
                    best.1 = self.digits_end(int + 1);
                }
            } else if at(sign) == Some(b'.') && self.digits_end(sign + 1) > sign + 1 {
                best = (Tok::Real, self.digits_end(sign + 1));
            }
        }
        if best.1 == start {
            return None;
        }
        self.matched(start, best.1);
        Some(self.token(best.0, start, 0))
    }

    /// The rules of markup mode, after the shared ones.
    fn markup(&mut self, start: usize, scopes: &mut dyn Scopes) -> Option<Token> {
        let text = self.text();
        let b = text[start];
        if b == b'{' || b == b'}' {
            self.matched(start, start + 1);
            return Some(self.token(Tok::Char(b), start, 0));
        }
        if b == b'\\' {
            if text.get(start + 1) == Some(&b'"') {
                self.matched(start, start + 2);
                return self.quote(start, true, scopes);
            }
            let command = self.symbol_end(start + 1);
            if command > start + 1 {
                self.matched(start, command);
                self.check_utf8(start, command);
                let kind = match &text[start + 1..command] {
                    b"score" => Tok::Keyword(keyword("score")),
                    b"score-lines" => Tok::ScoreLines,
                    // A markup command, then a markup list command, then
                    // `scan_escaped_word`.
                    name => {
                        let name = String::from_utf8_lossy(name).into_owned();
                        if let Some((id, value)) = scopes.markup_command(&name, &mut self.diags) {
                            return Some(self.token(Tok::Identifier(id), start, value));
                        }
                        return Some(self.escaped_word(start, name, scopes));
                    }
                };
                return Some(self.token(kind, start, 0));
            }
            // `<*>.[\200-\277]*`
            let end = start
                + 1
                + text[start + 1..]
                    .iter()
                    .take_while(|&&c| c & 0xc0 == 0x80)
                    .count();
            self.matched(start, end);
            self.check_utf8(start, end);
            let shown = String::from_utf8_lossy(&text[start..end]).into_owned();
            self.error(alloc::format!("invalid character: `{shown}'"));
            return Some(self.token(Tok::Char(b'%'), start, 0));
        }
        let end = start
            + text[start..]
                .iter()
                .position(|c| b"$#{}\"\\ \t\n\r\x0c".contains(c))
                .unwrap_or(text.len() - start);
        self.matched(start, end);
        self.check_utf8(start, end);
        Some(self.token(Tok::Symbol, start, 0))
    }

    /// `scan_escaped_word`: a keyword, a name bound in `scopes` as its
    /// value's kind, or an unknown command, returned as a string.
    fn escaped_word(&mut self, start: usize, name: String, scopes: &mut dyn Scopes) -> Token {
        if let Some(k) = KEYWORDS.iter().position(|&k| k == name) {
            return self.token(Tok::Keyword(k as u8), start, 0);
        }
        if let Some((id, value)) = scopes.classify(&name) {
            return self.token(Tok::Identifier(id), start, value);
        }
        self.error(alloc::format!("unknown command: `\\{name}'"));
        self.string_token(start, name)
    }

    /// The `incl` start condition after `\include`: white space, then a
    /// quoted name; a `\name` whose value is a string (P11, MS2 step 34);
    /// or a `#` or `$` form whose value is one. Its file is looked for where
    /// LilyPond looks and lexed next; the token after it. None when the
    /// scopes give no files or no name follows, and `\include` is a word as
    /// before. A `\name` or a form whose value is no string is LilyPond's
    /// error, what was found written to Guile's error port, and the lexer
    /// stays in the `incl` start condition ([`Lexer::incl`]).
    fn include(&mut self, mode: Mode, scopes: &mut dyn Scopes) -> Option<Token> {
        let text = self.text();
        let open = self.pos
            + text[self.pos..]
                .iter()
                .take_while(|&&c| is_white(c))
                .count();
        let name = match text.get(open) {
            Some(b'"') => {
                let close = open + 1 + text[open + 1..].iter().position(|&c| c == b'"')?;
                self.matched(open, close + 1);
                String::from_utf8_lossy(&text[open + 1..close]).into_owned()
            }
            Some(b'\\') => {
                let end = self.symbol_end(open + 1);
                if end == open + 1 {
                    return None;
                }
                let word = String::from_utf8_lossy(&text[open + 1..end]).into_owned();
                let value = scopes.include_name(&word)?;
                self.matched(open, end);
                self.check_utf8(open, end);
                match value {
                    Ok(name) => name,
                    Err(shown) => {
                        self.error(alloc::format!("wrong or undefined identifier: `{word}'"));
                        self.diags.output(&alloc::format!(
                            "This value was found in the table: {shown}"
                        ));
                        self.incl = true;
                        return Some(self.lex(mode, scopes));
                    }
                }
            }
            Some(b'#' | b'$') => {
                let (form, end) = scopes.read(self.src, self.file, open + 1, &mut self.diags)?;
                self.read_to(form.is_some(), end);
                self.matched(open, open + 1);
                let value =
                    scopes.include_form(self.src, self.file, form, open + 1, &mut self.diags)?;
                self.pos = end;
                match value {
                    Ok(name) => name,
                    Err(shown) => {
                        self.error("string expected after \\include");
                        if let Some(shown) = shown {
                            self.diags
                                .output(&alloc::format!("This value was found instead: {shown}"));
                        }
                        self.incl = true;
                        return Some(self.lex(mode, scopes));
                    }
                }
            }
            _ => return None,
        };
        self.incl = false;
        let (current, main) = (self.src.name(), self.main.0.name());
        let mut found = None;
        let candidates = source::include_candidates(&name, current, main);
        for (i, candidate) in candidates.iter().enumerate() {
            match scopes.include_file(candidate)? {
                Hosted::Text(file) => {
                    found = Some((candidate.clone(), file));
                    break;
                }
                Hosted::Missing => {}
                Hosted::Ask => {
                    // The candidates after it are asked at the same stop, up to
                    // one the host has (OPEN F, D263): the parse runs again
                    // once, not once a directory.
                    for later in &candidates[i + 1..] {
                        if let Hosted::Text(_) = scopes.include_file(later)? {
                            break;
                        }
                    }
                    self.waiting = true;
                    return Some(self.token(Tok::Eof, self.pos, 0));
                }
            }
        }
        match found {
            Some((candidate, file)) => self.include_source(&candidate, file),
            None => self.error(source::cannot_find(&name, current, main)),
        }
        Some(self.lex(mode, scopes))
    }

    /// `scan_shorthand` on `start..end`: bound in `scopes` as its value's
    /// kind, or undefined, returned as a string.
    fn shorthand(&mut self, start: usize, end: usize, scopes: &mut dyn Scopes) -> Token {
        self.matched(start, end);
        self.check_utf8(start, end);
        let text = String::from_utf8_lossy(&self.text()[start..end]).into_owned();
        if let Some((id, value)) = scopes.classify(&text) {
            return self.token(Tok::Identifier(id), start, value);
        }
        self.error(alloc::format!("undefined character or shorthand: {text}"));
        self.string_token(start, text)
    }

    /// The `quote` and `commandquote` start conditions, after the opening
    /// `"` or `\"`: a string, or for `\"name"` the word it quotes. None when
    /// the input ends first.
    fn quote(&mut self, start: usize, command: bool, scopes: &mut dyn Scopes) -> Option<Token> {
        let text = self.text();
        let mut value = Vec::new();
        loop {
            let p = self.pos;
            let Some(&b) = text.get(p) else {
                self.error("EOF found inside string");
                return None;
            };
            match b {
                b'"' => {
                    self.matched(p, p + 1);
                    break;
                }
                b'\\' => match text.get(p + 1) {
                    Some(&c) if ESCAPED.contains(&c) => {
                        value.push(match c {
                            b'n' => b'\n',
                            b't' => b'\t',
                            c => c,
                        });
                        self.matched(p, p + 2);
                    }
                    _ => {
                        value.push(b'\\');
                        self.matched(p, p + 1);
                    }
                },
                _ => {
                    let end = p + text[p..]
                        .iter()
                        .position(|&c| c == b'"' || c == b'\\')
                        .unwrap_or(text.len() - p);
                    self.matched(p, end);
                    self.check_utf8(p, end);
                    value.extend_from_slice(&text[p..end]);
                }
            }
        }
        let value = String::from_utf8_lossy(&value).into_owned();
        Some(if command {
            self.escaped_word(start, value, scopes)
        } else {
            self.string_token(start, value)
        })
    }

    /// The `longcomment` start condition, after `%{`.
    fn long_comment(&mut self) {
        let text = self.text();
        loop {
            let p = self.pos;
            if p == text.len() {
                self.error("EOF found inside a comment");
                return;
            }
            if text[p] == b'%' {
                let run = p + text[p..].iter().take_while(|&&c| c == b'%').count();
                if text.get(run) == Some(&b'}') {
                    self.matched(p, run + 1);
                    return;
                }
                let end = run
                    + text[run..]
                        .iter()
                        .take_while(|&&c| c != b'}' && c != b'%')
                        .count();
                self.matched(p, end);
                self.check_utf8(p, end);
            } else {
                let end = p + text[p..]
                    .iter()
                    .position(|&c| c == b'%')
                    .unwrap_or(text.len() - p);
                self.matched(p, end);
                self.check_utf8(p, end);
            }
        }
    }

    /// The `version` start condition, after `\version` and its white space.
    /// Returns [`Tok::Invalid`] for a version that is no good.
    fn version(&mut self) -> Option<Token> {
        let text = self.text();
        loop {
            let p = self.pos;
            let &b = text.get(p)?;
            if b == b'\r' {
                self.matched(p, p + 1);
                continue;
            }
            if b != b'"' {
                self.matched(p, p + 1);
                self.error("quoted string expected after \\version");
                return None;
            }
            let Some(close) = text[p + 1..]
                .iter()
                .position(|&c| c == b'"')
                .map(|n| p + 1 + n)
            else {
                self.matched(p, text.len());
                self.error("end quote missing");
                return None;
            };
            self.matched(p, close + 1);
            self.check_utf8(p, close + 1);
            let version = String::from_utf8_lossy(&text[p + 1..close]).into_owned();
            if self.check_version(&version) {
                self.version = VersionSeen::Valid(version);
                return None;
            }
            self.version = VersionSeen::Invalid;
            return Some(self.token(Tok::Invalid, p, 0));
        }
    }

    /// The `sourcefilename` start condition (P12): a quoted name, which the
    /// file's locations print from now on (`name_`), defined in the
    /// innermost scope as `input-file-name`. LilyPond's message that the
    /// input is renamed is an info, which `--loglevel=WARNING` leaves out.
    fn source_file_name(&mut self, scopes: &mut dyn Scopes) {
        let text = self.text();
        loop {
            let p = self.pos;
            let Some(&b) = text.get(p) else {
                return;
            };
            if b == b'\r' {
                self.matched(p, p + 1);
                continue;
            }
            if b != b'"' {
                self.matched(p, p + 1);
                self.error("quoted string expected after \\sourcefilename");
                return;
            }
            let Some(close) = text[p + 1..]
                .iter()
                .position(|&c| c == b'"')
                .map(|n| p + 1 + n)
            else {
                self.matched(p, text.len());
                self.error("end quote missing");
                return;
            };
            self.matched(p, close + 1);
            self.check_utf8(p, close + 1);
            let name = String::from_utf8_lossy(&text[p + 1..close]).into_owned();
            self.src.rename(&name);
            scopes.define_string("input-file-name", &name);
            return;
        }
    }

    /// The `sourcefileline` start condition (P12): an `INT`, the number of
    /// its own line from now on (`set_line`).
    fn source_file_line(&mut self) {
        let text = self.text();
        loop {
            let p = self.pos;
            if p >= text.len() {
                return;
            }
            if text[p] == b'\r' {
                self.matched(p, p + 1);
                continue;
            }
            let digits = usize::from(text[p] == b'-');
            let n = text[p + digits..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if n == 0 {
                self.matched(p, p + 1);
                self.error("integer expected after \\sourcefileline");
                return;
            }
            let end = p + digits + n;
            self.matched(p, end);
            // `sscanf`'s `%d`, past an int's range as C's would not be.
            let line = core::str::from_utf8(&text[p..end])
                .ok()
                .and_then(|t| t.parse::<i64>().ok())
                .unwrap_or(0)
                .clamp(i32::MIN.into(), i32::MAX.into());
            self.src.set_line(p as u32, line as isize);
            return;
        }
    }

    /// `parse-and-check-version` (`lily-library.scm`), whose errors have no location.
    fn check_version(&mut self, version: &str) -> bool {
        let parts: Vec<&str> = version.split('.').collect();
        let numbers: Option<Vec<Ratio>> = match parts.len() {
            // A fourth part is a symbol: no number, and never compared.
            3 | 4 => parts[..3].iter().map(|p| scheme_number(p)).collect(),
            2 => match (scheme_number(parts[0]), scheme_number(parts[1])) {
                (Some(_), Some(Ratio(minor, 1))) if minor % 2 != 0 => {
                    self.diags.error(
                        At::Nowhere,
                        "version with third number omitted is only allowed for stable releases \
                         (when the second number is even)",
                    );
                    return false;
                }
                (Some(major), Some(minor)) => Some([major, minor].into()),
                _ => None,
            },
            _ => None,
        };
        let Some(numbers) = numbers else {
            self.diags.error(
                At::Nowhere,
                alloc::format!("Invalid version string \"{version}\""),
            );
            return false;
        };
        // lexicographic-list-compare? with <, over the shorter list.
        let newer = VERSION_NUMBERS
            .into_iter()
            .zip(numbers)
            .find(|(c, v)| !c.equals(*v))
            .is_some_and(|(c, v)| c.less(v));
        if newer {
            self.diags.error(
                At::Nowhere,
                alloc::format!("program too old: {LILYPOND_VERSION} (file requires: {version})"),
            );
            return false;
        }
        true
    }

    /// `YYText_utf8`: a warning for each sequence of `start..end` that is no
    /// UTF-8, or is overlong, a surrogate or above U+10FFFF. As there, the
    /// byte that ends a broken sequence is skipped with it, and a NUL byte
    /// ends the check; unlike there, the check never reads past `end`.
    fn check_utf8(&mut self, start: usize, end: usize) {
        let p = &self.text()[start..end];
        let at = |i: usize| p.get(i).copied().unwrap_or(0);
        let mut i = 0;
        while i < p.len() && p[i] != 0 {
            let b = p[i];
            if b < 0x80 {
                i += 1;
                continue;
            }
            let old = i;
            i += 1;
            let next = at(i);
            let mut more = match b {
                0xc2..=0xdf => 1,
                0xe0 if next < 0xa0 => 0,
                0xed if next >= 0xa0 => 0,
                0xe0..=0xef => 2,
                0xf0 if next < 0x90 => 0,
                0xf4 if next >= 0x90 => 0,
                0xf0..=0xf4 => 3,
                _ => 0,
            };
            if more > 0 {
                loop {
                    let c = at(i);
                    i += 1;
                    if c & 0xc0 != 0x80 {
                        break;
                    }
                    more -= 1;
                    if more == 0 {
                        break;
                    }
                }
                if more == 0 {
                    continue;
                }
            }
            let at = At::Span(self.span((start + old, start + i.min(p.len()))));
            self.diags.warning(at, "non-UTF-8 input");
        }
    }
}

/// A number of a version, exact: Guile's `1e3` is 1000.0, which compares
/// as 1000 does. The denominator is never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ratio(i128, i128);

impl Ratio {
    fn equals(self, other: Ratio) -> bool {
        self.0 * other.1 == other.0 * self.1
    }

    fn less(self, other: Ratio) -> bool {
        self.0 * other.1 < other.0 * self.1
    }
}

/// Guile's `string->number` for what a part of a version can hold, which
/// has no `.`: `2`, `+2`, `-2`, `1e3`, `1e-3`, `1/2`. Exact, so that no
/// float parsing is linked; None also for numbers too large to compare.
fn scheme_number(s: &str) -> Option<Ratio> {
    const MAX: i128 = 1 << 60;
    fn integer(s: &str, signed: bool) -> Option<i128> {
        let (negative, digits) = match s.as_bytes().first() {
            Some(b'+' | b'-') if signed => (s.starts_with('-'), &s[1..]),
            _ => (false, s),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut n: i128 = 0;
        for b in digits.bytes() {
            n = n * 10 + i128::from(b - b'0');
            if n > MAX {
                return None;
            }
        }
        Some(if negative { -n } else { n })
    }
    if let Some((num, den)) = s.split_once('/') {
        let den = integer(den, false).filter(|&d| d != 0)?;
        return Some(Ratio(integer(num, true)?, den));
    }
    let Some((mantissa, exponent)) = s.split_once(['e', 'E']) else {
        return Some(Ratio(integer(s, true)?, 1));
    };
    let (mantissa, exponent) = (integer(mantissa, true)?, integer(exponent, true)?);
    let scale =
        (0..exponent.unsigned_abs()).try_fold(1i128, |p, _| Some(p * 10).filter(|&p| p <= MAX))?;
    Some(if exponent < 0 {
        Ratio(mantissa, scale)
    } else {
        Ratio(mantissa.checked_mul(scale)?, 1)
    })
}
