//! The in-bounds rule of `offset` (spec 13, section 3): an offset stays inside the object its
//! base points into, or one past its end. The verifier checks it where both are known: a chain
//! of constant offsets from the address of a slot or of a data object.

use crate::entity::Value;
use crate::func::Def;
use crate::inst::{Address, InstKind};

use super::{Checker, Entity, Malformed, Reason};

/// Where a pointer is, within the object it points into.
struct Place {
    size: u64,
    at: i128,
}

impl Place {
    fn inside(&self) -> bool {
        (0..=i128::from(self.size)).contains(&self.at)
    }
}

impl Checker<'_> {
    /// Every offset whose place is known stays inside its object. Run once dominance holds, so
    /// the chains of bases end.
    pub(super) fn bounds(&self) -> Result<(), Malformed> {
        for (inst, data) in self.func.insts.iter() {
            let (InstKind::Offset { .. }, Some(result)) = (&data.kind, data.result) else { continue };
            if let Some(place) = self.place(result)
                && !place.inside()
            {
                let reason = Reason::OutOfBounds { size: place.size, at: place.at };
                return Self::fail(Entity::Inst(self.id, inst), reason);
            }
        }
        Ok(())
    }

    /// The object `value` points into and how far into it, when a chain of constant offsets
    /// from its address gives both.
    fn place(&self, mut value: Value) -> Option<Place> {
        let mut at = 0;
        loop {
            let Def::Inst(inst) = self.func.values[value].def else { return None };
            let size = match self.func.insts[inst].kind {
                InstKind::SlotAddr(slot) => self.func.slots[slot].size,
                InstKind::Addr(Address::Data(d)) | InstKind::ThreadAddr(d) => {
                    self.unit.data[d].bytes.len() as u64
                }
                InstKind::Offset { base, offset } => {
                    at += self.signed(self.func.constant(offset)?.1);
                    value = base;
                    continue;
                }
                _ => return None,
            };
            return Some(Place { size, at });
        }
    }

    /// `bits` of an address-sized integer, read as signed.
    fn signed(&self, bits: u128) -> i128 {
        let shift = u128::BITS - self.word().bits(self.unit.pointer_bits);
        (bits << shift).cast_signed() >> shift
    }
}
