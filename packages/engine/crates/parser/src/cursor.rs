//! The tokens an argument list reads (02 §3.2, §5.3): the lexer's, in
//! front of which an argument that an optional parameter's predicate
//! rejected is put back for the next parameter, as Bison's `BACKUP` puts it
//! back with `push_extra_token`.
//!
//! What is put back is the argument as it was read, one cached token that
//! carries its value: Scheme is not run again, music is not parsed again
//! and no error is reported twice, however many tokens the argument took.
//! A `#` form's value goes back as an `SCM_ARG` ([`Tok::ScmArg`]), a word
//! as a `STRING`, a pitch as a `PITCH_IDENTIFIER`, a post-event as an
//! `EVENT_IDENTIFIER`, a number as itself or as a `NUMBER_IDENTIFIER`; each
//! keeps the place of the argument, where an error about it is reported.
//!
//! `REPARSE`, which pushes a token back to be read as another kind, needs
//! no tokens here: the parameter's reading goes on from what it read.

use crate::Parser;
use crate::lexer::{Tok, Token};
use crate::source::Span;
use lily_engine_scheme::Value;

impl Parser<'_, '_> {
    /// `MYBACKUP`: `tokens` are read next, in their order, before the token
    /// the parser looked at, if it did.
    pub(crate) fn put_back(&mut self, tokens: &[Token]) {
        if let Some((ahead, _)) = self.ahead.take() {
            self.lexer.push_lookahead(ahead);
        }
        for &token in tokens.iter().rev() {
            self.lexer.push_token(token);
        }
    }

    /// A token of `kind` that carries `value`, kept, at `at`, where it was
    /// read.
    pub(crate) fn value_token(&mut self, kind: Tok, value: Value, at: Span) -> Token {
        Token {
            kind,
            span: at,
            loc: at,
            value: self.env.keep(value),
        }
    }
}
