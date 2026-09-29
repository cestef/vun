//! Sections: named ranges of the file of a kind, with what the loader does with them, and the
//! bytes they hold.

use std::ops::BitOr;

numbered! {
    /// What a section holds (`SHT_*`).
    pub enum SectionKind: u32 {
        Null = 0,
        /// Bytes the program defines: code and data.
        ProgBits = 1,
        SymTab = 2,
        StrTab = 3,
        /// Relocations with their addends.
        Rela = 4,
        /// Bytes the loader zeroes, none of them in the file.
        NoBits = 8,
        /// The build attributes of a 32-bit Arm object (`SHT_ARM_ATTRIBUTES`).
        ArmAttributes = 0x7000_0003,
    }
}

/// What the loader does with a section (`SHF_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SectionFlags(pub u64);

impl SectionFlags {
    pub const NONE: Self = Self(0);
    pub const WRITE: Self = Self(1);
    /// Loaded into memory.
    pub const ALLOC: Self = Self(2);
    pub const EXEC: Self = Self(4);
    /// `info` numbers a section: the one a relocation section applies to.
    pub const INFO_LINK: Self = Self(0x40);
    /// Storage per thread: each thread's copy starts as its bytes.
    pub const TLS: Self = Self(0x400);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for SectionFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// What a section holds in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Contents {
    Bytes(Vec<u8>),
    /// This many bytes the loader zeroes, none in the file.
    Zeros(u64),
    /// The sections' names, which the writer lays out from them and the reader reads them from.
    Names,
}

/// A section: its name, kind and flags, where it is loaded, what it holds, and the numbers its
/// kind gives meaning to (`link` and `info`: a relocation section's symbol table and the section
/// it applies to).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: Vec<u8>,
    pub kind: SectionKind,
    pub flags: SectionFlags,
    pub address: u64,
    pub contents: Contents,
    pub link: u32,
    pub info: u32,
    pub align: u64,
    /// The size of each entry of a table.
    pub entsize: u64,
}

impl Section {
    /// A section of `kind` named `name` holding `bytes`, aligned to `align`.
    #[must_use]
    pub fn new(name: &[u8], kind: SectionKind, flags: SectionFlags, bytes: Vec<u8>, align: u64) -> Self {
        Self {
            name: name.to_vec(),
            kind,
            flags,
            address: 0,
            contents: Contents::Bytes(bytes),
            link: 0,
            info: 0,
            align,
            entsize: 0,
        }
    }

    /// The table of the sections' names, where the writer lays them out.
    #[must_use]
    pub fn names() -> Self {
        Self {
            contents: Contents::Names,
            ..Self::new(b".shstrtab", SectionKind::StrTab, SectionFlags::NONE, Vec::new(), 1)
        }
    }

    /// The bytes it holds in the file; none for zeros, and none for the names, which are the
    /// sections'.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match &self.contents {
            Contents::Bytes(bytes) => bytes,
            Contents::Zeros(_) | Contents::Names => &[],
        }
    }

    /// Its size in memory, where it is loaded: the names never are.
    #[must_use]
    pub fn size(&self) -> u64 {
        match &self.contents {
            Contents::Bytes(bytes) => bytes.len() as u64,
            &Contents::Zeros(size) => size,
            Contents::Names => 0,
        }
    }
}
