//! The saved memos loaded, read as they are needed: a query's keys when one of them is first
//! looked for, a key's value and memo when that key is first asked for, and a memo's reads when
//! it is first verified. What a run never asks for is never read, so loading costs what is used.
//!
//! A file layered over a file of shared memos is read with it, as one: a query's keys are that
//! file's, then its own, and the memos it replaces stand for that file's.

use std::sync::{Arc, OnceLock};

use vun_codec::{Blob, Context};
use vun_hash::{Hash, Stable};
use vun_trace::{Activity, tracing};

use super::keys::{KeyHead, PackedKeys, SavedKey};
use super::saved::{Growth, SavedDep, SavedMemo, SavedQuery, Under};
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

/// A query's saved keys, found by their hashes without reading the others, and the slot each was
/// given in this process.
pub(crate) struct StoredKeys {
    stream: Arc<Context>,
    /// The shared file's keys, and the file's own.
    base_keys: Option<Table>,
    own_keys: Option<Table>,
    /// How many keys are the shared file's.
    pub(crate) base: u32,
    len: u32,
    /// The positions of the shared file's keys whose memo the file layered over it replaces, in
    /// order, and the memo that stands for each.
    pub(crate) replaced: Vec<u32>,
    memos: Vec<Blob>,
    slots: Box<[OnceLock<Slot>]>,
}

/// One file's keys of a query, as places in the stream.
#[derive(Clone, Copy)]
struct Table {
    count: u32,
    starts: Blob,
    probe: Blob,
    data: Blob,
}

impl Table {
    fn of(keys: &PackedKeys<Blob>) -> Self {
        Self { count: keys.count, starts: keys.starts, probe: keys.table, data: keys.data }
    }

    /// Where key `at` starts, as a place in the stream.
    fn start(&self, stream: &Context, at: u32) -> Option<Blob> {
        let starts = stream.bytes(self.starts).get(at as usize * 4..)?.first_chunk::<4>()?;
        let start = u32::from_le_bytes(*starts) as usize;
        Some(Blob { at: self.data.at + start, len: self.data.len.checked_sub(start)? })
    }

    fn hash(&self, stream: &Context, at: u32) -> Option<u64> {
        let head: KeyHead = stream.front_blob(self.start(stream, at)?).ok()?;
        Some(head.hash.0)
    }

    fn get(&self, stream: &Context, at: u32) -> Option<SavedKey<Blob>> {
        stream.front_blob(self.start(stream, at)?).ok()
    }

    /// The position of the first key of `hash`.
    #[expect(clippy::cast_possible_truncation, reason = "only the low bits of a hash index the table")]
    fn find(&self, stream: &Context, hash: u64) -> Option<u32> {
        let table = stream.bytes(self.probe);
        let mask = table.len() / 4 - 1;
        let mut slot = hash as usize & mask;
        loop {
            let held = u32::from_le_bytes(*table.get(slot * 4..)?.first_chunk::<4>()?);
            let at = held.checked_sub(1)?;
            if self.hash(stream, at)? == hash {
                return Some(at);
            }
            slot = (slot + 1) & mask;
        }
    }
}

impl Store {
    /// The read `dep` saved, by slot.
    pub(crate) fn dep(&self, db: &Db, dep: SavedDep) -> Option<Dep> {
        let query = self.queries.get(dep.query as usize)?;
        let table = query.table?;
        let base = query.keys(&self.stream)?.head(dep.key)?.fingerprint;
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
            let len = keys.len();
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
    pub(crate) fn keys(&self, stream: &Arc<Context>) -> Option<&StoredKeys> {
        self.keys
            .get_or_init(|| {
                let _timer = KEYS.start();
                let read = |blob: Option<Blob>| blob.map(|blob| stream.read_blob::<SavedQuery<Blob>>(blob));
                let (base, own) = (read(self.layers.base).transpose(), read(self.layers.own).transpose());
                match base.and_then(|base| Ok((base, own?))) {
                    Ok((base, own)) => StoredKeys::new(stream, base, own),
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
    fn new(
        stream: &Arc<Context>,
        base: Option<SavedQuery<Blob>>,
        own: Option<SavedQuery<Blob>>,
    ) -> Option<Self> {
        let base = match base {
            Some(base) if !base.replaced.is_empty() => return None,
            Some(base) => Some(Table::of(&base.keys)),
            None => None,
        };
        let first = base.map_or(0, |table| table.count);
        let (mut replaced, mut memos, mut own_keys) = (Vec::new(), Vec::new(), None);
        let mut len = first;
        if let Some(own) = own {
            for replacing in own.replaced {
                (replacing.at < first).then_some(())?;
                replaced.push(replacing.at);
                memos.push(replacing.memo);
            }
            let table = Table::of(&own.keys);
            len += table.count;
            own_keys = Some(table);
        }
        let slots = std::iter::repeat_with(OnceLock::new).take(len as usize).collect();
        Some(Self {
            stream: Arc::clone(stream),
            base_keys: base,
            own_keys,
            base: first,
            len,
            replaced,
            memos,
            slots,
        })
    }

    /// How many keys there are.
    pub(crate) fn len(&self) -> u32 {
        self.len
    }

    /// The key at `at`, with the memo that stands for it.
    pub(crate) fn get(&self, at: u32) -> Option<SavedKey<Blob>> {
        let mut key = match at.checked_sub(self.base) {
            Some(own) => self.own_keys?.get(&self.stream, own)?,
            None => self.base_keys?.get(&self.stream, at)?,
        };
        if let Ok(replaced) = self.replaced.binary_search(&at) {
            key.memo = Some(self.memos[replaced]);
        }
        Some(key)
    }

    /// What leads the key at `at`.
    pub(crate) fn head(&self, at: u32) -> Option<KeyHead> {
        let (table, at) = match at.checked_sub(self.base) {
            Some(own) => (self.own_keys?, own),
            None => (self.base_keys?, at),
        };
        self.stream.front_blob(table.start(&self.stream, at)?).ok()
    }

    /// The position of the first key of `hash`, the shared file's before the file's own.
    pub(crate) fn find(&self, hash: u64) -> Option<u32> {
        let base = self.base_keys.and_then(|table| table.find(&self.stream, hash));
        base.or_else(|| Some(self.base + self.own_keys?.find(&self.stream, hash)?))
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
        let at = self.keys()?.find(Stable::of(key))?;
        (self.key(at)? == *key).then_some(at)
    }

    /// The key saved at `at`, read from the file; `None` when it does not decode.
    pub(crate) fn key(&self, at: u32) -> Option<Q::Key> {
        let saved = self.keys()?.get(at)?;
        let _timer = KEYS.start();
        (self.key)(&self.store.stream, saved.key)
            .inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved key not read"))
            .ok()
    }

    /// The memo saved for the key at `at`, as never verified: its value and reads are read when
    /// first needed.
    pub(crate) fn memo(&self, at: u32) -> Option<Memo<Q>> {
        let key = self.keys()?.get(at)?;
        let _timer = RESTORE.start();
        let memo = self.store.stream.read_blob::<SavedMemo<Blob>>(key.memo?);
        let memo =
            memo.inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved memo not read"));
        let memo = memo.ok()?;
        let saved = self.value.zip(memo.value).map(|(read, at)| Saved { at, read });
        let under = memo.under.and_then(|at| self.store.stream.read_blob::<Under>(at).ok());
        Some(Memo::loaded(saved, memo.fingerprint.or(key.fingerprint())?, memo.deps, under))
    }

    /// The slot of the key at `at`, set once it is given one.
    pub(crate) fn slot(&self, at: u32) -> Option<&OnceLock<Slot>> {
        self.keys()?.slots.get(at as usize)
    }
}
