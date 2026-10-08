//! Durability: a memo holds as long as the inputs it reads, through all it read, do. Inputs are
//! shared as far as [`Sharing`] says, and a memo is as durable as the least shared of its inputs.
//! One fingerprint of every input shared as far as a level tells whether they changed: a memo
//! saved with the fingerprint of its level it held under holds at once, its reads never verified,
//! while that fingerprint is the same, as Salsa's durable memos hold while no input of their
//! durability changed. A library's memos, which every project reads, hold whatever the target;
//! those reading the target too, while it is the one chosen. An input not set stays out of every
//! fingerprint: setting it adds it, so a memo that read it unset never matches one that read it
//! set.

use std::sync::Arc;

use vun_trace::tracing;

use super::Revision;
use super::cx::Cx;
use super::db::{Db, Sharing};
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Durability};

impl Db {
    /// The fingerprint of every input shared as far as `level` or further in `revision`, found
    /// once per revision and again after an input is set. One found before an input was first set
    /// leaves that input out, which only a memo that never read it can match.
    pub(crate) fn shared_inputs(&self, revision: Revision, level: usize) -> Fingerprint {
        if let Some(known) = self.shared_inputs.load().as_deref()
            && known.0 == revision
        {
            return known.1[level];
        }
        let now = Sharing::LEVELS.map(|sharing| self.shared_inputs_now(sharing));
        self.shared_inputs.store(Some(Arc::new((revision, now))));
        now[level]
    }

    /// The fingerprint of every input shared as far as `level` or further as they are now: what a
    /// memo saved at that level is marked as holding under.
    pub(crate) fn shared_inputs_now(&self, level: Sharing) -> Fingerprint {
        self.tables.iter().fold(Fingerprint::default(), |all, (_, table)| {
            let inputs = table.shared_inputs(level);
            if inputs != Fingerprint::default() {
                tracing::trace!(query = table.name(), ?level, ?inputs, "shared inputs");
            }
            all.with(inputs)
        })
    }

    /// Whether a memo as durable as `durability` holds at once in `revision`: its inputs shared,
    /// and their fingerprint the one it held under.
    pub(crate) fn durable(&self, durability: Durability, revision: Revision) -> bool {
        let Some(level) = durability.level.level() else { return false };
        durability.under == Some(self.shared_inputs(revision, level))
    }
}

impl Cx<'_> {
    /// How durable a memo computed from `reads` is: as shared as the least shared of them. The
    /// read that makes it its own is logged at `trace`.
    pub(super) fn durability(&self, reads: &[Dep]) -> Durability {
        let db = self.db();
        let levels = reads.iter().map(|dep| (dep, db.any_table(dep.table).level(dep.slot)));
        let (least, level) = levels
            .min_by_key(|&(_, level)| level)
            .map_or((None, Sharing::Shared), |(dep, level)| (Some(dep), level));
        if let (Some(own), Sharing::Own) = (least, level) {
            tracing::trace!(read = db.any_table(own.table).name(), "its own: reads what is not shared");
        }
        Durability::at(level)
    }
}
