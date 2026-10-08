//! Durability: a memo holds as long as the inputs it reads, through all it read, do. Inputs are
//! grouped ([`Query::group`]) and shared as far as [`Sharing`] says; a memo that reads shared
//! inputs alone is saved with the groups it read through and the fingerprint of their inputs
//! when it held, and holds at once, its reads never verified, while that fingerprint is the
//! same, as Salsa's durable memos hold while no input of their durability changed. A library's
//! memos, which every project reads, hold whatever the project, and whatever other libraries
//! or options are set; those reading the target too, while it is the one chosen. An input not
//! set stays out of its group's fingerprint: setting it changes the group, so a memo that read
//! it unset never matches one that read it set.

use vun_trace::tracing;

use super::cx::Cx;
use super::db::Db;
use super::groups::Groups;
use super::memo::{Dep, Durability};

impl Db {
    /// Whether a memo as durable as `durability` holds at once: its inputs shared, and the
    /// fingerprint of their groups the one it held under.
    pub(crate) fn durable(&self, durability: Durability) -> bool {
        match durability {
            Durability { groups: Some(groups), under: Some(under) } => self.inputs_of(groups) == under,
            Durability { .. } => false,
        }
    }
}

impl Cx<'_> {
    /// How durable a memo computed from `reads` is: it reads through the groups of all their
    /// inputs, and is its own when one is. The read that makes it its own is logged at `trace`.
    pub(super) fn durability(&self, reads: &[Dep]) -> Durability {
        let db = self.db();
        let mut reaches = Vec::new();
        for dep in reads {
            let Some(groups) = db.any_table(dep.table).durability(dep.slot).groups else {
                tracing::trace!(read = db.any_table(dep.table).name(), "its own: reads what is not shared");
                return Durability::OWN;
            };
            if !reaches.contains(&groups) {
                reaches.push(groups);
            }
        }
        Durability::reading(Groups::all(&reaches))
    }
}
