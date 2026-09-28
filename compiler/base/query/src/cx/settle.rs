//! Verifying ahead: before the reads of a memo are verified in order, the memos they reach that
//! hold are found over threads and marked current, computing none. Verifying in order computes
//! what changed, in an order that never computes a read keyed by an earlier read's value before
//! that value is known to hold; finding what holds computes nothing, so any order finds the same.
//! Under [`Reuse::Check`] what it finds is only recorded, and verifying in order checks it.

use vun_par::Parallel;
use vun_trace::Activity;

use super::Cx;
use super::reuse::Reuse;
use crate::db::Db;
use crate::fingerprint::Fingerprint;
use crate::memo::Dep;
use crate::table::{Slot, Table, TableId};
use crate::{Query, Revision};

/// Verifying ahead, as timed.
const SETTLE: Activity = Activity::stage("settle memos");

/// Finding the memos that hold in one revision.
pub(crate) struct Settle<'db> {
    db: &'db Db,
    revision: Revision,
}

/// The memos being looked at, outermost first: one reached again is reached through its own reads.
type Path = Vec<(TableId, Slot)>;

impl Cx<'_> {
    /// Marks current, over threads, each memo `reads` reach whose reads all hold, when the memo
    /// reading them is the outermost one verified here and reads many.
    pub(super) fn settle(&self, reads: &[Dep], revision: Revision) {
        if reads.len() < Settle::WIDE || !self.verifying.borrow().is_empty() {
            return;
        }
        let _timer = SETTLE.start();
        Settle { db: self.db, revision }.all(reads, &mut Path::new());
    }
}

impl Settle<'_> {
    /// How many reads a memo has from which they are looked at over threads.
    const WIDE: usize = 16;

    /// Whether every one of `reads` holds with the fingerprint it was read with.
    fn all(&self, reads: &[Dep], path: &mut Path) -> bool {
        let holds = |dep: &Dep, path: &mut Path| {
            self.db.any_table(dep.table).settled(self, dep.slot, path) == Some(dep.fingerprint)
        };
        if reads.len() < Self::WIDE {
            return reads.iter().all(|dep| holds(dep, path));
        }
        reads.par_map(|dep| holds(dep, &mut path.clone())).into_iter().all(|held| held)
    }

    /// The fingerprint of the memo at `slot` if it holds, marked current then; `None` when there
    /// is none, when a read of it changed or cannot be told to hold without computing, and when
    /// it is reached again through its own reads. Each memo is looked at once in a revision.
    pub(crate) fn holds<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        path: &mut Path,
    ) -> Option<Fingerprint> {
        let memo = table.entry(slot).memo.load_full()?;
        if memo.current(self.revision) {
            return Some(memo.fingerprint);
        }
        let at = (table.id, slot);
        if let Some(&(revision, holds)) = self.db.settled.pin().get(&at)
            && revision == self.revision
        {
            return holds.then_some(memo.fingerprint);
        }
        if path.contains(&at) {
            return None;
        }
        let holds = memo.deps.get(self.db).is_some_and(|deps| {
            path.push(at);
            let holds = self.all(deps, path);
            path.pop();
            holds
        });
        self.db.settled.pin().insert(at, (self.revision, holds));
        if holds && self.db.reuse == Reuse::Trust {
            memo.verified.set(self.revision);
        }
        holds.then_some(memo.fingerprint)
    }
}

impl Db {
    /// Whether verifying ahead found the memo at `at` to hold in `revision`.
    pub(crate) fn settled_in(&self, at: (TableId, Slot), revision: Revision) -> bool {
        self.settled.pin().get(&at) == Some(&(revision, true))
    }
}
