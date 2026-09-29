//! What the primitives of `lang.float` compute at compile time (spec 04, section 2): IEEE 754
//! arithmetic and comparisons on exact binary values, as they run.

use std::cmp::Ordering;

use vun_desugar::Narrowing;
use vun_primitives::float::Float;
use vun_sema_float::{Binary, Format};
use vun_sema_int::{Extended, Integer, Interval};
use vun_sema_ty::float::FloatType;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{TyKind, Value};

use crate::env::Env;

/// What the primitives of `lang.float` compute (spec 04, section 2).
pub(crate) trait ApplyFloat {
    /// The value of this primitive applied to `args` at compile time: the float type, then the
    /// operands, compared ones through borrows, which at compile time are their values.
    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated>;

    /// The conversions: `from_int`, `to_int` and `convert`; `None` for another primitive.
    fn conversion(self, args: &[Value]) -> Option<Result<Value, Unevaluated>>;

    /// `x` rounded toward zero into the integer type `ty`, clamped: an infinity to the bound on
    /// its side, a NaN from 0.
    fn clamped(x: Binary, ty: &Value) -> Result<Value, Unevaluated>;

    /// The primitives of the encoding: `Bits`, `bits`, `from_bits` and `fraction_bits`; `None`
    /// for another.
    fn encoding(self, args: &[Value]) -> Option<Result<Value, Unevaluated>>;
}

impl ApplyFloat for Float {
    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        if let Some(encoded) = self.encoding(args) {
            return encoded;
        }
        if let Some(converted) = self.conversion(args) {
            return converted;
        }
        let operands = args.iter().skip(1).map(|a| match a {
            Value::Float(x) => Some(*x),
            _ => None,
        });
        let operands = operands.collect::<Option<Vec<_>>>().ok_or(Unevaluated::Unsupported)?;
        let compared = |order: fn(Option<Ordering>) -> bool| match operands.as_slice() {
            [a, b] => env.decide(order(a.compare(*b))),
            _ => Err(Unevaluated::Unsupported),
        };
        match (self, operands.as_slice()) {
            (Self::add, [a, b]) => Ok(Value::Float(*a + *b)),
            (Self::sub, [a, b]) => Ok(Value::Float(*a - *b)),
            (Self::mul, [a, b]) => Ok(Value::Float(*a * *b)),
            (Self::div, [a, b]) => Ok(Value::Float(*a / *b)),
            (Self::rem, [a, b]) => Ok(Value::Float(*a % *b)),
            (Self::neg, [a]) => Ok(Value::Float(-*a)),
            (Self::mul_add, [a, b, c]) => Ok(Value::Float(a.mul_add(*b, *c))),
            (Self::equal, _) => compared(|o| o == Some(Ordering::Equal)),
            (Self::unequal, _) => compared(|o| o != Some(Ordering::Equal)),
            (Self::less, _) => compared(|o| o == Some(Ordering::Less)),
            (Self::less_equal, _) => compared(|o| matches!(o, Some(Ordering::Less | Ordering::Equal))),
            (Self::greater, _) => compared(|o| o == Some(Ordering::Greater)),
            (Self::greater_equal, _) => compared(|o| matches!(o, Some(Ordering::Greater | Ordering::Equal))),
            (Self::total_order, [a, b]) => env.ordering(a.total_order(*b)),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    fn conversion(self, args: &[Value]) -> Option<Result<Value, Unevaluated>> {
        let format = |ty: &Value| ty.as_type().and_then(|t| Format::of(&t)).ok_or(Unevaluated::Unsupported);
        let value = match (self, args) {
            (Self::from_int, [_, Value::Int(n), ty]) => {
                format(ty).map(|f| Value::Float(Binary::of_integer(f, n)))
            }
            (Self::convert, [_, Value::Float(x), ty]) => format(ty).map(|f| Value::Float(x.converted(f))),
            (Self::to_int, [_, Value::Float(x), ty]) => Self::clamped(*x, ty),
            (Self::from_int | Self::convert | Self::to_int, _) => Err(Unevaluated::Unsupported),
            _ => return None,
        };
        Some(value)
    }

    fn clamped(x: Binary, ty: &Value) -> Result<Value, Unevaluated> {
        let range = ty.as_type().and_then(|t| t.interval()).ok_or(Unevaluated::Unsupported)?;
        let bound = if x.negative() { range.lo() } else { range.hi() };
        let exact = match (x.truncated(), bound) {
            (Some(n), _) => n,
            (None, _) if x.is_nan() => Integer::zero(),
            (None, Extended::Finite(bound)) => bound.clone(),
            (None, _) => return Err(Unevaluated::Unsupported),
        };
        Value::Int(exact).narrow(&range, Narrowing::Sat)
    }

    fn encoding(self, args: &[Value]) -> Option<Result<Value, Unevaluated>> {
        let format = || {
            args.first().and_then(Value::as_type).and_then(|t| Format::of(&t)).ok_or(Unevaluated::Unsupported)
        };
        let value = match (self, args) {
            (Self::Bits, _) => format().map(|f| {
                let top = Integer::power_of_two(f.bits());
                Value::Type(TyKind::int(
                    &Interval::half_open(Integer::zero(), &top).expect("a format has bits"),
                ))
            }),
            (Self::fraction_bits, _) => format().map(|f| Value::Int(Integer::from(u128::from(f.fraction())))),
            (Self::bits, [_, Value::Float(x)]) => Ok(Value::Int(Integer::from(u128::from(x.bits)))),
            (Self::from_bits, [_, Value::Int(n)]) => format().and_then(|format| {
                let bits = n.to_u128().and_then(|n| u64::try_from(n).ok()).ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Float(Binary { format, bits }))
            }),
            (Self::bits | Self::from_bits, _) => Err(Unevaluated::Unsupported),
            _ => return None,
        };
        Some(value)
    }
}
