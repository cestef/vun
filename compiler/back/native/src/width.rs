//! Sizes as both architectures read them: the width a general register holds a scalar in, the
//! width of a float part of a record, and an offset in a 32-bit displacement.

use vun_lir::Scalar;

/// An architecture's operand sizes.
pub trait Width: Copy {
    const B32: Self;
    const B64: Self;
    /// The size of `bytes`, a power of two up to 8.
    fn of_bytes(bytes: u64) -> Option<Self>;
}

impl Width for vun_asm_x86_64::Size {
    const B32: Self = Self::B32;
    const B64: Self = Self::B64;

    fn of_bytes(bytes: u64) -> Option<Self> {
        Self::of_bytes(bytes)
    }
}

impl Width for vun_asm_arm64::Size {
    const B32: Self = Self::B32;
    const B64: Self = Self::B64;

    fn of_bytes(bytes: u64) -> Option<Self> {
        Self::of_bytes(bytes)
    }
}

/// The size of an integer, an address or the bits of a float as a general register holds them;
/// `None` for a 128-bit integer and for `f16`, which legalization removes.
#[must_use]
pub fn size_of<S: Width>(scalar: Scalar) -> Option<S> {
    match scalar {
        Scalar::I128 | Scalar::F16 => None,
        _ => S::of_bytes(scalar.bytes(u64::BITS)),
    }
}

/// The size of a float's bits of `bytes` bytes, or of a record's part: a word or a doubleword.
pub(crate) fn part<S: Width>(bytes: u64) -> S {
    if bytes > 4 { S::B64 } else { S::B32 }
}

/// `bytes` as a displacement.
pub(crate) fn offset_of(bytes: u64) -> i32 {
    i32::try_from(bytes).expect("frames and arguments fit 2 GiB")
}
