//! What ends compile-time evaluation: library diagnostics (spec 10, section 3) and trap sites
//! (spec 07, section 3), reached as they would be at run time.

use crate::env::Unevaluated;
use crate::primitives::diag::Diag;
use crate::primitives::panic::Panic;
use crate::trap::{Failure, Reason};
use crate::ty::Value;

impl Diag {
    /// The value of this primitive applied to `args` at compile time.
    pub(crate) fn apply(self, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            (Self::error, [message, at]) => {
                let message = message.text().ok_or(Unevaluated::Unsupported)?;
                // An optional place: the origin a variant's payload holds, if it holds one.
                let at = match at {
                    Value::Variant { payload, .. } => match payload.as_slice() {
                        [Value::Origin(origin)] => Some(*origin),
                        _ => None,
                    },
                    _ => None,
                };
                Err(Unevaluated::trap(Reason::Reported { message, at }))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}

impl Panic {
    /// The value of this primitive applied to `args` at compile time: a trap site traps.
    pub(crate) fn apply(self, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            (Self::trap, [_]) => Err(Unevaluated::trap(Reason::Trap)),
            (Self::fail, [Value::Variant { name, payload, .. }]) => {
                let failure = match payload.as_slice() {
                    [message] if *name == vun_items::lang::panic_Reason::message.symbol() => {
                        Failure::Message(message.text().ok_or(Unevaluated::Unsupported)?)
                    }
                    [source, Value::Origin(at)]
                        if *name == vun_items::lang::panic_Reason::assertion.symbol() =>
                    {
                        Failure::Assertion { source: source.text().ok_or(Unevaluated::Unsupported)?, at: *at }
                    }
                    _ => return Err(Unevaluated::Unsupported),
                };
                Err(Unevaluated::trap(Reason::Failed(failure)))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
