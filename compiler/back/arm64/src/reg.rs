//! Registers, operation widths and condition codes, by their encodings.

use std::fmt;

use crate::generated::NAMES;
/// A general-purpose register. Register 31 is the stack pointer in some operands and the zero
/// register in others: the two are distinct registers here, and each operand kind takes the one
/// its field means (`Kind::Gpr`), so neither is ever guessed.
pub use crate::generated::Reg;

/// The encoding the stack pointer and the zero register share.
const SP_ZR: u8 = 31;

impl Reg {
    /// Its encoding, 0 to 31.
    #[must_use]
    pub const fn number(self) -> u8 {
        let n = self as u8;
        if n > SP_ZR { SP_ZR } else { n }
    }

    /// Its name at `size`: `x` at 64 bits, `w` below.
    #[must_use]
    pub const fn name(self, size: Size) -> Name {
        Name { reg: self, size }
    }
}

/// A general register's name at a width.
#[derive(Clone, Copy, Debug)]
pub struct Name {
    reg: Reg,
    size: Size,
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [w, x] = NAMES[self.reg as usize];
        f.write_str(if self.size == Size::B64 { x } else { w })
    }
}

/// A floating-point and vector register, by its number, 0 to 31.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct V(pub u8);

impl V {
    /// Its name at `size`: `b`, `h`, `s` or `d`.
    #[must_use]
    pub const fn name(self, size: Size) -> VName {
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
        const PREFIX: [char; 4] = ['b', 'h', 's', 'd'];
        write!(f, "{}{}", PREFIX[self.size as usize], self.v.0)
    }
}

/// The width of an operation, an access or an operand.
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

    /// The log2 of its bytes, which scaled offsets shift by.
    #[must_use]
    pub const fn shift(self) -> u32 {
        self as u32
    }

    #[must_use]
    pub const fn bytes(self) -> u32 {
        1 << self.shift()
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
}

/// A condition code, by its encoding. `al` and `nv` are left out: an unconditional branch is `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cc {
    Eq,
    Ne,
    /// Unsigned higher or same (carry set).
    Hs,
    /// Unsigned lower (carry clear).
    Lo,
    Mi,
    Pl,
    Vs,
    Vc,
    /// Unsigned higher.
    Hi,
    /// Unsigned lower or same.
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
}

impl Cc {
    pub const ALL: [Self; 14] = [
        Self::Eq,
        Self::Ne,
        Self::Hs,
        Self::Lo,
        Self::Mi,
        Self::Pl,
        Self::Vs,
        Self::Vc,
        Self::Hi,
        Self::Ls,
        Self::Ge,
        Self::Lt,
        Self::Gt,
        Self::Le,
    ];

    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The condition that holds where this one does not: the encoding's low bit flipped.
    #[must_use]
    pub const fn not(self) -> Self {
        Self::ALL[(self.number() ^ 1) as usize]
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        const NAMES: [&str; 14] =
            ["eq", "ne", "hs", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le"];
        NAMES[self.number() as usize]
    }
}

impl fmt::Display for Cc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
