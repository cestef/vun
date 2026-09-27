//! Which values are live into each block: the fixpoint of the usual backward dataflow, over the
//! values that are kept somewhere (not those recomputed where used).

use vun_index::{BitSet, IndexVec};
use vun_lir::analysis::Cfg;
use vun_lir::{Block, Function, Value};

/// The values live into each block, kept ones only.
pub(crate) struct Live {
    pub(crate) into: IndexVec<Block, BitSet<Value>>,
}

impl Live {
    /// # Panics
    /// Never: every block of `order` belongs to `f`.
    #[must_use]
    pub(crate) fn of(f: &Function, cfg: &Cfg, order: &[Block], kept: impl Fn(Value) -> bool) -> Self {
        let mut into: IndexVec<Block, BitSet<Value>> = IndexVec::filled(BitSet::new(), f.blocks.len());
        let mut changed = true;
        while changed {
            changed = false;
            for &block in order.iter().rev() {
                let mut live = Self::out(f, cfg, &into, block, &kept);
                let data = &f.blocks[block];
                for &inst in data.insts.iter().rev() {
                    if let Some(v) = f.insts[inst].result {
                        live.remove(v);
                    }
                    for v in f.insts[inst].kind.operands().filter(|&v| kept(v)) {
                        live.insert(v);
                    }
                }
                for &p in &data.params {
                    live.remove(p);
                }
                if live != into[block] {
                    into[block] = live;
                    changed = true;
                }
            }
        }
        Self { into }
    }

    /// The values live out of `block`: those live into its successors, and those its
    /// terminator reads.
    #[must_use]
    pub(crate) fn out(
        f: &Function,
        cfg: &Cfg,
        into: &IndexVec<Block, BitSet<Value>>,
        block: Block,
        kept: impl Fn(Value) -> bool,
    ) -> BitSet<Value> {
        let mut live = BitSet::new();
        for &succ in &cfg.succs[block] {
            live.union(&into[succ]);
        }
        for v in f.blocks[block].term.operands().filter(|&v| kept(v)) {
            live.insert(v);
        }
        live
    }
}
