//! Paths a walk or an edit refuses to follow.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a path was refused. It reaches callers inside an [`io::Error`] of kind
/// [`io::ErrorKind::InvalidData`], as its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A symbolic link, which would let a tree name files outside itself.
    Link(PathBuf),
    /// A name that is not Unicode, which would not hash alike on every platform.
    Name(PathBuf),
    /// A path that is not a plain relative path inside its tree: absolute, or with `..`.
    Outside(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link(path) => write!(f, "{} is a symbolic link", path.display()),
            Self::Name(path) => write!(f, "{} is not named in Unicode", path.display()),
            Self::Outside(path) => write!(f, "{path} leaves its tree"),
        }
    }
}

impl std::error::Error for Refusal {}

impl From<Refusal> for io::Error {
    fn from(refusal: Refusal) -> Self {
        Self::new(io::ErrorKind::InvalidData, refusal)
    }
}
