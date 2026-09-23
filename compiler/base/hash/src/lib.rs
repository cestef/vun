//! Content hashes, of bytes and of file trees: what identifies a package (spec 12, section 3) and
//! keys every cache.

use std::fmt;
use std::path::{Path, PathBuf};

use overt_fs::Tree;
use sha2::{Digest, Sha256};

/// The hash functions a [`Hash`] can come from; the text form names it, so that another can
/// follow without ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Algorithm {
    Sha256,
}

impl Algorithm {
    pub const ALL: &[Self] = &[Self::Sha256];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

/// A digest and the algorithm that produced it; written `sha256:<hex digest>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash {
    algorithm: Algorithm,
    digest: [u8; 32],
}

impl Hash {
    /// Separates the algorithm from the digest in the text form.
    pub const SEPARATOR: char = ':';

    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self { algorithm: Algorithm::Sha256, digest: Sha256::digest(bytes).into() }
    }

    /// Reads the text form; `None` for an unknown algorithm or a malformed digest.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (name, hex) = text.split_once(Self::SEPARATOR)?;
        let &algorithm = Algorithm::ALL.iter().find(|a| a.name() == name)?;
        let mut digest = [0; 32];
        if hex.len() != digest.len() * 2 || !hex.is_ascii() {
            return None;
        }
        for (byte, pair) in digest.iter_mut().zip(hex.as_bytes().chunks(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        Some(Self { algorithm, digest })
    }

    #[must_use]
    pub fn algorithm(self) -> Algorithm {
        self.algorithm
    }

    /// The hash of the files under `dir` named by `include` (files or directories), each with
    /// its path relative to `dir`, in path order. Entries that do not exist are left out.
    #[must_use]
    pub fn tree(dir: &Path, include: &[String]) -> Self {
        let mut files: Vec<PathBuf> =
            include.iter().flat_map(|entry| dir.join(entry).files().unwrap_or_default()).collect();
        files.sort();
        files.dedup();
        let mut hasher = Sha256::new();
        for file in &files {
            let Ok(bytes) = std::fs::read(file) else { continue };
            let name = file.strip_prefix(dir).unwrap_or(file).to_string_lossy();
            // Lengths first, so that no two trees feed the same bytes.
            for part in [name.as_bytes(), &bytes] {
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part);
            }
        }
        Self { algorithm: Algorithm::Sha256, digest: hasher.finalize().into() }
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.algorithm.name(), Self::SEPARATOR)?;
        self.digest.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

#[cfg(test)]
mod tests;
