//! The memo tables and the revision.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::cx::Cx;
use super::memo::Memo;
use super::persist::Codec;
use super::table::{Claim, Lookup, Map, Table, Waits};
use super::{Query, Revision};

/// Every query's memos, and the current revision. Threads share it, each through its own
/// [`Cx`]; reading a memo takes no lock.
#[derive(Default)]
pub struct Db {
    revision: AtomicU64,
    tables: Map<TypeId, Arc<dyn Any + Send + Sync>>,
    waits: Waits,
    /// The persisted queries.
    pub(crate) codecs: RwLock<HashMap<TypeId, Codec>>,
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
    pub fn set<Q: Query>(&self, key: Q::Key, value: Q::Value) {
        let hash = Q::interface(&value);
        let revision = match self.memo::<Q>(&key) {
            Some(current) if current.input && current.hash == hash => return,
            Some(_) => Revision(self.revision.fetch_add(1, Ordering::AcqRel) + 1),
            None => self.revision(),
        };
        self.store::<Q>(key, Memo::input(value, hash, revision));
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
        Arc::clone(resource).downcast::<T>().expect("one resource per type")
    }

    /// The value of `Q` at `key`, from a fresh context.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        Cx::new(self).get::<Q>(key)
    }

    /// Every key of `Q` computed or set so far, with its value: what inspection tools read,
    /// such as coverage, never a query, whose results must not depend on what ran before.
    #[must_use]
    pub fn computed<Q: Query>(&self) -> Vec<(Q::Key, Q::Value)> {
        self.memos::<Q>().into_iter().map(|(key, memo)| (key, memo.value.clone())).collect()
    }

    /// `f` of the table of `Q`, created empty on first use. Tables are never removed, so `f` may
    /// run as long as it takes, a whole computation included.
    fn with_table<Q: Query, R>(&self, f: impl FnOnce(&Table<Q>) -> R) -> R {
        let tables = self.tables.pin();
        let table = tables.get_or_insert_with(TypeId::of::<Q>(), || Arc::new(Table::<Q>::default()));
        f(table.downcast_ref().expect("one table type per query"))
    }

    pub(crate) fn memo<Q: Query>(&self, key: &Q::Key) -> Option<Arc<Memo<Q>>> {
        self.with_table::<Q, _>(|table| table.get(key))
    }

    /// The memo of `Q` at `key` as `revision` finds it.
    pub(crate) fn lookup<Q: Query>(&self, key: &Q::Key, revision: Revision) -> Option<Lookup<Q>> {
        self.with_table::<Q, _>(|table| table.lookup(key, revision))
    }

    /// Every memo of `Q`.
    pub(crate) fn memos<Q: Query>(&self) -> Vec<(Q::Key, Arc<Memo<Q>>)> {
        self.with_table::<Q, _>(Table::all)
    }

    pub(crate) fn store<Q: Query>(&self, key: Q::Key, memo: Memo<Q>) {
        self.with_table::<Q, _>(|table| table.insert(key, Arc::new(memo)));
    }

    /// Computes the memo of `Q` at `key` with `compute`, unless another thread is computing it:
    /// then waits for it and gives `None`, to be looked up again.
    pub(crate) fn computed_once<Q: Query>(
        &self,
        key: &Q::Key,
        compute: impl FnOnce() -> Memo<Q>,
    ) -> Option<Arc<Memo<Q>>> {
        self.with_table::<Q, _>(|table| {
            let owned = match table.claim(key, &self.waits) {
                Claim::Waited => return None,
                Claim::Owner(owned) => Some(owned),
                Claim::Alone => None,
            };
            let memo = Arc::new(compute());
            table.insert(key.clone(), Arc::clone(&memo));
            drop(owned);
            Some(memo)
        })
    }
}
