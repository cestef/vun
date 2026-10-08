//! Blocks, statements and the forms that choose and repeat.

use vun_grammar::{Level, TokenKind};

use super::Place;
use crate::literal::Writer;
use crate::make::{Arm, Block, Expr};

impl Writer {
    /// `{ statements; tail }`, a statement to a line; `{}` when empty.
    pub(super) fn block(&mut self, block: &Block, at: Place) {
        self.token(TokenKind::LBrace);
        let inner = at.deeper();
        for stmt in &block.stmts {
            self.newline(inner);
            self.statement(stmt, inner);
        }
        if let Some(tail) = &block.tail {
            self.newline(inner);
            self.operand(tail, Level::LOWEST, inner);
        }
        if !block.stmts.is_empty() || block.tail.is_some() {
            self.newline(at);
        }
        self.token(TokenKind::RBrace);
    }

    /// `stmt;`, or `stmt` alone where it ends at its closing brace.
    fn statement(&mut self, stmt: &Expr, at: Place) {
        self.operand(stmt, Level::LOWEST, at);
        if !stmt.block_like() {
            self.token(TokenKind::Semi);
        }
    }

    /// `if condition { } else otherwise`
    pub(super) fn if_form(&mut self, condition: &Expr, then: &Block, otherwise: Option<&Expr>, at: Place) {
        self.token(TokenKind::If).text(" ");
        self.operand(condition, Level::LOWEST, at.heading());
        self.text(" ");
        self.block(then, at);
        if let Some(otherwise) = otherwise {
            self.text(" ").token(TokenKind::Else).text(" ");
            self.expr(otherwise, at);
        }
    }

    /// `for head { }`
    pub(super) fn for_form(&mut self, head: Option<&Expr>, body: &Block, at: Place) {
        self.token(TokenKind::For).text(" ");
        if let Some(head) = head {
            self.operand(head, Level::LOWEST, at.heading());
            self.text(" ");
        }
        self.block(body, at);
    }

    /// `match scrutinee { arms }`, an arm to a line.
    pub(super) fn match_form(&mut self, scrutinee: &Expr, arms: &[Arm], at: Place) {
        self.token(TokenKind::Match).text(" ");
        self.operand(scrutinee, Level::LOWEST, at.heading());
        self.text(" ").token(TokenKind::LBrace);
        let inner = at.deeper();
        for arm in arms {
            self.newline(inner);
            self.operand(&arm.pattern, Level::LOWEST, inner.within());
            if let Some(guard) = &arm.guard {
                self.text(" ").token(TokenKind::If).text(" ");
                self.operand(guard, Level::LOWEST, inner.heading());
            }
            self.text(" ").token(TokenKind::FatArrow);
            if let Some(value) = &arm.value {
                self.text(" ");
                self.operand(value, Level::LOWEST, inner);
            }
            self.token(TokenKind::Comma);
        }
        if !arms.is_empty() {
            self.newline(at);
        }
        self.token(TokenKind::RBrace);
    }

    /// `'label: body`
    pub(super) fn labeled(&mut self, label: &str, body: &Expr, at: Place) {
        self.label(label).token(TokenKind::Colon).text(" ");
        self.expr(body, at);
    }

    /// `'label`
    pub(super) fn label(&mut self, label: &str) -> &mut Self {
        self.text("'").binder(label).expect("made labels are identifiers")
    }

    /// `word value`, a keyword and the value after it if there is one: `return x`.
    pub(super) fn jump(&mut self, word: TokenKind, label: Option<&str>, value: Option<&Expr>, at: Place) {
        self.token(word);
        if let Some(label) = label {
            self.text(" ").label(label);
        }
        if let Some(value) = value {
            self.text(" ");
            self.operand(value, Level::LOWEST, at.within());
        }
    }
}
