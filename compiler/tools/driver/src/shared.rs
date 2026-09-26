//! Sessions that share their queries, so that what one computes the others reuse: one database per
//! package, since a package compiles for one target and a database holds one (`TargetOf`). Files
//! outside any package share one database too.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_build::layout::MANIFEST;
use vun_par::Map;
use vun_query::Db;

use crate::Session;

/// The databases of the packages sessions were made for, by the directory holding the manifest.
#[derive(Default)]
pub struct Shared(Map<Option<PathBuf>, Arc<Db>>);

impl Shared {
    /// A session for the file at `path`, over the queries of the other files of its package.
    pub fn session(&self, path: &Path) -> Session {
        let package = path.ancestors().skip(1).find(|dir| dir.join(MANIFEST).is_file());
        Session::sharing(self.0.pin().get_or_insert_with(package.map(Path::to_path_buf), Arc::default))
    }
}
