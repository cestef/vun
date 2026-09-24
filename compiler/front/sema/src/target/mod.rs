//! The target compile-time code is evaluated for (spec 00, guarantee 4): the values of the
//! constants `lang.target` declares with `_`.

mod choose;
mod read;

pub use choose::{Choice, Chosen, Unchosen};
pub use read::{Read, Unread};

use std::sync::Arc;

use vun_query::{Cx, Query};
use vun_target::{Endian, TargetSpec};

use crate::Primitive;
use crate::int::Integer;
use crate::primitives::target::Target as Constant;
use crate::ty::Value;

/// What compile-time code sees of the target.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    /// The largest address: `usize` is `Int(0..=usize_max)`.
    pub usize_max: Integer,
    /// The order of the bytes of a value in memory.
    pub endian: Endian,
    /// The width of an address, in bits.
    pub address_bits: u32,
}

impl Target {
    /// What compile-time code sees of `spec`.
    #[must_use]
    pub fn of(spec: &TargetSpec) -> Self {
        let bits = spec.pointer_bits;
        Self {
            usize_max: &Integer::power_of_two(bits) - &Integer::from(1),
            endian: spec.endian,
            address_bits: bits,
        }
    }
}

/// The target of the compilation: an input the driver sets from the chosen `lang.target.Spec`,
/// unset until then, so that no width is assumed. What depends on it is unknown while it is
/// unset (`Unevaluated::Untargeted`).
pub struct TargetOf;

impl Query for TargetOf {
    const NAME: &'static str = "target";
    type Key = ();
    type Value = Option<Arc<Target>>;

    fn compute(_: &Cx<'_>, (): &()) -> Self::Value {
        None
    }
}

impl Primitive {
    /// How the value of a primitive constant of the target, such as `lang.target.usize_max`, is
    /// read from it; `None` for any other primitive, whose value is itself.
    #[must_use]
    pub fn target_constant(self) -> Option<fn(&Target) -> Value> {
        match self {
            Self::Target(Constant::usize_max) => Some(|t| Value::Int(t.usize_max.clone())),
            _ => None,
        }
    }
}
