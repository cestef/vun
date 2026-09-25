//! Stack bounds (spec 07, section 4): each function bound by `#[stack.max(n)]` may take at most
//! `n` bytes of stack, its frame and the deepest chain of calls it makes, from the frame sizes the
//! backend or its tools report.

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_lir::analysis::StackUse;
use vun_lir::{FuncId, Unit};

use crate::Session;

impl Session {
    /// Reports each function of `unit` whose stack use its bound does not hold, by the frame
    /// sizes `frames` that the backend `backend` built them with.
    ///
    /// # Errors
    /// Returns once a bound that does not hold is reported.
    pub(crate) fn check_stacks(
        &mut self,
        unit: &Unit,
        frames: &[(FuncId, u64)],
        backend: &str,
    ) -> Result<(), ErrorGuaranteed> {
        let uses = StackUse::of(unit, frames);
        let mut failed = None;
        for (id, f) in unit.functions.iter() {
            let Some(bound) = f.stack_max else { continue };
            let d = if frames.is_empty() {
                let msg = Msg::new()
                    .text("this function is bound by ")
                    .code("stack.max")
                    .text(", and backend ")
                    .code(backend)
                    .text(" reported no frame sizes");
                Diagnostic::error(errors::STACK_UNREPORTED, msg).at(f.origin, None)
            } else {
                match uses.of_function(id) {
                    Ok(bytes) if bytes <= bound => continue,
                    Ok(bytes) => {
                        let msg = Msg::new()
                            .text("this function may take ")
                            .num(bytes)
                            .text(" bytes of stack, over its bound of ")
                            .num(bound);
                        Diagnostic::error(errors::STACK_EXCEEDED, msg).at(f.origin, None)
                    }
                    Err(why) => {
                        let msg = Msg::new().text("the stack this function may take has no bound");
                        Diagnostic::error(errors::STACK_UNBOUNDED, msg)
                            .at(f.origin, None)
                            .label(why.at(unit), Some(why.msg()))
                    }
                }
            };
            failed = Some(self.diag.error(d));
        }
        failed.map_or(Ok(()), Err)
    }
}
