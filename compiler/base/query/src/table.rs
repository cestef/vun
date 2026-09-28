//! One query's memos: each key gets a slot the first time it is asked for, and the slot holds its
//! memo from then on. A read looks the key up once, lock-free; what it read is then recorded,
//! verified and compared by table and slot, two integers.

use std::any::Any;
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwapOption;
use vun_par::Map;

use super::Query;
use super::claim::Claims;
use super::cx::Cx;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::hashed::{Hashed, Probe};
use super::memo::{Dep, Deps, Memo};
use super::persist::Attached;

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
    slots: Map<Hashed<Q::Key>, Slot>,
    entries: boxcar::Vec<Entry<Q>>,
    /// The query's part of the saved memos loaded, whose memos are restored as their keys are
    /// first asked for.
    pub(crate) attached: OnceLock<Attached<Q>>,
}

/// A key, its memo once there is one, and the claim of whoever computes it.
pub(crate) struct Entry<Q: Query> {
    pub(crate) key: Q::Key,
    pub(crate) memo: ArcSwapOption<Memo<Q>>,
    pub(crate) claims: Claims,
    /// Its key's position among the query's keys in the saved memos loaded.
    pub(crate) at: OnceLock<u32>,
}

impl<Q: Query> Table<Q> {
    pub(crate) fn new(id: TableId) -> Self {
        Self { id, slots: Map::default(), entries: boxcar::Vec::new(), attached: OnceLock::new() }
    }

    /// The slot of `key`, made the first time it is asked for, holding the memo saved for it if
    /// there is one.
    pub(crate) fn slot(&self, key: &Q::Key) -> Slot {
        let probe = Probe::of(key);
        if let Some(&slot) = self.slots.pin().get(&probe) {
            return slot;
        }
        let at = self.attached.get().and_then(|attached| attached.find(key));
        self.insert(&probe, at)
    }

    /// The slot of the key saved at `at`, made the first time it is asked for.
    pub(crate) fn saved_slot(&self, at: u32) -> Option<Slot> {
        let attached = self.attached.get()?;
        if let Some(slot) = attached.linked(at) {
            return Some(slot);
        }
        let key = attached.key(at)?;
        Some(self.insert(&Probe::of(&key), Some(at)))
    }

    /// The slot of `key`, saved at `at` if anywhere, made with its saved memo if it has none.
    fn insert(&self, probe: &Probe<'_, Q::Key>, at: Option<u32>) -> Slot {
        let key = probe.key;
        // Another thread may add the same key meanwhile: the first slot stored stands for both.
        let slot = *self.slots.pin().get_or_insert_with(probe.stored(), || {
            let memo = at.and_then(|at| self.attached.get()?.memo(at)).map(Arc::new);
            let entry = Entry {
                key: key.clone(),
                memo: ArcSwapOption::new(memo),
                claims: Claims::default(),
                at: OnceLock::new(),
            };
            Slot::at(self.entries.push(entry))
        });
        if let Some(at) = at {
            self.link(slot, at);
        }
        slot
    }

    /// Records the key at `slot` as the one saved at `at`.
    fn link(&self, slot: Slot, at: u32) {
        let entry = self.entry(slot);
        entry.at.get_or_init(|| at);
        if let Some(attached) = self.attached.get() {
            attached.link(at, slot);
        }
    }

    /// Attaches the query's part of the saved memos loaded: keys asked for already are linked to
    /// theirs, and given their memos if they have none.
    pub(crate) fn attach(&self, attached: Attached<Q>) {
        if self.attached.set(attached).is_err() {
            return;
        }
        let attached = self.attached.get().expect("set above");
        for (slot, entry) in self.entries() {
            let Some(at) = attached.find(&entry.key) else { continue };
            self.link(slot, at);
            if entry.memo.load().is_none() {
                entry.memo.store(attached.memo(at).map(Arc::new));
            }
        }
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
    /// The slot of the key saved at `at` among the query's keys in the memos loaded.
    fn saved_slot(&self, at: u32) -> Option<Slot>;
    /// How many keys have slots.
    fn len(&self) -> usize;
    /// What the memo at `slot` holds and read, if it has one whose reads are held.
    fn facts(&self, db: &Db, slot: Slot) -> Option<Facts>;
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
        format!("{:?}", self.entry(slot).key)
    }

    fn saved_slot(&self, at: u32) -> Option<Slot> {
        Table::saved_slot(self, at)
    }

    fn len(&self) -> usize {
        self.entries.count()
    }

    fn facts(&self, db: &Db, slot: Slot) -> Option<Facts> {
        let memo = self.entry(slot).memo.load_full()?;
        let computed = matches!(memo.deps, Deps::Read(_)) && !memo.input;
        let deps = memo.deps.get(db)?.into();
        Some(Facts { input: memo.input, shared: memo.shared, computed, deps })
    }
}
