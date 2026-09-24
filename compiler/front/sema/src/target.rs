//! The target compile-time code is evaluated for (spec 00, guarantee 4): the values of the
//! constants `lang.target` declares with `_`.

use vun_target::Endian;

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
    /// The order of the bytes of a value in memory.
    pub endian: Endian,
}

impl Target {
    /// The width of an address, in bits.
    #[must_use]
    pub fn address_bits(&self) -> u32 {
        u32::try_from(self.usize_max.magnitude_bits()).unwrap_or(u32::MAX)
    }
}

/// Address bits of the target assumed until the driver sets one.
const DEFAULT_ADDRESS_BITS: u32 = 64;

impl Default for Target {
    fn default() -> Self {
        Self {
            usize_max: &Integer::power_of_two(DEFAULT_ADDRESS_BITS) - &Integer::from(1),
            endian: Endian::Little,
        }
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
