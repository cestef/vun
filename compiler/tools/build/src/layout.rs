//! Where namespaces live on disk (spec 01, section 8, and spec 12, section 1): a namespace is the
//! file `<base>.ov` merged with the directory `<base>/`, and a package's root namespace is its
//! name in the package directory.

use std::path::{Path, PathBuf};

use overt_grammar::TokenKind;
use overt_span::EXTENSION;

/// The file holding a package's manifest (spec 12, section 2).
pub const MANIFEST: &str = "build.ov";

/// Separates the members of a namespace path: `std.prelude`.
pub const SEPARATOR: &str = TokenKind::Dot.text().unwrap();

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

    #[must_use]
    pub fn member(&self, name: &str) -> Self {
        Self { base: self.base.join(name) }
    }

    /// Its source file, if it exists.
    #[must_use]
    pub fn file(&self) -> Option<PathBuf> {
        Some(self.base.with_extension(EXTENSION)).filter(|f| f.is_file())
    }

    #[must_use]
    pub fn exists(&self) -> bool {
        self.file().is_some() || self.base.is_dir()
    }

    /// The sources and directories inside, by name without extension, sorted; a file and a
    /// directory of the same name are one member.
    #[must_use]
    pub fn children(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.base) else { return Vec::new() };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir() || p.extension().is_some_and(|e| e == EXTENSION))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
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
