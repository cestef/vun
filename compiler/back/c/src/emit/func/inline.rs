//! Which values are written inline, as expressions in their one use, rather than as locals: a
//! value used once, in its own block, computed without touching memory, or loaded with nothing
//! writing memory or calling between the load and its use. A jump's argument is inlined only
//! when it reads no value, since the moves filling the target's parameters would overwrite what
//! it reads. This is how compare and branch read `if (v3 < v4)`, as TPDE fuses them.

use vun_index::IndexVec;
use vun_lir::analysis::{Liveness, Order};
use vun_lir::func::Def;
use vun_lir::{Access, Function, InstKind, Value};

pub(super) struct Inlined(IndexVec<Value, bool>);

impl Inlined {
    pub(super) fn of(func: &Function, order: &Order, live: &Liveness) -> Self {
        let mut inlined = IndexVec::filled(false, func.values.len());
        let once = |v: Value| live.range(v).is_some_and(|r| r.uses == 1 && r.first == r.last && !r.through);
        for &block in order.blocks() {
            let data = &func.blocks[block];
            let mut loads: Vec<Value> = Vec::new();
            for &inst in &data.insts {
                let inst = &func.insts[inst];
                let operands: Vec<Value> = inst.kind.operands().collect();
                loads.retain(|v| !operands.contains(v));
                if Self::writes(&inst.kind) {
                    for v in loads.drain(..) {
                        inlined[v] = false;
                    }
                }
                let Some(v) = inst.result.filter(|&v| once(v)) else { continue };
                match inst.kind {
                    InstKind::Load { access: Access::Plain, .. } => {
                        inlined[v] = true;
                        loads.push(v);
                    }
                    InstKind::Const { .. }
                    | InstKind::Binary { .. }
                    | InstKind::Compare { .. }
                    | InstKind::Cast { .. }
                    | InstKind::Select { .. }
                    | InstKind::Offset { .. }
                    | InstKind::SlotAddr(_)
                    | InstKind::Addr(_) => inlined[v] = true,
                    _ => {}
                }
            }
            for target in data.term.targets() {
                for &arg in &target.args {
                    if Self::reads_values(func, arg) {
                        inlined[arg] = false;
                    }
                }
            }
        }
        Self(inlined)
    }

    /// Whether an instruction writes memory or calls, which a load read after it must follow.
    fn writes(kind: &InstKind) -> bool {
        match kind {
            InstKind::Store { .. }
            | InstKind::Rmw { .. }
            | InstKind::CompareExchange { .. }
            | InstKind::Copy { .. }
            | InstKind::Call { .. } => true,
            InstKind::Load { access, .. } => *access != Access::Plain,
            _ => false,
        }
    }

    /// Whether the instruction defining `v` reads another value.
    fn reads_values(func: &Function, v: Value) -> bool {
        match func.values[v].def {
            Def::Inst(inst) => func.insts[inst].kind.operands().next().is_some(),
            Def::Param(..) => false,
        }
    }

    pub(super) fn contains(&self, v: Value) -> bool {
        self.0[v]
    }
}
