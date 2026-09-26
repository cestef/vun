//! Operands: registers, shifted and extended registers, memory, immediates, conditions, labels
//! and the symbols the linker places. `S` is whatever the caller names linked things by;
//! relocations carry it back.

use std::fmt;

use vun_asm::{Label, Via};

use crate::reg::{Cc, Reg, Size, V};

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

/// What a register index is multiplied by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    One,
    /// The width of the access, so that the index counts elements.
    Access,
}

/// A memory operand. Which offsets an instruction takes depends on its form: callers ask
/// (`Op::form`) and fold an offset or materialize the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mem<S> {
    /// `[base, #offset]`.
    Off { base: Reg, offset: i64 },
    /// `[base, #offset]!`: the base is advanced first, then read from.
    Pre { base: Reg, offset: i64 },
    /// `[base], #offset`: the base is read from, then advanced.
    Post { base: Reg, offset: i64 },
    /// `[base, index, extend]`.
    Index { base: Reg, index: Reg, extend: Extend, scale: Scale },
    /// `[base, :lo12:target]`: the low 12 bits of an address whose page `adrp` put in `base`.
    Lo12 { base: Reg, target: S, via: Via },
}

impl<S> Mem<S> {
    #[must_use]
    pub const fn at(base: Reg, offset: i64) -> Self {
        Self::Off { base, offset }
    }

    #[must_use]
    pub const fn index(base: Reg, index: Reg) -> Self {
        Self::Index { base, index, extend: Extend::LSL, scale: Scale::One }
    }

    /// `bytes` further on, for an operand at an offset.
    #[must_use]
    pub fn plus(self, bytes: i64) -> Option<Self> {
        match self {
            Self::Off { base, offset } => Some(Self::Off { base, offset: offset.checked_add(bytes)? }),
            _ => None,
        }
    }

    /// The base register.
    #[must_use]
    pub const fn base(&self) -> Reg {
        match self {
            Self::Off { base, .. }
            | Self::Pre { base, .. }
            | Self::Post { base, .. }
            | Self::Index { base, .. }
            | Self::Lo12 { base, .. } => *base,
        }
    }
}

/// An operand of an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operand<S> {
    Reg(Reg),
    V(V),
    /// The value the instruction reads, as the bits of an `i64`; each form says which values it
    /// encodes and how.
    Imm(i64),
    /// A register shifted before it is read; a register unshifted is `Reg`.
    Shifted(Reg, Shift),
    /// A register extended and shifted left by 0 to 4 before it is read; a register read as it
    /// is, `Reg`.
    Extended(Reg, Extend, u8),
    Mem(Mem<S>),
    Cond(Cc),
    Label(Label),
    /// The address of something the linker places, as a branch's or a call's target.
    Sym(S),
    /// The page of something the linker places, or of its entry in the global offset table.
    Page(S, Via),
    /// The low 12 bits of the address of something the linker places.
    Lo12(S),
}

impl<S> From<Reg> for Operand<S> {
    fn from(r: Reg) -> Self {
        Self::Reg(r)
    }
}

impl<S> From<V> for Operand<S> {
    fn from(v: V) -> Self {
        Self::V(v)
    }
}

impl<S> From<Mem<S>> for Operand<S> {
    fn from(m: Mem<S>) -> Self {
        Self::Mem(m)
    }
}

impl<S> From<Label> for Operand<S> {
    fn from(l: Label) -> Self {
        Self::Label(l)
    }
}

impl<S> From<Cc> for Operand<S> {
    fn from(cc: Cc) -> Self {
        Self::Cond(cc)
    }
}

impl<S> From<i64> for Operand<S> {
    fn from(imm: i64) -> Self {
        Self::Imm(imm)
    }
}
