//! Statements and the blocks of them.

use super::Draw;
use crate::int::Range;
use crate::ir::{Binding, Stmt};
use crate::ty::Ty;

/// The forms of statements, each drawn by its weight, 0 where a block has no room for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
enum Form {
    Bind,
    BindMutable,
    Assign,
    Branch,
    Count,
    Each,
    While,
    WhileSome,
    Drain,
    Yield,
    Defer,
    Escape,
    Exit,
}

impl Form {
    fn weight(self, within: &Draw<'_>) -> usize {
        let room = within.scope.nesting < within.config.nesting;
        match self {
            Self::Bind => 8,
            Self::BindMutable => 4,
            Self::Assign => 8,
            Self::Branch if room => 3,
            Self::Count if room => 3,
            Self::Each if room && !within.scope.find(|l| matches!(l.ty, Ty::Array(..))).is_empty() => 2,
            Self::While if room => 2,
            Self::WhileSome if room => 2,
            Self::Drain if room && within.iterators().next().is_some() => 3,
            Self::Yield if within.scope.yields.is_some() => 4,
            Self::Defer if within.defers() => 3,
            Self::Escape if within.scope.loops > 0 => 2,
            Self::Exit if !within.scope.exits.is_empty() => 3,
            _ => 0,
        }
    }
}

impl Draw<'_> {
    /// A block of statements: what it binds is visible to its end only.
    pub(super) fn block(&mut self) -> Vec<Stmt> {
        let seen = self.scope.visible.len();
        self.scope.nesting += 1;
        let count = self.entropy.between(0, self.config.stmts);
        let mut stmts = Vec::new();
        for _ in 0..count {
            if self.scope.steps >= self.config.steps {
                break;
            }
            stmts.extend(self.stmt());
        }
        self.scope.nesting -= 1;
        self.scope.visible.truncate(seen);
        stmts
    }

    fn stmt(&mut self) -> Vec<Stmt> {
        let label = self.entropy.chance(1, 3).then(|| self.scope.name_loop());
        let weights: Vec<usize> = Form::ALL.iter().map(|f| f.weight(self)).collect();
        let form = Form::ALL[self.entropy.weighted(&weights)];
        self.scope.step();
        let made = match form {
            Form::Bind => {
                let ty = self.ty(1);
                let init = self.expr(&ty, self.config.depth);
                let (local, binding) = if self.entropy.chance(1, 2) {
                    (self.scope.bind(init.ty.clone(), false), Binding::Inferred)
                } else {
                    let ty = self.wider(&init.ty);
                    (self.scope.bind(ty, false), Binding::Annotated)
                };
                Some(Stmt::Let { local, init, binding })
            }
            Form::BindMutable => {
                let ty = self.ty(1);
                let init = self.expr(&ty, self.config.depth);
                let ty = self.wider(&init.ty);
                let local = self.scope.bind(ty, true);
                Some(Stmt::Let { local, init, binding: Binding::Annotated })
            }
            Form::Assign => self.assign(),
            Form::Branch => {
                let condition = self.bool(self.config.depth);
                let then = self.block();
                let otherwise = if self.entropy.chance(1, 2) { self.block() } else { Vec::new() };
                Some(Stmt::If { condition, then, otherwise })
            }
            Form::Count => {
                let count = self.times();
                let seen = self.scope.visible.len();
                let local = self.scope.bind(Ty::Int(Range::new(0, count - 1)), false);
                let body = self.looped(u64::from(count), label);
                self.scope.visible.truncate(seen);
                Some(Self::named(label, Stmt::Count { local, count, body }))
            }
            Form::Each => {
                let arrays = self.scope.find(|l| matches!(l.ty, Ty::Array(..)));
                let array = *self.entropy.pick(&arrays);
                let Ty::Array(len, item) = self.scope.locals[array].ty.clone() else { return Vec::new() };
                let seen = self.scope.visible.len();
                let local = self.scope.bind(*item, false);
                self.scope.frozen.push(array);
                let body = self.looped(len as u64, label);
                self.scope.frozen.pop();
                self.scope.visible.truncate(seen);
                Some(Self::named(label, Stmt::Each { local, array, body }))
            }
            Form::While => return self.while_loop(label).unwrap_or_default(),
            Form::WhileSome => return self.while_some(label).unwrap_or_default(),
            Form::Drain => return self.drain(label).unwrap_or_default(),
            Form::Yield => {
                self.scope.yields.clone().map(|item| Stmt::Yield(self.expr(&item, self.config.depth)))
            }
            Form::Defer => self.assign().map(|assign| Stmt::Defer(Box::new(assign))),
            Form::Escape => Some(self.escape()),
            Form::Exit => self.exit(),
        };
        made.into_iter().collect()
    }
}
