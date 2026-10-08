//! The database: a table per query, the current revision, and the resources queries share.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use vun_codec::{Decode, Encode};
use vun_par::Map;

use super::claim::Waits;
use super::cx::{Cx, Found, Reuse};
use super::fingerprint::Fingerprint;
use super::groups::Groups;
use super::memo::{Dep, Memo, Source};
use super::persist::{Persisted, Store};
use super::table::{AnyTable, Slot, Table, TableId};
use super::{Query, Revision};

/// A database's number, unique in the process.
struct DbId(u64);

impl Default for DbId {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

thread_local! {
    /// The table of each query in each database this thread has read, found without the shared
    /// map every thread reads.
    static TABLES: RefCell<vun_hash::Map<(u64, TypeId), TableId>> =
        RefCell::default();
}

/// How far an input is the same in other databases, so that what reads such inputs alone is
/// shared with them; ordered from the least shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum Sharing {
    /// This database's own.
    Own,
    /// Seen alike by other databases that chose alike, as the target and the checks are.
    Chosen,
    /// Seen alike by every other database, as a library's sources are.
    Shared,
}

/// Every query's memos, and the current revision. Threads share it, each through its own
/// [`Cx`]; reading a memo takes no lock.
#[derive(Default)]
pub struct Db {
    /// Distinguishes it from every other database of the process, for the tables each thread
    /// remembers.
    id: DbId,
    revision: AtomicU64,
    pub(crate) tables: boxcar::Vec<Box<dyn AnyTable>>,
    /// Each query's table, by the query's type.
    ids: Map<TypeId, TableId>,
    pub(crate) waits: Waits,
    /// The saved memos loaded, restored as their keys are first asked for.
    pub(crate) store: OnceLock<Arc<Store>>,
    /// The query of the first memo computed that the saved ones did not already hold.
    pub(crate) changed: OnceLock<&'static str>,
    /// Whether a memo was computed since the shared memos were last saved.
    pub(crate) unshared: AtomicBool,
    /// The persisted queries, in the order registered.
    pub(crate) persisted: RwLock<Vec<Arc<dyn Persisted>>>,
    /// Whether memos reused without being computed are checked by computing them again.
    pub(crate) reuse: Reuse,
    /// What verifying ahead found of each memo it looked at, by the revision it looked in.
    pub(crate) settled: Map<(TableId, Slot, Revision), Found>,
    /// The fingerprint of the shared inputs of each group, by its identity ([`Db::inputs_of`]).
    pub(crate) groups: Map<Fingerprint, Fingerprint>,
    /// The lists of memos read that memos hold, each once: queries computed alike read alike.
    lists: Map<Arc<[Dep]>, ()>,
    resources: Map<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Db {
    /// What names this database among the others of the process.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id.0
    }

    /// A database reusing memos as the environment asks ([`Reuse::from_env`]).
    #[must_use]
    pub fn new() -> Self {
        Self::reusing(Reuse::from_env())
    }

    #[must_use]
    pub fn reusing(reuse: Reuse) -> Self {
        Self { reuse, ..Self::default() }
    }

    /// `reads` held once however many memos read them.
    pub(crate) fn list(&self, reads: &[Dep]) -> Arc<[Dep]> {
        let lists = self.lists.pin();
        if let Some((held, ())) = lists.get_key_value(reads) {
            return held.clone();
        }
        let list: Arc<[Dep]> = Arc::from(reads);
        lists.insert(list.clone(), ());
        list
    }

    #[must_use]
    pub fn revision(&self) -> Revision {
        Revision(self.revision.load(Ordering::Acquire))
    }

    /// Sets the input `key` of `Q`. A value whose interface differs from the current one starts
    /// a new revision; an equal one changes nothing, and neither does a key nothing has read.
    pub fn set<Q: Query>(&self, key: &Q::Key, value: Q::Value) {
        self.set_as::<Q>(key, value, Sharing::Own);
    }

    /// Sets the input `key` of `Q` as [`Db::set`] does, as one that other databases see alike: a
    /// memo reading such inputs alone is shared with them ([`Db::save_shared`]).
    pub fn share<Q: Query>(&self, key: &Q::Key, value: Q::Value) {
        self.set_as::<Q>(key, value, Sharing::Shared);
    }

    /// Sets the input `key` of `Q` as [`Db::set`] does, shared with other databases or not.
    ///
    /// # Panics
    /// If `Q` is not an input ([`Query::INPUT`]): what a query computes is never set.
    pub fn set_as<Q: Query>(&self, key: &Q::Key, value: Q::Value, sharing: Sharing) {
        assert!(Q::INPUT, "{} is set, but is not an input", Q::NAME);
        let fingerprint = Q::interface(&value);
        let table = self.table::<Q>();
        let entry = table.entry(table.slot(key));
        let source = Source::Set(sharing);
        let revision = match &*entry.memo.load() {
            Some(current) if current.source == Source::Set(sharing) && current.fingerprint == fingerprint => {
                return;
            }
            // Shared or not, an equal value leaves what read it current.
            Some(current) if current.is_input() && current.fingerprint == fingerprint => {
                current.verified.get()
            }
            Some(_) => {
                let next = Revision(self.revision.fetch_add(1, Ordering::AcqRel) + 1);
                // What was found of earlier revisions is never asked again.
                self.settled.pin().clear();
                Some(next)
            }
            None => Some(self.revision()),
        };
        let revision = revision.unwrap_or_else(|| self.revision());
        let before = entry.memo.load().as_deref().map(|memo| (memo.source, memo.fingerprint));
        self.regroup::<Q>(key, before, (source, fingerprint));
        let group = Groups::identity::<Q>(key);
        entry.memo.store(Some(Arc::new(Memo::input(value, fingerprint, revision, source, group))));
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
        let key = (self.id.0, query);
        if let Some(id) = TABLES.with(|tables| tables.borrow().get(&key).copied()) {
            return id;
        }
        let id = *self.ids.pin().get_or_insert_with(query, || {
            let index =
                self.tables.push_with(|index| make(TableId(u32::try_from(index).expect("few queries"))));
            TableId(u32::try_from(index).expect("few queries"))
        });
        TABLES.with(|tables| tables.borrow_mut().insert(key, id));
        id
    }

    pub(crate) fn any_table(&self, id: TableId) -> &dyn AnyTable {
        &*self.tables[id.0 as usize]
    }
}
