//! Where the packages a package depends on come from.

use std::path::{Path, PathBuf};

use vun_build::lock::Replaced;
use vun_build::manifest::{DepKind, Identity, Source, Version};
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_span::Span;
use vun_uri::Uri;

/// An identity replaced everywhere in the graph, the dependency loaded in its place, and how the
/// lockfile records it.
#[derive(Clone)]
pub struct Replace {
    pub identity: Identity,
    pub dep: Dep,
    pub recorded: Replaced,
}

/// A dependency: the name it is bound to, where it comes from, and where it is declared.
#[derive(Clone)]
pub struct Dep {
    pub name: Symbol,
    pub source: DepSource,
    pub kind: DepKind,
    /// `None` for a default dependency.
    pub span: Option<Span>,
}

#[derive(Clone)]
pub enum DepSource {
    /// A directory on disk.
    Dir(PathBuf),
    /// A package in the store, by its tree hash, with its declared version and the mirrors that
    /// serve it.
    Stored { hash: Hash, version: Version, urls: Vec<Uri> },
}

impl DepSource {
    /// Where a package declared in the manifest in `dir` comes from, `library` being the
    /// toolchain's.
    pub(super) fn of(library: &Path, dir: &Path, source: &Source) -> Self {
        match source {
            Source::Path(path) => Self::Dir(dir.join(path)),
            Source::Library(path) => Self::Dir(library.join(path)),
            Source::Archive { hash, version, urls } => {
                Self::Stored { hash: *hash, version: *version, urls: urls.clone() }
            }
        }
    }
}
