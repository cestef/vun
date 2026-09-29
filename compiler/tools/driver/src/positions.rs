//! Where the session shows positions: in the text of each file it reads now. An origin a query
//! saved by an earlier run recorded in an earlier text of a file is found in the current one
//! (`vun_origin::Resolve`).

use vun_hir::db::FileText;
use vun_origin::{OriginTable, Resolve};
use vun_query::Db;
use vun_span::{FileId, SourceFiles};

use crate::session::Session;

/// What reading the session's queries and placing their origins needs, shared between threads.
#[derive(Clone, Copy)]
pub struct Reader<'s> {
    pub db: &'s Db,
    sources: &'static SourceFiles,
    origins: &'static OriginTable,
}

impl Session {
    #[must_use]
    pub fn reader(&self) -> Reader<'_> {
        Reader { db: &self.db, sources: self.sources, origins: self.origins }
    }
}

impl Resolve for Reader<'_> {
    fn origins(&self) -> &OriginTable {
        self.origins
    }

    fn current(&self, file: FileId) -> FileId {
        let key = self.sources.key(file);
        let text = self.db.get::<FileText>(key);
        text.and_then(|text| self.sources.read(key, &text)).unwrap_or(file)
    }
}

impl Resolve for Session {
    fn origins(&self) -> &OriginTable {
        self.origins
    }

    fn current(&self, file: FileId) -> FileId {
        self.reader().current(file)
    }
}
