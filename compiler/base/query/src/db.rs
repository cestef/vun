//! The memo tables and the revision.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use vun_hash::Hash;

use super::cx::{Cx, Edge};
use super::persist::Codec;
use super::{Query, Revision};

/// A memoized result: the value, its interface hash, the trace of what computing it read, and
/// the revision it was last known current in, if any: a loaded memo has none until verified.
pub(crate) struct Memo<Q: Query> {
    pub(crate) value: Q::Value,
    pub(crate) hash: Hash,
    pub(crate) deps: Arc<[Arc<dyn Edge>]>,
    pub(crate) verified: Option<Revision>,
    /// Set with [`Db::set`] rather than computed.
    pub(crate) input: bool,
}

impl<Q: Query> Clone for Memo<Q> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            hash: self.hash,
            deps: Arc::clone(&self.deps),
            verified: self.verified,
            input: self.input,
        }
    }
}

type Table<Q> = HashMap<<Q as Query>::Key, Memo<Q>>;

/// Every query's memos, and the current revision.
#[derive(Default)]
pub struct Db {
    revision: AtomicU64,
    tables: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    /// The persisted queries.
    pub(crate) codecs: RwLock<HashMap<TypeId, Codec>>,
    resources: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
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
    /// a new revision; an equal one changes nothing.
    ///
    /// # Panics
    /// Panics if the tables were poisoned by a panicking query.
    pub fn set<Q: Query>(&self, key: Q::Key, value: Q::Value) {
        let hash = Q::interface(&value);
        if self.memo::<Q>(&key).is_some_and(|m| m.input && m.hash == hash) {
            return;
        }
        let revision = Revision(self.revision.fetch_add(1, Ordering::AcqRel) + 1);
        let memo = Memo::<Q> { value, hash, deps: Arc::new([]), verified: Some(revision), input: true };
        self.store(key, memo);
    }

    /// The shared table of type `T`, created empty on first use. Resources are append-only tables
    /// that every query adds to, such as origins and source files: interners, not inputs, so
    /// reading them records no dependency.
    ///
    /// # Panics
    /// Panics if the resources were poisoned by a panicking query.
    #[must_use]
    pub fn resource<T: Default + Send + Sync + 'static>(&self) -> Arc<T> {
        let found = self.resources.read().expect("query resources").get(&TypeId::of::<T>()).cloned();
        let resource = found.unwrap_or_else(|| {
            let mut resources = self.resources.write().expect("query resources");
            let entry = resources.entry(TypeId::of::<T>()).or_insert_with(|| Arc::new(T::default()));
            Arc::clone(entry)
        });
        resource.downcast::<T>().expect("one resource per type")
    }

    /// The value of `Q` at `key`, from a fresh context.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        Cx::new(self).get::<Q>(key)
    }

    /// Every key of `Q` computed or set so far, with its value: what inspection tools read,
    /// such as coverage, never a query, whose results must not depend on what ran before.
    ///
    /// # Panics
    /// Panics if the tables were poisoned by a panicking query.
    #[must_use]
    pub fn computed<Q: Query>(&self) -> Vec<(Q::Key, Q::Value)> {
        self.memos::<Q>().into_iter().map(|(key, memo)| (key, memo.value)).collect()
    }

    pub(crate) fn memo<Q: Query>(&self, key: &Q::Key) -> Option<Memo<Q>> {
        let tables = self.tables.read().expect("query tables");
        let table = tables.get(&TypeId::of::<Q>())?.downcast_ref::<Table<Q>>()?;
        table.get(key).cloned()
    }

    /// Every memo of `Q`.
    pub(crate) fn memos<Q: Query>(&self) -> Vec<(Q::Key, Memo<Q>)> {
        let tables = self.tables.read().expect("query tables");
        let table = tables.get(&TypeId::of::<Q>()).and_then(|t| t.downcast_ref::<Table<Q>>());
        table.map(|t| t.iter().map(|(k, m)| (k.clone(), m.clone())).collect()).unwrap_or_default()
    }

    pub(crate) fn store<Q: Query>(&self, key: Q::Key, memo: Memo<Q>) {
        let mut tables = self.tables.write().expect("query tables");
        let table = tables.entry(TypeId::of::<Q>()).or_insert_with(|| Box::new(Table::<Q>::new()));
        table.downcast_mut::<Table<Q>>().expect("one table type per query").insert(key, memo);
    }

    pub(crate) fn verified<Q: Query>(&self, key: &Q::Key, revision: Revision) {
        let mut tables = self.tables.write().expect("query tables");
        let table = tables.get_mut(&TypeId::of::<Q>()).and_then(|t| t.downcast_mut::<Table<Q>>());
        if let Some(memo) = table.and_then(|t| t.get_mut(key)) {
            memo.verified = Some(revision);
        }
    }
}
