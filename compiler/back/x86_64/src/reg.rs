//! Registers, operation widths and condition codes, by their encodings.

use std::fmt;

/// A general-purpose register, by its encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reg {
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

impl Reg {
    pub const ALL: [Self; 16] = [
        Self::Rax,
        Self::Rcx,
        Self::Rdx,
        Self::Rbx,
        Self::Rsp,
        Self::Rbp,
        Self::Rsi,
        Self::Rdi,
        Self::R8,
        Self::R9,
        Self::R10,
        Self::R11,
        Self::R12,
        Self::R13,
        Self::R14,
        Self::R15,
    ];

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
        const NAMES: [[&str; 4]; 16] = [
            ["al", "ax", "eax", "rax"],
            ["cl", "cx", "ecx", "rcx"],
            ["dl", "dx", "edx", "rdx"],
            ["bl", "bx", "ebx", "rbx"],
            ["spl", "sp", "esp", "rsp"],
            ["bpl", "bp", "ebp", "rbp"],
            ["sil", "si", "esi", "rsi"],
            ["dil", "di", "edi", "rdi"],
            ["r8b", "r8w", "r8d", "r8"],
            ["r9b", "r9w", "r9d", "r9"],
            ["r10b", "r10w", "r10d", "r10"],
            ["r11b", "r11w", "r11d", "r11"],
            ["r12b", "r12w", "r12d", "r12"],
            ["r13b", "r13w", "r13d", "r13"],
            ["r14b", "r14w", "r14d", "r14"],
            ["r15b", "r15w", "r15d", "r15"],
        ];
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

/// A condition code, as `jcc`, `setcc` and `cmovcc` encode it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cc {
    B = 0x2,
    Ae = 0x3,
    E = 0x4,
    Ne = 0x5,
    Be = 0x6,
    A = 0x7,
    /// Parity: after `ucomis`, the operands are unordered (a NaN among them).
    P = 0xa,
    Np = 0xb,
    L = 0xc,
    Ge = 0xd,
    Le = 0xe,
    G = 0xf,
}

impl Cc {
    /// The condition that holds where this one does not.
    #[must_use]
    pub const fn not(self) -> Self {
        match self {
            Self::B => Self::Ae,
            Self::Ae => Self::B,
            Self::E => Self::Ne,
            Self::Ne => Self::E,
            Self::Be => Self::A,
            Self::A => Self::Be,
            Self::P => Self::Np,
            Self::Np => Self::P,
            Self::L => Self::Ge,
            Self::Ge => Self::L,
            Self::Le => Self::G,
            Self::G => Self::Le,
        }
    }

    /// Its suffix in mnemonics (`jae`, `setnz`).
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::B => "b",
            Self::Ae => "ae",
            Self::E => "z",
            Self::Ne => "nz",
            Self::Be => "be",
            Self::A => "a",
            Self::P => "p",
            Self::Np => "np",
            Self::L => "l",
            Self::Ge => "ge",
            Self::Le => "le",
            Self::G => "g",
        }
    }
}
