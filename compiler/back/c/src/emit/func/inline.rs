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
            // Inlined values that read memory where they are written: plain loads, and the
            // inlined values reading them, which carry the load to their own use.
            let mut reading: Vec<Value> = Vec::new();
            for &inst in &data.insts {
                let inst = &func.insts[inst];
                let before = reading.len();
                reading.retain(|v| !inst.kind.operands().any(|o| o == *v));
                let carries = reading.len() < before;
                if inst.kind.acts() {
                    for v in reading.drain(..) {
                        inlined[v] = false;
                    }
                }
                let Some(v) = inst.result.filter(|&v| once(v)) else { continue };
                let loads = matches!(inst.kind, InstKind::Load { access: Access::Plain, .. });
                if loads || inst.kind.pure() {
                    inlined[v] = true;
                    if loads || carries {
                        reading.push(v);
                    }
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
