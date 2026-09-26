//! How a block decides where to go from one of its parameters: branching or switching on it, or
//! branching on its comparison with a constant.

use vun_index::IndexVec;

use crate::entity::{Block, Inst};
use crate::func::{Def, Function};
use crate::inst::{Cond, InstKind, Target, Terminator};
use crate::ty::Scalar;

/// A block's decision on its parameter at `param`.
pub(super) struct Decided {
    pub param: usize,
    how: How,
}

enum How {
    /// The branch or switch reads the parameter itself.
    Operand,
    /// The branch reads the comparison of the parameter with a constant.
    Compared { op: Cond, ty: Scalar, constant: u128, param_first: bool },
}

impl Function {
    /// How `block` decides, `homes` placing the instructions, when all it computes is constants and at most the comparison it
    /// branches on, and its targets read nothing it computes.
    pub(super) fn decides(&self, block: Block, homes: &IndexVec<Inst, Option<Block>>) -> Option<Decided> {
        let data = &self.blocks[block];
        let computed: Vec<Inst> = data
            .insts
            .iter()
            .copied()
            .filter(|&i| !matches!(self.insts[i].kind, InstKind::Const { .. }))
            .collect();
        let (param, how) = match (&data.term, computed.as_slice()) {
            (Terminator::Branch { cond: v, .. } | Terminator::Switch { value: v, .. }, []) => {
                (self.param(block, *v)?, How::Operand)
            }
            (Terminator::Branch { cond, .. }, &[compare]) => {
                let InstKind::Compare { cond: op, a, b } = self.insts[compare].kind else { return None };
                if self.insts[compare].result != Some(*cond) {
                    return None;
                }
                match (self.param(block, a), self.constant(b), self.param(block, b), self.constant(a)) {
                    (Some(at), Some((ty, constant)), _, _) => {
                        (at, How::Compared { op, ty, constant, param_first: true })
                    }
                    (_, _, Some(at), Some((ty, constant))) => {
                        (at, How::Compared { op, ty, constant, param_first: false })
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        let computed_here = |v| matches!(self.values[v].def, Def::Inst(i) if homes[i] == Some(block));
        if data.term.targets().flat_map(|t| &t.args).any(|&v| computed_here(v)) {
            return None;
        }
        Some(Decided { param, how })
    }
}

impl Decided {
    /// Where `term`, the deciding block's, goes with `bits` as the parameter.
    pub(super) fn taken<'t>(&self, term: &'t Terminator, bits: u128) -> Option<&'t Target> {
        match (term, &self.how) {
            (Terminator::Branch { then, otherwise, .. }, How::Operand) => {
                Some(if bits == 0 { otherwise } else { then })
            }
            (
                Terminator::Branch { then, otherwise, .. },
                &How::Compared { op, ty, constant, param_first },
            ) => {
                let (a, b) = if param_first { (bits, constant) } else { (constant, bits) };
                Some(if op.fold(ty, a, b)? { then } else { otherwise })
            }
            (Terminator::Switch { cases, default, .. }, How::Operand) => {
                Some(cases.iter().find(|(case, _)| *case == bits).map_or(default, |(_, target)| target))
            }
            _ => None,
        }
    }
}
