//! The running program's file.

use std::path::PathBuf;
use std::sync::LazyLock;

/// The file of the running program: the platform's answer, or, where it gives none (WASI), the
/// path its first argument names, which the host starting it sets.
pub static EXECUTABLE: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| std::env::current_exe().ok().or_else(|| std::env::args_os().next().map(PathBuf::from)));
