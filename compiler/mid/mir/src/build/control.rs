//! Control flow: `if` and the short-circuit `and` and `or` become branches between blocks, each
//! writing the expression's value to one local before joining.

use overt_grammar::TokenKind;
use overt_hir::tree::{BlockId as HirBlock, Cond, ExprId};
use overt_origin::OriginId;

use super::{Builder, Unbuilt};
use crate::body::{Block, BlockId, Local, LocalDecl, Operand, Rvalue, Terminator, TerminatorKind};
use crate::place::Place;

impl Builder<'_, '_, '_> {
    /// A new, empty block, returning until something terminates it.
    pub(super) fn fresh(&mut self, origin: OriginId) -> BlockId {
        self.blocks
            .push(Block { stmts: Vec::new(), term: Terminator { kind: TerminatorKind::Return, origin } })
    }

    /// Ends the current block with `kind`.
    pub(super) fn terminate(&mut self, kind: TerminatorKind, origin: OriginId) {
        self.blocks[self.current].term = Terminator { kind, origin };
    }

    /// A new local for the value of the expression at `origin`.
    pub(super) fn result(&mut self, origin: OriginId) -> Local {
        self.locals.push(LocalDecl { name: None, origin })
    }

    /// `if cond { then } else { otherwise }`; without `else`, the value is `()`.
    pub(super) fn branch(
        &mut self,
        cond: &Cond,
        then: HirBlock,
        otherwise: Option<ExprId>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let Cond::Expr(cond) = cond else { return Err(Unbuilt { at: origin }) };
        let cond = self.operand(*cond)?;
        let result = self.result(origin);
        let (yes, no, join) = (self.fresh(origin), self.fresh(origin), self.fresh(origin));
        self.terminate(TerminatorKind::Branch { cond, then: yes, otherwise: no }, origin);
        self.current = yes;
        let value = self.block(then)?;
        self.assign(Place::of(result), Rvalue::Use(value), origin);
        self.terminate(TerminatorKind::Goto(join), origin);
        self.current = no;
        let value = match otherwise {
            Some(e) => self.operand(e)?,
            None => Self::unit(),
        };
        self.assign(Place::of(result), Rvalue::Use(value), origin);
        self.terminate(TerminatorKind::Goto(join), origin);
        self.current = join;
        Ok(Operand::Place(Place::of(result)))
    }

    /// `left and right`, `left or right`: `right` runs only when `left` does not decide.
    pub(super) fn logic(
        &mut self,
        op: TokenKind,
        left: ExprId,
        right: ExprId,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let result = self.result(origin);
        let value = self.operand(left)?;
        self.assign(Place::of(result), Rvalue::Use(value), origin);
        let (rest, join) = (self.fresh(origin), self.fresh(origin));
        let cond = Operand::Place(Place::of(result));
        let kind = if op == TokenKind::And {
            TerminatorKind::Branch { cond, then: rest, otherwise: join }
        } else {
            TerminatorKind::Branch { cond, then: join, otherwise: rest }
        };
        self.terminate(kind, origin);
        self.current = rest;
        let value = self.operand(right)?;
        self.assign(Place::of(result), Rvalue::Use(value), origin);
        self.terminate(TerminatorKind::Goto(join), origin);
        self.current = join;
        Ok(Operand::Place(Place::of(result)))
    }
}
