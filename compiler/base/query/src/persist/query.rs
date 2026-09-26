//! How the memos of one persisted query are written and read back, whatever the query.

use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use vun_codec::{Blob, Context, Decode, Encode, Writer};
use vun_trace::tracing;

use super::saved::{SavedDep, SavedMemo, SavedQuery};
use crate::Query;
use crate::db::Db;
use crate::memo::{Dep, Memo, ReadValue, Saved};
use crate::table::{Slot, TableId};

pub(crate) trait Persisted: Send + Sync {
    fn name(&self) -> &'static str;

    fn query(&self) -> TypeId;

    fn table(&self, db: &Db) -> TableId;

    /// Every key with a slot, and the computed memos whose reads are all of persisted tables,
    /// given by their positions in the file, into a stream that `extends` the one loaded or
    /// starts anew.
    fn save(
        &self,
        db: &Db,
        w: &mut Writer<'_>,
        positions: &HashMap<TableId, u32>,
        extends: Option<&Context>,
    ) -> SavedQuery;

    /// The slot of a saved key; `None` for one that does not decode.
    fn slot(&self, db: &Db, stream: &Context, key: Blob) -> Option<Slot>;

    /// Stores `memos`, whose keys are at `slots`, as never verified, returning how many.
    fn restore(&self, db: &Db, memos: Vec<SavedMemo<Blob>>, slots: &[Option<Slot>], reads: &Reads) -> usize;
}

/// The persisted query `Q`, with its values or without.
pub(crate) struct Persisting<Q: Query> {
    values: Option<Values<Q>>,
    query: PhantomData<fn() -> Q>,
}

/// How values of `Q` are written and read, for a query persisted with them.
struct Values<Q: Query> {
    write: fn(&mut Writer<'_>, &Q::Value) -> Vec<u8>,
    read: ReadValue<Q::Value>,
}

/// The table and slots of each query of a file, by the query's position.
pub(crate) struct Reads(pub(super) Vec<Option<(TableId, Vec<Option<Slot>>)>>);

impl Reads {
    fn dep(&self, saved: SavedDep) -> Option<Dep> {
        let (table, slots) = self.0.get(saved.query as usize)?.as_ref()?;
        let slot = (*slots.get(saved.key as usize)?)?;
        Some(Dep { table: *table, slot, fingerprint: saved.fingerprint })
    }
}

impl<Q: Query> Persisting<Q> {
    pub(crate) fn trace() -> Self {
        Self { values: None, query: PhantomData }
    }

    pub(crate) fn values() -> Self
    where
        Q::Value: Encode + Decode,
    {
        let values = Values { write: |w, value| w.encode(value), read: Context::read_blob };
        Self { values: Some(values), query: PhantomData }
    }
}

impl<Q: Query<Key: Encode + Decode>> Persisted for Persisting<Q> {
    fn name(&self) -> &'static str {
        Q::NAME
    }

    fn query(&self) -> TypeId {
        TypeId::of::<Q>()
    }

    fn table(&self, db: &Db) -> TableId {
        db.table::<Q>().id
    }

    fn save(
        &self,
        db: &Db,
        w: &mut Writer<'_>,
        positions: &HashMap<TableId, u32>,
        extends: Option<&Context>,
    ) -> SavedQuery {
        let table = db.table::<Q>();
        let keys = table.entries().map(|(_, entry)| w.encode(&entry.key)).collect();
        let saved_dep = |dep: &Dep| {
            let Some(&query) = positions.get(&dep.table) else {
                let read = db.any_table(dep.table).name();
                tracing::debug!(query = Q::NAME, read, "not saved: reads a query not persisted");
                return None;
            };
            let key = u32::try_from(dep.slot.index()).expect("fewer than 2^32 keys per query");
            Some(SavedDep { query, key, fingerprint: dep.fingerprint })
        };
        let memos = table
            .entries()
            .filter_map(|(slot, entry)| {
                let memo = entry.memo.load_full().filter(|memo| !memo.input)?;
                let deps = memo.deps.iter().map(saved_dep).collect::<Option<_>>()?;
                // A value never read in this process is written as it was read into a stream
                // extending the one loaded, and read to be written anew into a new one.
                let value = self.values.as_ref().and_then(|values| match (memo.known(), extends) {
                    (Some(value), _) => Some((values.write)(w, value)),
                    (None, Some(stream)) => Some(stream.bytes(memo.saved.as_ref()?.at).to_vec()),
                    (None, None) => memo.value(db).map(|value| (values.write)(w, value)),
                });
                let key = u32::try_from(slot.index()).expect("fewer than 2^32 keys per query");
                Some(SavedMemo { key, value, fingerprint: memo.fingerprint, deps })
            })
            .collect();
        SavedQuery { name: Q::NAME.to_owned(), keys, memos }
    }

    fn slot(&self, db: &Db, stream: &Context, key: Blob) -> Option<Slot> {
        match stream.read_blob(key) {
            Ok(key) => Some(db.table::<Q>().slot(&key)),
            Err(malformed) => {
                tracing::debug!(query = Q::NAME, %malformed, "saved key not read");
                None
            }
        }
    }

    fn restore(&self, db: &Db, memos: Vec<SavedMemo<Blob>>, slots: &[Option<Slot>], reads: &Reads) -> usize {
        let table = db.table::<Q>();
        let restored = memos.into_iter().filter_map(|memo| {
            let slot = (*slots.get(memo.key as usize)?)?;
            let deps = memo.deps.iter().map(|&dep| reads.dep(dep)).collect::<Option<_>>()?;
            let saved =
                self.values.as_ref().zip(memo.value).map(|(values, at)| Saved { at, read: values.read });
            let entry = table.entry(slot);
            // A memo of this process stands: it is at least as recent.
            if entry.memo.load().is_none() {
                entry.memo.store(Some(Arc::new(Memo::loaded(saved, memo.fingerprint, deps))));
            }
            Some(())
        });
        restored.count()
    }
}
