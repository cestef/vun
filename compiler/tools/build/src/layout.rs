//! Where namespaces live on disk (spec 01, section 8, and spec 12, section 1): a namespace is the
//! file `<base>.ov` merged with the directory `<base>/`, and a package's root namespace is its
//! name in the package directory.

use std::path::{Path, PathBuf};

use overt_fs::Tree;
use overt_span::EXTENSION;

/// The file holding a package's manifest (spec 12, section 2).
pub const MANIFEST: &str = overt_span::source_file!("build");

/// Separates the members of a namespace path: `std.prelude`.
pub const SEPARATOR: &str = overt_grammar::PATH_SEPARATOR;

/// A namespace on disk, by the path its file and directory share.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    base: PathBuf,
}

impl Location {
    /// The root namespace of the package `name` whose manifest is in `dir`.
    #[must_use]
    pub fn root(dir: &Path, name: &str) -> Self {
        Self { base: dir.join(name) }
    }

    /// The namespace whose source file is `file`.
    #[must_use]
    pub fn of(file: &Path) -> Self {
        Self { base: file.with_extension("") }
    }

    #[must_use]
    pub fn member(&self, name: &str) -> Self {
        Self { base: self.base.join(name) }
    }

    /// Its source file, if it exists.
    #[must_use]
    pub fn file(&self) -> Option<PathBuf> {
        Some(self.base.with_extension(EXTENSION)).filter(|f| f.is_file())
    }

    /// Its directory, whether or not it exists.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.base
    }

    #[must_use]
    pub fn exists(&self) -> bool {
        self.file().is_some() || self.base.is_dir()
    }

    /// The sources and directories inside, by name without extension, sorted; a file and a
    /// directory of the same name are one member.
    #[must_use]
    pub fn children(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .base
            .entries()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_dir() || p.extension().is_some_and(|e| e == EXTENSION))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.dedup();
        names
    }

    /// The namespace path of the source `file` of the package in `dir`: in `lib/lang`,
    /// `lib/lang/lang/errors.ov` is `lang.errors`.
    #[must_use]
    pub fn path_of(dir: &Path, file: &Path) -> Option<Vec<String>> {
        let relative = file.strip_prefix(dir).ok()?.with_extension("");
        relative.iter().map(|s| s.to_str().map(str::to_owned)).collect()
    }
}
