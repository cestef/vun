//! Registers, operation widths and condition codes, by their encodings.

use std::fmt;

use crate::generated::NAMES;
pub use crate::generated::{Class, Reg};

impl Reg {
    /// Its number, 0 to 15.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The three bits `ModRM` and SIB hold.
    #[must_use]
    pub const fn low(self) -> u8 {
        self.number() & 7
    }

    /// Whether REX extends it.
    #[must_use]
    pub const fn high(self) -> bool {
        self.number() >= 8
    }

    /// Whether its low byte is reached only with a REX prefix (`spl`, `bpl`, `sil`, `dil`).
    #[must_use]
    pub const fn byte_needs_rex(self) -> bool {
        matches!(self, Self::Rsp | Self::Rbp | Self::Rsi | Self::Rdi)
    }

    /// Its name at `size`, as Intel syntax writes it.
    #[must_use]
    pub const fn name(self, size: Size) -> &'static str {
        NAMES[self.number() as usize][size as usize]
    }
}

/// An SSE register, by its encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Xmm(pub u8);

impl Xmm {
    pub const ALL: [Self; 16] = [
        Self(0),
        Self(1),
        Self(2),
        Self(3),
        Self(4),
        Self(5),
        Self(6),
        Self(7),
        Self(8),
        Self(9),
        Self(10),
        Self(11),
        Self(12),
        Self(13),
        Self(14),
        Self(15),
    ];

    #[must_use]
    pub const fn low(self) -> u8 {
        self.0 & 7
    }

    #[must_use]
    pub const fn high(self) -> bool {
        self.0 >= 8
    }
}

impl fmt::Display for Xmm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xmm{}", self.0)
    }
}

/// The width of an operation or an operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Size {
    B8,
    B16,
    B32,
    B64,
}

impl Size {
    pub const ALL: [Self; 4] = [Self::B8, Self::B16, Self::B32, Self::B64];
    /// Every size, widest first.
    pub const WIDEST: [Self; 4] = [Self::B64, Self::B32, Self::B16, Self::B8];

    #[must_use]
    pub const fn bytes(self) -> u32 {
        1 << self as u32
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.bytes() * 8
    }

    /// The size of `bytes`, a power of two up to 8.
    #[must_use]
    pub const fn of_bytes(bytes: u64) -> Option<Self> {
        match bytes {
            1 => Some(Self::B8),
            2 => Some(Self::B16),
            4 => Some(Self::B32),
            8 => Some(Self::B64),
            _ => None,
        }
    }

    /// The keyword Intel syntax sizes a memory operand of this width with.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::B8 => "byte",
            Self::B16 => "word",
            Self::B32 => "dword",
            Self::B64 => "qword",
        }
    }
}

use crate::generated::CONDITIONS;
pub use crate::generated::Cc;

/// A condition code, as `jcc`, `setcc` and `cmovcc` encode it: the encoding's low bit negates it.
impl Cc {
    /// The condition that holds where this one does not.
    #[must_use]
    pub const fn not(self) -> Self {
        Self::ALL[self as usize ^ 1]
    }

    /// Its suffix in mnemonics (`jae`, `setnz`).
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        CONDITIONS[self as usize]
    }
}
