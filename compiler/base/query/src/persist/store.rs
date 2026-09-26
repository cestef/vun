//! The saved memos loaded, read as they are needed: a query's keys when one of them is first
//! looked for, a key's value and memo when that key is first asked for, and a memo's reads when
//! it is first verified. What a run never asks for is never read, so loading costs what is used.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use vun_codec::{Blob, Context};
use vun_hash::Stable;
use vun_trace::tracing;

use super::saved::{Growth, SavedDep, SavedKey, SavedMemo, SavedQuery};
use crate::Query;
use crate::db::Db;
use crate::memo::{Dep, Memo, ReadValue, Saved};
use crate::table::{Slot, TableId};

/// A file of saved memos, loaded.
pub(crate) struct Store {
    pub(crate) stream: Context,
    pub(crate) growth: Growth,
    /// Each query of the file, by its position there.
    pub(crate) queries: Vec<StoredQuery>,
}

/// A query of the file: its table, if a query of that name is registered, and its keys once read.
pub(crate) struct StoredQuery {
    pub(crate) name: String,
    pub(crate) bytes: Blob,
    pub(crate) table: Option<TableId>,
    keys: OnceLock<Option<StoredKeys>>,
}

/// A query's saved keys, found by their hashes, and the slot each was given in this process.
pub(crate) struct StoredKeys {
    pub(crate) saved: Vec<SavedKey<Blob>>,
    /// The first position of each hash.
    index: HashMap<u64, u32, foldhash::fast::RandomState>,
    slots: Box<[OnceLock<Slot>]>,
}

impl Store {
    /// The read `dep` saved, by slot.
    pub(crate) fn dep(&self, db: &Db, dep: SavedDep) -> Option<Dep> {
        let query = self.queries.get(dep.query as usize)?;
        let table = query.table?;
        let base = query.keys(&self.stream)?.saved.get(dep.key as usize)?.fingerprint;
        let slot = db.any_table(table).saved_slot(dep.key)?;
        Some(Dep { table, slot, fingerprint: dep.fingerprint.or(base)? })
    }

    /// Gives every saved key of a registered query its slot, and so its memo: what a file written
    /// whole holds.
    pub(crate) fn restore_all(&self, db: &Db) {
        for query in &self.queries {
            let (Some(table), Some(keys)) = (query.table, query.keys(&self.stream)) else { continue };
            let table = db.any_table(table);
            for at in 0..keys.saved.len() {
                table.saved_slot(u32::try_from(at).expect("fewer than 2^32 keys per query"));
            }
        }
    }
}

impl StoredQuery {
    pub(crate) fn new(name: String, bytes: Blob, table: Option<TableId>) -> Self {
        Self { name, bytes, table, keys: OnceLock::new() }
    }

    /// Its keys, read the first time; `None` when they do not decode.
    pub(crate) fn keys(&self, stream: &Context) -> Option<&StoredKeys> {
        self.keys
            .get_or_init(|| match stream.read_blob::<SavedQuery<Blob>>(self.bytes) {
                Ok(query) => Some(StoredKeys::new(query.keys)),
                Err(malformed) => {
                    tracing::debug!(query = self.name, %malformed, "saved keys not read");
                    None
                }
            })
            .as_ref()
    }
}

impl StoredKeys {
    fn new(saved: Vec<SavedKey<Blob>>) -> Self {
        let mut index =
            HashMap::with_capacity_and_hasher(saved.len(), foldhash::fast::RandomState::default());
        for (at, key) in (0..).zip(&saved) {
            index.entry(key.hash).or_insert(at);
        }
        let slots = std::iter::repeat_with(OnceLock::new).take(saved.len()).collect();
        Self { saved, index, slots }
    }

    /// The slot given to the key at `at`, if one was.
    pub(crate) fn slot(&self, at: u32) -> Option<Slot> {
        self.slots.get(at as usize)?.get().copied()
    }
}

/// A table's view of its query's part of the file, and how its keys and values are read.
pub(crate) struct Attached<Q: Query> {
    store: Arc<Store>,
    /// The query's position in the file.
    at: usize,
    key: ReadValue<Q::Key>,
    /// `None` for a query persisted without its values.
    value: Option<ReadValue<Q::Value>>,
}

impl<Q: Query> Attached<Q> {
    pub(crate) fn new(
        store: Arc<Store>,
        at: usize,
        key: ReadValue<Q::Key>,
        value: Option<ReadValue<Q::Value>>,
    ) -> Self {
        Self { store, at, key, value }
    }

    pub(crate) fn keys(&self) -> Option<&StoredKeys> {
        self.store.queries[self.at].keys(&self.store.stream)
    }

    /// The position of `key` among the saved keys, if it is one of them.
    pub(crate) fn find(&self, key: &Q::Key) -> Option<u32> {
        let at = *self.keys()?.index.get(&Stable::of(key))?;
        (self.key(at)? == *key).then_some(at)
    }

    /// The key saved at `at`, read from the file; `None` when it does not decode.
    pub(crate) fn key(&self, at: u32) -> Option<Q::Key> {
        let saved = self.keys()?.saved.get(at as usize)?;
        (self.key)(&self.store.stream, saved.key)
            .inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved key not read"))
            .ok()
    }

    /// The memo saved for the key at `at`, as never verified: its value and reads are read when
    /// first needed.
    pub(crate) fn memo(&self, at: u32) -> Option<Memo<Q>> {
        let key = self.keys()?.saved.get(at as usize)?;
        let memo = self.store.stream.read_blob::<SavedMemo<Blob>>(key.memo?);
        let memo =
            memo.inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved memo not read"));
        let memo = memo.ok()?;
        let saved = self.value.zip(memo.value).map(|(read, at)| Saved { at, read });
        Some(Memo::loaded(saved, memo.fingerprint.or(key.fingerprint)?, memo.deps.into()))
    }

    /// The slot given to the key at `at`, if one was.
    pub(crate) fn linked(&self, at: u32) -> Option<Slot> {
        self.keys()?.slot(at)
    }

    /// Records `slot` as the one given to the key at `at`.
    pub(crate) fn link(&self, at: u32, slot: Slot) {
        if let Some(cell) = self.keys().and_then(|keys| keys.slots.get(at as usize)) {
            cell.get_or_init(|| slot);
        }
    }
}
