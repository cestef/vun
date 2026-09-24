//! Integer and float literals.

use vun_diag::{Msg, errors};
use vun_grammar::TokenKind;

use super::{Lexer, RADIX_PREFIXES, is_ident_continue};

impl Lexer<'_, '_, '_> {
    pub(super) fn number(&mut self) -> TokenKind {
        let lo = self.pos;
        let prefix = match (self.peek(), self.peek_second()) {
            (Some('0'), Some(p)) => RADIX_PREFIXES.iter().find(|&&(c, _)| c == p),
            _ => None,
        };
        let mut kind = TokenKind::Int;
        let mut well_formed = true;
        if let Some(&(_, radix)) = prefix {
            self.bump();
            self.bump();
            well_formed = self.eat_digits(radix) > 0;
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
        // Letters or digits glued to a literal make it malformed: `0b102`, `12ab`, `0x`.
        if self.peek().is_some_and(is_ident_continue) {
            self.eat_while(is_ident_continue);
            well_formed = false;
        }
        if !well_formed {
            let msg = Msg::new().text("malformed number literal ").code(&self.text[lo..self.pos]);
            self.error(lo, errors::SYNTAX_MALFORMED_NUMBER, msg);
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
