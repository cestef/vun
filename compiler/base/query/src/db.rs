//! The database: a table per query, the current revision, and the resources queries share.

use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use vun_codec::Context;
use vun_par::Map;

use super::claim::Waits;
use super::cx::Cx;
use super::memo::Memo;
use super::persist::{Growth, Persisted};
use super::table::{AnyTable, Table, TableId};
use super::{Query, Revision};

/// Every query's memos, and the current revision. Threads share it, each through its own
/// [`Cx`]; reading a memo takes no lock.
#[derive(Default)]
pub struct Db {
    revision: AtomicU64,
    tables: boxcar::Vec<Box<dyn AnyTable>>,
    /// Each query's table, by the query's type.
    ids: Map<TypeId, TableId>,
    pub(crate) waits: Waits,
    /// The saved memos loaded, whose values are read from it when first needed.
    pub(crate) stream: OnceLock<Context>,
    /// How the stream loaded has grown since last written whole.
    pub(crate) growth: OnceLock<Growth>,
    /// Whether a memo was computed that the saved ones did not already hold.
    pub(crate) changed: AtomicBool,
    /// The persisted queries, in the order registered.
    pub(crate) persisted: RwLock<Vec<Arc<dyn Persisted>>>,
    resources: Map<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Db {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn revision(&self) -> Revision {
        Revision(self.revision.load(Ordering::Acquire))
    }

    /// Sets the input `key` of `Q`. A value whose interface differs from the current one starts
    /// a new revision; an equal one changes nothing, and neither does a key nothing has read.
    pub fn set<Q: Query>(&self, key: &Q::Key, value: Q::Value) {
        let fingerprint = Q::interface(&value);
        let table = self.table::<Q>();
        let entry = table.entry(table.slot(key));
        let revision = match &*entry.memo.load() {
            Some(current) if current.input && current.fingerprint == fingerprint => return,
            Some(_) => Revision(self.revision.fetch_add(1, Ordering::AcqRel) + 1),
            None => self.revision(),
        };
        entry.memo.store(Some(Arc::new(Memo::input(value, fingerprint, revision))));
    }

    /// The value of `Q` at `key`, from a fresh context.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        Cx::new(self).get::<Q>(key)
    }

    /// The shared table of type `T`, created empty on first use. Resources are append-only tables
    /// that every query adds to, such as origins and source files: interners, not inputs, so
    /// reading them records no dependency.
    ///
    /// # Panics
    /// Never: each type holds one resource of that type.
    #[must_use]
    pub fn resource<T: Default + Send + Sync + 'static>(&self) -> Arc<T> {
        let resources = self.resources.pin();
        let resource = resources.get_or_insert_with(TypeId::of::<T>(), || Arc::new(T::default()));
        Arc::clone(resource).downcast().expect("one resource per type")
    }

    /// Every key of `Q` computed or set so far, with its value: what inspection tools read,
    /// such as coverage, never a query, whose results must not depend on what ran before.
    #[must_use]
    pub fn computed<Q: Query>(&self) -> Vec<(Q::Key, Q::Value)> {
        self.table::<Q>()
            .memos()
            .filter_map(|(key, memo)| Some((key.clone(), memo.value(self)?.clone())))
            .collect()
    }

    /// The table of `Q`, made on first use.
    pub(crate) fn table<Q: Query>(&self) -> &Table<Q> {
        let id = self.table_of(TypeId::of::<Q>(), |id| Box::new(Table::<Q>::new(id)));
        self.any_table(id).as_any().downcast_ref().expect("one table type per query")
    }

    /// The table of the query of type `query`, made by `make` on first use. What does not depend
    /// on the query is kept out of the code made for each one.
    #[inline(never)]
    fn table_of(&self, query: TypeId, make: fn(TableId) -> Box<dyn AnyTable>) -> TableId {
        *self.ids.pin().get_or_insert_with(query, || {
            let index =
                self.tables.push_with(|index| make(TableId(u32::try_from(index).expect("few queries"))));
            TableId(u32::try_from(index).expect("few queries"))
        })
    }

    pub(crate) fn any_table(&self, id: TableId) -> &dyn AnyTable {
        &*self.tables[id.0 as usize]
    }
}
