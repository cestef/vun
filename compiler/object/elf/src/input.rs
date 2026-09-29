//! Reading a file's bytes: at an offset, a field at a time, every read checked against the end.

use std::fmt;

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
    /// A name runs past the end of its string table.
    Name(u64),
    /// A segment's sections are not a run of the file's.
    Segment(usize),
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Truncated { at, end } => write!(f, "the file ends at {end}, before {at}"),
            Self::Magic => f.write_str("the file does not start as ELF does"),
            Self::Class(class) => write!(f, "{class} is no ELF class"),
            Self::BigEndian => f.write_str("the file is big-endian"),
            Self::Link(k) => write!(f, "a section links section {k}, which the file lacks"),
            Self::Name(at) => write!(f, "the name at {at} runs past its string table"),
            Self::Segment(k) => write!(f, "segment {k} covers no run of sections"),
        }
    }
}

impl std::error::Error for Malformed {}

/// Bytes read from an offset on.
#[derive(Clone, Copy)]
pub(crate) struct Input<'a> {
    bytes: &'a [u8],
    at: u64,
}

impl<'a> Input<'a> {
    /// `bytes`, read from `at` on.
    pub(crate) const fn at(bytes: &'a [u8], at: u64) -> Self {
        Self { bytes, at }
    }

    /// The next `n` bytes.
    pub(crate) fn take(&mut self, n: u64) -> Result<&'a [u8], Malformed> {
        let end = self.bytes.len() as u64;
        let truncated = Malformed::Truncated { at: self.at + n, end };
        let range = usize::try_from(self.at).ok().zip(usize::try_from(self.at + n).ok());
        let taken = range.and_then(|(from, to)| self.bytes.get(from..to)).ok_or(truncated)?;
        self.at += n;
        Ok(taken)
    }

    /// The next `N` bytes, as an array.
    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        Ok(self.take(N as u64)?.try_into().expect("N bytes taken"))
    }

    /// The zero-ended string at `at` of these bytes, a string table.
    pub(crate) fn string(bytes: &'a [u8], at: u64) -> Result<&'a [u8], Malformed> {
        let rest = usize::try_from(at).ok().and_then(|at| bytes.get(at..)).ok_or(Malformed::Name(at))?;
        let end = rest.iter().position(|&b| b == 0).ok_or(Malformed::Name(at))?;
        Ok(&rest[..end])
    }
}
