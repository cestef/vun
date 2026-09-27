//! Simplifying a unit after lowering, one function at a time, callees first: slots of
//! aggregates cut into scalars, locals promoted to values, operations on the constants that
//! makes folded, computations repeated reused, jumps threaded through the decisions that
//! folds, truncations narrowed, and what nothing reads swept, round after round while a round
//! changes something; then the small functions it calls inlined, simplified already, and the
//! rounds run again. A function is done before any caller decides whether it is small.

#[cfg(test)]
mod tests;

use vun_trace::Activity;

use crate::func::Function;
use crate::unit::Unit;

const SIMPLIFY: Activity = Activity::stage("simplify");

/// The most rounds of the passes over one function: each round mostly finishes what the one
/// before exposed, and few are left after three.
const ROUNDS: usize = 4;

impl Unit {
    /// Simplifies every function (see the module).
    pub fn simplify(&mut self) {
        let _timer = SIMPLIFY.start();
        for f in self.callees_first() {
            let bits = self.pointer_bits;
            self.functions[f].tidy(bits);
            if self.inline_into(f) > 0 {
                self.functions[f].tidy(bits);
            }
        }
        self.verified();
    }
}

impl Function {
    /// Runs `Function::scalarize`, `promote`, `fold`, `reuse`, `thread`, `narrow` and `sweep` in
    /// rounds until one changes nothing.
    fn tidy(&mut self, pointer_bits: u32) {
        for _ in 0..ROUNDS {
            let changed = self.scalarize(pointer_bits)
                + self.promote(pointer_bits)
                + self.fold()
                + self.reuse()
                + self.thread(pointer_bits)
                + self.narrow(pointer_bits)
                + self.sweep();
            if changed == 0 {
                return;
            }
        }
    }
}
