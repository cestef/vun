//! Live ranges (after Kohn, Leis and Neumann, 2018), coarse as a single-pass backend needs them:
//! for each value, the span of positions in [`Order`] from its definition to its last use, whether
//! it lives to the end of that last block, and how many uses it has. A use inside a loop that
//! does not hold the definition keeps the value live to the end of the outermost such loop, since
//! the loop's back edge reaches the use again.

use vun_index::IndexVec;

use crate::entity::{Block, Value};
use crate::func::{Def, Function};

use super::{Loops, Order};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    /// The position of the block defining the value.
    pub first: u32,
    /// The position of the last block it is live in.
    pub last: u32,
    /// Whether it lives to the end of `last`, rather than ending at its last use there.
    pub through: bool,
    pub uses: u32,
}

/// The range of each value; `None` for one defined where the entry does not reach.
pub struct Liveness(IndexVec<Value, Option<Range>>);

impl Liveness {
    #[must_use]
    pub fn of(func: &Function, loops: &Loops, order: &Order) -> Self {
        let mut home: IndexVec<crate::entity::Inst, Option<Block>> = IndexVec::filled(None, func.insts.len());
        for (block, data) in func.blocks.iter() {
            for &inst in &data.insts {
                home[inst] = Some(block);
            }
        }
        let ranges = func
            .values
            .iter()
            .map(|(_, data)| {
                let block = match data.def {
                    Def::Param(block, _) => Some(block),
                    Def::Inst(inst) => home[inst],
                };
                let first = block.and_then(|b| order.position(b))?;
                Some(Range { first, last: first, through: false, uses: 0 })
            })
            .collect::<Vec<_>>();
        let mut live = Self(ranges.into());
        for &block in order.blocks() {
            let data = &func.blocks[block];
            let operands = data.insts.iter().flat_map(|&i| func.insts[i].kind.operands());
            for value in operands.chain(data.term.operands()) {
                live.used(value, block, loops, order);
            }
        }
        live
    }

    fn used(&mut self, value: Value, block: Block, loops: &Loops, order: &Order) {
        let (Some(Some(range)), Some(at)) = (self.0.get(value).copied(), order.position(block)) else {
            return;
        };
        let mut outermost = None;
        let mut current = loops.innermost(block);
        while let Some(header) = current {
            let (start, end) = order.span(header);
            if (start..=end).contains(&range.first) {
                break;
            }
            outermost = Some(end);
            current = loops.parent(header);
        }
        let (end, through) = outermost.map_or((at, false), |end| (end, true));
        let range = self.0[value].as_mut().expect("checked above");
        range.uses += 1;
        if end > range.last {
            (range.last, range.through) = (end, through);
        } else if end == range.last {
            range.through |= through;
        }
    }

    #[must_use]
    pub fn range(&self, value: Value) -> Option<Range> {
        self.0.get(value).copied().flatten()
    }
}
