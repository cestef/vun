//! Operands: registers, shifted and extended registers, memory, immediates, conditions, labels
//! and the symbols the linker places. `S` is whatever the caller names linked things by;
//! relocations carry it back.

use vun_isa::{Label, Via};

use crate::reg::{Cc, Reg, V};

mod mem;
mod shift;
mod system;

pub use mem::{Mem, Scale};
pub use shift::{Extend, Shift, ShiftKind};
pub use system::{Barrier, SysReg};

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
    /// The low 12 bits of the address of something the linker places, or of its TLS
    /// descriptor.
    Lo12(S, Via),
    /// The domain and the accesses a barrier orders.
    Barrier(Barrier),
    /// A system register.
    Sys(SysReg),
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

impl<S> vun_isa::word::Named for Operand<S> {
    fn named(&self) -> Option<u8> {
        match self {
            Self::Reg(r) | Self::Shifted(r, _) | Self::Extended(r, ..) => Some(r.number()),
            Self::Mem(mem) => Some(mem.base().number()),
            _ => None,
        }
    }
}
