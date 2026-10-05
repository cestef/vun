//! The classes of locals and the widths, by their declarations' order.

use vun_inline::{ClassId, SizeId};

pub use crate::generated::{Class, Size};

impl Class {
    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> ClassId {
        ClassId(self as u8)
    }

    /// The class at `id`.
    #[must_use]
    pub const fn of(id: ClassId) -> Self {
        Self::ALL[id.0 as usize]
    }
}

impl Size {
    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> SizeId {
        SizeId(self as u8)
    }
}

impl vun_isa::Width for Size {
    const B32: Self = Self::B32;
    const B64: Self = Self::B64;

    fn of_bytes(bytes: u64) -> Option<Self> {
        match bytes {
            4 => Some(Self::B32),
            8 => Some(Self::B64),
            _ => None,
        }
    }

    fn bits(self) -> u32 {
        match self {
            Self::B32 => 32,
            Self::B64 => 64,
        }
    }
}
