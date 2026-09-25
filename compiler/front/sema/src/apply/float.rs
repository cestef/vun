//! What the primitives of `lang.float` compute at compile time (spec 04, section 2): IEEE 754
//! arithmetic and comparisons on exact binary values, as they run.

use std::cmp::Ordering;

use crate::env::{Env, Unevaluated};
use crate::primitives::float::Float;
use crate::ty::Value;

impl Float {
    /// The value of this primitive applied to `args` at compile time: the float type, then the
    /// operands, compared ones through borrows, which at compile time are their values.
    pub(crate) fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
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
}
