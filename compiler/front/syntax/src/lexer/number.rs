//! Integer and float literals.

use vun_grammar::TokenKind;

use super::report::Malformed;
use super::{Lexer, RADIX_PREFIXES, is_ident_continue};

impl Lexer<'_, '_, '_> {
    pub(super) fn number(&mut self) -> TokenKind {
        let lo = self.pos;
        let prefix = match (self.peek(), self.peek_second()) {
            (Some('0'), Some(p)) => RADIX_PREFIXES.iter().find(|&&(c, _)| c == p),
            _ => None,
        };
        let mut kind = TokenKind::Int;
        let mut radix = 10;
        let mut malformed = None;
        if let Some(&(_, base)) = prefix {
            self.bump();
            self.bump();
            radix = base;
            if self.eat_digits(radix) == 0 {
                malformed = Some(Malformed::Bare);
            }
        } else {
            self.eat_digits(10);
            if !self.after_dot && self.fraction_follows() {
                self.bump();
                self.eat_digits(10);
                kind = TokenKind::Float;
            }
            if !self.after_dot && self.exponent_follows() {
                self.bump();
                if matches!(self.peek(), Some('+' | '-')) {
                    self.bump();
                }
                self.eat_digits(10);
                kind = TokenKind::Float;
            }
        }
        // Letters or digits glued to a literal make it malformed: `0b102`, `12ab`.
        if self.peek().is_some_and(is_ident_continue) {
            malformed = Some(Malformed::Glued { at: self.pos, radix });
            self.eat_while(is_ident_continue);
        }
        if let Some(malformed) = malformed {
            self.malformed(lo, malformed);
        }
        kind
    }

    pub(super) fn fraction_follows(&self) -> bool {
        self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit())
    }

    pub(super) fn exponent_follows(&self) -> bool {
        let mut rest = self.rest().chars();
        matches!(rest.next(), Some('e' | 'E')) && {
            let after = rest.next();
            let digit = if matches!(after, Some('+' | '-')) { rest.next() } else { after };
            digit.is_some_and(|c| c.is_ascii_digit())
        }
    }
}
