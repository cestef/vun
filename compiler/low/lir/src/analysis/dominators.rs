//! The dominator tree (Cooper, Harvey and Kennedy, 2001): each block's immediate dominator,
//! iterated to the fixed point over the reverse postorder. A block no path reaches is dominated
//! by every block, so nothing in it is rejected for its values.

use vun_index::IndexVec;

use crate::entity::Block;
use crate::func::Function;

use super::{Cfg, Loops};

pub struct Dominators {
    /// The immediate dominator of each block reached; the entry's is itself.
    idom: IndexVec<Block, Option<Block>>,
    /// Each block's position in the reverse postorder.
    order: IndexVec<Block, usize>,
    rpo: Vec<Block>,
    /// Each block's span in a walk of the tree: a block dominates those inside its own.
    spans: IndexVec<Block, (u32, u32)>,
}

impl Dominators {
    #[must_use]
    pub fn of(func: &Function, cfg: &Cfg) -> Self {
        let rpo = Loops::of(func, cfg).rpo().to_vec();
        let count = func.blocks.len();
        let mut order = IndexVec::filled(usize::MAX, count);
        for (i, &block) in rpo.iter().enumerate() {
            order[block] = i;
        }
        let mut idom: IndexVec<Block, Option<Block>> = IndexVec::filled(None, count);
        let spans = IndexVec::filled((u32::MAX, 0), count);
        let Some(&entry) = rpo.first() else { return Self { idom, order, rpo, spans } };
        idom[entry] = Some(entry);
        let mut changed = true;
        while changed {
            changed = false;
            for &block in &rpo[1..] {
                let mut preds = cfg.preds[block].iter().copied().filter(|&p| idom[p].is_some());
                let Some(first) = preds.next() else { continue };
                let next = preds.fold(first, |a, b| Self::meet(&idom, &order, a, b));
                if idom[block] != Some(next) {
                    idom[block] = Some(next);
                    changed = true;
                }
            }
        }
        let mut tree = Self { idom, order, rpo, spans };
        tree.span(entry);
        tree
    }

    /// Numbers the blocks in a walk of the tree from `entry`.
    fn span(&mut self, entry: Block) {
        let children = self.children();
        let mut clock = 0;
        let mut open = vec![(entry, 0)];
        while let Some((block, next)) = open.pop() {
            if next == 0 {
                self.spans[block].0 = clock;
                clock += 1;
            }
            match children[block].get(next) {
                Some(&child) => {
                    open.push((block, next + 1));
                    open.push((child, 0));
                }
                None => self.spans[block].1 = clock,
            }
        }
    }

    /// The nearest block dominating both `a` and `b`, walking up the tree built so far.
    fn meet(
        idom: &IndexVec<Block, Option<Block>>,
        order: &IndexVec<Block, usize>,
        mut a: Block,
        mut b: Block,
    ) -> Block {
        while a != b {
            while order[a] > order[b] {
                a = idom[a].expect("a block walked up from is reached");
            }
            while order[b] > order[a] {
                b = idom[b].expect("a block walked up from is reached");
            }
        }
        a
    }

    /// Whether every path from the entry to `block` goes through `def`.
    #[must_use]
    pub fn dominates(&self, def: Block, block: Block) -> bool {
        if self.idom.get(block).copied().flatten().is_none() {
            return true;
        }
        let (def, block) = (self.spans[def], self.spans[block]);
        def.0 <= block.0 && block.1 <= def.1
    }

    /// The block immediately dominating `block`, when it is reached and not the entry.
    #[must_use]
    pub fn idom(&self, block: Block) -> Option<Block> {
        self.idom.get(block).copied().flatten().filter(|&up| up != block)
    }

    /// The blocks each block immediately dominates, in reverse postorder.
    #[must_use]
    pub fn children(&self) -> IndexVec<Block, Vec<Block>> {
        let mut children = IndexVec::filled(Vec::new(), self.idom.len());
        for &block in self.rpo.iter().skip(1) {
            if let Some(up) = self.idom[block] {
                children[up].push(block);
            }
        }
        children
    }

    /// The blocks reached, in reverse postorder: each after its dominators.
    #[must_use]
    pub fn rpo(&self) -> &[Block] {
        &self.rpo
    }

    /// Whether `a` comes before `b` in reverse postorder.
    #[must_use]
    pub fn before(&self, a: Block, b: Block) -> bool {
        self.order[a] < self.order[b]
    }
}
