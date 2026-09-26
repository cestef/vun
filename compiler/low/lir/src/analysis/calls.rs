//! The call graph of a unit, walked callees first: each function after every function it calls,
//! but for calls closing a cycle, so a pass over each function sees its callees already done.

use vun_index::IndexVec;

use crate::entity::FuncId;
use crate::func::Function;
use crate::inst::{Callee, InstKind};
use crate::unit::Unit;

impl Function {
    /// The functions of the unit this calls directly, in the order of the calls, repeated.
    pub fn callees(&self) -> impl Iterator<Item = FuncId> {
        self.blocks.iter().flat_map(|(_, b)| &b.insts).filter_map(|&i| match self.insts[i].kind {
            InstKind::Call { callee: Callee::Func(f), .. } => Some(f),
            _ => None,
        })
    }
}

impl Unit {
    /// Every function, each after those it calls, but where they call it back.
    #[must_use]
    pub fn callees_first(&self) -> Vec<FuncId> {
        let mut seen: IndexVec<FuncId, bool> = IndexVec::filled(false, self.functions.len());
        let mut order = Vec::with_capacity(self.functions.len());
        for (root, _) in self.functions.iter() {
            if std::mem::replace(&mut seen[root], true) {
                continue;
            }
            let mut stack = vec![(root, self.functions[root].callees().collect::<Vec<_>>())];
            while let Some((f, pending)) = stack.last_mut() {
                match pending.pop() {
                    Some(g) if !std::mem::replace(&mut seen[g], true) => {
                        let callees = self.functions[g].callees().collect();
                        stack.push((g, callees));
                    }
                    Some(_) => {}
                    None => {
                        order.push(*f);
                        stack.pop();
                    }
                }
            }
        }
        order
    }
}
