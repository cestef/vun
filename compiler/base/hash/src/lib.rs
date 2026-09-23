//! Content hashes, of bytes and of file trees: what identifies a package (spec 12, section 3) and
//! keys every cache.

mod base32;
mod tree;

use std::fmt;

pub use tree::{Change, Entry, Snapshot};

/// The hash functions a [`Hash`] can come from; the text form names it, so that another can
/// follow without ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Algorithm {
    Blake3,
}

impl Algorithm {
    pub const ALL: &[Self] = &[Self::Blake3];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
        }
    }
}

/// A digest and the algorithm that produced it; written `blake3:<base32 digest>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash {
    algorithm: Algorithm,
    digest: [u8; 32],
}

impl Hash {
    /// Separates the algorithm from the digest in the text form.
    pub const SEPARATOR: char = ':';

    /// The hash of a file's contents, which is also its leaf in a tree hash.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self { algorithm: Algorithm::Blake3, digest: *blake3::hash(bytes).as_bytes() }
    }

    /// Reads the text form; `None` for an unknown algorithm or a malformed digest.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (name, digest) = text.split_once(Self::SEPARATOR)?;
        let &algorithm = Algorithm::ALL.iter().find(|a| a.name() == name)?;
        Self::from_digest(algorithm, digest)
    }

    /// The hash of `algorithm` whose digest has the text encoding `digest`.
    #[must_use]
    pub fn from_digest(algorithm: Algorithm, digest: &str) -> Option<Self> {
        Some(Self { algorithm, digest: base32::decode(digest)? })
    }

    #[must_use]
    pub fn algorithm(self) -> Algorithm {
        self.algorithm
    }

    /// The digest in its text encoding, without the algorithm.
    #[must_use]
    pub fn digest(self) -> String {
        base32::encode(&self.digest)
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.algorithm.name(), Self::SEPARATOR, self.digest())
    }
}

#[cfg(test)]
mod tests;
