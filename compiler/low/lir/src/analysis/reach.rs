//! The functions of a unit that can run: those exported, those whose address is taken where
//! something that runs or data holds it or an assembly block names them, and those these call.
//! Inlining leaves functions only their callers named; a backend need not emit what nothing
//! reaches.

use vun_index::IndexVec;

use crate::entity::FuncId;
use crate::func::Linkage;
use crate::inst::{Address, InstKind};
use crate::unit::Unit;

pub struct Reached(IndexVec<FuncId, bool>);

impl Reached {
    #[must_use]
    pub fn of(unit: &Unit) -> Self {
        let mut reached: IndexVec<FuncId, bool> = IndexVec::filled(false, unit.functions.len());
        let exported = unit.functions.iter().filter(|(_, f)| matches!(f.linkage, Linkage::Export(_)));
        let held = unit.data.iter().flat_map(|(_, d)| &d.relocs).filter_map(|r| match r.target {
            Address::Func(f) => Some(f),
            _ => None,
        });
        let mut pending: Vec<FuncId> = exported.map(|(id, _)| id).chain(held).collect();
        while let Some(f) = pending.pop() {
            if std::mem::replace(&mut reached[f], true) {
                continue;
            }
            let func = &unit.functions[f];
            let insts = func.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &func.insts[i].kind);
            let checked = func.stack_check.map(|check| check.call);
            pending.extend(insts.flat_map(InstKind::addresses).chain(checked).filter_map(|a| match a {
                Address::Func(g) => Some(g),
                Address::Data(_) | Address::Import(_) => None,
            }));
        }
        Self(reached)
    }

    #[must_use]
    pub fn contains(&self, f: FuncId) -> bool {
        self.0.get(f).copied().unwrap_or(false)
    }

    /// How many functions are reached.
    #[must_use]
    pub fn count(&self) -> usize {
        self.0.iter().filter(|(_, r)| **r).count()
    }
}
