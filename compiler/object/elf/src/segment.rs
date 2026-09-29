//! Segments: what a program's loader maps, each named by what it covers, so that the writer lays
//! them out with the sections and the reader finds them again from their program headers.

use std::ops::{BitOr, Range};

numbered! {
    /// What a segment is (`PT_*`).
    pub enum SegmentKind: u32 {
        Null = 0,
        /// Mapped into memory.
        Load = 1,
        Dynamic = 2,
        /// The program's interpreter.
        Interp = 3,
        Note = 4,
        /// The program headers themselves.
        Phdr = 6,
        /// The template of thread-local storage.
        Tls = 7,
        /// Whether the stack is executable, by its flags alone.
        GnuStack = 0x6474_e551,
        /// What is read-only once relocated.
        GnuRelro = 0x6474_e552,
    }
}

/// How a segment's memory may be used (`PF_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SegmentFlags(pub u32);

impl SegmentFlags {
    pub const NONE: Self = Self(0);
    pub const EXEC: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const READ: Self = Self(4);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for SegmentFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// What a segment covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Covers {
    /// No bytes: a segment of flags alone (`PT_GNU_STACK`).
    Nothing,
    /// The program headers.
    ProgramHeaders,
    /// A run of sections, by their places in `File::sections`, after the file header and the
    /// program headers where `headers` says so, as a linker's first loaded segment is.
    Sections { headers: bool, sections: Range<usize> },
}

/// A segment: its kind, flags and alignment, and what it covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub flags: SegmentFlags,
    pub align: u64,
    pub covers: Covers,
}
