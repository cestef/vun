//! Why a memo is computed, as logs name it.

use crate::db::Db;
use crate::memo::Dep;
use crate::tally::Event;

/// Why a memo is computed.
#[derive(Clone, Copy, Debug)]
pub(super) enum Why {
    /// There is none.
    Absent,
    /// This read has changed since.
    Stale(Dep),
    /// It holds, but was loaded without its value.
    Unloaded,
    /// It was loaded, reading a memo no query of this process holds.
    Unresolved,
    /// Verifying what it read reached it again: its reads were a cycle, which computing it
    /// recovers from as any cycle.
    Cycle,
    /// It holds, and is computed again to check that it does ([`super::Reuse::Check`]).
    Checked,
}

impl Why {
    /// The computation as [`Db::tally`] counts it.
    pub(super) fn event(self) -> Event {
        match self {
            Self::Absent => Event::Absent,
            Self::Stale(_) => Event::Stale,
            Self::Unloaded => Event::Unloaded,
            Self::Unresolved => Event::Unresolved,
            Self::Cycle => Event::Cycle,
            Self::Checked => Event::Checked,
        }
    }

    pub(super) fn show(self, db: &Db) -> String {
        match self {
            Self::Absent => "absent".to_owned(),
            Self::Stale(read) => {
                let table = db.any_table(read.table);
                format!("read {}({}) changed", table.name(), table.show(read.slot))
            }
            Self::Unloaded => "loaded without its value".to_owned(),
            Self::Unresolved => "loaded reading a query not held".to_owned(),
            Self::Cycle => "reached again while verifying it".to_owned(),
            Self::Checked => "checking that reusing it holds".to_owned(),
        }
    }
}
