//! How the memos of one persisted query are written and read back, whatever the query.

use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

use vun_codec::{Context, Decode, Encode, Writer};
use vun_hash::{Map, Stable};
use vun_trace::tracing;

use super::saved::{SavedDep, SavedKey, SavedMemo, SavedQuery};
use super::shared::Included;
use super::store::{Attached, Store};
use crate::Query;
use crate::db::Db;
use crate::fingerprint::Fingerprint;
use crate::memo::{Dep, Deps, Memo, ReadValue};
use crate::table::TableId;

pub(crate) trait Persisted: Send + Sync {
    fn name(&self) -> &'static str;

    fn query(&self) -> TypeId;

    fn table(&self, db: &Db) -> TableId;

    /// Attaches the query at `at` in `store`, the saved memos loaded, to the query's table.
    fn attach(&self, db: &Db, store: &Arc<Store>, at: usize);

    /// Each slot's place in the file saved, by slot: the position a key has in the file loaded
    /// when the file saved `extends` it, the next one after them otherwise; none for a slot
    /// outside `include`, when the file holds only those.
    fn number(&self, db: &Db, extends: bool, include: Option<&Included>) -> Vec<Option<Number>>;

    /// Every key, with its memo if it is computed and reads persisted queries alone, into a
    /// stream that `extends` the one loaded or starts anew. A stream extending the one loaded
    /// keeps every key it holds, as it was unless this process gave it a slot.
    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, extends: Option<&Store>) -> SavedQuery;
}

/// A key's place in the file saved: its position among its query's keys, and the fingerprint
/// its memo and reads of it are saved relative to.
#[derive(Clone, Copy)]
pub(crate) struct Number {
    at: u32,
    base: Option<Fingerprint>,
}

/// Where each persisted table's keys go in the file saved.
pub(crate) struct Layout {
    /// Each table's query's position among the queries.
    pub(super) positions: Map<TableId, u32>,
    /// Each table's keys, by slot; none for a key the file leaves out.
    pub(super) numbers: Map<TableId, Vec<Option<Number>>>,
}

impl Layout {
    /// `dep` as saved; `None` when it reads a query not persisted.
    fn dep(&self, db: &Db, dep: &Dep) -> Option<SavedDep> {
        let (Some(&query), Some(numbers)) = (self.positions.get(&dep.table), self.numbers.get(&dep.table))
        else {
            tracing::debug!(read = db.any_table(dep.table).name(), "not saved: reads a query not persisted");
            return None;
        };
        let Some(Some(number)) = numbers.get(dep.slot.index()) else {
            tracing::debug!(read = db.any_table(dep.table).name(), "not saved: reads a key left out");
            return None;
        };
        let fingerprint = (number.base != Some(dep.fingerprint)).then_some(dep.fingerprint);
        Some(SavedDep { query, key: number.at, fingerprint })
    }
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

    fn attach(&self, db: &Db, store: &Arc<Store>, at: usize) {
        let value = self.values.as_ref().map(|values| values.read);
        db.table::<Q>().attach(Attached::new(Arc::clone(store), at, Context::read_blob, value));
    }

    fn number(&self, db: &Db, extends: bool, include: Option<&Included>) -> Vec<Option<Number>> {
        let table = db.table::<Q>();
        let loaded = table.attached.get().filter(|_| extends).and_then(Attached::keys);
        let mut next = loaded.map_or(0, |keys| keys.saved.len());
        let numbers = table.entries().map(|(slot, entry)| {
            if include.is_some_and(|include| !include.contains(&(table.id, slot))) {
                return None;
            }
            if let (Some(keys), Some(&at)) = (loaded, entry.at.get()) {
                return Some(Number { at, base: keys.saved[at as usize].fingerprint });
            }
            let at = u32::try_from(next).expect("fewer than 2^32 keys per query");
            next += 1;
            Some(Number { at, base: entry.memo.load().as_ref().map(|memo| memo.fingerprint) })
        });
        numbers.collect()
    }

    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, extends: Option<&Store>) -> SavedQuery {
        let table = db.table::<Q>();
        let numbers = &layout.numbers[&table.id];
        let loaded = extends.zip(table.attached.get().and_then(Attached::keys));
        let mut keys: Vec<Option<SavedKey>> = loaded.map_or_else(Vec::new, |(store, keys)| {
            let copy = |blob| store.stream.bytes(blob).to_vec();
            let saved = keys.saved.iter().map(|key| SavedKey {
                key: copy(key.key),
                hash: key.hash,
                fingerprint: key.fingerprint,
                memo: key.memo.map(copy),
            });
            saved.map(Some).collect()
        });
        let len = numbers.iter().flatten().map(|number| number.at as usize + 1).max().unwrap_or(0);
        keys.resize_with(keys.len().max(len), || None);
        for (slot, entry) in table.entries() {
            let Some(&Some(number)) = numbers.get(slot.index()) else { continue };
            let memo = self.memo(db, w, layout, extends, number, entry.memo.load_full().as_deref());
            // A key loaded is kept as it was, with the memo it has now.
            let key = keys[number.at as usize].get_or_insert_with(|| SavedKey {
                key: w.encode(&entry.key),
                hash: Stable::of(&entry.key),
                fingerprint: number.base,
                memo: None,
            });
            key.memo = memo;
        }
        SavedQuery { keys: keys.into_iter().flatten().collect() }
    }
}

impl<Q: Query<Key: Encode + Decode>> Persisting<Q> {
    /// `memo` as saved at `number`, if it is computed and reads persisted queries alone.
    fn memo(
        &self,
        db: &Db,
        w: &mut Writer<'_>,
        layout: &Layout,
        extends: Option<&Store>,
        number: Number,
        memo: Option<&Memo<Q>>,
    ) -> Option<Vec<u8>> {
        let memo = memo.filter(|memo| !memo.input)?;
        // Reads as loaded keep their positions in a stream extending the one they were read from.
        let deps = match (&memo.deps, extends) {
            (Deps::Saved { saved, .. }, Some(_)) => saved.to_vec(),
            (deps, _) => deps.get(db)?.iter().map(|dep| layout.dep(db, dep)).collect::<Option<_>>()?,
        };
        // A value never read in this process is written as it was read into a stream extending
        // the one loaded, and read to be written anew into a new one.
        let value = self.values.as_ref().and_then(|values| match (memo.known(), extends) {
            (Some(value), _) => Some((values.write)(w, value)),
            (None, Some(store)) => Some(store.stream.bytes(memo.saved.as_ref()?.at).to_vec()),
            (None, None) => memo.value(db).map(|value| (values.write)(w, value)),
        });
        let fingerprint = (number.base != Some(memo.fingerprint)).then_some(memo.fingerprint);
        Some(SavedMemo { fingerprint, value, deps }.encoded())
    }
}
