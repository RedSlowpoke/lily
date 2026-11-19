//! Syntax errors in Bison's terms (02 §3.4, §5.6): the display names of
//! `parser.yy`'s tokens, and the tokens Bison lists after `expecting` at each
//! error point of the parser.
//!
//! `yysyntax_error` lists the tokens that have an action of their own in the
//! state where the error is detected, and none when there are more than four.
//! That state is not always the one the grammar is in at the error: a token
//! with no action of its own sets off the state's default reduction first,
//! and Bison then lists what the state after it expects. So `title = }`
//! expects `\header`: `}` reduces `header_modification`'s empty mid-rule
//! action, after which only `\header` can follow. The sets below were read
//! from the states of `parser.yy` 2.26.0 (`bison --report=state`), with the
//! reductions a token of no use there sets off, and each is checked against
//! what LilyPond prints by a test.

use crate::lexer::{KEYWORDS, Tok, keyword};
use alloc::string::String;

/// What Bison lists after `expecting` at an error point, in its token order;
/// empty when it lists nothing, because more than four tokens could follow.
pub(crate) type Expected = &'static [Tok];

/// More than four tokens could follow: in music, at toplevel, in a header's
/// body, and wherever a value or an argument starts.
pub(crate) const MANY: Expected = &[];

/// After a template's `embedded_lilypond` (`start_symbol`).
pub(crate) const END_OF_INPUT: Expected = &[Tok::Eof];

/// `"\\header" • '{'`, `"\\score" • '{'`, and the same after `\book`,
/// `\bookpart`, `\paper`, `\layout` and `\midi`.
pub(crate) const OPEN_BRACE: Expected = &[Tok::Char(b'{')];

/// `assignment_id • '=' identifier_init`, and `symbol_list_part`'s `'.'`
/// of a `name.path` (P3).
pub(crate) const ASSIGNMENT_ID: Expected = &[Tok::Char(b'.'), Tok::Char(b'=')];

/// `assignment_id '.' property_path • '=' identifier_init`: `'.'` and `','`
/// of the path reduce it first.
pub(crate) const EQUALS: Expected = &[Tok::Char(b'=')];

/// `assignment_id '=' • identifier_init`, at a token no value starts with:
/// it reduces `header_modification`'s mid-rule action, and leaves
/// `header_modification: $@3 • lilypond_header`.
pub(crate) const IDENTIFIER_INIT: Expected = &[Tok::Keyword(keyword("header"))];

/// A parameter's argument, at a token no argument starts with: as for
/// [`IDENTIFIER_INIT`], `embedded_scm_bare_arg`'s `header_modification`
/// reduces its mid-rule action first.
pub(crate) const ARGUMENT: Expected = IDENTIFIER_INIT;

/// An optional parameter's argument when a mandatory one follows: what
/// does not start an argument would end the skipped run
/// (`function_arglist_skip_backup • BACKUP`).
pub(crate) const BACKED_UP: Expected = &[Tok::Backup];

/// `"\\score" '{' score_body • '}'`: a token no score item starts with
/// reduces `score_items` to `score_body` first.
pub(crate) const SCORE_BODY: Expected = &[Tok::Char(b'}')];

/// `number_expression '+' • number_term`, `number_factor '*' •
/// number_factor` and the like (P5): what starts a number factor.
pub(crate) const NUMBER_FACTOR: Expected = &[
    Tok::Unsigned,
    Tok::Identifier(crate::classify::Id::Number),
    Tok::Real,
    Tok::Char(b'-'),
];

/// `scalar: '-' • bare_number` (P6).
pub(crate) const BARE_NUMBER: Expected = &[
    Tok::Unsigned,
    Tok::Identifier(crate::classify::Id::Number),
    Tok::Real,
];

/// `symbol` and `simple_string` (P6): after `\new`, `\context`, `\change`
/// and `=`.
pub(crate) const SYMBOL: Expected = &[
    Tok::Identifier(crate::classify::Id::Scm),
    Tok::ScmToken,
    Tok::String,
    Tok::Symbol,
];

/// `chord_separator: CHORD_SLASH • steno_tonic_pitch`, and after
/// `CHORD_BASS` (P10).
pub(crate) const TONICNAME_PITCH: Expected = &[Tok::TonicnamePitch];

/// `mode_changing_head • grouped_music_list`, and after
/// `mode_changing_head_with_context optional_context_mods`, which an empty
/// `context_modification_mods_list` reduces to first (P10).
pub(crate) const GROUPED_MUSIC_LIST: Expected = &[
    Tok::Keyword(keyword("sequential")),
    Tok::Keyword(keyword("simultaneous")),
    Tok::DoubleAngleOpen,
    Tok::Char(b'{'),
];

/// `syntax error, unexpected X`, and `, expecting A or B …` when Bison
/// lists what could follow, as `yysyntax_error` words it.
pub(crate) fn message(unexpected: Tok, expected: Expected) -> String {
    let mut message = alloc::format!("syntax error, unexpected {}", unexpected.display());
    for (i, tok) in expected.iter().enumerate() {
        message.push_str(if i == 0 { ", expecting " } else { " or " });
        message.push_str(&tok.display());
    }
    message
}

impl Tok {
    /// The token's name in Bison's messages: `parser.yy`'s string, unquoted as
    /// `yytnamerr` does, or the token's own name.
    pub fn display(self) -> String {
        match self {
            Tok::Eof => "end of input".into(),
            Tok::Keyword(k) => alloc::format!("\\{}", KEYWORDS[k as usize]),
            Tok::ScoreLines => "\\score-lines".into(),
            Tok::String => "STRING".into(),
            Tok::Symbol => "SYMBOL".into(),
            Tok::Hyphen => "--".into(),
            Tok::Extender => "__".into(),
            Tok::NotenamePitch => "NOTENAME_PITCH".into(),
            Tok::TonicnamePitch => "TONICNAME_PITCH".into(),
            Tok::DrumPitch => "DRUM_PITCH".into(),
            Tok::ChordModifier => "CHORD_MODIFIER".into(),
            Tok::ChordColon => ":".into(),
            Tok::ChordMinus => "-".into(),
            Tok::ChordCaret => "^".into(),
            Tok::ChordSlash => "/".into(),
            Tok::ChordBass => "/+".into(),
            Tok::FigureOpen => "FIGURE_OPEN".into(),
            Tok::FigureClose => "FIGURE_CLOSE".into(),
            Tok::FigureSpace => "_".into(),
            Tok::EPlus => "\\+".into(),
            Tok::EExclamation => "\\!".into(),
            Tok::EBackslash => "\\\\".into(),
            Tok::FigureAlteration => "FIGURE_ALTERATION_EXPR".into(),
            Tok::RestName => "RESTNAME".into(),
            Tok::ChordRepetition => "CHORD_REPETITION".into(),
            Tok::MultiMeasureRest => "MULTI_MEASURE_REST".into(),
            // Bison writes the quote as C does.
            Tok::Char(b'\'') => "'\\''".into(),
            // What `<*>.` returns, which the grammar has no token for.
            Tok::Char(b'%') => "invalid token".into(),
            Tok::Unsigned => "UNSIGNED".into(),
            Tok::EUnsigned => "E_UNSIGNED".into(),
            Tok::Fraction => "FRACTION".into(),
            Tok::Real => "REAL".into(),
            Tok::Char(c) => alloc::format!("'{}'", c as char),
            Tok::AngleOpen => "<".into(),
            Tok::AngleClose => ">".into(),
            Tok::DoubleAngleOpen => "<<".into(),
            Tok::DoubleAngleClose => ">>".into(),
            Tok::ScmToken => "SCM_TOKEN".into(),
            // What `$` gives depends on its value (P4).
            Tok::ScmImmediate => "SCM_IDENTIFIER".into(),
            Tok::ScmArg => "SCM_ARG".into(),
            Tok::Backup => "(backed-up?)".into(),
            Tok::Invalid => "\\version-error".into(),
            Tok::Identifier(id) => id.display().into(),
        }
    }
}
