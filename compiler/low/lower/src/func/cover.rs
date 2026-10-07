//! Coverage (spec 11, section 6): a block of a covered program adds 1 to its count in
//! `lang.cover.map` as it starts, holding at the greatest `u32`.

use vun_index::Idx;
use vun_lir::{Access, Address, BinOp, Cond, Scalar};
use vun_mir::{BlockId, Map, Region};

use super::FnLower;
use crate::unlowered::Unlowered;

/// The bytes of a count.
const COUNT: u64 = 4;

impl FnLower<'_, '_, '_> {
    /// Counts the block `block` as it starts, when the program counts them.
    pub(super) fn count(&mut self, block: BlockId) -> Result<(), Unlowered> {
        let Some(i) = self.l.covering.clone() else { return Ok(()) };
        let Some(map) = Map::of(&self.l.env) else { return Ok(()) };
        let number = Region::number(&i, block.index(), map.size);
        let data = self.l.global(&map.decl, map.ty, true, self.at)?;
        let base = self.b.addr(Address::Data(data), self.at);
        let at = self.at_offset(base, number * COUNT);
        let count = self.b.load(Scalar::I32, at, Access::Plain, self.at);
        let one = self.b.constant(Scalar::I32, 1, self.at);
        let next = self.b.binary(BinOp::Add, count, one, self.at);
        let zero = self.b.constant(Scalar::I32, 0, self.at);
        let wrapped = self.b.compare(Cond::Eq, next, zero, self.at);
        let kept = self.b.select(wrapped, count, next, self.at);
        self.b.store(kept, at, Access::Plain, self.at);
        Ok(())
    }
}
