//! Memos that outlive the process (spec 13, section 2): the computed memos of the queries
//! registered with [`Db::persist`] or [`Db::trace`], with what they read, in the encoding of
//! `vun-codec`. Saved memos are restored as their keys are first asked for, so a run reads what
//! it uses. A loaded memo is verified before its first use, like any memo from an earlier
//! revision, so a stale one is recomputed and never trusted. A query traced without its values
//! still saves its memos' reads and fingerprints: a loaded one whose reads hold stands for its
//! value in verifying its dependents, and the value is computed only if it is read.

mod census;
mod keys;
mod load;
mod query;
mod saved;
mod shared;
mod store;

use std::collections::HashMap;
use std::sync::Arc;

use vun_codec::{Context, Decode, Encode, Writer};
use vun_hash::Hash;
use vun_trace::{Activity, tracing};

pub use self::census::{Census, Counted};
use self::keys::PackedKeys;
pub(crate) use self::load::Taking;
pub(crate) use self::query::Persisted;
use self::query::{Layout, Numbers, Persisting};
use self::saved::{Body, FORMAT, Named, SavedQuery};
pub use self::saved::{Build, Layering, Unloaded};
pub(crate) use self::saved::{Saved, SavedDep};
pub(crate) use self::store::{Attached, Store};
use self::store::{StoredKeys, StoredQuery};
use super::Query;
use super::db::{Db, Sharing};

/// Loading saved memos, as timed: the rest is read as it is used.
const LOAD: Activity = Activity::stage("load memos");
/// Saving memos, as timed.
const SAVE: Activity = Activity::stage("save memos");

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
        self.any_table(query.table(self)).persist();
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
        self.save_keys(None).0
    }

    /// The memos of every key of `include`, or of every key, as [`Db::save`] says; a file holding
    /// some keys alone is written whole, alone, and named by the hash of its body, so that files
    /// can be layered over it. The places the keys were given come with it.
    fn save_keys(&self, include: Option<&shared::Included>) -> (Vec<u8>, Numbers) {
        let _timer = SAVE.start();
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let store = self.store.get().map(Arc::as_ref);
        let keeps = Keeps::of(store, include.is_some());
        if let Some(store) = store
            && keeps.extends().is_none()
        {
            store.restore_all(self, matches!(keeps, Keeps::Base { .. }));
        }
        let parts = Part::order(&persisted, keeps.store());
        let tables = parts.iter().filter_map(|part| match part {
            Part::Table(p) => Some(p.table(self)),
            Part::Carried { .. } => None,
        });
        let positions = tables.clone().zip(0..).collect();
        let numbers = parts.iter().filter_map(|part| match part {
            Part::Table(p) => Some(p.number(self, keeps, include)),
            Part::Carried { .. } => None,
        });
        let layout = Layout {
            positions,
            numbers: tables.zip(numbers).collect(),
            revision: self.revision(),
            shared: Sharing::LEVELS.map(|level| self.shared_inputs_now(level)),
        };
        let mut w = keeps.writer();
        let queries = parts.iter().map(|part| part.save(self, &mut w, &layout, keeps)).collect();
        let body = Body { pools: w.into_pools(), queries }.encoded();
        let size = u64::try_from(body.len()).expect("a body fits in 64 bits");
        let compacted =
            keeps.extends().and_then(|store| store.growth).map_or(size, |growth| growth.compacted);
        let (id, base) = (include.map(|_| Hash::of(&body)), keeps.store().and_then(|store| store.base));
        tracing::debug!(
            size,
            compacted,
            whole = keeps.extends().is_none(),
            layered = base.is_some(),
            "memos saved"
        );
        let saved = Saved { format: FORMAT, build: Build::identity(), compacted, id, base, body };
        (saved.encoded(), layout.numbers)
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
        self.store.get().is_none() || self.changed.get().is_some()
    }

    /// The query of the first memo computed that the memos loaded did not hold.
    #[must_use]
    pub fn first_changed(&self) -> Option<&'static str> {
        self.changed.get().copied()
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
    /// A file keeping the positions of the one loaded, extending it or layered over the shared
    /// file under it, keeps its queries at their positions and adds the others after them; one
    /// written whole alone holds the registered queries in the order registered.
    fn order(persisted: &'a [Arc<dyn Persisted>], keeps: Option<&'a Store>) -> Vec<Self> {
        let Some(store) = keeps else { return persisted.iter().map(Part::Table).collect() };
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let loaded = store.queries.iter().map(|query| match by_name.get(query.name.as_str()) {
            Some(p) => Part::Table(p),
            None => Part::Carried { store, query },
        });
        let added = persisted.iter().filter(|p| store.queries.iter().all(|q| q.name != p.name()));
        loaded.chain(added.map(Part::Table)).collect()
    }

    fn save(&self, db: &Db, w: &mut Writer<'_>, layout: &Layout, keeps: Keeps<'_>) -> Named {
        match self {
            Self::Table(p) => {
                Named { name: p.name().to_owned(), query: p.save(db, w, layout, keeps).encoded() }
            }
            // Kept as it was in a file extending it, and holding nothing in one written whole.
            Self::Carried { store, query } => {
                let own = query.layers.own.filter(|_| keeps.extends().is_some());
                let bytes = own.map(|blob| store.stream.bytes(blob).to_vec());
                let empty =
                    || SavedQuery::<Vec<u8>> { keys: PackedKeys::pack(&[]), replaced: Vec::new() }.encoded();
                Named { name: query.name.clone(), query: bytes.unwrap_or_else(empty) }
            }
        }
    }
}

/// What a file saved keeps of the files loaded.
#[derive(Clone, Copy)]
pub(crate) enum Keeps<'a> {
    /// Nothing: it is written whole, alone.
    Nothing,
    /// The shared file the file loaded is layered over: it is written whole over it.
    Base { store: &'a Store, under: &'a Context },
    /// The file loaded, which it extends.
    Loaded(&'a Store),
}

impl<'a> Keeps<'a> {
    /// A file holding some keys alone is written alone; another extends the file loaded until it
    /// outgrows it, then is written whole over the shared file it is layered over, if any.
    fn of(store: Option<&'a Store>, some: bool) -> Self {
        match store {
            Some(store) if !some && store.growth.is_some_and(|growth| !growth.outgrown()) => {
                Self::Loaded(store)
            }
            Some(store) if !some => {
                store.stream.base().map_or(Self::Nothing, |under| Self::Base { store, under })
            }
            _ => Self::Nothing,
        }
    }

    fn store(self) -> Option<&'a Store> {
        match self {
            Self::Nothing => None,
            Self::Base { store, .. } | Self::Loaded(store) => Some(store),
        }
    }

    pub(crate) fn extends(self) -> Option<&'a Store> {
        match self {
            Self::Loaded(store) => Some(store),
            Self::Nothing | Self::Base { .. } => None,
        }
    }

    fn writer(self) -> Writer<'a> {
        match self {
            Self::Nothing => Writer::new(),
            Self::Base { under, .. } => Writer::over(under),
            Self::Loaded(store) => Writer::extending(&store.stream),
        }
    }

    /// How many of a query's `keys` are left to the shared file.
    pub(crate) fn first(self, keys: &StoredKeys) -> u32 {
        match self {
            Self::Nothing => 0,
            Self::Base { .. } | Self::Loaded(_) => keys.base,
        }
    }

    /// How many of a query's `keys` keep their positions.
    pub(crate) fn kept(self, keys: &StoredKeys) -> u32 {
        match self {
            Self::Nothing => 0,
            Self::Base { .. } => keys.base,
            Self::Loaded(_) => keys.len(),
        }
    }
}
