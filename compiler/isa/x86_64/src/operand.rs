//! Operands: registers, memory, immediates, labels and the symbols the linker places. `S` is
//! whatever the caller names linked things by; relocations carry it back.

use vun_isa::{Label, Via};

use crate::reg::{Cc, Reg, Xmm};

/// What a memory operand's address is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Base<S> {
    Reg(Reg),
    /// The sum of two registers, the second a byte index (never `rsp`).
    Sum(Reg, Reg),
    /// The instruction's own address: something the linker places.
    Rip(S, Via),
    /// The calling thread's block, in the segment `fs` names: `disp` is an offset in it, `0`
    /// holding the thread pointer itself.
    Thread,
}

/// A memory operand: `disp` bytes past its base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mem<S> {
    pub base: Base<S>,
    pub disp: i32,
}

impl<S> Mem<S> {
    #[must_use]
    pub const fn at(base: Reg, disp: i32) -> Self {
        Self { base: Base::Reg(base), disp }
    }

    /// `disp` bytes past the sum of `base` and `index`.
    #[must_use]
    pub const fn sum(base: Reg, index: Reg, disp: i32) -> Self {
        Self { base: Base::Sum(base, index), disp }
    }

    #[must_use]
    pub const fn rip(target: S, via: Via) -> Self {
        Self { base: Base::Rip(target, via), disp: 0 }
    }

    /// `disp` bytes into the calling thread's block.
    #[must_use]
    pub const fn thread(disp: i32) -> Self {
        Self { base: Base::Thread, disp }
    }

    /// `bytes` further on, when the displacement still fits.
    #[must_use]
    pub fn plus(self, bytes: i64) -> Option<Self> {
        let disp = i32::try_from(i64::from(self.disp) + bytes).ok()?;
        Some(Self { disp, ..self })
    }
}

/// An operand of an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operand<S> {
    Reg(Reg),
    Xmm(Xmm),
    Mem(Mem<S>),
    /// The value the instruction reads, as the bits of an `i64`; each form says how many of
    /// them it encodes and how it extends them.
    Imm(i64),
    Label(Label),
    /// The address of something the linker places, as a call's target.
    Sym(S),
    Cond(Cc),
}

impl<S> From<Reg> for Operand<S> {
    fn from(r: Reg) -> Self {
        Self::Reg(r)
    }
}

impl<S> From<Xmm> for Operand<S> {
    fn from(x: Xmm) -> Self {
        Self::Xmm(x)
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
