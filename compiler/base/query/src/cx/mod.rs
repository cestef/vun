//! Computing queries: the stack of queries being computed on one thread, what each has read, and
//! cycles.

mod all;
mod cycle;
mod frame;
mod guard;
mod reuse;
mod settle;
mod stored;
mod verify;
mod why;

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_hash::{Map, Set};
use vun_trace::{Activity, tracing};

pub use self::cycle::Cycle;
use self::frame::{Frame, Standing};
use self::guard::Entered;
pub use self::reuse::{Differs, Reuse, Unsound};
pub(crate) use self::settle::{Found, Settle};
use self::why::Why;
use super::claim::Claim;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Memo, Novelty};
use super::table::{Slot, Table, TableId};
use super::tally::Event;
use super::{Query, Revision};

/// The log target naming each saved memo computed anew and why, asked for alone without slowing
/// the rest: `VUN_LOG=vun_query::changed=debug`.
const CHANGED: &str = "vun_query::changed";

/// The queries being computed on one thread, and what each has read so far.
pub struct Cx<'db> {
    db: &'db Db,
    /// The queries being computed by the contexts that lent this one its work (`get_all`), the
    /// outermost first: frames of the path below this context's own.
    above: Arc<[(TableId, Slot)]>,
    /// The lowest of those whose cycle value a computation here read.
    reached: Cell<Option<usize>>,
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
        Self::under(db, Arc::from([]))
    }

    /// A context computing work lent by the contexts computing `above`.
    fn under(db: &'db Db, above: Arc<[(TableId, Slot)]>) -> Self {
        Self {
            db,
            above,
            reached: Cell::new(None),
            stack: RefCell::default(),
            computing: RefCell::default(),
            verifying: RefCell::default(),
        }
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

    /// The value at `slot` computed on this thread, or the value of the cycle it closes; `None`
    /// when another thread computed it meanwhile.
    fn computed<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
        why: Why,
    ) -> Option<(Q::Value, Fingerprint)> {
        if let Some(cycle) = self.cycle(table.id, slot) {
            return Some(self.closed(table, slot, &cycle));
        }
        let owned = match table.entry(slot).claims.claim(&self.db.waits) {
            Claim::Waited(waited) => {
                tracing::debug!(query = Q::NAME, key = ?table.key(slot), ?waited, "waited for another thread");
                table.counts.add(Event::Waited);
                return None;
            }
            Claim::Owner(owned) => Some(owned),
            Claim::Alone => None,
        };
        table.counts.add(why.event());
        let entered = self.enter(table.id, slot, why);
        let value = Q::compute(self, table.key(slot));
        let (reads, standing) = entered.leave();
        Some(self.stored(table, slot, revision, why, (value, reads, standing), owned))
    }

    /// Starts computing `slot` of `table`, timed until it is left.
    #[inline(never)]
    fn enter(&self, table: TableId, slot: Slot, why: Why) -> Entered<'_, 'db> {
        let any = self.db.any_table(table);
        tracing::debug!(query = any.name(), why = %why.show(self.db), "computing");
        let mut stack = self.stack.borrow_mut();
        let parent = stack.last().map(|frame| (self.db.any_table(frame.table), frame.slot));
        tracing::trace!(
            query = any.name(),
            key = any.show(slot),
            depth = stack.len(),
            parent = parent.map(|(table, _)| table.name()),
            parent_key = parent.map(|(table, slot)| table.show(slot)),
            "key"
        );
        self.computing.borrow_mut().insert((table, slot), stack.len());
        stack.push(Frame { table, slot, reads: Vec::new(), head: None });
        Entered::new(self, Activity::query(any.name()).start())
    }

    /// Ends the computation on top of the stack, returning what it read and whether its value
    /// stands: a cycle value read of a frame below it makes the frame reading it provisional too,
    /// and the frame below it, or the lender's, reads that cycle value in turn.
    #[inline(never)]
    fn leave(&self) -> (Box<[Dep]>, Standing) {
        let frame = self.stack.borrow_mut().pop().expect("a frame entered");
        self.computing.borrow_mut().remove(&(frame.table, frame.slot));
        let depth = self.above.len() + self.stack.borrow().len();
        let standing = match frame.head {
            Some(head) if head < depth => {
                self.reach(head);
                Standing::Provisional
            }
            _ => Standing::Final,
        };
        (Self::once(frame.reads), standing)
    }

    /// Records a memo of `table` computed with `fingerprint` from `reads` in place of `replaced`,
    /// returning how new it is: one of a query not saved, or equal to it, as one loaded computed
    /// again for its value or out of a cycle among its reads, changes nothing saved.
    fn computing<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        why: Why,
        replaced: Option<&Memo<Q>>,
        fingerprint: Fingerprint,
        reads: &[Dep],
    ) -> Novelty {
        let equal =
            replaced.filter(|memo| memo.fingerprint == fingerprint && memo.deps.get(self.db) == Some(reads));
        if let Some(memo) = equal {
            return memo.novelty.get();
        }
        if table.persisted.load(Ordering::Relaxed) {
            tracing::debug!(
                target: CHANGED,
                query = Q::NAME,
                key = ?table.key(slot),
                asked = why.show(self.db),
                differs = self.differs(replaced.map(|m| (m.fingerprint, m.deps.get(self.db))), fingerprint, reads),
                "a saved memo computed anew"
            );
            _ = self.db.changed.set(Q::NAME);
            self.db.unshared.store(true, Ordering::Relaxed);
        }
        Novelty::New
    }

    /// How a memo computed differs from the one it replaces, given as its fingerprint and its
    /// reads (`None` when they cannot be resolved): what makes it new.
    fn differs(
        &self,
        replaced: Option<(Fingerprint, Option<&[Dep]>)>,
        fingerprint: Fingerprint,
        reads: &[Dep],
    ) -> String {
        let show = |dep: &Dep| {
            let table = self.db.any_table(dep.table);
            format!("{}({})", table.name(), table.show(dep.slot))
        };
        match replaced {
            None => "none was held".to_owned(),
            Some((held, _)) if held != fingerprint => "its value differs".to_owned(),
            Some((_, None)) => "the reads saved with it are not all held".to_owned(),
            Some((_, Some(held))) => match held.iter().zip(reads).find(|(a, b)| a != b) {
                Some((was, now)) => format!("it reads {} where it read {}", show(now), show(was)),
                None => format!("it reads {} memos where it read {}", reads.len(), held.len()),
            },
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
