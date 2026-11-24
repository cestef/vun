//! Why a text is no [`Hash`](crate::Hash) or [`Snapshot`](crate::Snapshot).

use std::error::Error;
use std::fmt;

/// Why a text is no hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotHash {
    /// No separator between the algorithm and the digest.
    Separator,
    Algorithm,
    Digest,
}

impl fmt::Display for NotHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Separator => "a hash is written algorithm:digest",
            Self::Algorithm => "unknown hash algorithm",
            Self::Digest => "malformed hash digest",
        })
    }
}

impl Error for NotHash {}

/// What is wrong with a line of a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Fields,
    Hash(NotHash),
    Mode,
    Path,
}

/// Why a text is no snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotSnapshot {
    /// A line, counted from 1, with a fault.
    Line { line: usize, fault: Fault },
    /// A file is also a directory of the snapshot.
    Nested,
}

impl fmt::Display for NotSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Line { line, fault } => {
                let what = match fault {
                    Fault::Fields => "expected `hash mode path`",
                    Fault::Hash(_) => "malformed hash",
                    Fault::Mode => "the mode is `-` or `x`",
                    Fault::Path => "the path is not a plain relative path",
                };
                write!(f, "snapshot line {line}: {what}")
            }
            Self::Nested => f.write_str("a file of a snapshot cannot hold files"),
        }
    }
}

impl Error for NotSnapshot {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Line { fault: Fault::Hash(reason), .. } => Some(reason),
            _ => None,
        }
    }
}
