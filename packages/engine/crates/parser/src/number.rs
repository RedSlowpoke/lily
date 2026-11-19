//! Number expressions (02 §6's P5): `parser.yy`'s `number_expression`,
//! `number_term`, `number_factor` and `bare_number`, an assignment's value
//! with `+`, `-`, `*` and `/`, a unary `-`, and units. A unit is a
//! `NUMBER_IDENTIFIER` after an `UNSIGNED` or a `REAL`, multiplied in:
//! `\mm`, `\cm`, `\in`, `\pt` and `\bp`, which only the scopes of output
//! definitions define (`output_def.rs`).
//!
//! A term is one factor, or two with `*` or `/` between them: `2 * 3 * 4`
//! is a syntax error at its second `*`, as in LilyPond. Bison reduces a
//! number expression with the token after it as lookahead. The tokens are
//! lexed in the block's mode ([`Parser::base`]): in an output definition,
//! `-2` is one `REAL`.

use crate::bison;
use crate::classify::Id;
use crate::lexer::{Tok, Token};
use crate::{Parsed, Parser};
use lily_engine_scheme::Value;

impl Parser<'_, '_> {
    /// `number_expression` whose first factor, `first`, was read: terms
    /// after `+` and `-`, left to right.
    pub(crate) fn number_expression(&mut self, first: Value) -> Parsed<Value> {
        let mut sum = self.number_term(first)?;
        loop {
            let op = self.peek(self.base);
            if !matches!(op.kind, Tok::Char(b'+' | b'-')) {
                return Ok(sum);
            }
            self.next(self.base);
            let factor = self.number_factor()?;
            let term = self.number_term(factor)?;
            sum = self.arithmetic(&op, sum, term);
        }
    }

    /// `number_term` whose first factor was read: it, or it and a second
    /// after `*` or `/`.
    fn number_term(&mut self, first: Value) -> Parsed<Value> {
        let op = self.peek(self.base);
        if !matches!(op.kind, Tok::Char(b'*' | b'/')) {
            return Ok(first);
        }
        self.next(self.base);
        let second = self.number_factor()?;
        Ok(self.arithmetic(&op, first, second))
    }

    /// `number_factor`: `-` and a factor, negated, or `bare_number`.
    pub(crate) fn number_factor(&mut self) -> Parsed<Value> {
        let token = self.peek(self.base);
        match token.kind {
            Tok::Char(b'-') => {
                self.next(self.base);
                let factor = self.number_factor()?;
                let negated = self.env.vm().heap_mut().negate(factor).unwrap_or(factor);
                self.env.keep(negated);
                Ok(negated)
            }
            Tok::Unsigned | Tok::Real => {
                self.next(self.base);
                let value = self.number_value(&token);
                Ok(self.bare_number(value))
            }
            Tok::Identifier(Id::Number) => {
                self.next(self.base);
                Ok(self.number_value(&token))
            }
            _ => Err(self.syntax_error(&token, bison::NUMBER_FACTOR)),
        }
    }

    /// An `UNSIGNED`'s, a `REAL`'s or a `NUMBER_IDENTIFIER`'s value, rooted.
    pub(crate) fn number_value(&mut self, token: &Token) -> Value {
        let value = match token.kind {
            Tok::Identifier(_) => self.env.value(token.value),
            _ => self.number(token, false),
        };
        self.env.keep(value);
        value
    }

    /// `bare_number` after an `UNSIGNED` or a `REAL` of value `value`: a
    /// `NUMBER_IDENTIFIER` after it is its unit, multiplied in.
    pub(crate) fn bare_number(&mut self, value: Value) -> Value {
        let unit = self.peek(self.base);
        if unit.kind != Tok::Identifier(Id::Number) {
            return value;
        }
        self.next(self.base);
        let unit = self.env.value(unit.value);
        let product = self.env.vm().heap_mut().mul(value, unit).unwrap_or(value);
        self.env.keep(product);
        product
    }

    /// `scm_sum`, `scm_difference`, `scm_product` or `scm_divide` of `x`
    /// and `y` for the operator `op`, rooted. A division by an exact zero
    /// is Guile's `Numerical overflow` in LilyPond, which ends the run; it
    /// gives `x` here.
    fn arithmetic(&mut self, op: &Token, x: Value, y: Value) -> Value {
        let heap = self.env.vm().heap_mut();
        let result = match op.kind {
            Tok::Char(b'+') => heap.add(x, y),
            Tok::Char(b'-') => heap.sub(x, y),
            Tok::Char(b'*') => heap.mul(x, y),
            _ => heap.div(x, y),
        };
        let value = result.unwrap_or(x);
        self.env.keep(value);
        value
    }
}
