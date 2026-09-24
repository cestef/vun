//! What primitives do when applied at compile time. Sema and the interpreter both apply them
//! through here, so each primitive has one definition.

use crate::Primitive;
use crate::env::Unevaluated;
use crate::primitives::lang::Lang;
use crate::primitives::mem::Mem;
use crate::ty::{Signature, TyKind, Value};

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
            // `lang.Fn(params, result)`: a function type (appendix B).
            (Self::Lang(Lang::Fn), [params, result]) => {
                let (Some(params), Some(result)) = (params.as_type(), result.as_type()) else {
                    return Err(Unevaluated::Unsupported);
                };
                let TyKind::Tuple(params) = &*params else { return Err(Unevaluated::Unsupported) };
                Ok(Value::Type(Signature { params: params.clone(), result }.ty()))
            }
            // `lang.interface(f)` wraps `f` so that it can constrain `$` parameters.
            (Self::Lang(Lang::interface), [Value::Fn(f)]) => Ok(Value::Interface(f.clone())),
            // Borrow, pointer and cell types have no structure beyond their argument.
            (
                Self::Mem(Mem::Ref | Mem::RefMut | Mem::RefOut | Mem::RefTake | Mem::Ptr | Mem::Cell),
                [arg],
            ) => {
                let arg = arg.as_type().ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Type(TyKind::Primitive(self, vec![Value::Type(arg)]).intern()))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
