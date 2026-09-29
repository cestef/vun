//! The target compile-time code is evaluated for (spec 00, guarantee 4): the values of the
//! constants `lang.target` declares with `_`.

mod choose;
mod members;
mod read;

use std::sync::Arc;

pub use choose::{Choice, Chosen, Unchosen};
pub use members::Members;
pub use read::{Read, Unread, Variants};
use vun_codec::{Decode, Encode};
use vun_inline::Needed;
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::target::Target as Constant;
use vun_query::{Cx, Query};
use vun_sema_float::Format;
use vun_sema_int::Integer;
use vun_sema_ty::Value;
use vun_sema_ty::float::FloatType;
use vun_target::{Endian, Floats, TargetSpec, Threads};

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
    /// The width of a general register, in bits.
    pub register_bits: u32,
    /// The instruction set its code is written in, which a block of instructions names.
    pub isa: Symbol,
    /// What its processor has of the instruction set beyond what every processor of it has
    /// (`lang.target.Spec.features`).
    pub features: Vec<Symbol>,
    /// The floats its processor computes (`lang.target.Spec.floats`).
    pub floats: Floats,
    /// How many threads its programs run (`lang.target.Spec.threads`).
    pub threads: Threads,
    /// The value of its declaration: `lang.target.current`.
    pub declared: Value,
}

impl Target {
    /// What compile-time code sees of `target`.
    #[must_use]
    pub fn of(target: &Declared) -> Self {
        Self {
            endian: target.spec.endian,
            address_bits: target.spec.pointer_bits,
            register_bits: target.spec.register_bits,
            isa: target.spec.isa,
            features: target.spec.features.clone(),
            floats: target.spec.floats,
            threads: target.spec.threads,
            declared: target.value.clone(),
        }
    }

    /// Whether its processor computes with the type `value` names, which `lang.float.soft` and
    /// `lang.mem.soft` do otherwise (`lang.lower.soft`): a float type it lists, and an integer
    /// type, or the range of one, no wider than a register; any other value names nothing it
    /// lacks.
    #[must_use]
    pub fn computes(&self, value: &Value) -> bool {
        let range = match value {
            Value::Range(range) => range.clone(),
            Value::Type(ty) if let Some(format) = Format::of(ty) => {
                return self.floats.computes(format.bits());
            }
            Value::Type(ty) => ty.interval(),
            _ => None,
        };
        range.and_then(|r| r.storage()).is_none_or(|s| s.bits() <= u64::from(self.register_bits))
    }

    /// Whether its processor has what an instruction needs.
    #[must_use]
    pub fn has(&self, needed: Needed) -> bool {
        match needed {
            Needed::Feature(feature) => self.features.contains(&feature),
            Needed::Float(bits) => self.floats.computes(bits),
        }
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

/// A primitive of `lang.target` whose value is a fact of the target.
pub trait TargetConstant {
    /// How the value of a primitive constant of the target, such as `lang.target.usize_max`, is
    /// read from it; `None` for any other primitive, whose value is itself.
    #[must_use]
    fn target_constant(self) -> Option<fn(&Target) -> Value>;
}

impl TargetConstant for Primitive {
    fn target_constant(self) -> Option<fn(&Target) -> Value> {
        match self {
            Self::Target(Constant::usize_max) => Some(|t| Value::Int(t.usize_max())),
            Self::Target(Constant::current) => Some(|t| t.declared.clone()),
            _ => None,
        }
    }
}
