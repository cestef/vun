//! What the primitives of `lang.meta` compute (spec 03, section 6): reflection on types, code
//! values and compile-time sequences.

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::int::Integer;
use crate::layout::Shapes;
use crate::primitives::meta::Meta;
use crate::trap::Reason;
use crate::ty::{TyKind, Value};

impl Meta {
    /// The value of this primitive applied to `args` at compile time.
    pub(crate) fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            (Self::items, [ty]) => match &*ty.as_type().ok_or(Unevaluated::Unsupported)? {
                TyKind::Tuple(items) => Ok(Value::Array(items.iter().map(|&t| Value::Type(t)).collect())),
                _ => Err(Unevaluated::Unsupported),
            },
            // Reflection (spec 03, section 6).
            (Self::variants, [ty]) => env.variants(ty.as_type().ok_or(Unevaluated::Unsupported)?),
            (Self::variant, [_, value]) => env.variant_index(value),
            (Self::fields, [ty]) => env.fields(ty.as_type().ok_or(Unevaluated::Unsupported)?),
            (Self::Part, [ty, variant, index]) => {
                env.part_type(ty.as_type().ok_or(Unevaluated::Unsupported)?, variant, index)
            }
            (Self::part, [ty, value, variant, index]) => {
                let args = [Some(ty.clone()), None, Some(variant.clone()), Some(index.clone())];
                let option = env.primitive_signature(Primitive::Meta(self), &args)?.result;
                env.part(option, value, variant, index)
            }
            (Self::name_of, [ty]) => {
                Ok(Value::string(&ty.as_type().ok_or(Unevaluated::Unsupported)?.to_string()))
            }
            (meta @ (Self::size | Self::align), [ty]) => {
                let ty = ty.as_type().ok_or(Unevaluated::Unsupported)?;
                let layout = Shapes::layout(env.cx, &ty).ok_or(Unevaluated::Unsupported)?;
                let bytes = if meta == Self::size { layout.size } else { layout.align };
                Ok(Value::Int(Integer::from(u128::from(bytes))))
            }
            // A coroutine function's result is its frame type.
            (Self::Frame, [f, _]) => {
                let signature = f.as_type().and_then(|f| f.signature()).ok_or(Unevaluated::Unsupported)?;
                match &*signature.result {
                    TyKind::Frame(_) => Ok(Value::Type(signature.result)),
                    _ => Err(Unevaluated::trap(Reason::NoFrame)),
                }
            }
            (Self::same, [_, a, b]) => env.decide(a == b),
            (Self::any, [ty, value]) => {
                let ty = ty.as_type().ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Any { ty, value: Box::new(value.clone()) })
            }
            (Self::concat, [_, Value::Array(a), Value::Array(b)]) => {
                Ok(Value::Array(a.iter().chain(b).cloned().collect()))
            }
            (Self::append, [_, Value::Array(items), item]) => {
                Ok(Value::Array(items.iter().chain([item]).cloned().collect()))
            }
            (Self::span, [_, Value::Code(code)]) => Ok(Value::Origin(code.expr.expr.origin)),
            (Self::source, [_, Value::Code(code)]) => {
                let env = Env::quoted(env.cx, code).ok_or(Unevaluated::Absent)?;
                Ok(Value::string(&env.source(code.expr.expr.id)))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
