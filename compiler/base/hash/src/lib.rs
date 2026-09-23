//! Content hashes, of bytes and of file trees: what identifies a package (spec 12, section 3) and
//! keys every cache.

use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Names the algorithm in the text form, so that another can follow.
    pub const PREFIX: &str = "sha256-";

    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The hash of the files under `dir` named by `include` (files or directories), each with
    /// its path relative to `dir`, in path order. Entries that do not exist are left out.
    #[must_use]
    pub fn tree(dir: &Path, include: &[String]) -> Self {
        let mut files = Vec::new();
        for entry in include {
            Self::files(&dir.join(entry), &mut files);
        }
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
        Self(hasher.finalize().into())
    }

    /// The files at or below `path`.
    fn files(path: &Path, out: &mut Vec<PathBuf>) {
        match std::fs::read_dir(path) {
            Ok(entries) => entries.filter_map(Result::ok).for_each(|e| Self::files(&e.path(), out)),
            Err(_) if path.is_file() => out.push(path.to_path_buf()),
            Err(_) => {}
        }
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::PREFIX)?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}
