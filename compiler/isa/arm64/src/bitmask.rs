//! Bitmask immediates, which the logical instructions take: an element of 2 to 64 bits holding
//! one run of ones, rotated, repeated across the register. They are encoded as `N:immr:imms`:
//! `N` and the high bits of `imms` give the element's size, the rest of `imms` the run's length
//! less one, `immr` its rotation to the right.

use vun_isa::imm::truncate;

/// The six bits of `imms`.
const IMMS: u32 = 0x3f;

/// A bitmask immediate, as its 13 bits `N:immr:imms`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitmask(u32);

impl Bitmask {
    /// The encoding of `value` as an operation of `bits` (32 or 64) reads it, if it is a bitmask.
    #[must_use]
    pub fn of(value: i64, bits: u32) -> Option<Self> {
        let mut v = truncate(value, bits);
        if bits == 32 {
            v |= v << 32;
        }
        if v == 0 || v == u64::MAX {
            return None;
        }
        let mut size = 64;
        while size > 2 {
            let half = size / 2;
            let mask = (1 << half) - 1;
            if v & mask != (v >> half) & mask {
                break;
            }
            size = half;
        }
        let mask = if size == 64 { u64::MAX } else { (1 << size) - 1 };
        let element = v & mask;
        // One run, rotated, starts at exactly one bit: a one with a zero below it, cyclically.
        let below = (element << 1 | element >> (size - 1)) & mask;
        let starts = element & !below;
        if starts.count_ones() != 1 {
            return None;
        }
        let rotation = (size - starts.trailing_zeros()) % size;
        // The element's size as the ones above a zero, then the run's length less one.
        let length = (!(size - 1) << 1 & IMMS) | (element.count_ones() - 1);
        Some(Self(u32::from(size == 64) << 12 | rotation << 6 | length))
    }

    /// The 13 bits `N:immr:imms`.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}
