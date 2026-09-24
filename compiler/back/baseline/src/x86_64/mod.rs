//! x86-64: registers, operands and condition codes, the encoder, the System V calling convention
//! and the single-pass function compiler.

pub mod abi;
pub mod encode;
pub mod func;

use vun_lir::{Address, Cond, Scalar};

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
    /// The registers values are held in, lowest first: those a call may overwrite, but the two
    /// kept for the compiler's own moves.
    pub const ALLOCATABLE: [Self; 7] =
        [Self::Rax, Self::Rcx, Self::Rdx, Self::Rsi, Self::Rdi, Self::R8, Self::R9];
    /// Carries a value between two memory places, or an operand an instruction needs apart.
    pub const SCRATCH: Self = Self::R11;
    /// Holds a second operand apart, or the temporary of a cycle of moves.
    pub const TEMP: Self = Self::R10;

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
}

/// The width of an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    B8,
    B16,
    B32,
    B64,
}

impl Size {
    /// The size of an integer or address; `None` for a float or a 128-bit integer.
    #[must_use]
    pub const fn of(scalar: Scalar) -> Option<Self> {
        match scalar {
            Scalar::I8 => Some(Self::B8),
            Scalar::I16 => Some(Self::B16),
            Scalar::I32 => Some(Self::B32),
            Scalar::I64 | Scalar::Ptr => Some(Self::B64),
            Scalar::I128 | Scalar::F16 | Scalar::F32 | Scalar::F64 => None,
        }
    }

    #[must_use]
    pub const fn bytes(self) -> u32 {
        match self {
            Self::B8 => 1,
            Self::B16 => 2,
            Self::B32 => 4,
            Self::B64 => 8,
        }
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

    /// The width arithmetic runs at: bits above a value's own are undefined in a register, so
    /// narrow operations run on 32 bits.
    #[must_use]
    pub const fn wide(self) -> Self {
        match self {
            Self::B64 => Self::B64,
            _ => Self::B32,
        }
    }
}

/// A condition code, as `jcc`, `setcc` and `cmovcc` encode it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cc {
    B = 0x2,
    Ae = 0x3,
    E = 0x4,
    Ne = 0x5,
    Be = 0x6,
    A = 0x7,
    L = 0xc,
    Ge = 0xd,
    Le = 0xe,
    G = 0xf,
}

impl Cc {
    /// The condition an integer comparison holds on, after `cmp a, b`; `None` for floats.
    #[must_use]
    pub const fn of(cond: Cond) -> Option<Self> {
        Some(match cond {
            Cond::Eq => Self::E,
            Cond::Ne => Self::Ne,
            Cond::SLt => Self::L,
            Cond::SLe => Self::Le,
            Cond::SGt => Self::G,
            Cond::SGe => Self::Ge,
            Cond::ULt => Self::B,
            Cond::ULe => Self::Be,
            Cond::UGt => Self::A,
            Cond::UGe => Self::Ae,
            Cond::FEq | Cond::FNe | Cond::FLt | Cond::FLe | Cond::FGt | Cond::FGe => return None,
        })
    }
}

/// What a memory operand's address is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Reg(Reg),
    /// The instruction's own address: something the linker places, reached directly, or through
    /// the global offset table for a symbol defined elsewhere.
    Rip(Address, Via),
}

/// How a RIP-relative operand reaches its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Direct,
    Got,
}

/// A memory operand: `disp` bytes past its base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mem {
    pub base: Base,
    pub disp: i32,
}

impl Mem {
    #[must_use]
    pub const fn at(base: Reg, disp: i32) -> Self {
        Self { base: Base::Reg(base), disp }
    }

    /// `bytes` further on.
    #[must_use]
    pub const fn plus(self, bytes: i32) -> Self {
        Self { disp: self.disp + bytes, ..self }
    }
}

/// An operand that is a register or memory: what `ModRM`'s `rm` field names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rm {
    Reg(Reg),
    Mem(Mem),
}
