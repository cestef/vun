//! Verifying a memo of an earlier revision or another process: current, holding at once for one
//! reading shared inputs alone whose fingerprint is the one it held under, or every read, brought
//! up to date, unchanged.

use std::sync::Arc;

use vun_trace::Activity;

use super::guard::Verifying;
use super::why::Why;
use super::{Cx, Differs, Reuse};
use crate::memo::{Dep, Memo};
use crate::table::{Slot, Table};
use crate::tally::Event;
use crate::{Query, Revision};

/// Verifying memos from an earlier revision or another process, as timed: what they read
/// compared, those computed again meanwhile timed on their own.
const VERIFY: Activity = Activity::stage("verify memos");

impl Cx<'_> {
    /// The memo at `slot` if it holds in `revision`: current, durable under the shared inputs as
    /// they are, or everything it read unchanged. Verifying reads other memos, which may take
    /// long: the memo is held on its own meanwhile.
    pub(super) fn verified<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
    ) -> Result<Arc<Memo<Q>>, Why> {
        let memo = table.entry(slot).memo.load_full().ok_or(Why::Absent)?;
        if memo.current(revision) {
            return Ok(memo);
        }
        if self.db.durable(memo.durability) {
            return Ok(self.held(table, slot, memo, revision, Event::Durable));
        }
        let deps = memo.deps.get(self.db).ok_or(Why::Unresolved)?;
        if self.verifying.borrow().contains(&(table.id, slot)) {
            return Err(Why::Cycle);
        }
        self.settle(deps, revision);
        let verifying = Verifying::start(self, (table.id, slot));
        let timer = VERIFY.start();
        let stale = self.stale(deps);
        drop((timer, verifying));
        if let Some(read) = stale {
            if self.db.settled_in((table.id, slot), revision) {
                Self::unsound(table, slot, Differs::Settled);
            }
            return Err(Why::Stale(read));
        }
        Ok(self.held(table, slot, memo, revision, Event::Held))
    }

    /// `memo`, found to hold as `event` counts, marked current: checked first under
    /// [`Reuse::Check`], or the memo a cycle among its reads computed meanwhile.
    fn held<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        memo: Arc<Memo<Q>>,
        revision: Revision,
        event: Event,
    ) -> Arc<Memo<Q>> {
        if self.db.reuse == Reuse::Check && !memo.is_input() {
            self.check(table, slot, &memo);
        }
        // Recovering from a cycle among its reads may have computed it meanwhile.
        if let Some(computed) = table.entry(slot).memo.load_full()
            && !Arc::ptr_eq(&computed, &memo)
            && computed.current(revision)
        {
            return computed;
        }
        memo.verified.set(revision);
        table.counts.add(event);
        memo
    }

    /// The first of `reads` that changed, each brought up to date in turn.
    #[inline(never)]
    pub(super) fn stale(&self, reads: &[Dep]) -> Option<Dep> {
        reads
            .iter()
            .copied()
            .find(|dep| !self.db.any_table(dep.table).unchanged(self, dep.slot, dep.fingerprint))
    }
}
