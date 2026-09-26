//! Reusing values (spec 13, section 3): a pure instruction (`InstKind::pure`) equal to one
//! dominating it computes what that one already has; it goes, and its uses read that one's
//! value. The dominator tree is walked from the entry holding the computations of the blocks on
//! the path, so each is looked up once. Lowering computes an address or constant where it is
//! used, and inlining and scalarizing repeat them.

use vun_hash::{Map, Set};

use crate::analysis::{Cfg, Dominators};
use crate::entity::{Block, Inst, Value};
use crate::func::Function;
use crate::inst::InstKind;
use crate::replaced::Replaced;

/// A step of the walk down the dominator tree.
enum Step {
    Enter(Block),
    /// Leaving a block: the computations recorded from this many on are no longer seen.
    Leave(usize),
}

impl Function {
    /// Removes every pure instruction computing what one dominating it does, returning how many
    /// went.
    pub fn reuse(&mut self) -> usize {
        let Some(entry) = self.entry() else { return 0 };
        let children = Dominators::of(self, &Cfg::of(self)).children();
        let mut seen: Map<InstKind, Value> = Map::default();
        let mut recorded: Vec<InstKind> = Vec::new();
        let mut replaced = Replaced::default();
        let mut dropped: Set<Inst> = Set::default();
        let mut steps = vec![Step::Enter(entry)];
        while let Some(step) = steps.pop() {
            let block = match step {
                Step::Leave(from) => {
                    for kind in recorded.drain(from..) {
                        seen.remove(&kind);
                    }
                    continue;
                }
                Step::Enter(block) => block,
            };
            steps.push(Step::Leave(recorded.len()));
            for &inst in &self.blocks[block].insts {
                let data = &mut self.insts[inst];
                data.kind.map_values(|v| replaced.resolve(v));
                let Some(result) = data.result.filter(|_| data.kind.pure()) else { continue };
                let kind = data.kind.ordered();
                if let Some(&same) = seen.get(&kind) {
                    replaced.insert(result, same);
                    dropped.insert(inst);
                } else {
                    seen.insert(kind.clone(), result);
                    recorded.push(kind);
                }
            }
            steps.extend(children[block].iter().rev().map(|&child| Step::Enter(child)));
        }
        if !dropped.is_empty() {
            for (_, data) in self.blocks.iter_mut() {
                data.insts.retain(|inst| !dropped.contains(inst));
                for &inst in &data.insts {
                    self.insts[inst].kind.map_values(|v| replaced.resolve(v));
                }
                data.term.map_values(|v| replaced.resolve(v));
            }
            self.compact();
        }
        dropped.len()
    }
}
