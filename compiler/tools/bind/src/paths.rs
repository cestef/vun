//! Where declarations are written, compared as the files are: a header included as
//! `../dep/dep.h` is in `dep/`, not in the directory of the header that includes it.

use std::path::{Path, PathBuf};

/// `path` as the file system has it, symbolic links and `..` resolved; as written, made
/// absolute, when it does not exist.
pub(crate) fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}
