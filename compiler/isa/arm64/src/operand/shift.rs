//! How a register operand is shifted or extended before it is read.

use std::fmt;

use crate::reg::Size;

/// How a register operand is shifted, by its encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
    /// Rotation, which only the logical instructions take.
    Ror,
}

impl fmt::Display for ShiftKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Lsl => "lsl",
            Self::Lsr => "lsr",
            Self::Asr => "asr",
            Self::Ror => "ror",
        })
    }
}

/// A register shifted by a constant before the operation reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shift {
    pub kind: ShiftKind,
    pub amount: u8,
}

/// How a register operand is extended to the operation's width, by its encoding (`option`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extend {
    Uxtb,
    Uxth,
    Uxtw,
    Uxtx,
    Sxtb,
    Sxth,
    Sxtw,
    Sxtx,
}

impl Extend {
    /// A 64-bit index as it is: the extension a memory operand writes `lsl`.
    pub const LSL: Self = Self::Uxtx;

    #[must_use]
    pub const fn number(self) -> u32 {
        self as u32
    }

    /// The width of the register it reads.
    #[must_use]
    pub const fn reads(self) -> Size {
        match self {
            Self::Uxtx | Self::Sxtx => Size::B64,
            _ => Size::B32,
        }
    }

    /// The extension that leaves a register of `size` as it is.
    #[must_use]
    pub const fn identity(size: Size) -> Self {
        if matches!(size, Size::B64) { Self::Uxtx } else { Self::Uxtw }
    }
}

impl fmt::Display for Extend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Uxtb => "uxtb",
            Self::Uxth => "uxth",
            Self::Uxtw => "uxtw",
            Self::Uxtx => "uxtx",
            Self::Sxtb => "sxtb",
            Self::Sxth => "sxth",
            Self::Sxtw => "sxtw",
            Self::Sxtx => "sxtx",
        })
    }
}
