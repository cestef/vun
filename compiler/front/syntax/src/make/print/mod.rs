//! Writing made source: items one after the other with a blank line between, attributes on the
//! line before their item, a block's statements and a struct type's fields one per line.

mod control;
mod item;
mod operator;
mod value;

use vun_grammar::Level;

use super::{Expr, File};
use crate::literal::{INDENT, Writer};

pub(super) fn file(file: &File) -> String {
    let mut w = Writer::default();
    w.file(file);
    w.out
}

pub(super) fn attributes(attrs: &[Expr]) -> String {
    let mut w = Writer::default();
    w.attributes(attrs, Place::TOP);
    w.out
}

pub(super) fn expr(expr: &Expr) -> String {
    let mut w = Writer::default();
    w.expr(expr, Place::TOP);
    w.out
}

/// Where a form is written: how deep its lines are indented, and whether it is the head of an
/// `if`, `for` or `match`, where a `{` opens the body.
#[derive(Clone, Copy)]
pub(super) struct Place {
    depth: usize,
    head: bool,
}

impl Place {
    pub(super) const TOP: Self = Self { depth: 0, head: false };

    /// One level inside a pair of braces.
    pub(super) const fn deeper(self) -> Self {
        Self { depth: self.depth + 1, head: false }
    }

    /// Inside a delimiter pair, where a head's `{` can no longer be mistaken.
    pub(super) const fn within(self) -> Self {
        Self { head: false, ..self }
    }

    /// The head of a control form.
    pub(super) const fn heading(self) -> Self {
        Self { head: true, ..self }
    }
}

impl Writer {
    /// A line break, then the indentation of `at`.
    pub(super) fn newline(&mut self, at: Place) -> &mut Self {
        self.line(&INDENT.repeat(at.depth))
    }

    /// `expr`, in parentheses unless it holds its operands at least as tight as `min`, and is not
    /// a braced form standing in a head.
    pub(super) fn operand(&mut self, expr: &Expr, min: Level, at: Place) -> &mut Self {
        if expr.level() < min || (at.head && expr.braced()) {
            self.text("(");
            self.expr(expr, at.within());
            self.text(")")
        } else {
            self.expr(expr, at);
            self
        }
    }

    /// `a, b`, each at `min`.
    pub(super) fn list(&mut self, exprs: &[Expr], at: Place) -> &mut Self {
        for (i, expr) in exprs.iter().enumerate() {
            if i > 0 {
                self.text(", ");
            }
            self.operand(expr, Level::LOWEST, at.within());
        }
        self
    }
}
