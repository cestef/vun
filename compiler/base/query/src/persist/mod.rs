//! Memos that outlive the process (spec 13, section 2): the computed memos of the queries
//! registered with [`Db::persist`] or [`Db::trace`], with what they read, in the encoding of
//! `vun-codec`. Saved memos are restored as their keys are first asked for, so a run reads what
//! it uses. A loaded memo is verified before its first use, like any memo from an earlier
//! revision, so a stale one is recomputed and never trusted. A query traced without its values
//! still saves its memos' reads and fingerprints: a loaded one whose reads hold stands for its
//! value in verifying its dependents, and the value is computed only if it is read.

mod census;
mod query;
mod saved;
mod store;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_codec::{Blob, Context, Decode, Encode, Writer};
use vun_trace::{Activity, tracing};

pub use self::census::{Census, Counted};
pub(crate) use self::query::Persisted;
use self::query::{Layout, Persisting};
pub use self::saved::Unloaded;
use self::saved::{Body, FORMAT, Loaded, Named};
pub(crate) use self::saved::{Growth, Saved, SavedDep};
use self::store::StoredQuery;
pub(crate) use self::store::{Attached, Store};
use super::Query;
use super::db::Db;

/// Loading saved memos, as timed: the rest is read as it is used.
const LOAD: Activity = Activity::stage("load memos");

impl Db {
    /// Registers `Q` as persisted with its values.
    pub fn persist<Q: Query<Key: Encode + Decode, Value: Encode + Decode>>(&self) {
        self.register(Arc::new(Persisting::<Q>::values()));
    }

    /// Registers `Q` as persisted without its values: its memos keep their reads and fingerprints,
    /// and memos reading it can be persisted.
    pub fn trace<Q: Query<Key: Encode + Decode>>(&self) {
        self.register(Arc::new(Persisting::<Q>::trace()));
    }

    /// Saved memos name their queries, so no two persisted queries share a name.
    ///
    /// # Panics
    /// If another query of the same name is registered, or if the registry was poisoned by a
    /// panicking query.
    fn register(&self, query: Arc<dyn Persisted>) {
        let mut persisted = self.persisted.write().expect("persisted queries");
        match persisted.iter().position(|p| p.name() == query.name()) {
            Some(at) => {
                assert!(
                    persisted[at].query() == query.query(),
                    "two persisted queries are named {}",
                    query.name()
                );
                persisted[at] = query;
            }
            None => persisted.push(query),
        }
    }

    /// Every key of every persisted query, with its memo if it is computed and reads persisted
    /// queries alone. The stream extends the one loaded, if any, keeping every key it holds and
    /// every value never read as it was read; once the body loaded has outgrown its size when
    /// last written whole, it is written whole, every saved memo read to be written anew.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let store = self.store.get().map(Arc::as_ref);
        let growth = store.map(|store| store.growth).filter(|growth| !growth.outgrown());
        let extends = growth.and(store);
        if let (None, Some(store)) = (extends, store) {
            store.restore_all(self);
        }
        let parts = Part::order(&persisted, extends);
        let tables = parts.iter().filter_map(|part| match part {
            Part::Table(p) => Some(p.table(self)),
            Part::Carried { .. } => None,
        });
        let positions = tables.clone().zip(0..).collect();
        let numbers = parts.iter().filter_map(|part| match part {
            Part::Table(p) => Some(p.number(self, extends.is_some())),
            Part::Carried { .. } => None,
        });
        let layout = Layout { positions, numbers: tables.zip(numbers).collect() };
        let mut w = extends.map_or_else(Writer::new, |store| Writer::extending(&store.stream));
        let queries = parts.iter().map(|part| part.save(self, &mut w, &layout, extends)).collect();
        let body = Body { pools: w.into_pools(), queries }.encoded();
        let size = u64::try_from(body.len()).expect("a body fits in 64 bits");
        let compacted = growth.map_or(size, |growth| growth.compacted);
        tracing::debug!(size, compacted, whole = extends.is_none(), "memos saved");
        Saved { format: FORMAT, compacted, body }.encoded()
    }

    /// Whether saved memos were loaded.
    #[must_use]
    pub fn restored(&self) -> bool {
        self.store.get().is_some()
    }

    /// Whether a memo was computed that the memos loaded did not hold, or none were: what
    /// [`Db::save`] would write differs from what was loaded.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.store.get().is_none() || self.changed.load(Ordering::Relaxed)
    }

    /// Loads saved memos, returning how many of their queries are registered. Nothing is read
    /// yet: a query's keys are read when one of them is first looked for, and a memo when its
    /// key is first asked for, as never verified. Memos of queries not registered, or that read
    /// one, are never restored.
    ///
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format, or if memos were loaded
    /// already.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    pub fn load(&self, bytes: Vec<u8>) -> Result<usize, Unloaded> {
        let _timer = LOAD.start();
        let format = Context::default().front::<u32>(&bytes).map_err(Unloaded::Decode)?;
        if format != FORMAT {
            return Err(Unloaded::Format { found: format });
        }
        let header = Saved::<Blob>::decoded(&bytes).map_err(Unloaded::Decode)?;
        let saved: Loaded = header.body.read(&bytes).map_err(Unloaded::Decode)?;
        let size = u64::try_from(header.body.len).expect("a body fits in 64 bits");
        let growth = Growth { size, compacted: header.compacted };
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let queries = saved.queries.into_iter().map(|q| {
            let persisted = by_name.get(q.name.as_str()).copied();
            (persisted, StoredQuery::new(q.name, q.query, persisted.map(|p| p.table(self))))
        });
        let (persisted, queries): (Vec<_>, Vec<_>) = queries.unzip();
        let stream = Context::within(bytes.into_boxed_slice(), saved.pools);
        let store = Arc::new(Store { stream, growth, queries });
        self.store.set(Arc::clone(&store)).map_err(|_| Unloaded::Again)?;
        let attached = persisted.iter().enumerate().filter_map(|(at, p)| Some((at, (*p)?)));
        Ok(attached.map(|(at, p)| p.attach(self, &store, at)).count())
    }
}

/// A query of the file saved, in the order written.
enum Part<'a> {
    Table(&'a Arc<dyn Persisted>),
    /// A query of the file loaded that is not registered, kept as it was in the stream extending
    /// that file.
    Carried {
        store: &'a Store,
        query: &'a StoredQuery,
    },
}

impl<'a> Part<'a> {
    /// A file extending the one loaded keeps its queries at their positions and adds the others
    /// after them; one written whole holds the registered queries in the order registered.
    fn order(persisted: &'a [Arc<dyn Persisted>], extends: Option<&'a Store>) -> Vec<Self> {
        let Some(store) = extends else { return persisted.iter().map(Part::Table).collect() };
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let loaded = store.queries.iter().map(|query| match by_name.get(query.name.as_str()) {
            Some(p) => Part::Table(p),
            None => Part::Carried { store, query },
        });
        let added = persisted.iter().filter(|p| store.queries.iter().all(|q| q.name != p.name()));
        loaded.chain(added.map(Part::Table)).collect()
    }

    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, extends: Option<&Store>) -> Named {
        match self {
            Self::Table(p) => {
                Named { name: p.name().to_owned(), query: p.save(db, w, layout, extends).encoded() }
            }
            Self::Carried { store, query } => {
                Named { name: query.name.clone(), query: store.stream.bytes(query.bytes).to_vec() }
            }
        }
    }
}
