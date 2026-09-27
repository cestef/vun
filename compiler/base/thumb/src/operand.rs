//! Operands: general and special registers, and the named choices of masks and barriers.

use crate::reg::{Barrier, Mask, Reg, Special};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    Reg(Reg),
    Special(Special),
    Mask(Mask),
    Barrier(Barrier),
}

impl Operand {
    /// Its name, as GNU as reads it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reg(r) => r.name(),
            Self::Special(s) => s.name(),
            Self::Mask(m) => m.name(),
            Self::Barrier(b) => b.name(),
        }
    }
}

impl From<Reg> for Operand {
    fn from(r: Reg) -> Self {
        Self::Reg(r)
    }
}
