//! What primitives do when applied at compile time. Sema and the interpreter both apply them
//! through here, so each primitive has one definition.

use overt_grammar::Narrowing;

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::int::Integer;
use crate::primitives::bounds::Bounds;
use crate::primitives::int::Int;
use crate::primitives::lang::Lang;
use crate::primitives::mem::Mem;
use crate::primitives::meta::Meta;
use crate::ty::{Signature, TyKind, Value};

impl Primitive {
    /// The value of `self(args)` at compile time.
    ///
    /// # Errors
    /// Returns why the application has no value: arguments of the wrong shape, or a primitive
    /// not applied at compile time yet.
    pub fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
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
            // `lang.mem.Array(n, T)`: `[n]T`.
            (Self::Mem(Mem::Array), [Value::Int(len), elem]) => {
                Ok(Value::Type(TyKind::array(len.clone(), elem.as_type().ok_or(Unevaluated::Unsupported)?)))
            }
            // Bounds, borrow, pointer, cell, slice and code types have no structure beyond their
            // argument.
            (
                Self::Meta(Meta::Code)
                | Self::Lang(Lang::Bounds)
                | Self::Mem(
                    Mem::Ref | Mem::RefMut | Mem::RefOut | Mem::RefTake | Mem::Ptr | Mem::Cell | Mem::Slice,
                ),
                [arg],
            ) => {
                let arg = arg.as_type().ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Type(TyKind::Primitive(self, vec![Value::Type(arg)]).intern()))
            }
            // What bounds hold: a compile-time range is the set of its items.
            (Self::Bounds(Bounds::empty), [_, Value::Range(items)]) => env.decide(items.is_none()),
            (Self::Bounds(Bounds::first), [_, Value::Range(Some(items))]) => {
                Ok(Value::Int(items.lo().finite().ok_or(Unevaluated::Unsupported)?.clone()))
            }
            (Self::Bounds(Bounds::ends), [_, Value::Range(Some(items)), Value::Int(x)]) => {
                env.decide(items.hi().finite() == Some(x))
            }
            // `lang.mem.len(T, xs)`: the number of items `xs` borrows.
            (Self::Mem(Mem::len), [_, Value::Array(items)]) => Ok(Value::Int(Integer::from(items.len()))),
            // Narrowing an integer to the integer type `T` (spec 04, rule 3).
            (Self::Int(int), [Value::Range(_), x, Value::Type(ty)]) => {
                let range = ty.interval().ok_or(Unevaluated::Unsupported)?;
                match int {
                    Int::to => x.narrow(&range, Narrowing::To),
                    Int::wrap => x.narrow(&range, Narrowing::Wrap),
                    Int::sat => x.narrow(&range, Narrowing::Sat),
                    Int::fits => env.decide(x.narrow(&range, Narrowing::To).is_ok()),
                }
            }
            // Reflection (spec 03, section 6).
            (Self::Meta(Meta::fields), [ty]) => {
                env.fields(self, ty.as_type().ok_or(Unevaluated::Unsupported)?)
            }
            (Self::Meta(Meta::name_of), [ty]) => {
                Ok(Value::string(&ty.as_type().ok_or(Unevaluated::Unsupported)?.to_string()))
            }
            (Self::Meta(Meta::source), [_, Value::Code(code)]) => {
                let env = Env::quoted(env.cx, code).ok_or(Unevaluated::Absent)?;
                Ok(Value::string(&env.source(code.expr.expr.id)))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
