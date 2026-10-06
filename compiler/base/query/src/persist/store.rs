//! The saved memos loaded, read as they are needed: a query's keys when one of them is first
//! looked for, a key's value and memo when that key is first asked for, and a memo's reads when
//! it is first verified. What a run never asks for is never read, so loading costs what is used.
//!
//! A file layered over a file of shared memos is read with it, as one: a query's keys are that
//! file's, then its own, and the memos it replaces stand for that file's.

use std::sync::{Arc, OnceLock};

use vun_codec::{Blob, Context};
use vun_hash::{Hash, Map, Stable};
use vun_trace::{Activity, tracing};

use super::saved::{Growth, SavedDep, SavedKey, SavedMemo, SavedQuery};
use crate::Query;
use crate::db::Db;
use crate::memo::{Dep, Memo, ReadValue, Saved};
use crate::table::{Slot, TableId};

/// Reading saved keys, a query's table of them and each key, as timed.
const KEYS: Activity = Activity::stage("read keys");
/// Reading a key's saved memo, as timed.
const RESTORE: Activity = Activity::stage("restore memos");

/// A file of saved memos, loaded, with the file of shared memos it is layered over.
pub(crate) struct Store {
    /// The file's stream, layered over the shared file's.
    pub(crate) stream: Arc<Context>,
    /// The file's own growth; `None` when only the shared file was loaded.
    pub(crate) growth: Option<Growth>,
    /// Each query of the files, by its position there.
    pub(crate) queries: Vec<StoredQuery>,
    /// The shared file the file is layered over.
    pub(crate) base: Option<Hash>,
}

/// A query of the files: its table, if a query of that name is registered, and its keys once
/// read.
pub(crate) struct StoredQuery {
    pub(crate) name: String,
    /// Its [`SavedQuery`] in the shared file, and in the file layered over it.
    pub(crate) layers: Layers,
    pub(crate) table: Option<TableId>,
    keys: OnceLock<Option<StoredKeys>>,
}

/// A query's part of the shared file, and of the file loaded: of that file alone when it is not
/// layered.
#[derive(Clone, Copy)]
pub(crate) struct Layers {
    pub(crate) base: Option<Blob>,
    pub(crate) own: Option<Blob>,
}

/// A query's saved keys, found by their hashes, and the slot each was given in this process.
pub(crate) struct StoredKeys {
    /// The shared file's keys, then the file's own, each with the memo that stands.
    pub(crate) saved: Vec<SavedKey<Blob>>,
    /// How many keys are the shared file's.
    pub(crate) base: u32,
    /// The positions of the shared file's keys whose memo the file layered over it replaces.
    pub(crate) replaced: Vec<u32>,
    /// The first position of each hash.
    index: Map<u64, u32>,
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
    /// whole holds. A file written whole over the shared file holds none of that file's keys but
    /// those whose memo it replaces.
    pub(crate) fn restore_all(&self, db: &Db, over_base: bool) {
        for query in &self.queries {
            let (Some(table), Some(keys)) = (query.table, query.keys(&self.stream)) else { continue };
            let table = db.any_table(table);
            let len = u32::try_from(keys.saved.len()).expect("fewer than 2^32 keys per query");
            let (first, replaced) = if over_base { (keys.base, &keys.replaced[..]) } else { (0, &[][..]) };
            for at in (first..len).chain(replaced.iter().copied()) {
                table.saved_slot(at);
            }
        }
    }
}

impl StoredQuery {
    pub(crate) fn new(name: String, layers: Layers, table: Option<TableId>) -> Self {
        Self { name, layers, table, keys: OnceLock::new() }
    }

    /// Its keys, read the first time; `None` when they do not decode.
    pub(crate) fn keys(&self, stream: &Context) -> Option<&StoredKeys> {
        self.keys
            .get_or_init(|| {
                let _timer = KEYS.start();
                let read = |blob: Option<Blob>| blob.map(|blob| stream.read_blob::<SavedQuery<Blob>>(blob));
                let (base, own) = (read(self.layers.base).transpose(), read(self.layers.own).transpose());
                match base.and_then(|base| Ok((base, own?))) {
                    Ok((base, own)) => StoredKeys::new(base, own),
                    Err(malformed) => {
                        tracing::debug!(query = self.name, %malformed, "saved keys not read");
                        None
                    }
                }
            })
            .as_ref()
    }
}

impl StoredKeys {
    /// The keys of the shared file's query and of the file's own; `None` when the shared file's
    /// replaces memos, or the file's replaces a key the shared file lacks.
    fn new(base: Option<SavedQuery<Blob>>, own: Option<SavedQuery<Blob>>) -> Option<Self> {
        let mut saved = Vec::new();
        if let Some(base) = base {
            if !base.replaced.is_empty() {
                return None;
            }
            saved = base.keys;
        }
        let first = u32::try_from(saved.len()).ok()?;
        let mut replaced = Vec::new();
        if let Some(own) = own {
            for replacing in own.replaced {
                (replacing.at < first).then_some(())?;
                saved[replacing.at as usize].memo = Some(replacing.memo);
                replaced.push(replacing.at);
            }
            saved.extend(own.keys);
        }
        let mut index = Map::default();
        index.reserve(saved.len());
        for (at, key) in (0..).zip(&saved) {
            index.entry(key.hash).or_insert(at);
        }
        let slots = std::iter::repeat_with(OnceLock::new).take(saved.len()).collect();
        Some(Self { saved, base: first, replaced, index, slots })
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
        let _timer = KEYS.start();
        (self.key)(&self.store.stream, saved.key)
            .inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved key not read"))
            .ok()
    }

    /// The memo saved for the key at `at`, as never verified: its value and reads are read when
    /// first needed.
    pub(crate) fn memo(&self, at: u32) -> Option<Memo<Q>> {
        let key = self.keys()?.saved.get(at as usize)?;
        let _timer = RESTORE.start();
        let memo = self.store.stream.read_blob::<SavedMemo<Blob>>(key.memo?);
        let memo =
            memo.inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved memo not read"));
        let memo = memo.ok()?;
        let saved = self.value.zip(memo.value).map(|(read, at)| Saved { at, read });
        Some(Memo::loaded(saved, memo.fingerprint.or(key.fingerprint)?, memo.deps, memo.under))
    }

    /// The slot of the key at `at`, set once it is given one.
    pub(crate) fn slot(&self, at: u32) -> Option<&OnceLock<Slot>> {
        self.keys()?.slots.get(at as usize)
    }
}
