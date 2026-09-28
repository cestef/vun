//! Operands: registers, shifted registers, memory, immediates, conditions, lists of registers,
//! labels and the symbols the linker places, and the names of special registers and choices. `S`
//! is whatever the caller names linked things by; relocations carry it back.

use std::fmt;

use vun_isa::Label;

use vun_inline::RegRef;

use crate::reg::{Barrier, Cc, Class, Mask, Reg, Special, Status, V};

/// How a register operand is shifted, by its encoding (`type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
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

/// A memory operand. Which offsets an instruction takes depends on its form: callers ask
/// (`Op::form`) and fold an offset or compute the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mem {
    /// `[base, #offset]`.
    Off { base: Reg, offset: i64 },
    /// `[base, #offset]!`: the base is advanced first, then read from.
    Pre { base: Reg, offset: i64 },
    /// `[base], #offset`: the base is read from, then advanced.
    Post { base: Reg, offset: i64 },
    /// `[base, index, lsl #shift]`.
    Index { base: Reg, index: Reg, shift: u8 },
}

impl Mem {
    #[must_use]
    pub const fn at(base: Reg, offset: i64) -> Self {
        Self::Off { base, offset }
    }

    #[must_use]
    pub const fn index(base: Reg, index: Reg) -> Self {
        Self::Index { base, index, shift: 0 }
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
            | Self::Index { base, .. } => *base,
        }
    }
}

/// Registers a `push`, `pop`, `vpush` or `vpop` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum List {
    /// General registers, a bit each by number.
    Regs(u16),
    /// `count` consecutive double-precision registers from `first`.
    Doubles { first: V, count: u8 },
}

impl List {
    /// The list of `regs` as a block writes them: general registers, each once, or
    /// floating-point ones, each the double-precision register after the one before (`d8, d9`);
    /// `None` for any other.
    #[must_use]
    pub fn of(regs: &[RegRef]) -> Option<Self> {
        let class = |r: &RegRef| Class::ALL[usize::from(r.class.0)];
        let first = regs.first()?;
        match class(first) {
            Class::General => regs
                .iter()
                .try_fold(0_u16, |bits, r| {
                    let bit = 1 << r.number;
                    (class(r) == Class::General && bits & bit == 0).then_some(bits | bit)
                })
                .map(Self::Regs),
            Class::Fp => {
                let doubles = regs.iter().enumerate().all(|(i, r)| {
                    class(r) == Class::Fp && usize::from(r.number) == usize::from(first.number) + 2 * i
                });
                let count = u8::try_from(regs.len()).ok()?;
                doubles.then(|| Self::Doubles { first: V::ALL[usize::from(first.number)], count })
            }
            Class::Special | Class::Status => None,
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
    Mem(Mem),
    Cond(Cc),
    Label(Label),
    /// The address of something the linker places, as a branch's or a call's target.
    Sym(S),
    /// The low or high 16 bits of the address of something the linker places.
    Low(S),
    High(S),
    List(List),
    Special(Special),
    Status(Status),
    Mask(Mask),
    Barrier(Barrier),
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

impl<S> From<Mem> for Operand<S> {
    fn from(m: Mem) -> Self {
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

impl<S> vun_isa::word::Named for Operand<S> {
    fn named(&self) -> Option<u8> {
        match self {
            Self::Reg(r) | Self::Shifted(r, _) => Some(r.number()),
            Self::Mem(mem) => Some(mem.base().number()),
            _ => None,
        }
    }
}
