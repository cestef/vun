//! `cargo xtask lines`: no source file is monolithic. A file past [`MAX_LINES`] is split into a
//! directory module by concern (`CLAUDE.md`, code rules). Generated files are held to it too:
//! generators emit one module per declaration or group.

use std::path::{Path, PathBuf};
use vun_fs::Tree;

use vun_span::EXTENSION;

pub(crate) const MAX_LINES: u32 = 300;

/// Directories checked, relative to the repository root, with the extension of their sources.
const CHECKED: &[(&str, &str)] =
    &[("compiler", "rs"), ("fuzz/fuzz_targets", "rs"), ("lib", EXTENSION), ("corpus", EXTENSION)];

pub(crate) struct Oversized {
    pub(crate) path: PathBuf,
    pub(crate) lines: u32,
}

impl Oversized {
    /// Every checked file under `root` longer than [`MAX_LINES`], in path order.
    ///
    /// # Errors
    /// Returns an error if a checked directory or file cannot be read.
    pub(crate) fn all(root: &Path) -> std::io::Result<Vec<Self>> {
        let mut found = Vec::new();
        for &(dir, ext) in CHECKED {
            for path in root.join(dir).files_with(ext)? {
                let count = std::fs::read_to_string(&path)?.lines().count();
                let lines = u32::try_from(count).unwrap_or(u32::MAX);
                if lines > MAX_LINES {
                    found.push(Self { path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(), lines });
                }
            }
        }
        Ok(found)
    }
}
