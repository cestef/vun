//! Calls, as the framework sees them: a call may write any register but the callee-saved ones,
//! so what is used after it is stored home first, and the values passed in registers are moved
//! there together, straight from where they are.

use vun_lir::Value;

use super::Cx;
use crate::Arch;

impl<A: Arch> Cx<'_, A> {
    /// Stores home every value held in a register and still used, but those whose every
    /// remaining use is a register argument in `passed`: the call reads them from their
    /// registers.
    pub fn spill_for_call(&mut self, passed: &[(Value, A::Reg)]) {
        for &r in A::ALLOCATABLE.iter().chain(A::CALLEE_SAVED) {
            let Some(v) = self.regs.holds(r) else { continue };
            let reads = passed.iter().filter(|&&(p, _)| p == v).count();
            if u32::try_from(reads).is_ok_and(|n| n == self.values[v].left) {
                continue;
            }
            self.store_home(v, r);
        }
    }

    /// Writes each value of `passed` into its register, as one parallel move: a register is
    /// written once no value still to be passed is in it, and a cycle is broken by storing one
    /// of its values home.
    ///
    /// # Panics
    /// Never: a move is blocked only by a register holding a value.
    pub fn pass_in_regs(&mut self, passed: &[(Value, A::Reg)]) {
        let mut pending = passed.to_vec();
        while !pending.is_empty() {
            let blocks = |dst: A::Reg, v: Value, pending: &[(Value, A::Reg)], cx: &Self| {
                cx.regs.holds(dst).is_some_and(|w| w != v && pending.iter().any(|&(u, _)| u == w))
            };
            if let Some(at) = pending.iter().position(|&(v, dst)| !blocks(dst, v, &pending, self)) {
                let (v, dst) = pending.remove(at);
                self.reg_to(v, dst);
                if self.regs.holds(dst).is_some_and(|w| w != v) {
                    self.unbind(dst);
                }
            } else {
                let (_, dst) = pending[0];
                let w = self.regs.holds(dst).expect("a register blocking a move holds a value");
                self.store_home(w, dst);
                self.unbind(dst);
            }
        }
    }
}
