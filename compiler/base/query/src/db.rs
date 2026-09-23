//! The memo tables and the revision.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use overt_hash::Hash;

use super::cx::{Cx, Edge};
use super::{Query, Revision};

/// A memoized result: the value, its interface hash, the trace of what computing it read, and
/// the revision it was last known current in.
pub(crate) struct Memo<Q: Query> {
    pub(crate) value: Q::Value,
    pub(crate) hash: Hash,
    pub(crate) deps: Arc<[Arc<dyn Edge>]>,
    pub(crate) verified: Revision,
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
        let memo = Memo::<Q> { value, hash, deps: Arc::new([]), verified: revision, input: true };
        self.store(key, memo);
    }

    /// The value of `Q` at `key`, from a fresh context.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        Cx::new(self).get::<Q>(key)
    }

    pub(crate) fn memo<Q: Query>(&self, key: &Q::Key) -> Option<Memo<Q>> {
        let tables = self.tables.read().expect("query tables");
        let table = tables.get(&TypeId::of::<Q>())?.downcast_ref::<Table<Q>>()?;
        table.get(key).cloned()
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
            memo.verified = revision;
        }
    }
}
