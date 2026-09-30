//! Coverage (spec 11, section 6): the programs `vun fuzz` builds, and those `--cover` asks for,
//! mark each block they run in `lang.cover.map`. The query input `CoverageOf`, which what it
//! changes depends on, as the checks are.

use vun_command::{Args, Param};
use vun_query::Sharing;
use vun_sema::cover::{Coverage, CoverageOf};

use crate::Session;

pub const COVER: Param = Param::flag("cover")
    .about("mark each block the program runs in `lang.cover.map`, as `vun fuzz` builds it");

/// Applies `--cover` to the session.
pub fn configure(session: &mut Session, args: &Args) {
    if args.flag(&COVER) {
        session.cover(Coverage::Blocks);
    }
}

impl Session {
    /// Builds the session's programs recording `coverage` of what they run.
    pub fn cover(&mut self, coverage: Coverage) {
        self.db.set_as::<CoverageOf>(&(), coverage, Sharing::Chosen);
    }

    /// What the session's programs record of what they run.
    #[must_use]
    pub fn covered(&self) -> Coverage {
        self.db.get::<CoverageOf>(&())
    }
}
