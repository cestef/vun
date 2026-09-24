//! What primitives do when applied at compile time. Sema and the interpreter both apply them
//! through here, so each primitive has one definition.

mod int;

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::int::Integer;
use crate::layout::Shapes;
use crate::primitives::bounds::Bounds;
use crate::primitives::diag::Diag;
use crate::primitives::lang::Lang;
use crate::primitives::mem::Mem;
use crate::primitives::meta::Meta;
use crate::trap::Reason;
use crate::ty::{Signature, TyKind, Value};

impl Primitive {
    /// Whether a call with the compile-time arguments `args` may trap, where they decide (spec
    /// 04, rules 6 and 8); `None` where the primitive's markers say it all.
    #[must_use]
    pub fn may_trap(self, args: &[Option<Value>]) -> Option<bool> {
        match self {
            Self::Int(int) => int.may_trap(args),
            _ => None,
        }
    }

    /// The value of `self(args)` at compile time.
    ///
    /// # Errors
    /// Returns why the application has no value: arguments of the wrong shape, or a primitive
    /// not applied at compile time yet.
    pub fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        env.check_arguments(self, args)?;
        match (self, args) {
            // `lang.Int(range)`: the integers of the range (spec 04, section 1).
            (Self::Lang(Lang::Int), [Value::Range(Some(range))]) => Ok(Value::Type(TyKind::int(range))),
            // No integer is in an empty range: the empty type, of what never gives a value.
            (Self::Lang(Lang::Int), [Value::Range(None)]) => env.item(vun_items::lang::never),
            // `lang.Fn(params, result)`: a function type (appendix B).
            (Self::Lang(Lang::Fn), [params, result]) => {
                let (Some(params), Some(result)) = (params.as_type(), result.as_type()) else {
                    return Err(Unevaluated::Unsupported);
                };
                let TyKind::Tuple(params) = &*params else { return Err(Unevaluated::Unsupported) };
                Ok(Value::Type(Signature::new(params.clone(), result).ty()))
            }
            // A cell's value at compile time is what it holds.
            (Self::Mem(Mem::cell), [_, value]) => Ok(value.clone()),
            // `lang.interface(f)` wraps `f` so that it can constrain `$` parameters.
            (Self::Lang(Lang::interface), [Value::Fn(f)]) => Ok(Value::Interface(f.clone())),
            // `lang.mem.Array(n, T)`: `[n]T`.
            (Self::Mem(Mem::Array), [Value::Int(len), elem]) => {
                Ok(Value::Type(TyKind::array(len.clone(), elem.as_type().ok_or(Unevaluated::Unsupported)?)))
            }
            // Bounds, borrow, pointer, cell, slice and code types have no structure beyond their
            // argument.
            (p, [arg])
                if p.borrow_mode().is_some()
                    || matches!(
                        p,
                        Self::Meta(Meta::Code)
                            | Self::Lang(Lang::Bounds)
                            | Self::Mem(Mem::Ptr | Mem::Cell | Mem::Slice)
                    ) =>
            {
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
            (Self::Bounds(Bounds::after), [_, Value::Range(Some(items)), Value::Int(x)]) => {
                let last = items.hi().finite() == Some(x);
                Ok(Value::Int(if last { x.clone() } else { x + &Integer::from(1) }))
            }
            // `lang.mem.len(T, xs)`: the number of items `xs` borrows.
            (Self::Mem(Mem::len), [_, Value::Array(items)]) => Ok(Value::Int(Integer::from(items.len()))),
            (Self::Int(int), args) => int.apply(env, args),
            // Library diagnostics (spec 10, section 3).
            (Self::Diag(Diag::error), [message, at]) => {
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
            (Self::Meta(Meta::items), [ty]) => match &*ty.as_type().ok_or(Unevaluated::Unsupported)? {
                TyKind::Tuple(items) => Ok(Value::Array(items.iter().map(|&t| Value::Type(t)).collect())),
                _ => Err(Unevaluated::Unsupported),
            },
            // Reflection (spec 03, section 6).
            (Self::Meta(Meta::variants), [ty]) => {
                env.variants(self, ty.as_type().ok_or(Unevaluated::Unsupported)?)
            }
            (Self::Meta(Meta::variant), [_, value]) => env.variant_index(value),
            (Self::Meta(Meta::fields), [ty]) => {
                env.fields(self, ty.as_type().ok_or(Unevaluated::Unsupported)?)
            }
            (Self::Meta(Meta::name_of), [ty]) => {
                Ok(Value::string(&ty.as_type().ok_or(Unevaluated::Unsupported)?.to_string()))
            }
            (Self::Meta(meta @ (Meta::size | Meta::align)), [ty]) => {
                let ty = ty.as_type().ok_or(Unevaluated::Unsupported)?;
                let layout = Shapes::layout(env.cx, &ty).ok_or(Unevaluated::Unsupported)?;
                let bytes = if meta == Meta::size { layout.size } else { layout.align };
                Ok(Value::Int(Integer::from(u128::from(bytes))))
            }
            (Self::Meta(Meta::same), [a, b]) => {
                let (a, b) = (
                    a.as_type().ok_or(Unevaluated::Unsupported)?,
                    b.as_type().ok_or(Unevaluated::Unsupported)?,
                );
                env.decide(a == b)
            }
            (Self::Meta(Meta::any), [ty, value]) => {
                let ty = ty.as_type().ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Any { ty, value: Box::new(value.clone()) })
            }
            (Self::Meta(Meta::concat), [_, Value::Array(a), Value::Array(b)]) => {
                Ok(Value::Array(a.iter().chain(b).cloned().collect()))
            }
            (Self::Meta(Meta::append), [_, Value::Array(items), item]) => {
                Ok(Value::Array(items.iter().chain([item]).cloned().collect()))
            }
            (Self::Meta(Meta::span), [_, Value::Code(code)]) => Ok(Value::Origin(code.expr.expr.origin)),
            (Self::Meta(Meta::source), [_, Value::Code(code)]) => {
                let env = Env::quoted(env.cx, code).ok_or(Unevaluated::Absent)?;
                Ok(Value::string(&env.source(code.expr.expr.id)))
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
