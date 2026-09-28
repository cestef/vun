//! Registers, conditions and widths, by their declarations' order, which is their encoding.

use std::fmt;

pub use crate::generated::{Barrier, Cc, Choice, Class, Mask, Reg, Size, Special, Status, V};
use crate::generated::{CONDITIONS, NAMES};

impl Reg {
    /// Its encoding, 0 to 15.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// Whether a 16-bit form's three-bit field names it.
    #[must_use]
    pub const fn is_low(self) -> bool {
        self.number() < 8
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        NAMES[self as usize][0]
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl V {
    /// Its number as a single-precision register, 0 to 31.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The register at `size`'s number in its fields: `s{n}` is `n`, `d{k}` is `k`, which only an
    /// even single-precision number is.
    #[must_use]
    pub const fn at(self, size: Size) -> Option<u8> {
        match size {
            Size::B64 if self.number() % 2 == 0 => Some(self.number() / 2),
            Size::B64 => None,
            _ => Some(self.number()),
        }
    }

    /// Its name at `size`: `s{n}`, or `d{n/2}` at 64 bits.
    #[must_use]
    pub const fn name_at(self, size: Size) -> VName {
        VName { v: self, size }
    }
}

/// A floating-point register's name at a width.
#[derive(Clone, Copy, Debug)]
pub struct VName {
    v: V,
    size: Size,
}

impl fmt::Display for VName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.size {
            Size::B64 => write!(f, "d{}", self.v.number() / 2),
            _ => write!(f, "s{}", self.v.number()),
        }
    }
}

impl Size {
    #[must_use]
    pub const fn bits(self) -> u32 {
        8 << self as u32
    }

    #[must_use]
    pub const fn bytes(self) -> u32 {
        self.bits() / 8
    }

    /// `log2` of its bytes: what a count of accesses is shifted by.
    #[must_use]
    pub const fn shift(self) -> u32 {
        self as u32
    }
}

impl Cc {
    /// Its encoding, 0 to 14.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The condition that holds where this one does not.
    ///
    /// # Panics
    /// For `al`, which always holds.
    #[must_use]
    pub const fn not(self) -> Self {
        Self::ALL[(self as usize) ^ 1]
    }

    /// Its name, as a mnemonic's suffix writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        CONDITIONS[self as usize]
    }
}

impl fmt::Display for Cc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Special {
    /// Its `SYSm` number, as `mrs` and `msr` encode it (Armv7-M, section B5.1.1).
    #[must_use]
    pub const fn field(self) -> u32 {
        match self {
            Self::Primask => 16,
            Self::Basepri => 17,
            Self::Faultmask => 19,
            Self::Control => 20,
        }
    }
}

impl Barrier {
    /// Its `option` field.
    #[must_use]
    pub const fn field(self) -> u32 {
        match self {
            Self::Sy => 0xf,
        }
    }
}

impl Mask {
    /// Its bit among `cpsid`'s `I` and `F`.
    #[must_use]
    pub const fn field(self) -> u32 {
        1 << self as u32
    }
}
