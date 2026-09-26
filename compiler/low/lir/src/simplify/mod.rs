//! Simplifying a unit after lowering, one function at a time, callees first: locals promoted to
//! values, operations on the constants that makes folded, jumps threaded through the decisions
//! that folds; then the small functions it calls inlined, simplified already, and it simplified
//! again; truncations narrowed; and what nothing reads swept. A function is done before any
//! caller decides whether it is small.

#[cfg(test)]
mod tests;

use crate::entity::FuncId;
use crate::unit::Unit;

impl Unit {
    /// Simplifies every function (see the module).
    pub fn simplify(&mut self) {
        for f in self.callees_first() {
            self.simplify_function(f);
        }
    }

    fn simplify_function(&mut self, f: FuncId) {
        let bits = self.pointer_bits;
        self.functions[f].tidy(bits);
        if self.inline_into(f) > 0 {
            self.functions[f].tidy(bits);
        }
        let function = &mut self.functions[f];
        if function.narrow(bits) > 0 {
            function.fold();
        }
        function.sweep();
    }
}

impl crate::func::Function {
    /// Promotes, folds and threads (`Function::promote`, `Function::fold`, `Function::thread`).
    fn tidy(&mut self, pointer_bits: u32) {
        self.promote(pointer_bits);
        self.fold();
        self.thread();
    }
}
