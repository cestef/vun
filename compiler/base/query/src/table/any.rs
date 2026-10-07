//! A table whatever its query, as what reads recorded by table and slot are verified and shown
//! through.

use std::any::Any;
use std::sync::atomic::Ordering;

use vun_hash::Stable;

use super::{Slot, Table, TableId};
use crate::Query;
use crate::cx::{Cx, Found, Settle};
use crate::db::{Db, Sharing};
use crate::fingerprint::Fingerprint;
use crate::memo::{Dep, Source};
use crate::tally::Counts;

/// A table whatever its query: what reads recorded by table and slot are verified and shown
/// through.
pub(crate) trait AnyTable: Send + Sync {
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    /// Whether the memo at `slot`, brought up to date, still has `fingerprint`.
    fn unchanged(&self, cx: &Cx<'_>, slot: Slot, fingerprint: Fingerprint) -> bool;
    /// The key at `slot`, as a cycle names it.
    fn show(&self, slot: Slot) -> String;
    /// The slot of the key saved at `at` among the query's keys in the memos loaded.
    fn saved_slot(&self, at: u32) -> Option<Slot>;
    /// How many slots were made, with those of threads that lost the race to add their key.
    fn len(&self) -> usize;
    /// What the memo at `slot` holds and read, if it has one whose reads are held.
    fn facts(&self, db: &Db, slot: Slot) -> Option<Facts>;
    /// Marks its memos as saved.
    fn persist(&self);
    /// What was done with its memos.
    fn counts(&self) -> &Counts;
    /// How far the inputs the memo at `slot` reads are shared; [`Sharing::Own`] without one.
    fn level(&self, slot: Slot) -> Sharing;
    /// The fingerprint of its inputs set as shared as far as `level` or further, by query, key
    /// and value.
    fn shared_inputs(&self, level: Sharing) -> Fingerprint;
    /// What verifying ahead finds of the memo at `slot`, with its fingerprint ([`Settle::holds`]).
    fn settled(
        &self,
        settle: &Settle<'_>,
        slot: Slot,
        path: &mut Vec<(TableId, Slot)>,
    ) -> (Found, Option<Fingerprint>);
}

/// What a memo is, as sharing it with other databases asks.
pub(crate) struct Facts {
    /// An input, set or holding what it computes while unset.
    pub(crate) input: bool,
    /// An input set with [`Db::share`].
    pub(crate) shared: bool,
    /// Computed in this process rather than loaded.
    pub(crate) computed: bool,
    pub(crate) deps: Box<[Dep]>,
}

impl<Q: Query> AnyTable for Table<Q> {
    fn name(&self) -> &'static str {
        Q::NAME
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn unchanged(&self, cx: &Cx<'_>, slot: Slot, fingerprint: Fingerprint) -> bool {
        cx.fingerprint(self, slot) == fingerprint
    }

    fn show(&self, slot: Slot) -> String {
        format!("{:?}", self.key(slot))
    }

    fn saved_slot(&self, at: u32) -> Option<Slot> {
        Table::saved_slot(self, at)
    }

    fn len(&self) -> usize {
        self.entries.count()
    }

    fn facts(&self, db: &Db, slot: Slot) -> Option<Facts> {
        let memo = self.entry(slot).memo.load_full()?;
        let computed = memo.fresh();
        let deps = memo.deps.get(db)?.into();
        let shared = match memo.source {
            Source::Set(sharing) => sharing != Sharing::Own,
            Source::Unset => true,
            Source::Computed => false,
        };
        Some(Facts { input: memo.is_input(), shared, computed, deps })
    }

    fn persist(&self) {
        self.persisted.store(true, Ordering::Relaxed);
    }

    fn counts(&self) -> &Counts {
        &self.counts
    }

    fn level(&self, slot: Slot) -> Sharing {
        self.entry(slot).memo.load().as_ref().map_or(Sharing::Own, |memo| memo.durability.level)
    }

    fn shared_inputs(&self, level: Sharing) -> Fingerprint {
        if !Q::INPUT {
            return Fingerprint::default();
        }
        let shared = self.entries().filter_map(|(slot, entry)| {
            let memo = entry.memo.load_full()?;
            let Source::Set(sharing) = memo.source else { return None };
            (sharing >= level)
                .then(|| Fingerprint::of(&(Q::NAME, Stable::of(self.key(slot)), memo.fingerprint)))
        });
        shared.fold(Fingerprint::default(), Fingerprint::with)
    }

    fn settled(
        &self,
        settle: &Settle<'_>,
        slot: Slot,
        path: &mut Vec<(TableId, Slot)>,
    ) -> (Found, Option<Fingerprint>) {
        settle.holds(self, slot, path)
    }
}
