//! Memory operands.

use vun_isa::Via;

use crate::reg::Reg;

use super::Extend;

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
