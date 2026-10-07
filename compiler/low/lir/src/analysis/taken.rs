//! The functions of a unit whose address is taken: written into data, made a value by an
//! instruction, named by an assembly block or called by a check. Others are only called, by name,
//! so a backend may pass what they take as it likes.

use vun_index::IndexVec;

use crate::entity::FuncId;
use crate::inst::{Address, InstKind};
use crate::unit::Unit;

pub struct Taken(IndexVec<FuncId, bool>);

impl Taken {
    #[must_use]
    pub fn of(unit: &Unit) -> Self {
        let mut taken: IndexVec<FuncId, bool> = IndexVec::filled(false, unit.functions.len());
        let held = unit.data.iter().flat_map(|(_, d)| &d.relocs).map(|r| r.target);
        let used = unit.functions.iter().flat_map(|(_, f)| {
            let insts = f.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &f.insts[i].kind);
            let named =
                insts.filter(|kind| !matches!(kind, InstKind::Call { .. })).flat_map(InstKind::addresses);
            named.chain(f.stack_check.map(|check| check.call))
        });
        for address in held.chain(used) {
            if let Address::Func(f) = address {
                taken[f] = true;
            }
        }
        Self(taken)
    }

    #[must_use]
    pub fn contains(&self, f: FuncId) -> bool {
        self.0.get(f).copied().unwrap_or(false)
    }
}
