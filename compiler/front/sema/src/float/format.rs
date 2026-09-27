//! The binary interchange formats: each by the widths of its exponent and fraction fields, from
//! which every other fact of the format is derived.

use crate::Primitive;
use crate::primitives::lang::Lang;
use crate::int::Integer;
use crate::ty::{Ty, TyKind, Value};
use vun_codec::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Format {
    F16,
    F32,
    F64,
}

impl Format {
    /// The format of a float literal that nothing gives a float type (spec 04, section 2).
    pub const DEFAULT: Self = Self::F64;

    pub const ALL: [Self; 3] = [Self::F16, Self::F32, Self::F64];

    /// The format of `bits` bits, if there is one: what `lang.Float(bits)` is.
    #[must_use]
    pub fn of_bits(bits: &Integer) -> Option<Self> {
        Self::ALL.into_iter().find(|f| *bits == Integer::from(i128::from(f.bits())))
    }

    /// The widths of the exponent and fraction fields.
    const fn fields(self) -> (u32, u32) {
        match self {
            Self::F16 => (5, 10),
            Self::F32 => (8, 23),
            Self::F64 => (11, 52),
        }
    }

    /// The width of the whole value, in bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        let (exponent, fraction) = self.fields();
        1 + exponent + fraction
    }

    pub(crate) const fn fraction(self) -> u32 {
        self.fields().1
    }

    /// The significand's bits, the implicit one included.
    pub(crate) const fn precision(self) -> u32 {
        self.fraction() + 1
    }

    /// The exponent field with every bit set: infinities and NaNs.
    pub(crate) const fn exponent_all(self) -> u64 {
        (1 << self.fields().0) - 1
    }

    /// The sign bit, where `negative`.
    pub(crate) const fn sign(self, negative: bool) -> u64 {
        if negative { 1 << (self.bits() - 1) } else { 0 }
    }

    /// The exponent of the least significant bit of the smallest values: subnormals and the
    /// smallest normal numbers.
    pub(crate) fn ulp_min(self) -> i64 {
        let bias = (1i64 << (self.fields().0 - 1)) - 1;
        1 - bias - i64::from(self.fraction())
    }

    /// The exponent of the least significant bit of the largest finite values.
    pub(crate) fn ulp_max(self) -> i64 {
        i64::try_from(self.exponent_all()).expect("under 2^11") - 2 + self.ulp_min()
    }

    /// The format of the float type `ty`, if it is one.
    #[must_use]
    pub fn of(ty: &TyKind) -> Option<Self> {
        match ty {
            TyKind::Primitive(Primitive::Lang(Lang::Float), args) => match args.as_slice() {
                [Value::Int(bits)] => Self::of_bits(bits),
                _ => None,
            },
            _ => None,
        }
    }

    /// The float type of this format, `lang.Float(bits)`.
    #[must_use]
    pub fn ty(self) -> Ty {
        let bits = Value::Int(Integer::from(i128::from(self.bits())));
        TyKind::Primitive(Primitive::Lang(Lang::Float), vec![bits]).intern()
    }
}
