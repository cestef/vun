//! clang's resource directory, which holds the headers the compiler provides itself
//! (`stddef.h`, `stdint.h`): the `clang` executable finds it next to itself, a loaded libclang
//! does not, so it is found next to the library, where LLVM installs both.

use std::path::{Path, PathBuf};

/// The resource directory of the libclang at `library`: `<prefix>/lib/clang/<major>` for a
/// library in `<prefix>/lib` or `<prefix>/lib64`, preferring the library's own major version.
pub(crate) fn find(library: &Path) -> Option<PathBuf> {
    let library = std::fs::canonicalize(library).ok()?;
    let dir = library.parent()?;
    let prefix = dir.parent()?;
    let major = major(library.file_name()?.to_str()?);
    let mut found: Vec<(Option<u32>, PathBuf)> =
        [dir.join("clang"), prefix.join("lib/clang"), prefix.join("lib64/clang")]
            .iter()
            .filter_map(|parent| std::fs::read_dir(parent).ok())
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let version = path.file_name()?.to_str()?.parse().ok();
                path.join("include/stddef.h").is_file().then_some((version, path))
            })
            .collect();
    // The library's own version first, then the newest.
    found.sort_by_key(|(version, _)| (*version == major && major.is_some(), *version));
    found.pop().map(|(_, path)| path)
}

/// The major version in a library's file name: `libclang.so.21.1.8`, `libclang-18.so.1`.
pub(crate) fn major(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("libclang")?;
    let digits = rest.trim_start_matches(['-', '.', 's', 'o']);
    digits.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}
