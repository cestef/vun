//! Computing queries: the stack of queries being computed on one thread, what each has read, and
//! cycles.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use super::claim::Claim;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Memo, Verified};
use super::table::{Slot, Table, TableId};
use super::{Query, Revision};

/// A query being computed, and each memo it read.
struct Frame {
    table: TableId,
    slot: Slot,
    reads: Vec<Dep>,
}

/// Queries that depend on themselves: each by name and key, from the one reached again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub path: Vec<(&'static str, String)>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("query cycle: ")?;
        for (i, (name, key)) in self.path.iter().enumerate() {
            if i > 0 {
                f.write_str(" then ")?;
            }
            write!(f, "{name}({key})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Cycle {}

/// The queries being computed on one thread, and what each has read so far.
pub struct Cx<'db> {
    db: &'db Db,
    stack: RefCell<Vec<Frame>>,
}

impl<'db> Cx<'db> {
    #[must_use]
    pub fn new(db: &'db Db) -> Self {
        Self { db, stack: RefCell::default() }
    }

    #[must_use]
    pub fn db(&self) -> &'db Db {
        self.db
    }

    /// The value of `Q` at `key`, recorded as read by the query being computed.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        let table = self.db.table::<Q>();
        let slot = table.slot(key);
        let (value, fingerprint) = self.fetch(table, slot);
        if let Some(top) = self.stack.borrow_mut().last_mut() {
            top.reads.push(Dep { table: table.id, slot, fingerprint });
        }
        value
    }

    /// The current value at `slot` and its fingerprint: the memo when it is current or what it
    /// read still holds, otherwise computed again, by this thread or, when another is computing
    /// it already, by that one.
    pub(crate) fn fetch<Q: Query>(&self, table: &Table<Q>, slot: Slot) -> (Q::Value, Fingerprint) {
        let entry = table.entry(slot);
        loop {
            let revision = self.db.revision();
            if let Some(memo) = &*entry.memo.load()
                && memo.current(revision)
            {
                return (memo.value.clone(), memo.fingerprint);
            }
            // Verifying reads other memos, which may take long: the memo is held on its own.
            if let Some(memo) = entry.memo.load_full()
                && memo.deps.iter().all(|&dep| self.unchanged(dep))
            {
                memo.verified.set(revision);
                return (memo.value.clone(), memo.fingerprint);
            }
            if let Some(found) = self.computed(table, slot, revision) {
                return found;
            }
        }
    }

    fn unchanged(&self, dep: Dep) -> bool {
        self.db.any_table(dep.table).unchanged(self, dep.slot, dep.fingerprint)
    }

    /// The value at `slot` computed on this thread, or the value of the cycle it closes; `None`
    /// when another thread computed it meanwhile.
    fn computed<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
    ) -> Option<(Q::Value, Fingerprint)> {
        let entry = table.entry(slot);
        let reached = self.stack.borrow().iter().position(|f| f.table == table.id && f.slot == slot);
        if let Some(from) = reached {
            let path = self.stack.borrow()[from..]
                .iter()
                .map(|f| {
                    let frame = self.db.any_table(f.table);
                    (frame.name(), frame.show(f.slot))
                })
                .chain([(Q::NAME, format!("{:?}", entry.key))])
                .collect();
            let value = Q::cycle(self, &entry.key, &Cycle { path });
            let fingerprint = Q::interface(&value);
            return Some((value, fingerprint));
        }
        let owned = match entry.claims.claim(&self.db.waits) {
            Claim::Waited => return None,
            Claim::Owner(owned) => Some(owned),
            Claim::Alone => None,
        };
        self.stack.borrow_mut().push(Frame { table: table.id, slot, reads: Vec::new() });
        let value = Q::compute(self, &entry.key);
        let reads = self.stack.borrow_mut().pop().expect("the frame pushed above").reads;
        let fingerprint = Q::interface(&value);
        let memo = Memo::computed(value.clone(), fingerprint, Self::once(reads), Verified::at(revision));
        entry.memo.store(Some(Arc::new(memo)));
        drop(owned);
        Some((value, fingerprint))
    }

    /// `reads` with each memo once, in the order it was first read: verifying follows it, since a
    /// later read may be of a key an earlier one's value named.
    fn once(mut reads: Vec<Dep>) -> Box<[Dep]> {
        let mut seen = HashSet::with_capacity_and_hasher(reads.len(), foldhash::fast::RandomState::default());
        reads.retain(|dep| seen.insert((dep.table, dep.slot)));
        reads.into_boxed_slice()
    }
}
