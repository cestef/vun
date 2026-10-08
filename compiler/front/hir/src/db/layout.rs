//! Where namespaces are in a package (spec 01, section 8), read from the list of its files: the
//! namespace `a.b` of the package `p` is the file `p/a/b.vun` merged with the directory `p/a/b/`.

use vun_codec::{Decode, Encode};
use vun_hash::Snapshot;
use vun_intern::Symbol;
use vun_span::EXTENSION;

use super::input::{PackageData, PackageId};

/// A package's root namespace, or one below it by member names.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct Namespace {
    pub package: PackageId,
    pub path: Vec<Symbol>,
}

const SEPARATOR: char = Snapshot::PATH_SEPARATOR;

impl Namespace {
    /// Whether this namespace is `outer` or nested in it.
    #[must_use]
    pub fn within(&self, outer: &Self) -> bool {
        self.package == outer.package && self.path.starts_with(&outer.path)
    }
}

impl PackageData {
    /// The path shared by the file and the directory of the namespace at `path`: `p/a/b`. `None`
    /// outside any package.
    fn base(&self, path: &[Symbol]) -> Option<String> {
        let names = std::iter::once(self.name?).chain(path.iter().copied());
        Some(names.map(Symbol::as_str).collect::<Vec<_>>().join(&SEPARATOR.to_string()))
    }

    /// The directory of the namespace at `path`, below the package's: `p/a/b`.
    #[must_use]
    pub fn directory(&self, path: &[Symbol]) -> Option<String> {
        self.base(path)
    }

    /// The source file of the namespace at `path`, if it has one.
    #[must_use]
    pub fn file_of(&self, path: &[Symbol]) -> Option<&str> {
        let file = format!("{}.{EXTENSION}", self.base(path)?);
        self.files.iter().find(|f| **f == file).map(String::as_str)
    }

    /// The members the directory of the namespace at `path` holds: its files without their
    /// extension and its subdirectories, sorted, each once.
    #[must_use]
    pub fn children(&self, path: &[Symbol]) -> Vec<Symbol> {
        let Some(base) = self.base(path) else { return Vec::new() };
        let prefix = format!("{base}{SEPARATOR}");
        let below = self.files.iter().filter_map(|f| f.strip_prefix(&prefix));
        let names = below.map(|rest| {
            let first = rest.split(SEPARATOR).next().unwrap_or(rest);
            first.strip_suffix(&format!(".{EXTENSION}")).unwrap_or(first)
        });
        let mut names: Vec<Symbol> = names.map(Symbol::intern).collect();
        names.sort();
        names.dedup();
        names
    }

    /// Whether a file or a directory holds the namespace at `path`.
    #[must_use]
    pub fn holds(&self, path: &[Symbol]) -> bool {
        self.file_of(path).is_some() || !self.children(path).is_empty()
    }

    /// The namespace whose source file is `file`, if it is one of the package's namespaces.
    #[must_use]
    pub fn namespace_of(&self, file: &str) -> Option<Vec<Symbol>> {
        let base = file.strip_suffix(&format!(".{EXTENSION}"))?;
        let mut names = base.split(SEPARATOR);
        (names.next()? == self.name?.as_str()).then(|| names.map(Symbol::intern).collect())
    }
}
