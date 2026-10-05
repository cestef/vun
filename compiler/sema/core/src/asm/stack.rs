//! The stack of a block of a stack machine's instructions (spec 07, section 6): each instruction
//! pops the values its form says, of the types it says, and pushes its results; a block starts and
//! ends with an empty stack, except past an instruction that ends the code, after which any
//! stack goes.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_inline::Inst;
use vun_isa::{Slot, Stack};
use vun_origin::OriginId;

use super::check::Checker;
use super::table::{Flow, Table};

impl Checker<'_, '_, '_> {
    /// Whether `insts`, with the effect `stacks` gives each, take and leave the stack as they
    /// must; where not, reports the first instruction that does not, or `at` for values left.
    pub(super) fn stack(
        &mut self,
        at: OriginId,
        insts: &[Inst<ExprId, ExprId, ExprId>],
        stacks: &[Option<Stack>],
    ) -> bool {
        let mut held: Vec<Slot> = Vec::new();
        for (inst, stack) in insts.iter().zip(stacks) {
            let Some(stack) = stack else { return true };
            for &want in stack.pops.iter().rev() {
                let Some(top) = held.pop() else {
                    return self.mismatch(inst.origin, "pops a value off an empty stack");
                };
                if matches!((want, top), (Slot::Class(a), Slot::Class(b)) if a != b) {
                    return self.mismatch(inst.origin, "pops a value of another type than it takes");
                }
            }
            held.extend(&stack.pushes);
            let form = self.table.form(inst.op, inst.form);
            if form.is_some_and(|f| Table::flow(self.arch, f) == Flow::Ends) {
                return true;
            }
        }
        held.is_empty() || self.mismatch(at, "ends with values on the stack")
    }

    /// Reports a stack that does not fit at `origin`; always `false`.
    fn mismatch(&mut self, origin: OriginId, what: &'static str) -> bool {
        let msg = Msg::new().text("this instruction ").text(what);
        self.diag.error(Diagnostic::error(errors::ASM_STACK, msg).at(origin, None));
        false
    }
}
