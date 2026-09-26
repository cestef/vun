//! What the primitives of `lang.float` compute at compile time (spec 04, section 2): IEEE 754
//! arithmetic and comparisons on exact binary values, as they run.

use std::cmp::Ordering;

use crate::env::{Env, Unevaluated};
use crate::float::{Binary, Format};
use crate::int::{Integer, Interval};
use crate::primitives::float::Float;
use crate::ty::{TyKind, Value};

impl Float {
    /// The value of this primitive applied to `args` at compile time: the float type, then the
    /// operands, compared ones through borrows, which at compile time are their values.
    pub(crate) fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        if let Some(encoded) = self.encoding(args) {
            return encoded;
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

    /// The primitives of the encoding: `Bits`, `bits`, `from_bits` and `fraction_bits`; `None`
    /// for another.
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
