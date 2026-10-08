//! Folding a function's operations on constants after a pass made them so, as promotion does
//! when a load becomes the constant stored: what the builder folds as it builds, and a branch on
//! a constant becomes a jump.

#[cfg(test)]
mod tests;

use vun_hash::Set;

use crate::replaced::Replaced;
use vun_lir::analysis::{Cfg, Loops};
use vun_lir::entity::{Block, Inst, Value};
use vun_lir::func::Function;
use vun_lir::inst::{BinOp, InstKind, Right, Terminator};
use vun_lir::ty::Scalar;

pub trait Fold {
    /// Folds every operation whose operands are constants into the constant it gives, drops
    /// identities and selects on constants for the value they give, and turns a branch or switch
    /// on a constant into a jump, returning how many were.
    ///
    /// # Panics
    /// Never: an operation folding to one of its operands defines a value.
    fn fold(&mut self) -> usize;
}

impl Fold for Function {
    fn fold(&mut self) -> usize {
        let mut replaced = Replaced::default();
        let mut dropped: Set<Inst> = Set::default();
        for block in self.definitions_first() {
            for inst in self.blocks[block].insts.clone() {
                let mut kind = self.insts[inst].kind.clone();
                kind.map_values(|v| replaced.resolve(v));
                // A constant operand of a commutative operation goes right, where folds and
                // instructions taking an immediate read it.
                if let InstKind::Binary { op, a, b } = &mut kind
                    && op.commutes()
                    && self.constant(*a).is_some()
                    && self.constant(*b).is_none()
                {
                    std::mem::swap(a, b);
                }
                let result = self.insts[inst].result;
                match self.folded(&kind) {
                    Some(Folded::Constant(ty, bits)) => kind = InstKind::Const { ty, bits },
                    Some(Folded::Value(v)) => {
                        replaced.insert(result.expect("a folded operation defines its value"), v);
                        dropped.insert(inst);
                    }
                    None => {}
                }
                self.insts[inst].kind = kind;
            }
        }
        let mut count = replaced.len();
        for (_, data) in self.blocks.iter_mut() {
            data.insts.retain(|inst| !dropped.contains(inst));
            // A block no path reaches is visited after the rest, in no order of definitions, so
            // an operation there may read a value another folded after it was read.
            for &inst in &data.insts {
                self.insts[inst].kind.map_values(|v| replaced.resolve(v));
            }
            data.term.map_values(|v| replaced.resolve(v));
        }
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).collect();
        for block in blocks {
            if let Some(jump) = self.decided(&self.blocks[block].term) {
                self.blocks[block].term = jump;
                count += 1;
            }
        }
        if !dropped.is_empty() {
            self.compact();
        }
        count
    }
}

trait Steps {
    /// The blocks, those the entry reaches in reverse postorder, so a definition comes before
    /// its uses, then the others.
    fn definitions_first(&self) -> Vec<Block>;

    fn folded(&self, kind: &InstKind) -> Option<Folded>;

    /// Whether a comparison defines `value`.
    fn compared(&self, value: Value) -> bool;

    /// The jump a branch or switch on a constant takes, or one whose targets are all one.
    fn decided(&self, term: &Terminator) -> Option<Terminator>;
}

impl Steps for Function {
    fn definitions_first(&self) -> Vec<Block> {
        let loops = Loops::of(self, &Cfg::of(self));
        let reached: Set<Block> = loops.rpo().iter().copied().collect();
        let rest = self.blocks.iter().map(|(b, _)| b).filter(|b| !reached.contains(b));
        loops.rpo().iter().copied().chain(rest).collect()
    }

    fn folded(&self, kind: &InstKind) -> Option<Folded> {
        match *kind {
            InstKind::Binary { op, a, b } => {
                let ty = self.ty(a);
                let kb = self.constant(b).map(|(_, k)| k);
                if let (Some((_, ka)), Some(kb)) = (self.constant(a), kb)
                    && let Some(bits) = op.fold(ty, ka, kb)
                {
                    return Some(Folded::Constant(ty, bits));
                }
                // A comparison is 0 or 1: its low bit decides a mask.
                if op == BinOp::And
                    && self.compared(a)
                    && let Some(kb) = kb
                {
                    return Some(if kb & 1 == 1 { Folded::Value(a) } else { Folded::Constant(ty, 0) });
                }
                match op.with_right(ty, kb?)? {
                    Right::Left => Some(Folded::Value(a)),
                    Right::Constant(bits) => Some(Folded::Constant(ty, bits)),
                }
            }
            InstKind::Compare { cond, a, b } => {
                let ((ty, ka), (_, kb)) = (self.constant(a)?, self.constant(b)?);
                cond.fold(ty, ka, kb).map(|holds| Folded::Constant(Scalar::I8, u128::from(holds)))
            }
            InstKind::Cast { op, value, to } => {
                let (from, bits) = self.constant(value)?;
                op.fold(from, to, bits).map(|bits| Folded::Constant(to, bits))
            }
            InstKind::Select { cond, then, otherwise } => {
                let (_, bits) = self.constant(cond)?;
                Some(Folded::Value(if bits == 0 { otherwise } else { then }))
            }
            _ => None,
        }
    }

    fn compared(&self, value: Value) -> bool {
        matches!(self.values[value].def, vun_lir::func::Def::Inst(i) if matches!(self.insts[i].kind, InstKind::Compare { .. }))
    }

    fn decided(&self, term: &Terminator) -> Option<Terminator> {
        let mut targets = term.targets();
        if let Some(first) = targets.next()
            && matches!(term, Terminator::Branch { .. } | Terminator::Switch { .. })
            && targets.all(|t| t == first)
        {
            return Some(Terminator::Jump(first.clone()));
        }
        let taken = match term {
            Terminator::Branch { cond, then, otherwise } => {
                let (_, bits) = self.constant(*cond)?;
                if bits == 0 { otherwise } else { then }
            }
            Terminator::Switch { value, cases, default } => {
                let (_, bits) = self.constant(*value)?;
                cases.iter().find(|(case, _)| *case == bits).map_or(default, |(_, target)| target)
            }
            _ => return None,
        };
        Some(Terminator::Jump(taken.clone()))
    }
}

/// What an operation folds to.
enum Folded {
    Constant(Scalar, u128),
    /// One of its operands, as `x * 1` is.
    Value(Value),
}
