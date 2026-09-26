//! Promoting stack slots to values (spec 13, section 3): lowering gives every local of a body a
//! slot, and a slot that only plain loads and stores of one scalar filling it reach is a value
//! in disguise. Its stores become definitions and its loads uses of the definition reaching
//! them, through block parameters where definitions meet, and the slot goes. Built as Braun et
//! al. build SSA, with every block's predecessors known.

mod ssa;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::entity::{Slot, Value};
use crate::func::Function;
use crate::inst::{Access, InstKind};
use crate::ty::Scalar;
use ssa::Promotion;

impl Function {
    /// Promotes each slot that only plain loads and stores of one scalar as wide as it reach, by
    /// its address alone, returning how many were. A function whose entry is jumped to keeps its
    /// slots, since its entry's parameters are its signature's.
    pub fn promote(&mut self, pointer_bits: u32) -> usize {
        let Some(entry) = self.entry() else { return 0 };
        if self.blocks.iter().any(|(_, b)| b.term.targets().any(|t| t.block == entry)) {
            return 0;
        }
        let uses = Uses::of(self, pointer_bits);
        if uses.promoted.is_empty() {
            return 0;
        }
        let count = uses.promoted.len();
        Promotion::new(self, uses).run();
        vun_trace::tracing::trace!(count, "slots promoted");
        count
    }
}

/// How a slot is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// By plain loads and stores of this scalar, as wide as the slot.
    Scalar(Scalar),
    /// Otherwise: its address is kept, passed, offset or stored, or it is read in parts.
    Escaped,
}

/// The slots of a function to promote, with their scalars, and the addresses of them.
struct Uses {
    promoted: HashMap<Slot, Scalar>,
    addresses: HashMap<Value, Slot>,
}

impl Uses {
    fn of(func: &Function, pointer_bits: u32) -> Self {
        let insts = || func.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &func.insts[i]);
        let addresses: HashMap<Value, Slot> = insts()
            .filter_map(|inst| match inst.kind {
                InstKind::SlotAddr(slot) => Some((inst.result?, slot)),
                _ => None,
            })
            .collect();
        let mut uses: HashMap<Slot, Use> = HashMap::new();
        let mut reach = |slot: Slot, ty: Option<Scalar>| {
            let fits = ty.filter(|ty| ty.bytes(pointer_bits) == func.slots[slot].size);
            let seen = uses.entry(slot).or_insert(fits.map_or(Use::Escaped, Use::Scalar));
            if fits.map(Use::Scalar) != Some(*seen) {
                *seen = Use::Escaped;
            }
        };
        for inst in insts() {
            match &inst.kind {
                InstKind::Load { ty, addr, access: Access::Plain } if addresses.contains_key(addr) => {
                    reach(addresses[addr], Some(*ty));
                }
                InstKind::Store { value, addr, access: Access::Plain } if addresses.contains_key(addr) => {
                    reach(addresses[addr], Some(func.ty(*value)));
                    if let Some(&stored) = addresses.get(value) {
                        reach(stored, None);
                    }
                }
                kind => {
                    for slot in kind.operands().filter_map(|v| addresses.get(&v)) {
                        reach(*slot, None);
                    }
                }
            }
        }
        let terms = func.blocks.iter().flat_map(|(_, b)| b.term.operands());
        for slot in terms.filter_map(|v| addresses.get(&v).copied()).collect::<Vec<_>>() {
            reach(slot, None);
        }
        let promoted = uses
            .into_iter()
            .filter_map(|(slot, used)| match used {
                Use::Scalar(ty) => Some((slot, ty)),
                Use::Escaped => None,
            })
            .collect();
        Self { promoted, addresses }
    }
}
