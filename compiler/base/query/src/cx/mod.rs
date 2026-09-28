//! Computing queries: the stack of queries being computed on one thread, what each has read, and
//! cycles.

mod all;
mod cycle;
mod frame;
mod guard;
mod reuse;
mod why;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_hash::{Map, Set};
use vun_trace::{Activity, tracing};

pub use self::cycle::Cycle;
use self::frame::{Frame, Standing};
use self::guard::{Entered, Verifying};
pub use self::reuse::{Differs, Reuse, Unsound};
use self::why::Why;
use super::claim::Claim;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Memo};
use super::table::{Slot, Table, TableId};
use super::{Query, Revision};

/// The queries being computed on one thread, and what each has read so far.
pub struct Cx<'db> {
    db: &'db Db,
    stack: RefCell<Vec<Frame>>,
    /// Where each memo computed on the stack is on it, so that reaching one again is found at
    /// once however deep the stack is.
    computing: RefCell<Map<(TableId, Slot), usize>>,
    /// The memos being verified on this thread.
    verifying: RefCell<Set<(TableId, Slot)>>,
}

impl<'db> Cx<'db> {
    #[must_use]
    pub fn new(db: &'db Db) -> Self {
        Self { db, stack: RefCell::default(), computing: RefCell::default(), verifying: RefCell::default() }
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
        if self.verifying.borrow().contains(&(table.id, slot)) {
            return Err(Why::Cycle);
        }
        let verifying = Verifying::start(self, (table.id, slot));
        let stale = self.stale(deps);
        drop(verifying);
        if let Some(read) = stale {
            return Err(Why::Stale(read));
        }
        if self.db.reuse == Reuse::Check && !memo.input {
            self.check(table, slot, &memo);
        }
        // Recovering from a cycle among its reads may have computed it meanwhile.
        if let Some(computed) = table.entry(slot).memo.load_full()
            && !Arc::ptr_eq(&computed, &memo)
            && computed.current(revision)
        {
            return Ok(computed);
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
        let entered = self.enter(table.id, slot, why);
        let value = Q::compute(self, &entry.key);
        let (reads, standing) = entered.leave();
        let fingerprint = Q::interface(&value);
        if standing == Standing::Provisional {
            tracing::debug!(query = Q::NAME, "not kept: computed from a cycle value");
            return Some((value, fingerprint));
        }
        let memo = if Q::INPUT {
            // An input not set holds what it computes until it is, alike in every database.
            Memo::input(value.clone(), fingerprint, revision, true)
        } else {
            self.computing(entry.memo.load().as_deref(), fingerprint, &reads);
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
        let mut stack = self.stack.borrow_mut();
        let from = *self.computing.borrow().get(&(table, slot))?;
        let top = stack.last_mut().expect("the frame reaching it");
        top.head = Some(top.head.map_or(from, |head| head.min(from)));
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

    /// Starts computing `slot` of `table`, timed until it is left.
    #[inline(never)]
    fn enter(&self, table: TableId, slot: Slot, why: Why) -> Entered<'_, 'db> {
        let any = self.db.any_table(table);
        tracing::debug!(query = any.name(), why = %why.show(self.db), "computing");
        tracing::trace!(query = any.name(), key = any.show(slot), "key");
        let mut stack = self.stack.borrow_mut();
        self.computing.borrow_mut().insert((table, slot), stack.len());
        stack.push(Frame { table, slot, reads: Vec::new(), head: None });
        Entered::new(self, Activity::query(any.name()).start())
    }

    /// Ends the computation on top of the stack, returning what it read and whether its value
    /// stands: a cycle value read of a frame below it makes the frame reading it provisional too.
    #[inline(never)]
    fn leave(&self) -> (Box<[Dep]>, Standing) {
        let mut stack = self.stack.borrow_mut();
        let frame = stack.pop().expect("a frame entered");
        self.computing.borrow_mut().remove(&(frame.table, frame.slot));
        let at = stack.len();
        let standing = match frame.head {
            Some(head) if head < at => {
                let parent = stack.last_mut().expect("a frame below the head");
                parent.head = Some(parent.head.map_or(head, |h| h.min(head)));
                Standing::Provisional
            }
            _ => Standing::Final,
        };
        (Self::once(frame.reads), standing)
    }

    /// Records a memo computed with `fingerprint` from `reads` in place of `replaced`: one equal
    /// to it, as one loaded computed again for its value or out of a cycle among its reads,
    /// changes nothing saved.
    fn computing<Q: Query>(&self, replaced: Option<&Memo<Q>>, fingerprint: Fingerprint, reads: &[Dep]) {
        let equal = replaced
            .is_some_and(|memo| memo.fingerprint == fingerprint && memo.deps.get(self.db) == Some(reads));
        if !equal {
            tracing::trace!(query = Q::NAME, replaced = replaced.is_some(), "a memo changed");
            self.db.changed.store(true, Ordering::Relaxed);
        }
    }

    /// `reads` with each memo once, in the order it was first read: verifying follows it, since a
    /// later read may be of a key an earlier one's value named.
    fn once(mut reads: Vec<Dep>) -> Box<[Dep]> {
        let same = |a: &Dep, b: &Dep| (a.table, a.slot) == (b.table, b.slot);
        if reads.len() <= Self::SCANNED {
            // Few reads are compared with those kept, faster than hashing them.
            let mut kept = 0;
            for at in 0..reads.len() {
                if !reads[..kept].iter().any(|k| same(k, &reads[at])) {
                    reads.swap(kept, at);
                    kept += 1;
                }
            }
            reads.truncate(kept);
        } else {
            let mut seen = Set::default();
            seen.reserve(reads.len());
            reads.retain(|dep| seen.insert((dep.table, dep.slot)));
        }
        reads.into_boxed_slice()
    }

    /// How many reads are deduplicated by comparing them, past which they are hashed.
    const SCANNED: usize = 32;
}
