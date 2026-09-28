//! Immediate encodings, each placing its fields itself where they are split (`i:imm3:imm8`).

use crate::generated::Place;

/// An immediate encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imm {
    /// A byte, shifted or repeated, in the fields `i:imm3:imm8` (the manual's `ThumbExpandImm`).
    Modified,
    /// 12 bits in the fields `i:imm3:imm8`: `addw`, `subw`.
    Plain,
    /// 16 bits in the fields `imm4:i:imm3:imm8`: `movw`, `movt`.
    Half,
    /// An unsigned value of this many bits, in one field.
    Unsigned(u8),
    /// A multiple of 4, as a count of words of this many bits, in one field.
    Words(u8),
    /// A left shift, 0 to 31.
    Lsl(Place),
    /// A right shift, 1 to 32, 32 written as 0.
    Shr(Place),
    /// Zero, which `rsbs` subtracts from and `vcmp` compares with.
    Zero,
}

/// The bits of a shift's amount, and the most a left shift moves.
const SHIFT_BITS: u32 = 5;
const WORD_BITS: u32 = 32;

impl Imm {
    /// The fields holding `value`, the bits of a 32-bit operand, if this encoding holds it.
    #[must_use]
    pub fn encode(self, value: i64) -> Option<u32> {
        // The operation's 32 bits; a wider value is not one of them.
        let v = u32::try_from(value).ok().or_else(|| i32::try_from(value).ok().map(i32::cast_unsigned))?;
        match self {
            Self::Modified => modified(v).map(split12),
            Self::Plain => (v < 1 << 12).then(|| split12(v)),
            Self::Half => (v <= 0xffff).then(|| (v >> 12) << 16 | split12(v & 0xfff)),
            Self::Unsigned(bits) => (v < 1 << bits).then_some(v),
            Self::Words(bits) => (v % 4 == 0 && v / 4 < 1 << bits).then_some(v / 4),
            Self::Lsl(place) => (v < WORD_BITS).then(|| place.shift(v)),
            Self::Shr(place) => (1..=WORD_BITS).contains(&v).then(|| place.shift(v % WORD_BITS)),
            Self::Zero => (v == 0).then_some(0),
        }
    }
}

impl Place {
    /// A shift's amount, 0 to 31, in its fields.
    #[must_use]
    pub const fn shift(self, amount: u32) -> u32 {
        match self {
            Self::Narrow => amount,
            Self::Wide => (amount >> 2) << 12 | (amount & 3) << 6,
        }
    }

    /// The amount a shift's fields hold.
    #[must_use]
    pub const fn amount(self, fields: u32) -> u32 {
        match self {
            Self::Narrow => fields & ((1 << SHIFT_BITS) - 1),
            Self::Wide => (fields >> 12 & 7) << 2 | (fields >> 6 & 3),
        }
    }
}

/// The 12 bits `i:imm3:imm8` in their fields: `i` at 26, `imm3` at 12, `imm8` at 0.
const fn split12(imm12: u32) -> u32 {
    (imm12 >> 11) << 26 | (imm12 >> 8 & 7) << 12 | imm12 & 0xff
}

/// The 12 bits giving `v` as the manual's `ThumbExpandImm` expands them: a byte, the byte
/// repeated in alternate bytes or in every one, or eight bits starting with a one rotated right
/// by 8 to 31.
const fn modified(v: u32) -> Option<u32> {
    let [b0, b1, b2, b3] = v.to_le_bytes();
    if v <= 0xff {
        return Some(v);
    }
    if b1 == 0 && b3 == 0 && b0 == b2 {
        return Some(0x100 | b0 as u32);
    }
    if b0 == 0 && b2 == 0 && b1 == b3 {
        return Some(0x200 | b1 as u32);
    }
    if b0 == b1 && b1 == b2 && b2 == b3 {
        return Some(0x300 | b0 as u32);
    }
    let mut rotation = 8;
    while rotation < WORD_BITS {
        let byte = v.rotate_left(rotation);
        if byte <= 0xff && byte >> 7 == 1 {
            return Some(rotation << 7 | byte & 0x7f);
        }
        rotation += 1;
    }
    None
}
