//! Where namespaces live on disk (spec 01, section 8, and spec 12, section 1): a namespace is the
//! file `<base>.vun` merged with the directory `<base>/`, and a package's root namespace is its
//! name in the package directory.

use std::path::{Path, PathBuf};

use vun_fs::Tree;
use vun_span::EXTENSION;

/// The file holding a package's manifest (spec 12, section 2).
pub const MANIFEST: &str = vun_span::source_file!("build");

/// Separates the members of a namespace path: `std.prelude`.
pub const SEPARATOR: &str = vun_grammar::PATH_SEPARATOR;

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

    /// The namespace at the binding `path` of the package in `dir`, whose first name is the
    /// package's: in `lib/lang`, `lang.errors` is `lib/lang/lang/errors`. The inverse of
    /// [`Self::path_of`].
    #[must_use]
    pub fn at<S: AsRef<str>>(dir: &Path, path: &[S]) -> Self {
        Self { base: path.iter().fold(dir.to_path_buf(), |base, name| base.join(name.as_ref())) }
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

    /// The file its path names with `extension` in place of the source's, whether or not it
    /// exists: `lang.errors.int.does_not_fit` with `md` is `lang/errors/int/does_not_fit.md`.
    #[must_use]
    pub fn with_extension(&self, extension: &str) -> PathBuf {
        self.base.with_extension(extension)
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
    /// `lib/lang/lang/errors.vun` is `lang.errors`.
    #[must_use]
    pub fn path_of(dir: &Path, file: &Path) -> Option<Vec<String>> {
        let relative = file.strip_prefix(dir).ok()?.with_extension("");
        relative.iter().map(|s| s.to_str().map(str::to_owned)).collect()
    }
}
