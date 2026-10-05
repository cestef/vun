//! Blocks of inline assembly (spec 07, section 6) read as WebAssembly's instructions: a block names
//! instructions and forms by their index in the table, whose declaration order `generated` keeps.
//! Bindings are locals, which the backend names; there are no registers to pick.

use vun_inline::{FormId, OpId};
use vun_isa::{Blocks, Fit, Fixed, Given, Misfit};

use crate::Wasm32;

impl Blocks for Wasm32 {
    const ARCH: vun_items::Item = vun_items::lang::arch::wasm32;

    fn fit<V: Clone, P: Clone, S: Copy>(
        op: OpId,
        form: FormId,
        fixed: Fixed,
        givens: &[Given<V, P, S>],
    ) -> Result<Fit<V, P, S>, Misfit> {
        crate::fit::fit(op, form, fixed, givens)
    }
}
