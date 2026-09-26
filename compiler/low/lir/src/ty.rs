//! The types of LIR values: scalars. Aggregates live in memory and have no LIR type.

use std::fmt;

use vun_codec::{Decode, Encode};

/// A scalar: an integer of a width, a float, or an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Scalar {
    I8,
    I16,
    I32,
    I64,
    I128,
    F16,
    F32,
    F64,
    Ptr,
}

impl Scalar {
    /// The integers, narrowest first.
    pub const INTEGERS: [Self; 5] = [Self::I8, Self::I16, Self::I32, Self::I64, Self::I128];

    /// Its width in bits; an address's is the target's, `pointer_bits`.
    #[must_use]
    pub const fn bits(self, pointer_bits: u32) -> u32 {
        match self {
            Self::I8 => 8,
            Self::I16 | Self::F16 => 16,
            Self::I32 | Self::F32 => 32,
            Self::I64 | Self::F64 => 64,
            Self::I128 => 128,
            Self::Ptr => pointer_bits,
        }
    }

    /// The bits of the integer `n` as a constant of this integer type: its two's complement,
    /// truncated to the width.
    #[must_use]
    pub fn truncate(self, n: i128) -> u128 {
        #[expect(clippy::cast_sign_loss, reason = "two's complement bits")]
        let bits = n as u128;
        match self.bits(0) {
            128 => bits,
            width => bits & ((1 << width) - 1),
        }
    }

    /// Its width in bytes.
    #[must_use]
    pub const fn bytes(self, pointer_bits: u32) -> u64 {
        (self.bits(pointer_bits) / u8::BITS) as u64
    }

    /// The integer of `bits` bits, if one is that wide.
    #[must_use]
    pub fn int(bits: u32) -> Option<Self> {
        Self::INTEGERS.into_iter().find(|i| i.bits(0) == bits)
    }

    #[must_use]
    pub const fn is_int(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128)
    }

    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(self, Self::F16 | Self::F32 | Self::F64)
    }

    /// The name the text format writes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::F16 => "f16",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Ptr => "ptr",
        }
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
