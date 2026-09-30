//! One query's memos: each key gets a slot the first time it is asked for, and the slot holds its
//! memo from then on. A read looks the key up once, lock-free; what it read is then recorded,
//! verified and compared by table and slot, two integers. A key of the saved memos loaded gets
//! its slot by its position there, once, and is read from them the first time it is needed:
//! verifying a memo reaches the memos it read without reading their keys.

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwapOption;
use vun_hash::Stable;
use vun_par::Map;

use super::Query;
use super::claim::Claims;
use super::cx::{Cx, Found, Settle};
use super::db::{Db, Sharing};
use super::fingerprint::Fingerprint;
use super::hashed::{Hashed, Probe};
use super::memo::{Dep, Memo, Source};
use super::persist::{Attached, Taking};
use super::tally::{Counts, Event};

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
    /// The slot of each key asked for by its value.
    slots: Map<Hashed<Q::Key>, Slot>,
    entries: boxcar::Vec<Entry<Q>>,
    /// The query's part of the saved memos loaded, whose memos are restored as their keys are
    /// first asked for.
    pub(crate) attached: OnceLock<Attached<Q>>,
    /// Whether its memos are saved: what computing one changes of the memos loaded.
    pub(crate) persisted: AtomicBool,
    /// What was done with its memos.
    pub(crate) counts: Counts,
}

/// A key, its memo once there is one, and the claim of whoever computes it.
pub(crate) struct Entry<Q: Query> {
    /// Unset for a key given its slot by its position in the saved memos, until it is read.
    key: OnceLock<Q::Key>,
    pub(crate) memo: ArcSwapOption<Memo<Q>>,
    pub(crate) claims: Claims,
    /// Its key's position among the query's keys in the saved memos loaded.
    pub(crate) at: OnceLock<u32>,
}

impl<Q: Query> Table<Q> {
    pub(crate) fn new(id: TableId) -> Self {
        Self {
            id,
            slots: Map::default(),
            entries: boxcar::Vec::new(),
            attached: OnceLock::new(),
            persisted: AtomicBool::new(false),
            counts: Counts::default(),
        }
    }

    /// The slot of `key`, made the first time it is asked for, holding the memo saved for it if
    /// there is one.
    pub(crate) fn slot(&self, key: &Q::Key) -> Slot {
        let probe = Probe::of(key);
        if let Some(&slot) = self.slots.pin().get(&probe) {
            return slot;
        }
        // Finding a saved key reads it, to tell it from another of the same hash.
        let at = self.attached.get().and_then(|attached| attached.find(key));
        if at.is_some() {
            self.counts.add(Event::KeyRead);
        }
        if let Some(slot) = at.and_then(|at| self.saved_slot(at)) {
            self.entry(slot).key.get_or_init(|| key.clone());
            self.slots.pin().insert(probe.stored(), slot);
            return slot;
        }
        // Another thread may add the same key meanwhile: the first slot stored stands for both.
        *self.slots.pin().get_or_insert_with(probe.stored(), || self.push(OnceLock::from(key.clone()), None))
    }

    /// The slot of the key saved at `at`, made the first time it is asked for, its key unread.
    pub(crate) fn saved_slot(&self, at: u32) -> Option<Slot> {
        let cell = self.attached.get()?.slot(at)?;
        Some(*cell.get_or_init(|| self.push(OnceLock::new(), Some(at))))
    }

    /// A new slot for `key`, saved at `at` if anywhere, holding the memo saved there.
    fn push(&self, key: OnceLock<Q::Key>, at: Option<u32>) -> Slot {
        let memo = at.and_then(|at| self.attached.get()?.memo(at)).map(Arc::new);
        if memo.is_some() {
            self.counts.add(Event::Restored);
        }
        let entry = Entry {
            key,
            memo: ArcSwapOption::new(memo),
            claims: Claims::default(),
            at: at.map_or_else(OnceLock::new, OnceLock::from),
        };
        Slot::at(self.entries.push(entry))
    }

    /// The key at `slot`, read from the saved memos the first time for one given its slot there.
    ///
    /// # Panics
    /// If the saved key does not read: the memos loaded are this build's own (`Build::identity`),
    /// so one it wrote and cannot read is a defect of its encoding.
    pub(crate) fn key(&self, slot: Slot) -> &Q::Key {
        let entry = self.entry(slot);
        entry.key.get_or_init(|| {
            self.counts.add(Event::KeyRead);
            let at = entry.at.get().copied();
            let key = at.and_then(|at| self.attached.get()?.key(at));
            key.unwrap_or_else(|| panic!("the key {} saved at {at:?} does not read", Q::NAME))
        })
    }

    /// Attaches the query's part of the saved memos loaded: keys asked for already are linked to
    /// theirs, and given their memos as `taking` says.
    pub(crate) fn attach(&self, attached: Attached<Q>, taking: Taking<'_>) {
        if self.attached.set(attached).is_err() {
            return;
        }
        let attached = self.attached.get().expect("set above");
        let written = match taking {
            Taking::Missing => None,
            Taking::Written(numbers) => Some(numbers.get(&self.id).map_or(&[][..], Vec::as_slice)),
        };
        for (slot, entry) in self.entries() {
            let at = match written {
                None => entry.key.get().and_then(|key| attached.find(key)),
                Some(numbers) => numbers.get(slot.index()).copied().flatten().map(|number| number.at),
            };
            let Some(at) = at else { continue };
            entry.at.get_or_init(|| at);
            if let Some(cell) = attached.slot(at) {
                cell.get_or_init(|| slot);
            }
            match entry.memo.load().as_deref() {
                None => entry.memo.store(attached.memo(at).map(Arc::new)),
                Some(memo) if written.is_some() => memo.novelty.saved(),
                Some(_) => {}
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
        self.entries().filter_map(|(slot, e)| Some((self.key(slot), e.memo.load_full()?)))
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
