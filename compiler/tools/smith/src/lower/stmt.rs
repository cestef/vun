//! Statements as source.

use vun_grammar::TokenKind;
use vun_syntax::make::{Bind, Block, Expr as Source};

use super::{Body, names};
use crate::ir::{Binding, Expr, Kind, Stmt};
use crate::ty::Ty;

impl Body<'_> {
    /// The statements, then the tail if there is one, as a block.
    pub(super) fn block(&self, stmts: &[Stmt], tail: Option<&Expr>) -> Block {
        let mut lines: Vec<Source> = stmts.iter().map(|s| self.stmt(s)).collect();
        match tail {
            // A tail call is a statement, `return lang.flow.tail(..)`, the block giving no value.
            Some(call @ Expr { kind: Kind::TailCall(..), .. }) => {
                lines.push(Source::Return(Some(Box::new(self.expr(call)))));
                Block::new(lines, None)
            }
            _ => Block::new(lines, tail.map(|t| self.expr(t))),
        }
    }

    fn stmt(&self, stmt: &Stmt) -> Source {
        match stmt {
            Stmt::Let { local, init, binding } => {
                let info = &self.function.locals[*local];
                let name = self.name(*local);
                let value = self.expr(init);
                let bind = match binding {
                    Binding::Inferred if !info.mutable => Bind::new(&name, value),
                    Binding::Inferred if matches!(info.ty, Ty::Closure(..)) => {
                        Bind { mutable: true, ..Bind::new(&name, value) }
                    }
                    Binding::Inferred | Binding::Annotated => Bind {
                        mutable: info.mutable,
                        ..Bind::typed(&name, self.lowering.ty(&info.ty), value)
                    },
                };
                Source::bind(bind)
            }
            Stmt::Assign { place, op, value } => {
                Source::assign(op.token(), self.place(place), self.expr(value))
            }
            Stmt::If { condition, then, otherwise } => Source::when(
                self.expr(condition),
                self.block(then, None),
                (!otherwise.is_empty()).then(|| Source::Block(self.block(otherwise, None))),
            ),
            Stmt::Count { local, count, body } => {
                let range = Source::infix(TokenKind::DotDot, Source::int(0usize), Source::int(*count));
                self.looping(*local, range, body)
            }
            Stmt::Each { local, array, body } => self.looping(*local, self.local(*array), body),
            Stmt::While { condition, body } => {
                Source::For(Some(Box::new(self.expr(condition))), self.block(body, None))
            }
            Stmt::WhileSome { local, option, body } => {
                let pattern = Source::variant("some", vec![self.local(*local)]);
                let head = Source::Test(Box::new(pattern), Box::new(self.expr(option)));
                Source::For(Some(Box::new(head)), self.block(body, None))
            }
            Stmt::Drain { local, function, args, body } => {
                let call = Source::call(
                    Source::local(&names::function(*function)),
                    args.iter().map(|a| self.expr(a)).collect(),
                );
                self.looping(*local, call, body)
            }
            Stmt::Yield(value) => Source::Yield(Box::new(self.expr(value))),
            Stmt::Defer(inner) => Source::Defer(Box::new(self.stmt(inner))),
            Stmt::Labeled(label, inner) => Source::Labeled(names::label(*label), Box::new(self.stmt(inner))),
            Stmt::Break(label, value) => {
                Source::Break(label.map(names::label), value.as_ref().map(|v| Box::new(self.expr(v))))
            }
            Stmt::Continue(label) => Source::Continue(label.map(names::label)),
        }
    }

    /// `for local in items { body }`
    fn looping(&self, local: crate::ir::LocalId, items: Source, body: &[Stmt]) -> Source {
        let head = Source::In(Box::new(self.local(local)), Box::new(items));
        Source::For(Some(Box::new(head)), self.block(body, None))
    }
}
