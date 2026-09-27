//! Where the session shows positions: in the text of each file it reads now. An origin a query
//! saved by an earlier run recorded in an earlier text of a file is found in the current one
//! (`vun_origin::Resolve`).

use vun_hir::db::FileText;
use vun_origin::{OriginTable, Resolve};
use vun_span::FileId;

use crate::session::Session;

impl Resolve for Session {
    fn origins(&self) -> &OriginTable {
        self.origins
    }

    fn current(&self, file: FileId) -> FileId {
        let key = self.sources.key(file);
        let text = self.db.get::<FileText>(key);
        text.and_then(|text| self.sources.read(key, &text)).unwrap_or(file)
    }
}
