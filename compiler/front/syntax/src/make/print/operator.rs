//! Operators: what holds an operand on one side or both, and the calls and accesses that follow one.

use vun_grammar::{Assoc, Level, Spacing, TokenKind};

use super::Place;
use crate::literal::Writer;
use crate::make::{Borrow, Expr, Property};

impl Writer {
    /// `left op right`: each side parenthesized where the level of `op` asks, and a chain of
    /// operators that do not chain kept apart.
    pub(super) fn infix(&mut self, op: TokenKind, left: &Expr, right: Option<&Expr>, at: Place) {
        let infix = op.infix().expect("made infix operators are in the table");
        let tighter = infix.level.tighter();
        let left_min = if infix.assoc == Assoc::Left { infix.level } else { tighter };
        self.operand(left, left_min, at);
        let spaced = infix.around == Spacing::Spaced;
        if spaced {
            self.text(" ");
        }
        self.token(op);
        if let Some(right) = right {
            if spaced {
                self.text(" ");
            }
            self.operand(right, tighter, at);
        }
    }

    /// `token operand`
    pub(super) fn prefix(&mut self, token: TokenKind, inner: &Expr, at: Place) {
        self.token(token);
        // A keyword prefix, such as `unsafe`, is a word apart from what follows.
        if TokenKind::KEYWORDS.contains(&token) {
            self.text(" ");
        }
        self.operand(inner, Self::held(token, inner), at);
    }

    /// `&qualifier mode operand`
    pub(super) fn borrow(&mut self, borrow: Borrow, inner: &Expr, at: Place) {
        self.token(TokenKind::Amp);
        let words = [borrow.qualifier, borrow.mode].into_iter().flatten();
        for word in words {
            self.text(word.text().expect("borrow words are spelled")).text(" ");
        }
        self.operand(inner, Expr::prefix_level(), at);
    }

    /// The level an operand of the prefix `token` must hold: its own, unless it is a word before
    /// a braced form, which is written as it stands.
    fn held(token: TokenKind, inner: &Expr) -> Level {
        if Expr::words(token) && inner.braced() { Level::LOWEST } else { Expr::prefix_level() }
    }

    /// `operand token`
    pub(super) fn postfix(&mut self, token: TokenKind, inner: &Expr, at: Place) {
        self.operand(inner, Expr::postfix_level(), at);
        self.token(token);
    }

    /// `place op value`
    pub(super) fn assign(&mut self, op: TokenKind, place: &Expr, value: &Expr, at: Place) {
        self.operand(place, Expr::prefix_level(), at);
        self.text(" ").token(op).text(" ");
        self.operand(value, Expr::value_level(), at);
    }

    /// `object.property`: a number is parenthesized, as one read with the dot as its fraction.
    pub(super) fn member(&mut self, object: &Expr, property: &Property, at: Place) {
        let fraction = object.numeric()
            && (!matches!(object, Expr::Member(..)) || matches!(property, Property::Index(_)));
        if fraction {
            self.text("(");
            self.expr(object, at.within());
            self.text(")");
        } else {
            self.operand(object, Expr::postfix_level(), at);
        }
        self.token(TokenKind::Dot);
        match property {
            Property::Name(name) => {
                self.binder(name).expect("made names are identifiers");
            }
            Property::Index(index) => {
                self.text(&index.to_string());
            }
        }
    }

    /// `callee(args)`
    pub(super) fn call(&mut self, callee: &Expr, args: &[Expr], at: Place) {
        self.operand(callee, Expr::postfix_level(), at);
        self.token(TokenKind::LParen).list(args, at);
        self.token(TokenKind::RParen);
    }

    /// `object[subscript]`
    pub(super) fn index(&mut self, object: &Expr, subscript: &Expr, at: Place) {
        self.operand(object, Expr::postfix_level(), at);
        self.token(TokenKind::LBracket);
        self.operand(subscript, Level::LOWEST, at.within());
        self.token(TokenKind::RBracket);
    }
}
