//! The target compile-time code is evaluated for (spec 00, guarantee 4): the values of the
//! constants `lang.target` declares with `_`.

mod choose;
mod read;

pub use choose::{Choice, Chosen, Unchosen};
pub(crate) use read::Members;
pub use read::{Read, Unread, Variants};

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_query::{Cx, Query};
use vun_target::{Endian, TargetSpec};

use crate::Primitive;
use crate::int::Integer;
use crate::primitives::target::Target as Constant;
use crate::ty::Value;
use vun_items::lang::target_Spec as spec;

/// A target as its declaration gives it: the description read from it, and the value itself.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Declared {
    pub spec: TargetSpec,
    pub value: Value,
}

impl Read for Declared {
    fn read(value: &Value) -> Result<Self, Unread> {
        Ok(Self { spec: TargetSpec::read(value)?, value: value.clone() })
    }
}

/// What compile-time code sees of the target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Target {
    /// The order of the bytes of a value in memory.
    pub endian: Endian,
    /// The width of an address, in bits.
    pub address_bits: u32,
    /// The instruction set its code is written in, which a block of instructions names.
    pub isa: Symbol,
    /// The float types its processor computes, as types (`lang.target.Spec.floats`).
    pub floats: Vec<Value>,
    /// The value of its declaration: `lang.target.current`.
    pub declared: Value,
}

impl Target {
    /// What compile-time code sees of `target`.
    #[must_use]
    pub fn of(target: &Declared) -> Self {
        let floats = Members::of(&target.value).and_then(|m| m.get(spec::floats)).ok().cloned();
        Self {
            endian: target.spec.endian,
            address_bits: target.spec.pointer_bits,
            isa: target.spec.isa,
            floats: floats.and_then(Value::items).unwrap_or_default(),
            declared: target.value.clone(),
        }
    }

    /// Whether its processor computes floats of the type `ty`, which `lang.float.soft` computes
    /// otherwise (`lang.lower.soft`).
    #[must_use]
    pub fn computes(&self, ty: &Value) -> bool {
        self.floats.contains(ty)
    }

    /// The largest address: `usize` is `Int(0..=usize_max)`.
    #[must_use]
    pub fn usize_max(&self) -> Integer {
        &Integer::power_of_two(self.address_bits) - &Integer::from(1)
    }
}

/// The target of the compilation: an input the driver sets from the chosen `lang.target.Spec`,
/// unset until then, so that no width is assumed. What depends on it is unknown while it is
/// unset (`Unevaluated::Untargeted`).
pub struct TargetOf;

impl Query for TargetOf {
    const NAME: &'static str = "target";
    const INPUT: bool = true;
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
            Self::Target(Constant::usize_max) => Some(|t| Value::Int(t.usize_max())),
            Self::Target(Constant::current) => Some(|t| t.declared.clone()),
            _ => None,
        }
    }
}
