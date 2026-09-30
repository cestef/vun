use vun_span::{SourceFiles, SourceMap};

use crate::{OriginId, OriginTable};

/// An origin shown as where it is written, `path:line:column`, found through the origins and
/// source files of the process; what logs name a place by.
#[derive(Clone, Copy, Debug)]
pub struct Located(OriginId);

impl OriginId {
    #[must_use]
    pub fn located(self) -> Located {
        Located(self)
    }
}

impl std::fmt::Display for Located {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        SourceFiles::global().locate(OriginTable::global().source_span(self.0)).fmt(f)
    }
}
