//! What primitives do when applied at compile time. Sema and the interpreter both apply them
//! through here, so each primitive has one definition.

use crate::Primitive;
use crate::env::Unevaluated;
use crate::primitives::lang::Lang;
use crate::ty::{TyKind, Value};

impl Primitive {
    /// The value of `self(args)` at compile time.
    ///
    /// # Errors
    /// Returns why the application has no value: arguments of the wrong shape, or a primitive
    /// not applied at compile time yet.
    pub fn apply(self, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            // `lang.Int(range)`: the integers of the range (spec 04, section 1).
            (Self::Lang(Lang::Int), [Value::Range(Some(range))]) => Ok(Value::Type(TyKind::int(range))),
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
