//! What the register allocator reads of the function compiled: the machine, the values
//! recomputed where used, which hold no register, and what each point needs, as the
//! architecture's instruction compilers describe it. The plan made is checked in debug builds.

use vun_index::Idx;
use vun_lir::{Block, Function, Inst, Scalar, Value};
use vun_regalloc::{Allocation, Bank, Describe, Machine, Needs, Out, Remade};

use super::Cx;
use crate::Isel;

pub(super) struct Described<'c, 'a, A: Isel>(&'c Cx<'a, A>);

impl<A: Isel> Described<'_, '_, A> {
    /// Plans where every value of the function `cx` compiles lives.
    ///
    /// # Panics
    /// In debug builds, when the plan fails its check.
    pub(super) fn plan(cx: &Cx<'_, A>) -> Allocation {
        let described = Described(cx);
        let plan = Allocation::of(cx.f, &described);
        if cfg!(debug_assertions)
            && let Err(wrong) = plan.check(cx.f, &described)
        {
            panic!(
                "the register allocator planned {wrong} in the function at {}\n{plan}",
                cx.f.origin.located()
            );
        }
        plan
    }
}

impl<A: Isel> Describe for Described<'_, '_, A> {
    fn machine(&self) -> &Machine {
        &self.0.machine
    }

    fn bank(&self, ty: Scalar) -> Bank {
        A::bank(ty)
    }

    fn remade(&self, v: Value) -> Option<Remade> {
        self.0.values[v].made.map(|(_, remade)| remade)
    }

    fn inst(&self, f: &Function, inst: Inst, needs: &mut Needs) {
        let cx = self.0;
        // Made by the branch it is fused into, or where used: nothing here; held, into a
        // register of its own.
        match f.insts[inst].result {
            Some(v) if self.remade(v) == Some(Remade::Held) => needs.defs.push((v, Out::Reg)),
            Some(v) if self.recomputed(v) || cx.is_fused(v) => {}
            _ => A::inst_needs(cx, inst, needs),
        }
    }

    fn term(&self, _: &Function, block: Block, needs: &mut Needs) {
        A::term_needs(self.0, block, needs);
    }

    fn entry(&self, _: &Function, needs: &mut Needs) {
        A::entry_needs(self.0, needs);
    }
}

impl<A: Isel> Cx<'_, A> {
    /// Whether `v` is a comparison the branch ending its block makes. Found for every value at
    /// once, when first asked: the allocator asks for each instruction.
    fn is_fused(&self, v: Value) -> bool {
        let fused = self.fusing.get_or_init(|| {
            let mut fused = vec![false; self.f.values.len()];
            for (block, _) in self.f.blocks.iter() {
                if let Some(made) = self.fused(block) {
                    fused[made.value.index()] = true;
                }
            }
            fused
        });
        fused[v.index()]
    }
}
