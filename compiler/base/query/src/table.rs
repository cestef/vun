//! One query's memos: each key gets a slot the first time it is asked for, and the slot holds its
//! memo from then on. A read looks the key up once, lock-free; what it read is then recorded,
//! verified and compared by table and slot, two integers.

use std::any::Any;

use arc_swap::ArcSwapOption;
use vun_par::Map;

use super::Query;
use super::claim::Claims;
use super::cx::Cx;
use super::fingerprint::Fingerprint;
use super::memo::Memo;

/// A table of a database, by the order it was made in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TableId(pub(crate) u32);

/// A key of a table, by the order it was first asked for in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Slot(u32);

impl Slot {
    pub(crate) fn at(index: usize) -> Self {
        Self(u32::try_from(index).expect("fewer than 2^32 keys per query"))
    }

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

pub(crate) struct Table<Q: Query> {
    pub(crate) id: TableId,
    slots: Map<Q::Key, Slot>,
    entries: boxcar::Vec<Entry<Q>>,
}

/// A key, its memo once there is one, and the claim of whoever computes it.
pub(crate) struct Entry<Q: Query> {
    pub(crate) key: Q::Key,
    pub(crate) memo: ArcSwapOption<Memo<Q>>,
    pub(crate) claims: Claims,
}

impl<Q: Query> Table<Q> {
    pub(crate) fn new(id: TableId) -> Self {
        Self { id, slots: Map::default(), entries: boxcar::Vec::new() }
    }

    /// The slot of `key`, made the first time it is asked for.
    pub(crate) fn slot(&self, key: &Q::Key) -> Slot {
        let slots = self.slots.pin();
        if let Some(&slot) = slots.get(key) {
            return slot;
        }
        // Another thread may add the same key meanwhile: the first slot stored stands for both.
        *slots.get_or_insert_with(key.clone(), || {
            let entry = Entry { key: key.clone(), memo: ArcSwapOption::empty(), claims: Claims::default() };
            Slot::at(self.entries.push(entry))
        })
    }

    pub(crate) fn entry(&self, slot: Slot) -> &Entry<Q> {
        &self.entries[slot.index()]
    }

    /// Every entry, by slot.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (Slot, &Entry<Q>)> {
        self.entries.iter().map(|(index, entry)| (Slot::at(index), entry))
    }

    /// Every entry with a memo.
    pub(crate) fn memos(&self) -> impl Iterator<Item = (&Q::Key, std::sync::Arc<Memo<Q>>)> {
        self.entries.iter().filter_map(|(_, e)| Some((&e.key, e.memo.load_full()?)))
    }
}

/// A table whatever its query: what reads recorded by table and slot are verified and shown
/// through.
pub(crate) trait AnyTable: Send + Sync {
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    /// Whether the memo at `slot`, brought up to date, still has `fingerprint`.
    fn unchanged(&self, cx: &Cx<'_>, slot: Slot, fingerprint: Fingerprint) -> bool;
    /// The key at `slot`, as a cycle names it.
    fn show(&self, slot: Slot) -> String;
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
        format!("{:?}", self.entry(slot).key)
    }
}
