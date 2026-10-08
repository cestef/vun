//! One query's memos: each key gets a slot the first time it is asked for, and the slot holds its
//! memo from then on. A read looks the key up once, lock-free; what it read is then recorded,
//! verified and compared by table and slot, two integers. A key of the saved memos loaded gets
//! its slot by its position there, once, and is read from them the first time it is needed:
//! verifying a memo reaches the memos it read without reading their keys.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwapOption;
use vun_par::Map;

use super::Query;
use super::claim::Claims;
use super::hashed::{Hashed, Probe};
use super::memo::Memo;
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
    key: OnceLock<Arc<Q::Key>>,
    pub(crate) memo: ArcSwapOption<Memo<Q>>,
    pub(crate) claims: Claims,
    /// Its key's position among the query's keys in the saved memos loaded.
    pub(crate) at: OnceLock<u32>,
    /// Set for the entry of a thread that lost the race to add its key: the slot stored stands
    /// for it.
    lost: AtomicBool,
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
            let held = self.entry(slot).key.get_or_init(|| Arc::new(key.clone()));
            self.slots.pin().insert(probe.shared(held.clone()), slot);
            return slot;
        }
        // Another thread may add the same key meanwhile: the first slot stored stands for both.
        let stored = probe.stored();
        let held = stored.key().clone();
        let mut pushed = None;
        let slot = *self.slots.pin().get_or_insert_with(stored, || {
            let slot = self.push(OnceLock::from(held), None);
            pushed = Some(slot);
            slot
        });
        if let Some(lost) = pushed.filter(|&pushed| pushed != slot) {
            self.entry(lost).lost.store(true, Ordering::Relaxed);
        }
        slot
    }

    /// The slot of the key saved at `at`, made the first time it is asked for, its key unread.
    pub(crate) fn saved_slot(&self, at: u32) -> Option<Slot> {
        let cell = self.attached.get()?.slot(at)?;
        Some(*cell.get_or_init(|| self.push(OnceLock::new(), Some(at))))
    }

    /// A new slot for `key`, saved at `at` if anywhere, holding the memo saved there.
    fn push(&self, key: OnceLock<Arc<Q::Key>>, at: Option<u32>) -> Slot {
        let memo = at.and_then(|at| self.attached.get()?.memo(at)).map(Arc::new);
        if memo.is_some() {
            self.counts.add(Event::Restored);
        }
        let entry = Entry {
            key,
            memo: ArcSwapOption::new(memo),
            claims: Claims::default(),
            at: at.map_or_else(OnceLock::new, OnceLock::from),
            lost: AtomicBool::new(false),
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
            Arc::new(key.unwrap_or_else(|| panic!("the key {} saved at {at:?} does not read", Q::NAME)))
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

    /// Every entry of a key, by slot: not those pushed by a thread another added the key before.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (Slot, &Entry<Q>)> {
        let entries = self.entries.iter().map(|(index, entry)| (Slot::at(index), entry));
        entries.filter(|(_, entry)| !entry.lost.load(Ordering::Relaxed))
    }

    /// Every entry with a memo.
    pub(crate) fn memos(&self) -> impl Iterator<Item = (&Q::Key, std::sync::Arc<Memo<Q>>)> {
        self.entries().filter_map(|(slot, e)| Some((self.key(slot), e.memo.load_full()?)))
    }
}

mod any;

pub(crate) use any::AnyTable;
