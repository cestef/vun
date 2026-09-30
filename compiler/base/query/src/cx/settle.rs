//! Verifying ahead: before the reads of a memo are verified in order, the memos they reach that
//! hold are found over threads and marked current, computing none. Verifying in order computes
//! what changed, in an order that never computes a read keyed by an earlier read's value before
//! that value is known to hold; finding what holds computes nothing, so any order finds the same.
//! Each memo is looked at by one thread: another reaching it meanwhile leaves it to verifying in
//! order. Under [`Reuse::Check`] what it finds is only recorded, and verifying in order checks it.

use vun_par::Parallel;
use vun_trace::Activity;

use super::Cx;
use super::reuse::Reuse;
use crate::db::Db;
use crate::fingerprint::Fingerprint;
use crate::memo::Dep;
use crate::table::{Slot, Table, TableId};
use crate::tally::Event;
use crate::{Query, Revision};

/// Verifying ahead, as timed.
const SETTLE: Activity = Activity::stage("settle memos");

/// Finding the memos that hold in one revision.
pub(crate) struct Settle<'db> {
    db: &'db Db,
    revision: Revision,
}

/// What verifying ahead found of a memo, from the least known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Found {
    /// It does not hold, or cannot be told to without computing.
    Fails,
    /// Another thread is looking at it, or it is reached again through its own reads: verifying
    /// in order tells.
    Unsure,
    Holds,
    /// Being looked at by the thread that recorded it.
    Looking,
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

    /// What `reads` found together, each with the fingerprint it was read with: the least known
    /// of them. Only the outermost reads are spread over threads: those of the memos they reach
    /// are looked at by the thread that reached them, which the others are busy beside, so that
    /// no call waits on threads for work smaller than waking them.
    fn all(&self, reads: &[Dep], path: &mut Path) -> Found {
        let found =
            |dep: &Dep, path: &mut Path| match self.db.any_table(dep.table).settled(self, dep.slot, path) {
                (Found::Holds, fingerprint) if fingerprint != Some(dep.fingerprint) => Found::Fails,
                (found, _) => found,
            };
        if reads.len() < Self::WIDE || !path.is_empty() {
            let mut all = Found::Holds;
            for dep in reads {
                all = all.min(found(dep, path));
                if all == Found::Fails {
                    break;
                }
            }
            return all;
        }
        let each = reads.par_map(|dep| found(dep, &mut path.clone()));
        each.into_iter().min().unwrap_or(Found::Holds)
    }

    /// What verifying ahead finds of the memo at `slot`, with its fingerprint: current, holding
    /// at once, or every read holding; a memo found to hold is marked current. Each memo is
    /// looked at by one thread in a revision, which records what it found.
    pub(crate) fn holds<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        path: &mut Path,
    ) -> (Found, Option<Fingerprint>) {
        let Some(memo) = table.entry(slot).memo.load_full() else { return (Found::Fails, None) };
        if memo.current(self.revision) {
            return (Found::Holds, Some(memo.fingerprint));
        }
        let at = (table.id, slot, self.revision);
        if path.contains(&(table.id, slot)) {
            return (Found::Unsure, None);
        }
        match self.db.settled.pin().try_insert(at, Found::Looking) {
            Ok(_) => {}
            Err(recorded) if *recorded.current == Found::Looking => return (Found::Unsure, None),
            Err(recorded) => return (*recorded.current, Some(memo.fingerprint)),
        }
        let durable = self.db.durable(memo.durability, self.revision);
        let found = if durable {
            Found::Holds
        } else {
            memo.deps.get(self.db).map_or(Found::Fails, |deps| {
                path.push((table.id, slot));
                let found = self.all(deps, path);
                path.pop();
                found
            })
        };
        if found == Found::Holds {
            table.counts.add(if durable { Event::Durable } else { Event::Settled });
            if self.db.reuse == Reuse::Trust {
                memo.verified.set(self.revision);
            }
        }
        // An unsure memo is left to verifying in order, as are those reaching it.
        if found == Found::Unsure {
            self.db.settled.pin().remove(&at);
        } else {
            self.db.settled.pin().insert(at, found);
        }
        (found, Some(memo.fingerprint))
    }
}

impl Db {
    /// Whether verifying ahead found the memo at `at` to hold in `revision`.
    pub(crate) fn settled_in(&self, (table, slot): (TableId, Slot), revision: Revision) -> bool {
        self.settled.pin().get(&(table, slot, revision)) == Some(&Found::Holds)
    }
}
