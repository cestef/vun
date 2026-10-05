//! Stack checks (spec 13, section 8.7): under the `stack` checks, a function's frame is checked
//! against the stack of the program by `lang.check.stack`, which the backend calls with the frame's
//! lowest address before the stack moves to it. Lowering names the call and the site it fails at;
//! the size of the frame is the backend's to know.

use vun_items::lang::check;
use vun_lir::StackCheck;
use vun_sema::checks::ChecksOf;
use vun_target::Check;

use super::FnLower;
use crate::unlowered::Unlowered;

impl FnLower<'_, '_, '_> {
    /// Gives the function the check of its frame, where the program is built with it and the
    /// function is one the checks apply in.
    pub(super) fn stack_check(&mut self) -> Result<(), Unlowered> {
        if !self.checked || !self.l.types.cx.get::<ChecksOf>(&()).contains(Check::Stack) {
            return Ok(());
        }
        let site = self.new_site(self.at, None)?;
        let call = self.provided(check::stack)?;
        self.b.check_stack(StackCheck { site, call });
        Ok(())
    }
}
