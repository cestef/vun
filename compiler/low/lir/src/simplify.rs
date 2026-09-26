//! Simplifying a unit after lowering: locals promoted to values, operations on the constants
//! that makes folded, then jumps threaded through the decisions that folds.

use crate::unit::Unit;

impl Unit {
    /// Promotes, folds and threads every function (`Function::promote`, `Function::fold`,
    /// `Function::thread`).
    pub fn simplify(&mut self) {
        let bits = self.pointer_bits;
        for (_, function) in self.functions.iter_mut() {
            function.promote(bits);
            function.fold();
            function.thread();
        }
    }
}
