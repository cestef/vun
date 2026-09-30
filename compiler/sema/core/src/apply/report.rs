//! What ends compile-time evaluation: library diagnostics (spec 10, section 3) and trap sites
//! (spec 07, section 3), reached as they would be at run time.

use vun_primitives::diag::Diag;
use vun_primitives::panic::Panic;
use vun_sema_ty::Value;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::trap::{Failure, Reason};

/// What the primitives of `lang.diag` compute.
pub(crate) trait ApplyDiag {
    /// The value of this primitive applied to `args` at compile time.
    fn apply(self, args: &[Value]) -> Result<Value, Unevaluated>;
}

impl ApplyDiag for Diag {
    fn apply(self, args: &[Value]) -> Result<Value, Unevaluated> {
        match self {
            Self::error => Err(Unevaluated::trap(Reason::reported(args).ok_or(Unevaluated::Unsupported)?)),
        }
    }
}

/// What the primitives of `lang.panic` compute.
pub(crate) trait ApplyPanic {
    /// The value of this primitive applied to `args` at compile time: a trap site traps.
    fn apply(self, args: &[Value]) -> Result<Value, Unevaluated>;
}

impl ApplyPanic for Panic {
    fn apply(self, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            (Self::trap, [_]) => Err(Unevaluated::trap(Reason::Trap)),
            (Self::fail, [reason]) => {
                let failure = Failure::of(reason).ok_or(Unevaluated::Unsupported)?;
                Err(Unevaluated::trap(Reason::Failed(failure)))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
