//! How the memos of one persisted query are written and read back, whatever the query.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use vun_codec::{Context, Decode, Encode, Malformed, Writer};

use super::saved::{SavedDep, SavedMemo, SavedQuery};
use crate::Query;
use crate::db::Db;
use crate::memo::{Dep, Memo, Verified};
use crate::table::{Slot, TableId};

pub(crate) trait Persisted: Send + Sync {
    fn name(&self) -> &'static str;

    fn table(&self, db: &Db) -> TableId;

    /// Every key with a slot, and the computed memos whose reads are all of persisted tables,
    /// given by their positions in the file.
    fn save(&self, db: &Db, w: &mut Writer<'_>, positions: &HashMap<TableId, u32>) -> SavedQuery;

    /// The slot of each saved key; `None` for one that does not decode.
    fn slots(&self, db: &Db, cx: &Context<'_>, keys: &[Vec<u8>]) -> Vec<Option<Slot>>;

    /// Stores `memos`, whose keys are at `slots`, as never verified, returning how many.
    fn restore(
        &self,
        db: &Db,
        cx: &Context<'_>,
        memos: &[SavedMemo],
        slots: &[Option<Slot>],
        reads: &Reads,
    ) -> usize;
}

/// The persisted query `Q`, with its values or without.
pub(crate) struct Persisting<Q: Query> {
    values: Option<Values<Q>>,
    query: PhantomData<fn() -> Q>,
}

/// How values of `Q` are written and read, for a query persisted with them.
struct Values<Q: Query> {
    encode: fn(&mut Writer<'_>, &Q::Value) -> Vec<u8>,
    decode: fn(&Context<'_>, &[u8]) -> Result<Q::Value, Malformed>,
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
        let values = Values { encode: |w, value| w.encode(value), decode: |cx, bytes| cx.read(bytes) };
        Self { values: Some(values), query: PhantomData }
    }
}

impl<Q: Query<Key: Encode + Decode>> Persisted for Persisting<Q> {
    fn name(&self) -> &'static str {
        Q::NAME
    }

    fn table(&self, db: &Db) -> TableId {
        db.table::<Q>().id
    }

    fn save(&self, db: &Db, w: &mut Writer<'_>, positions: &HashMap<TableId, u32>) -> SavedQuery {
        let table = db.table::<Q>();
        let keys = table.entries().map(|(_, entry)| w.encode(&entry.key)).collect();
        let saved_dep = |dep: &Dep| {
            let query = *positions.get(&dep.table)?;
            let key = u32::try_from(dep.slot.index()).expect("fewer than 2^32 keys per query");
            Some(SavedDep { query, key, fingerprint: dep.fingerprint })
        };
        let memos = table
            .entries()
            .filter_map(|(slot, entry)| {
                let memo = entry.memo.load_full().filter(|memo| !memo.input)?;
                let deps = memo.deps.iter().map(saved_dep).collect::<Option<_>>()?;
                let value = self.values.as_ref().and_then(|v| Some((v.encode)(w, memo.value.as_ref()?)));
                let key = u32::try_from(slot.index()).expect("fewer than 2^32 keys per query");
                Some(SavedMemo { key, value, fingerprint: memo.fingerprint, deps })
            })
            .collect();
        SavedQuery { name: Q::NAME.to_owned(), keys, memos }
    }

    fn slots(&self, db: &Db, cx: &Context<'_>, keys: &[Vec<u8>]) -> Vec<Option<Slot>> {
        let table = db.table::<Q>();
        keys.iter().map(|key| Some(table.slot(&cx.read(key).ok()?))).collect()
    }

    fn restore(
        &self,
        db: &Db,
        cx: &Context<'_>,
        memos: &[SavedMemo],
        slots: &[Option<Slot>],
        reads: &Reads,
    ) -> usize {
        let table = db.table::<Q>();
        let restored = memos.iter().filter_map(|memo| {
            let slot = (*slots.get(memo.key as usize)?)?;
            let value = match (&self.values, &memo.value) {
                (Some(values), Some(bytes)) => Some((values.decode)(cx, bytes).ok()?),
                _ => None,
            };
            let deps = memo.deps.iter().map(|&dep| reads.dep(dep)).collect::<Option<_>>()?;
            let entry = table.entry(slot);
            // A memo of this process stands: it is at least as recent.
            if entry.memo.load().is_none() {
                entry.memo.store(Some(Arc::new(Memo::computed(
                    value,
                    memo.fingerprint,
                    deps,
                    Verified::default(),
                ))));
            }
            Some(())
        });
        restored.count()
    }
}
