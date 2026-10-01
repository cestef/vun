//! PE files as data (Microsoft's Portable Executable format): the machine a program is for, where
//! it is loaded and entered, its sections with their bytes, and the names it exports and imports.
//! Addresses are virtual: the image's base and the offset a table or section is loaded at.

mod headers;
mod tables;
#[cfg(test)]
mod tests;

use std::fmt;

use vun_bytes::Truncated;

pub use headers::Malformed;

/// The machine a file's code is for (`IMAGE_FILE_MACHINE_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Machine {
    X86,
    X86_64,
    Arm,
    Arm64,
    Other(u16),
}

impl Machine {
    /// The machine numbered `number`.
    #[must_use]
    pub const fn of(number: u16) -> Self {
        match number {
            0x014c => Self::X86,
            0x8664 => Self::X86_64,
            0x01c4 => Self::Arm,
            0xaa64 => Self::Arm64,
            other => Self::Other(other),
        }
    }
}

/// The characteristics of a section (`IMAGE_SCN_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionFlags(pub u32);

impl SectionFlags {
    pub const CODE: Self = Self(0x20);
    pub const EXECUTE: Self = Self(0x2000_0000);
    pub const READ: Self = Self(0x4000_0000);
    pub const WRITE: Self = Self(0x8000_0000);

    /// Whether every flag of `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// A section of the image, as loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: Vec<u8>,
    /// Where it is loaded: the image's base and its offset.
    pub address: u64,
    /// Its size in memory.
    pub size: u64,
    /// What the file holds of it, at most `size` bytes: the rest of it is zeros.
    pub data: Vec<u8>,
    pub flags: SectionFlags,
}

impl Section {
    /// Whether it holds code the loader may run.
    #[must_use]
    pub fn executable(&self) -> bool {
        self.flags.contains(SectionFlags::EXECUTE) || self.flags.contains(SectionFlags::CODE)
    }
}

/// A name the image makes available to others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    /// None for a function exported by its ordinal alone.
    pub name: Option<Vec<u8>>,
    pub ordinal: u32,
    /// Where it is, or, for a forwarder, where its text names the function that stands for it.
    pub address: u64,
    pub forwarder: Option<Vec<u8>>,
}

/// A function or value the image takes from a library, by a slot its loader fills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub library: Vec<u8>,
    /// None for one taken by its ordinal.
    pub name: Option<Vec<u8>>,
    pub ordinal: Option<u16>,
    /// The slot, whose address the program calls or jumps through.
    pub slot: u64,
}

/// A PE file: a program or a library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub machine: Machine,
    /// Whether it is a library (a DLL) rather than a program.
    pub library: bool,
    /// Whether it is the 64-bit format, PE32+.
    pub wide: bool,
    /// Where it is loaded unless that is taken.
    pub base: u64,
    /// Where it is entered; 0 where it has no entry.
    pub entry: u64,
    pub sections: Vec<Section>,
    pub exports: Vec<Export>,
    pub imports: Vec<Import>,
}

impl fmt::Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => f.write_str("x86"),
            Self::X86_64 => f.write_str("x86_64"),
            Self::Arm => f.write_str("arm"),
            Self::Arm64 => f.write_str("arm64"),
            Self::Other(n) => write!(f, "machine {n:#x}"),
        }
    }
}

impl From<Truncated> for Malformed {
    fn from(truncated: Truncated) -> Self {
        Self::Truncated(truncated)
    }
}
