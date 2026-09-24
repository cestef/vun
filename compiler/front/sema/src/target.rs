//! The target compile-time code is evaluated for (spec 00, guarantee 4): the values of the
//! constants `lang.target` declares with `_`.

use crate::Primitive;
use crate::int::Integer;
use crate::primitives::target::Target as Constant;
use crate::ty::Value;

/// A resource of the query database; the driver sets it once targets are chosen, and until then
/// it describes a 64-bit target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The largest address: `usize` is `Int(0..=usize_max)`.
    pub usize_max: Integer,
}

/// Address bits of the target assumed until the driver sets one.
const DEFAULT_ADDRESS_BITS: u32 = 64;

impl Default for Target {
    fn default() -> Self {
        Self { usize_max: &Integer::power_of_two(DEFAULT_ADDRESS_BITS) - &Integer::from(1) }
    }
}

impl Primitive {
    /// The value of a primitive constant of the target, such as `lang.target.usize_max`; `None`
    /// for any other primitive, whose value is itself.
    #[must_use]
    pub fn constant(self, target: &Target) -> Option<Value> {
        match self {
            Self::Target(Constant::usize_max) => Some(Value::Int(target.usize_max.clone())),
            _ => None,
        }
    }
}
