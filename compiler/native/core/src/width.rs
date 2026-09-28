//! Sizes as every architecture reads them: the width a general register holds a scalar in, the
//! width of a float part of a record, and an offset in a 32-bit displacement.

use vun_asm::Width;
use vun_lir::Scalar;

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
#[must_use]
pub fn part<S: Width>(bytes: u64) -> S {
    if bytes > 4 { S::B64 } else { S::B32 }
}

/// `bytes` as a displacement.
/// # Panics
/// For a frame or arguments past 2 GiB.
#[must_use]
pub fn offset_of(bytes: u64) -> i32 {
    i32::try_from(bytes).expect("frames and arguments fit 2 GiB")
}
