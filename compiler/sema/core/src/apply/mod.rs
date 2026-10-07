//! What primitives do when applied at compile time. Sema and the interpreter both apply them
//! through here, so each primitive has one definition.

mod float;
mod int;
mod meta;
mod report;

use vun_desugar::Borrow;
use vun_primitives::Primitive;
use vun_primitives::bounds::Bounds;
use vun_primitives::lang::Lang;
use vun_primitives::lint::Lint;
use vun_primitives::mem::Mem;
use vun_primitives::meta::Meta;
use vun_primitives::simd::Simd;
use vun_sema_float::Format;
use vun_sema_int::Integer;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::trap::Reason;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, TyKind, Value};

use crate::apply::float::ApplyFloat;
use crate::apply::int::ApplyInt;
use crate::apply::meta::ApplyMeta;
use crate::apply::report::ApplyDiag;
use crate::apply::report::ApplyPanic;
use crate::env::Env;

/// A primitive applied at compile time.
pub trait Apply {
    /// Whether a call with the compile-time arguments `args` may trap, where they decide (spec
    /// 04, rules 6 and 8); `None` where the primitive's markers say it all.
    #[must_use]
    fn may_trap(self, args: &[Option<Value>]) -> Option<bool>;

    /// The value of `self(args)` at compile time.
    ///
    /// # Errors
    /// Returns why the application has no value: arguments of the wrong shape, or a primitive
    /// not applied at compile time yet.
    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated>;
}

impl Apply for Primitive {
    fn may_trap(self, args: &[Option<Value>]) -> Option<bool> {
        match self {
            Self::Int(int) => int.may_trap(args),
            _ => None,
        }
    }

    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        env.check_arguments(self, args)?;
        // Primitives read bounds as the `lang.Range` of their items.
        let ranged: Vec<Value>;
        let args = if args.iter().any(|a| matches!(a, Value::Bounds(_))) {
            ranged = args.iter().cloned().map(Value::ranged).collect();
            &ranged
        } else {
            args
        };
        match (self, args) {
            // `lang.Int(range)`: the integers of the range (spec 04, section 1).
            (Self::Lang(Lang::Int), [Value::Range(Some(range))]) => Ok(Value::Type(TyKind::int(range))),
            // `lang.Float(bits)`: the IEEE 754 binary floats of that width.
            (Self::Lang(Lang::Float), [Value::Int(bits)]) => Format::of_bits(bits)
                .map(|f| Value::Type(f.ty()))
                .ok_or_else(|| Unevaluated::trap(Reason::Argument)),
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
            // `lang.Pack(P)`: the parameter gathering the arguments after the others into a tuple.
            (Self::Lang(Lang::Pack), [items]) => match items.as_type() {
                Some(items) if matches!(&*items, TyKind::Tuple(_)) => {
                    Ok(Value::Type(TyKind::Primitive(self, vec![Value::Type(items)]).intern()))
                }
                _ => Err(Unevaluated::trap(Reason::Argument)),
            },
            // A cell's value at compile time is what it holds, and a vector is the array of its
            // lanes.
            (Self::Mem(Mem::cell), [_, value]) | (Self::Simd(Simd::lanes | Simd::of), [_, _, value]) => {
                Ok(value.clone())
            }
            // What a cell holds changes at run time: a pointer into it reads a run-time place. Storage
            // with no value has none to know when compiling.
            (Self::Mem(Mem::cell_pointer | Mem::uninit), _) => Err(Unevaluated::Runtime),
            // `lang.interface(f)` wraps `f` so that it can constrain `$` parameters.
            (Self::Lang(Lang::interface), [Value::Fn(f)]) => Ok(Value::Interface(f.clone())),
            // `lang.mem.Array(n, T)`: `[n]T`.
            (Self::Mem(Mem::Array), [Value::Int(len), elem]) => {
                Ok(Value::Type(TyKind::array(len.clone(), elem.as_type().ok_or(Unevaluated::Unsupported)?)))
            }
            // `lang.simd.Vector(n, T)`: `n` lanes of `T`, at least one.
            (Self::Simd(Simd::Vector), [Value::Int(lanes), elem]) if lanes > &Integer::zero() => {
                let elem = elem.as_type().ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Type(
                    TyKind::Primitive(self, vec![Value::Int(lanes.clone()), Value::Type(elem)]).intern(),
                ))
            }
            (Self::Simd(Simd::Vector), [Value::Int(_), _]) => Err(Unevaluated::trap(Reason::Argument)),
            (Self::Simd(Simd::Lane), [ty]) => {
                let ty = ty.as_type().ok_or(Unevaluated::Unsupported)?;
                ty.lanes()
                    .map(|(_, lane)| Value::Type(lane))
                    .ok_or_else(|| Unevaluated::trap(Reason::Argument))
            }
            // Bounds, borrow, pointer, cell, uninitialized, slice and code types have no structure beyond their
            // argument.
            (p, [arg])
                if Borrow::of(p.item()).is_some()
                    || matches!(
                        p,
                        Self::Meta(Meta::Code)
                            | Self::Lang(Lang::Bounds)
                            | Self::Mem(Mem::Ptr | Mem::Cell | Mem::Slice | Mem::Uninit)
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
            (Self::Bounds(Bounds::last), [_, Value::Range(Some(items))]) => {
                let last = items.hi().finite().ok_or_else(|| Unevaluated::trap(Reason::Bounds))?;
                Ok(Value::Int(last.clone()))
            }
            (Self::Bounds(Bounds::before), [_, Value::Range(Some(items)), Value::Int(x)]) => {
                let first = items.lo().finite() == Some(x);
                Ok(Value::Int(if first { x.clone() } else { x - &Integer::from(1) }))
            }
            (Self::Bounds(Bounds::ends), [_, Value::Range(Some(items)), Value::Int(x)]) => {
                env.decide(items.hi().finite() == Some(x))
            }
            (Self::Bounds(Bounds::after), [_, Value::Range(Some(items)), Value::Int(x)]) => {
                let last = items.hi().finite() == Some(x);
                Ok(Value::Int(if last { x.clone() } else { x + &Integer::from(1) }))
            }
            // A link symbol's address is known once the program is linked: pointing to it is that
            // address, as borrowing it is.
            (Self::Mem(Mem::pointer_to), [_, linked @ Value::Linked(_)]) => Ok(linked.clone()),
            // `lang.mem.len(T, xs)`: the number of items `xs` borrows.
            (Self::Mem(Mem::len), [_, Value::Array(items)]) => Ok(Value::Int(Integer::from(items.len()))),
            // A lint's call node holds the value of what the call calls.
            (Self::Lint(Lint::calls), [_, callee, f]) => env.decide(callee == f),
            (Self::Int(int), args) => int.apply(env, args),
            (Self::Meta(meta), args) => meta.apply(env, args),
            (Self::Float(float), args) => float.apply(env, args),
            (Self::Diag(diag), args) => diag.apply(args),
            (Self::Panic(panic), args) => panic.apply(args),
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
