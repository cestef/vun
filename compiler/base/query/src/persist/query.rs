//! How the memos of one persisted query are written and read back, whatever the query.

use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

use vun_codec::{Context, Decode, Encode, Writer};
use vun_hash::{Map, Stable};
use vun_trace::tracing;

use super::Keeps;
use super::load::Taking;
use super::saved::{Replaced, SavedDep, SavedKey, SavedMemo, SavedQuery};
use super::shared::Included;
use super::store::{Attached, Store};
use crate::db::{Db, Sharing};
use crate::fingerprint::Fingerprint;
use crate::memo::{Dep, Deps, Memo, ReadValue};
use crate::table::{AnyTable, TableId};
use crate::{Query, Revision};

pub(crate) trait Persisted: Send + Sync {
    fn name(&self) -> &'static str;

    fn query(&self) -> TypeId;

    fn table(&self, db: &Db) -> TableId;

    /// Attaches the query at `at` in `store`, the saved memos loaded, to the query's table.
    fn attach(&self, db: &Db, store: &Arc<Store>, at: usize, taking: Taking<'_>);

    /// Each slot's place in the file saved, by slot: the position a key has in the files loaded
    /// when the file saved `keeps` it, the next one after those kept otherwise; none for a slot
    /// outside `include`, when the file holds only those.
    fn number(&self, db: &Db, keeps: Keeps<'_>, include: Option<&Included>) -> Vec<Option<Number>>;

    /// Every key but those left to the shared file, with its memo if it is computed and reads
    /// persisted queries alone, and the memos of those left that this process computed or the
    /// file loaded replaced. A stream extending the one loaded keeps every key it holds, as it
    /// was unless this process gave it a slot.
    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, keeps: Keeps<'_>) -> SavedQuery;
}

/// A key's place in the file saved: its position among its query's keys, and the fingerprint
/// its memo and reads of it are saved relative to.
#[derive(Clone, Copy)]
pub(crate) struct Number {
    pub(crate) at: u32,
    base: Option<Fingerprint>,
}

/// Each persisted table's keys' places in the file saved, by slot; none for a key it leaves out.
pub(crate) type Numbers = Map<TableId, Vec<Option<Number>>>;

/// Where each persisted table's keys go in the file saved.
pub(crate) struct Layout {
    /// Each table's query's position among the queries.
    pub(super) positions: Map<TableId, u32>,
    pub(super) numbers: Numbers,
    /// The revision saved, and the fingerprint of the inputs shared as far as each level in it,
    /// which a memo of that level holding in it is saved as holding under.
    pub(super) revision: Revision,
    pub(super) shared: [Fingerprint; Sharing::LEVELS.len()],
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

    fn attach(&self, db: &Db, store: &Arc<Store>, at: usize, taking: Taking<'_>) {
        let value = self.values.as_ref().map(|values| values.read);
        db.table::<Q>().attach(Attached::new(Arc::clone(store), at, Context::read_blob, value), taking);
    }

    fn number(&self, db: &Db, keeps: Keeps<'_>, include: Option<&Included>) -> Vec<Option<Number>> {
        let table = db.table::<Q>();
        let loaded = keeps.store().and(table.attached.get()).and_then(Attached::keys);
        let kept = loaded.map_or(0, |keys| keeps.kept(keys));
        let mut next = kept;
        let mut numbers = vec![None; table.len()];
        for (slot, entry) in table.entries() {
            if include.is_some_and(|include| !include.contains(&(table.id, slot))) {
                continue;
            }
            numbers[slot.index()] = Some(match (loaded, entry.at.get()) {
                (Some(keys), Some(&at)) if at < kept => {
                    Number { at, base: keys.saved[at as usize].fingerprint }
                }
                _ => {
                    next += 1;
                    Number { at: next - 1, base: entry.memo.load().as_ref().map(|memo| memo.fingerprint) }
                }
            });
        }
        numbers
    }

    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, keeps: Keeps<'_>) -> SavedQuery {
        let table = db.table::<Q>();
        let numbers = &layout.numbers[&table.id];
        let loaded = keeps.store().and(table.attached.get()).and_then(Attached::keys);
        let first = loaded.map_or(0, |keys| keeps.first(keys));
        let extends = keeps.extends();
        let mut keys: Vec<Option<SavedKey>> = Vec::new();
        let mut replaced: Map<u32, Vec<u8>> = Map::default();
        if let (Some(store), Some(loaded)) = (extends, loaded) {
            let copy = |blob| store.stream.bytes(blob).to_vec();
            let saved = loaded.saved[first as usize..].iter().map(|key| SavedKey {
                key: copy(key.key),
                hash: key.hash,
                fingerprint: key.fingerprint,
                memo: key.memo.map(copy),
            });
            keys = saved.map(Some).collect();
            let memos = loaded.replaced.iter().map(|&at| (at, loaded.saved[at as usize].memo));
            replaced = memos.filter_map(|(at, memo)| Some((at, copy(memo?)))).collect();
        }
        let len = numbers.iter().flatten().map(|number| number.at + 1).max().unwrap_or(0);
        keys.resize_with(keys.len().max(len.saturating_sub(first) as usize), || None);
        for (slot, entry) in table.entries() {
            let Some(&Some(number)) = numbers.get(slot.index()) else { continue };
            let memo = entry.memo.load_full();
            if number.at < first {
                // A key left to the shared file: its memo there stands unless replaced.
                let replacing = memo.as_deref().is_some_and(Memo::fresh)
                    || loaded.is_some_and(|keys| keys.replaced.binary_search(&number.at).is_ok());
                if let Some(memo) = memo.as_deref().filter(|_| replacing)
                    && let Some(bytes) = self.memo(db, w, layout, extends, number, Some(memo))
                {
                    replaced.insert(number.at, bytes);
                }
                continue;
            }
            let memo = self.memo(db, w, layout, extends, number, memo.as_deref());
            // A key loaded is kept as it was, with the memo it has now.
            let key = keys[(number.at - first) as usize].get_or_insert_with(|| SavedKey {
                key: w.encode(table.key(slot)),
                hash: Stable::of(table.key(slot)),
                fingerprint: number.base,
                memo: None,
            });
            key.memo = memo;
        }
        let mut replaced: Vec<Replaced> =
            replaced.into_iter().map(|(at, memo)| Replaced { at, memo }).collect();
        replaced.sort_unstable_by_key(|replaced| replaced.at);
        SavedQuery { keys: keys.into_iter().flatten().collect(), replaced }
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
        let memo = memo.filter(|memo| !memo.is_input())?;
        // Reads as loaded keep their positions in a stream extending the one they were read from.
        let deps = match (&memo.deps, extends) {
            (Deps::Saved { at, .. }, Some(store)) => store.stream.bytes(*at).to_vec(),
            (deps, _) => {
                let deps = deps.get(db)?.iter().map(|dep| layout.dep(db, dep));
                deps.collect::<Option<Vec<SavedDep>>>()?.encoded()
            }
        };
        // A value never read in this process is written as it was read into a stream extending
        // the one loaded, and read to be written anew into a new one.
        let value = self.values.as_ref().and_then(|values| match (memo.known(), extends) {
            (Some(value), _) => Some((values.write)(w, value)),
            (None, Some(store)) => Some(store.stream.bytes(memo.saved.as_ref()?.at).to_vec()),
            (None, None) => memo.value(db).map(|value| (values.write)(w, value)),
        });
        let fingerprint = (number.base != Some(memo.fingerprint)).then_some(memo.fingerprint);
        let level = memo.durability.level;
        let now = level.level().filter(|_| memo.current(layout.revision)).map(|at| layout.shared[at]);
        let under = now.or(memo.durability.under).map(|under| (level, under));
        Some(SavedMemo { fingerprint, value, deps, under }.encoded())
    }
}
