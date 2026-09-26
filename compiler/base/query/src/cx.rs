//! Computing queries: the stack of queries being computed on one thread, what each has read, and
//! cycles.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_trace::{Activity, Timer, tracing};

use super::claim::Claim;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Memo};
use super::table::{Slot, Table, TableId};
use super::{Query, Revision};

/// A query being computed, and each memo it read.
struct Frame {
    table: TableId,
    slot: Slot,
    reads: Vec<Dep>,
}

/// Why a memo is computed.
#[derive(Clone, Copy, Debug)]
enum Why {
    /// There is none.
    Absent,
    /// This read has changed since.
    Stale(Dep),
    /// It holds, but was loaded without its value.
    Unloaded,
    /// It was loaded, reading a memo no query of this process holds.
    Unresolved,
}

impl Why {
    fn show(self, db: &Db) -> String {
        match self {
            Self::Absent => "absent".to_owned(),
            Self::Stale(read) => {
                let table = db.any_table(read.table);
                format!("read {}({}) changed", table.name(), table.show(read.slot))
            }
            Self::Unloaded => "loaded without its value".to_owned(),
            Self::Unresolved => "loaded reading a query not held".to_owned(),
        }
    }
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
        self.record(Dep { table: table.id, slot, fingerprint });
        value
    }

    /// Records `read` as read by the query being computed.
    fn record(&self, read: Dep) {
        if let Some(top) = self.stack.borrow_mut().last_mut() {
            top.reads.push(read);
        }
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
                && let Some(value) = memo.value(self.db)
            {
                return (value.clone(), memo.fingerprint);
            }
            let why = match self.verified(table, slot, revision) {
                Ok(memo) => match memo.value(self.db) {
                    Some(value) => return (value.clone(), memo.fingerprint),
                    None => Why::Unloaded,
                },
                Err(why) => why,
            };
            if let Some(found) = self.computed(table, slot, revision, why) {
                return found;
            }
        }
    }

    /// The current fingerprint at `slot`: a memo whose reads still hold stands for its value,
    /// which is not computed when it was not loaded.
    pub(crate) fn fingerprint<Q: Query>(&self, table: &Table<Q>, slot: Slot) -> Fingerprint {
        loop {
            let revision = self.db.revision();
            let why = match self.verified(table, slot, revision) {
                Ok(memo) => return memo.fingerprint,
                Err(why) => why,
            };
            if let Some((_, fingerprint)) = self.computed(table, slot, revision, why) {
                return fingerprint;
            }
        }
    }

    /// The memo at `slot` if it holds in `revision`: current, or everything it read unchanged.
    /// Verifying reads other memos, which may take long: the memo is held on its own meanwhile.
    fn verified<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
    ) -> Result<Arc<Memo<Q>>, Why> {
        let memo = table.entry(slot).memo.load_full().ok_or(Why::Absent)?;
        if memo.current(revision) {
            return Ok(memo);
        }
        let deps = memo.deps.get(self.db).ok_or(Why::Unresolved)?;
        if let Some(read) = self.stale(deps) {
            return Err(Why::Stale(read));
        }
        memo.verified.set(revision);
        Ok(memo)
    }

    /// The first of `reads` that changed, each brought up to date in turn.
    #[inline(never)]
    fn stale(&self, reads: &[Dep]) -> Option<Dep> {
        reads
            .iter()
            .copied()
            .find(|dep| !self.db.any_table(dep.table).unchanged(self, dep.slot, dep.fingerprint))
    }

    /// The value at `slot` computed on this thread, or the value of the cycle it closes; `None`
    /// when another thread computed it meanwhile.
    fn computed<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
        why: Why,
    ) -> Option<(Q::Value, Fingerprint)> {
        let entry = table.entry(slot);
        if let Some(cycle) = self.cycle(table.id, slot) {
            let value = Q::cycle(self, &entry.key, &cycle);
            let fingerprint = Q::interface(&value);
            return Some((value, fingerprint));
        }
        let owned = match entry.claims.claim(&self.db.waits) {
            Claim::Waited => return None,
            Claim::Owner(owned) => Some(owned),
            Claim::Alone => None,
        };
        let timer = self.enter(table.id, slot, why);
        let value = Q::compute(self, &entry.key);
        drop(timer);
        let reads = self.leave();
        let fingerprint = Q::interface(&value);
        let memo = if Q::INPUT {
            // An input not set holds what it computes until it is.
            Memo::input(value.clone(), fingerprint, revision)
        } else {
            let loaded = entry.memo.load().as_ref().map(|m| m.fingerprint);
            self.computing(why, loaded, fingerprint);
            Memo::computed(value.clone(), fingerprint, reads, revision)
        };
        entry.memo.store(Some(Arc::new(memo)));
        drop(owned);
        Some((value, fingerprint))
    }

    /// What the stack reaches again by computing `slot` of `table`, if it computes it already.
    /// What does not depend on the query is kept out of the code made for each one.
    #[inline(never)]
    fn cycle(&self, table: TableId, slot: Slot) -> Option<Cycle> {
        let stack = self.stack.borrow();
        let from = stack.iter().position(|f| f.table == table && f.slot == slot)?;
        let path = stack[from..]
            .iter()
            .map(|f| (f.table, f.slot))
            .chain([(table, slot)])
            .map(|(table, slot)| {
                let table = self.db.any_table(table);
                (table.name(), table.show(slot))
            })
            .collect();
        let cycle = Cycle { path };
        tracing::debug!(%cycle, "cycle recovered");
        Some(cycle)
    }

    /// Starts computing `slot` of `table`, timed until the timer returned is dropped.
    #[inline(never)]
    fn enter(&self, table: TableId, slot: Slot, why: Why) -> Timer {
        let any = self.db.any_table(table);
        tracing::debug!(query = any.name(), why = %why.show(self.db), "computing");
        tracing::trace!(query = any.name(), key = any.show(slot), "key");
        self.stack.borrow_mut().push(Frame { table, slot, reads: Vec::new() });
        Activity::query(any.name()).start()
    }

    /// Ends the computation [`Cx::enter`] started, returning what it read.
    #[inline(never)]
    fn leave(&self) -> Box<[Dep]> {
        Self::once(self.stack.borrow_mut().pop().expect("a frame entered").reads)
    }

    /// Records a memo computed with `fingerprint`, where the one `loaded` had its own: what was
    /// loaded already, computed again for its value, changes nothing saved.
    fn computing(&self, why: Why, loaded: Option<Fingerprint>, fingerprint: Fingerprint) {
        if !(matches!(why, Why::Unloaded) && loaded == Some(fingerprint)) {
            self.db.changed.store(true, Ordering::Relaxed);
        }
    }

    /// `reads` with each memo once, in the order it was first read: verifying follows it, since a
    /// later read may be of a key an earlier one's value named.
    fn once(mut reads: Vec<Dep>) -> Box<[Dep]> {
        let mut seen = HashSet::with_capacity_and_hasher(reads.len(), foldhash::fast::RandomState::default());
        reads.retain(|dep| seen.insert((dep.table, dep.slot)));
        reads.into_boxed_slice()
    }
}
