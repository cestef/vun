//! A file's class: the width of its addresses and offsets, which lays out its records.

/// `ELFCLASS32` or `ELFCLASS64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Elf32,
    Elf64,
}

impl Class {
    /// The class of a machine whose addresses are `bits` wide.
    #[must_use]
    pub const fn of_bits(bits: u32) -> Self {
        match bits {
            64 => Self::Elf64,
            _ => Self::Elf32,
        }
    }

    /// Its `EI_CLASS` number.
    #[must_use]
    pub const fn ident(self) -> u8 {
        match self {
            Self::Elf32 => 1,
            Self::Elf64 => 2,
        }
    }

    /// The class numbered `ident`, if one is.
    #[must_use]
    pub const fn of_ident(ident: u8) -> Option<Self> {
        match ident {
            1 => Some(Self::Elf32),
            2 => Some(Self::Elf64),
            _ => None,
        }
    }

    /// The bytes of an address or an offset, which tables of them are aligned to.
    #[must_use]
    pub const fn word(self) -> u64 {
        match self {
            Self::Elf32 => 4,
            Self::Elf64 => 8,
        }
    }
}
