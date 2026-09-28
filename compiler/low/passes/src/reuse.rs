//! Reusing values (spec 13, section 3): a pure instruction (`InstKind::pure`) equal to one
//! dominating it computes what that one already has; it goes, and its uses read that one's
//! value. A block entered by one edge of a branch or switch also knows what the edge implies: a
//! condition is 0 past its `else`, a comparison 1 past its `then`, `x == k` makes `x` the
//! constant `k`, and a switch's case its value; blocks it dominates read those. The dominator
//! tree is walked from the entry holding what the blocks on the path computed and learned, so
//! each is looked up once. Lowering computes an address or constant where it is used, and
//! inlining and scalarizing repeat them.

use vun_hash::{Map, Set};
use vun_index::IndexVec;

use crate::replaced::Replaced;
use vun_lir::analysis::{Cfg, Dominators};
use vun_lir::entity::{Block, Inst, Value};
use vun_lir::func::{Def, Function};
use vun_lir::inst::{Cond, InstKind, Terminator};
use vun_lir::ty::Scalar;

/// A step of the walk down the dominator tree.
enum Step {
    Enter(Block),
    /// Leaving a block: what was recorded and learned from these counts on is forgotten.
    Leave(usize, usize),
}

/// What the walk holds on the path from the entry.
#[derive(Default)]
struct Walk {
    seen: Map<InstKind, Value>,
    recorded: Vec<InstKind>,
    /// Values known equal to another on the path, by what the edges taken imply.
    known: Map<Value, Value>,
    learned: Vec<Value>,
    /// Reads rewritten to a value known.
    rewritten: usize,
    replaced: Replaced,
    /// How many times each value is read, before the walk.
    uses: IndexVec<Value, u32>,
}

impl Walk {
    /// What stands for `value` here, without counting a rewrite.
    fn look(&self, value: Value) -> Value {
        let value = self.replaced.resolve(value);
        self.known.get(&value).copied().unwrap_or(value)
    }

    /// What stands for `value` here, where a read of it is rewritten.
    fn resolve(&mut self, value: Value) -> Value {
        let value = self.replaced.resolve(value);
        match self.known.get(&value) {
            Some(&known) => {
                self.rewritten += 1;
                known
            }
            None => value,
        }
    }

    /// Whether `value` is read other than by the branch deciding on it.
    fn read_beyond(&self, value: Value) -> bool {
        self.uses.get(value).is_some_and(|&n| n > 1)
    }

    fn learn(&mut self, value: Value, is: Value) {
        if value != is && !self.known.contains_key(&value) {
            self.known.insert(value, is);
            self.learned.push(value);
        }
    }
}

pub trait Reuse {
    /// Removes every pure instruction computing what one dominating it does, and reads what
    /// the edges into a block imply in the blocks it dominates, returning how many
    /// instructions went and reads were rewritten.
    fn reuse(&mut self) -> usize;
}

impl Reuse for Function {
    fn reuse(&mut self) -> usize {
        let Some(entry) = self.entry() else { return 0 };
        let cfg = Cfg::of(self);
        let children = Dominators::of(self, &cfg).children();
        let mut walk = Walk { uses: self.uses(), ..Walk::default() };
        let mut dropped: Set<Inst> = Set::default();
        let mut steps = vec![Step::Enter(entry)];
        while let Some(step) = steps.pop() {
            let block = match step {
                Step::Leave(recorded, known) => {
                    for kind in walk.recorded.drain(recorded..) {
                        walk.seen.remove(&kind);
                    }
                    for value in walk.learned.drain(known..) {
                        walk.known.remove(&value);
                    }
                    continue;
                }
                Step::Enter(block) => block,
            };
            steps.push(Step::Leave(walk.recorded.len(), walk.learned.len()));
            if let [pred] = cfg.preds[block].as_slice() {
                self.learn_edge(*pred, block, &mut walk);
            }
            for &inst in &self.blocks[block].insts {
                let data = &mut self.insts[inst];
                data.kind.map_values(|v| walk.resolve(v));
                let Some(result) = data.result.filter(|_| data.kind.pure()) else { continue };
                let kind = data.kind.ordered();
                if let Some(&same) = walk.seen.get(&kind) {
                    walk.replaced.insert(result, same);
                    dropped.insert(inst);
                } else {
                    walk.seen.insert(kind.clone(), result);
                    walk.recorded.push(kind);
                }
            }
            self.blocks[block].term.map_values(|v| walk.resolve(v));
            steps.extend(children[block].iter().rev().map(|&child| Step::Enter(child)));
        }
        for (_, data) in self.blocks.iter_mut() {
            data.insts.retain(|inst| !dropped.contains(inst));
            for &inst in &data.insts {
                self.insts[inst].kind.map_values(|v| walk.replaced.resolve(v));
            }
            data.term.map_values(|v| walk.replaced.resolve(v));
        }
        if !dropped.is_empty() {
            self.compact();
        }
        dropped.len() + walk.rewritten
    }
}

trait Steps {
    /// Learns what the edge from `pred`, its only predecessor, implies in `block`.
    fn learn_edge(&mut self, pred: Block, block: Block, walk: &mut Walk);

    /// A constant defined first in `block`.
    fn constant_at(&mut self, block: Block, ty: Scalar, bits: u128) -> Value;
}

impl Steps for Function {
    fn learn_edge(&mut self, pred: Block, block: Block, walk: &mut Walk) {
        match self.blocks[pred].term.clone() {
            Terminator::Branch { cond, then, otherwise } if then.block != otherwise.block => {
                let cond = walk.look(cond);
                let holds = then.block == block;
                let compare = match self.values[cond].def {
                    Def::Inst(i) => match self.insts[i].kind {
                        InstKind::Compare { cond: op, a, b } => Some((op, a, b)),
                        _ => None,
                    },
                    Def::Param(..) => None,
                };
                if (!holds || compare.is_some()) && walk.read_beyond(cond) {
                    let known = self.constant_at(block, self.ty(cond), u128::from(holds));
                    walk.learn(cond, known);
                }
                if let Some((op, a, b)) = compare
                    && matches!((op, holds), (Cond::Eq, true) | (Cond::Ne, false))
                {
                    let (a, b) = (walk.look(a), walk.look(b));
                    match (self.constant(a), self.constant(b)) {
                        (None, Some(_)) => walk.learn(a, b),
                        (Some(_), None) => walk.learn(b, a),
                        _ => {}
                    }
                }
            }
            Terminator::Switch { value, cases, default } if default.block != block => {
                let mut to = cases.iter().filter(|(_, t)| t.block == block);
                if let (Some(&(case, _)), None) = (to.next(), to.next()) {
                    let value = walk.look(value);
                    if walk.read_beyond(value) {
                        let known = self.constant_at(block, self.ty(value), case);
                        walk.learn(value, known);
                    }
                }
            }
            _ => {}
        }
    }

    fn constant_at(&mut self, block: Block, ty: Scalar, bits: u128) -> Value {
        let origin = self.blocks[block].origin;
        let (inst, value) = self.define(InstKind::Const { ty, bits }, Some(ty), origin);
        self.blocks[block].insts.insert(0, inst);
        value.expect("a constant defines its value")
    }
}
