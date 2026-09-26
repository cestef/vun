//! Threading jumps (spec 13, section 3): a block that only compares a parameter with a constant
//! and branches, as the join of `if c { true } else { false }` does, is jumped over by each
//! predecessor passing it a constant, which goes where the comparison sends that constant; and a
//! block that only jumps is jumped over by its predecessors. Blocks no longer reached stay, and
//! backends leave them out.

#[cfg(test)]
mod tests;

use vun_hash::Set;

use crate::analysis::Cfg;
use crate::entity::{Block, Value};
use crate::func::{Def, Function};
use crate::inst::{InstKind, Target, Terminator};

impl Function {
    /// Threads jumps through blocks deciding on a constant and blocks only jumping, until none
    /// is left, returning how many jumps were moved.
    pub fn thread(&mut self) -> usize {
        let mut moved = 0;
        loop {
            let now = self.thread_constants() + self.forward();
            if now == 0 {
                return moved;
            }
            moved += now;
        }
    }

    /// Moves each jump passing a constant to a block that compares it and branches to where the
    /// branch goes for that constant.
    fn thread_constants(&mut self) -> usize {
        let preds = Cfg::of(self).preds;
        let outside = self.read_outside();
        let mut moves = Vec::new();
        for (join, data) in self.blocks.iter() {
            let Some(decided) = self.decides(join, &outside) else { continue };
            for &pred in &preds[join] {
                let Terminator::Jump(to) = &self.blocks[pred].term else { continue };
                let Some(bits) = to.args.get(decided.param).and_then(|&v| self.constant(v)) else { continue };
                let Some(holds) = decided.fold(bits.1) else { continue };
                let Terminator::Branch { then, otherwise, .. } = &data.term else {
                    unreachable!("a deciding block branches")
                };
                let taken = if holds { then } else { otherwise };
                // The join's parameters, as the predecessor passes them.
                let args = taken.args.iter().map(|&v| match data.params.iter().position(|&p| p == v) {
                    Some(at) => to.args[at],
                    None => v,
                });
                moves.push((pred, Target { block: taken.block, args: args.collect() }));
            }
        }
        let count = moves.len();
        for (pred, target) in moves {
            self.blocks[pred].term = Terminator::Jump(target);
        }
        count
    }

    /// Moves each jump to a block that holds only constants and jumps without arguments to
    /// where that block jumps.
    fn forward(&mut self) -> usize {
        let preds = Cfg::of(self).preds;
        let mut count = 0;
        let entry = self.entry();
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).filter(|&b| Some(b) != entry).collect();
        for through in blocks {
            let data = &self.blocks[through];
            let Terminator::Jump(onward) = &data.term else { continue };
            let only_constants =
                data.insts.iter().all(|&i| matches!(self.insts[i].kind, InstKind::Const { .. }));
            let local = |v: Value| matches!(self.values[v].def, Def::Inst(i) if data.insts.contains(&i));
            if !data.params.is_empty()
                || !only_constants
                || onward.block == through
                || onward.args.iter().any(|&v| local(v))
            {
                continue;
            }
            let onward = onward.clone();
            for &pred in &preds[through] {
                let term = &self.blocks[pred].term;
                // One edge per pair: a jump already going there with other arguments stays.
                if term.targets().any(|t| t.block == onward.block && t.args != onward.args) {
                    continue;
                }
                for target in self.blocks[pred].term.targets_mut().filter(|t| t.block == through) {
                    *target = onward.clone();
                    count += 1;
                }
            }
        }
        count
    }

    /// The values read outside the block defining them: what a block jumped over must not
    /// define, since the blocks after it would read what no longer is.
    fn read_outside(&self) -> Set<Value> {
        let mut outside = Set::default();
        for (block, data) in self.blocks.iter() {
            let here = |v: Value| match self.values[v].def {
                Def::Param(b, _) => b == block,
                Def::Inst(i) => data.insts.contains(&i),
            };
            let reads =
                data.insts.iter().flat_map(|&i| self.insts[i].kind.operands()).chain(data.term.operands());
            outside.extend(reads.filter(|&v| !here(v)));
        }
        outside
    }

    /// How `block` decides, when it only compares a parameter with a constant and branches on
    /// that, nothing it defines read `outside` it, and its targets reading nothing it defines
    /// but its parameters.
    fn decides(&self, block: Block, outside: &Set<Value>) -> Option<Decided> {
        let data = &self.blocks[block];
        let defined = data.insts.iter().filter_map(|&i| self.insts[i].result);
        if data.params.iter().copied().chain(defined).any(|v| outside.contains(&v)) {
            return None;
        }
        let Terminator::Branch { cond, then, otherwise } = &data.term else { return None };
        let mut compares =
            data.insts.iter().filter(|&&i| !matches!(self.insts[i].kind, InstKind::Const { .. }));
        let (Some(&compare), None) = (compares.next(), compares.next()) else { return None };
        let InstKind::Compare { cond: op, a, b } = self.insts[compare].kind else { return None };
        if self.insts[compare].result != Some(*cond) {
            return None;
        }
        let (param, constant, param_first) =
            match (self.param(block, a), self.constant(b), self.param(block, b), self.constant(a)) {
                (Some(at), Some(k), _, _) => (at, k, true),
                (_, _, Some(at), Some(k)) => (at, k, false),
                _ => return None,
            };
        let defined_here = |v: Value| match self.values[v].def {
            Def::Inst(i) => data.insts.contains(&i),
            Def::Param(..) => false,
        };
        if [then, otherwise].iter().flat_map(|t| &t.args).any(|&v| defined_here(v)) {
            return None;
        }
        Some(Decided { op, ty: constant.0, constant: constant.1, param, param_first })
    }

    /// The position of `value` among the parameters of `block`, if it is one.
    fn param(&self, block: Block, value: Value) -> Option<usize> {
        self.blocks[block].params.iter().position(|&p| p == value)
    }
}

/// A comparison of a block's parameter with a constant, on which the block branches.
struct Decided {
    op: crate::inst::Cond,
    ty: crate::ty::Scalar,
    constant: u128,
    /// The parameter's position.
    param: usize,
    /// Whether the parameter is the comparison's first operand.
    param_first: bool,
}

impl Decided {
    /// Whether the comparison holds with `bits` as the parameter.
    fn fold(&self, bits: u128) -> Option<bool> {
        let (a, b) = if self.param_first { (bits, self.constant) } else { (self.constant, bits) };
        self.op.fold(self.ty, a, b)
    }
}
