//! Simplifying a unit after lowering: locals promoted to values, operations on the constants
//! that makes folded, then jumps threaded through the decisions that folds; then small
//! functions inlined, and what their arguments make constant folded again.

use crate::unit::Unit;

impl Unit {
    /// Promotes, folds and threads every function (`Function::promote`, `Function::fold`,
    /// `Function::thread`), inlines small functions (`Unit::inline`), then does it again where
    /// something was inlined.
    pub fn simplify(&mut self) {
        let bits = self.pointer_bits;
        for (_, function) in self.functions.iter_mut() {
            function.promote(bits);
            function.fold();
            function.thread();
        }
        if self.inline() > 0 {
            for (_, function) in self.functions.iter_mut() {
                function.promote(bits);
                function.fold();
                function.thread();
            }
        }
    }
}
