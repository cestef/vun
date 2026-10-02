//! Why bytes are not an ELF file this crate reads.

use std::fmt;

use vun_bytes::{NAME_MAX, Overspent, Truncated};

/// Why bytes are not an ELF file this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Something ends past the file's end, which is at `end`.
    Truncated { at: u64, end: u64 },
    /// The first bytes are not ELF's.
    Magic,
    /// `EI_CLASS` names no class.
    Class(u8),
    /// `EI_DATA` is not little-endian.
    BigEndian,
    /// A section links one the file lacks.
    Link(u32),
    /// A name has no end in its string table, or is longer than the readers copy.
    Name(u64),
    /// A segment's sections are not a run of the file's.
    Segment(usize),
    /// A segment's file or memory range does not fit the address space.
    Range(usize),
    /// The sections hold more bytes than the file does: a well-formed file holds each byte of
    /// them once, so these overlap.
    Overlap,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Truncated { at, end } => write!(f, "the file ends at {end}, before {at}"),
            Self::Magic => f.write_str("the file does not start as ELF does"),
            Self::Class(class) => write!(f, "{class} is no ELF class"),
            Self::BigEndian => f.write_str("the file is big-endian"),
            Self::Link(k) => write!(f, "a section links section {k}, which the file lacks"),
            Self::Name(at) => {
                write!(f, "the name at {at} has no end in its string table within {NAME_MAX} bytes")
            }
            Self::Segment(k) => write!(f, "segment {k} covers no run of sections"),
            Self::Range(k) => write!(f, "segment {k} reaches past the end of the address space"),
            Self::Overlap => f.write_str("the sections hold more bytes than the file does"),
        }
    }
}

impl std::error::Error for Malformed {}

impl From<Truncated> for Malformed {
    fn from(Truncated { at, end }: Truncated) -> Self {
        Self::Truncated { at, end }
    }
}

impl From<Overspent> for Malformed {
    fn from(Overspent: Overspent) -> Self {
        Self::Overlap
    }
}
