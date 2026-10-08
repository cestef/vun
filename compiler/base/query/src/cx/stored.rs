//! What a computation leaves: the value of a cycle, or the memo of a value computed.

use std::sync::Arc;

use vun_trace::tracing;

use super::frame::Standing;
use super::why::Why;
use super::{Cx, Cycle};
use crate::claim::Owned;
use crate::fingerprint::Fingerprint;
use crate::groups::Groups;
use crate::memo::{Dep, Memo, Source};
use crate::table::{Slot, Table};
use crate::tally::Event;
use crate::{Query, Revision};

impl Cx<'_> {
    /// The value of the cycle `cycle` closes at `slot`. Kept out of [`Self::computed`], whose
    /// frame stays on the stack under every query it computes.
    #[inline(never)]
    pub(super) fn closed<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        cycle: &Cycle,
    ) -> (Q::Value, Fingerprint) {
        let value = Q::cycle(self, table.key(slot), cycle);
        let fingerprint = Q::interface(&value);
        (value, fingerprint)
    }

    /// Keeps what `computed` found in the memo of `slot`, unless it came from a cycle value.
    #[inline(never)]
    pub(super) fn stored<Q: Query>(
        &self,
        table: &Table<Q>,
        slot: Slot,
        revision: Revision,
        why: Why,
        (value, reads, standing): (Q::Value, Box<[Dep]>, Standing),
        owned: Option<Owned<'_>>,
    ) -> (Q::Value, Fingerprint) {
        let entry = table.entry(slot);
        let fingerprint = Q::interface(&value);
        if standing == Standing::Provisional {
            table.counts.add(Event::Provisional);
            tracing::debug!(query = Q::NAME, "not kept: computed from a cycle value");
            return (value, fingerprint);
        }
        let memo = if Q::INPUT {
            // An input not set holds what it computes until it is, alike in every database.
            Memo::input(
                value.clone(),
                fingerprint,
                revision,
                Source::Unset,
                Groups::identity::<Q>(table.key(slot)),
            )
        } else {
            let novelty = self.computing(table, slot, why, entry.memo.load().as_deref(), fingerprint, &reads);
            let durability = self.durability(&reads);
            Memo::computed(value.clone(), fingerprint, self.db.list(&reads), revision, durability, novelty)
        };
        entry.memo.store(Some(Arc::new(memo)));
        drop(owned);
        (value, fingerprint)
    }
}
