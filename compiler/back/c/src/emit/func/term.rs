//! Terminators: a jump assigns its target's parameters by the sequenced moves of `Moves`, then
//! goes there, unless the target is the block emitted next; a branch is an `if`, a switch a
//! `switch`, and a block control never leaves an empty infinite loop.

use vun_lir::analysis::{Location, Moves};
use vun_lir::{AbiType, Block, Target, Terminator};

use super::FnEmit;
use crate::ast::{Expr, Ident, Own, Stmt};
use crate::emit::signature::SignedC;
use vun_backend::Lack;

impl FnEmit<'_, '_> {
    pub(super) fn term(&mut self, block: Block) -> Result<(), Lack> {
        let func = self.func;
        let data = &func.blocks[block];
        self.at(data.origin);
        let next = self.order.next(block);
        match &data.term {
            Terminator::Jump(target) => {
                let stmts = self.edge(target, next);
                self.body.extend(stmts);
            }
            Terminator::Branch { cond, then, otherwise } => {
                let cond = self.operand(*cond).truth();
                let then = self.edge(then, None);
                let otherwise = self.edge(otherwise, next);
                self.body.push(Stmt::If(cond, then, Vec::new()));
                self.body.extend(otherwise);
            }
            Terminator::Switch { value, cases, default } => {
                let ty = func.ty(*value);
                let value = self.operand(*value);
                let mut arms = Vec::new();
                for (case, target) in cases {
                    arms.push((self.e.literal(*case, ty.bits(0), data.origin)?, self.edge(target, None)));
                }
                let default = self.edge(default, None);
                self.body.push(Stmt::Switch(value, arms, default));
            }
            Terminator::Return(value) => {
                let signature = &func.signature;
                let value = match (value, &signature.result) {
                    (Some(v), Some(result)) if let Some(s) = result.signed_c(signature.conv) => {
                        let ty = self.e.int(s.bits(0), true, data.origin)?;
                        Some(self.operand(*v).cast(ty))
                    }
                    (Some(v), _) => Some(self.operand(*v)),
                    (None, Some(AbiType::Record(_))) => Some(Expr::Name(Ident::Own(Own::Result))),
                    (None, _) => None,
                };
                self.body.push(Stmt::Return(value));
            }
            Terminator::Unreachable => self.body.push(Stmt::Halt),
        }
        Ok(())
    }

    /// The statements taking the jump `target`: its moves, then a `goto` unless the target is
    /// `next`, where control falls through.
    fn edge(&mut self, target: &Target, next: Option<Block>) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        for m in Moves::of(self.func, target).0 {
            let from = self.location(m.from);
            stmts.push(Stmt::Assign(self.location(m.to), from));
        }
        if next != Some(target.block) {
            stmts.push(Stmt::Goto(Ident::block(target.block)));
        }
        stmts
    }

    fn location(&mut self, location: Location) -> Expr {
        match location {
            Location::Value(v) => self.operand(v),
            Location::Temp(s) => {
                self.temps.insert(s);
                Expr::Name(Ident::Own(Own::Temp(s)))
            }
        }
    }
}
