//! What the driver sets once it has loaded a package graph: each package's description and the
//! text of its files. Everything else is computed from these by queries.

use std::path::Path;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::DiagCtxt;
use vun_intern::Symbol;
use vun_query::{Cx, Query};
use vun_span::Span;

pub use vun_span::{FileRef, PackageId};

/// A package as the driver loaded it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PackageData {
    /// `None` for files outside any package, which have no root namespace.
    pub name: Option<Symbol>,
    /// Its directory as shown to users; the paths of its files are relative to it.
    pub dir: String,
    pub deps: Vec<DepData>,
    /// The namespaces whose public names its files see, by path.
    pub prelude: Vec<Vec<Symbol>>,
    /// Where the manifest declares the prelude.
    pub prelude_span: Option<Span>,
    /// Its files, relative to `dir`, sorted.
    pub files: Vec<String>,
}

/// A dependency: the name it is bound to, and its package.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DepData {
    pub name: Symbol,
    pub package: Result<PackageId, Unloaded>,
}

/// Why a dependency's package is not there.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unloaded {
    /// Not on disk, or not matching its hash: these reports, returned where it is needed, so that
    /// a dependency nothing uses is never reported.
    Absent(DiagCtxt),
    /// Already reported while the graph was loaded, such as a version conflict.
    Reported,
}

impl PackageData {
    /// `path`, one of its files, as shown to users.
    #[must_use]
    pub fn display(&self, path: &str) -> String {
        Path::new(&self.dir).join(path).display().to_string()
    }
}

/// Input: a package's description; an unset one is empty.
pub struct Package;

impl Query for Package {
    const NAME: &'static str = "package";
    const INPUT: bool = true;
    type Key = PackageId;
    type Value = Arc<PackageData>;

    fn compute(_: &Cx<'_>, _: &PackageId) -> Arc<PackageData> {
        Arc::default()
    }
}

/// Input: a file's text; `None` for a file the driver did not read.
pub struct FileText;

impl Query for FileText {
    const NAME: &'static str = "file_text";
    const INPUT: bool = true;
    type Key = FileRef;
    type Value = Option<Arc<str>>;

    fn compute(_: &Cx<'_>, _: &FileRef) -> Option<Arc<str>> {
        None
    }
}
