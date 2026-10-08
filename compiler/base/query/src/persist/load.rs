//! Loading saved memos: a file alone, or a file of shared memos with the file layered over it.

use std::collections::HashSet;
use std::sync::Arc;

use vun_codec::{Blob, Context, Malformed, Owned, Pools};
use vun_hash::Hash;

use super::saved::{Build, Growth, Loaded, Named, Saved, Unloaded};
use super::store::{Layers, Store, StoredQuery};
use super::{LOAD, Numbers, Persisted};
use crate::db::Db;

impl Db {
    /// Loads saved memos, returning how many of their queries are registered. Nothing is read
    /// yet: a query's keys are read when one of them is first looked for, and a memo when its
    /// key is first asked for, as never verified. Memos of queries not registered, or that read
    /// one, are never restored.
    ///
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format and build, if they are
    /// layered over a file of shared memos, or if memos were loaded already.
    pub fn load(&self, bytes: impl AsRef<[u8]> + Send + Sync + 'static) -> Result<usize, Unloaded> {
        let _timer = LOAD.start();
        let file = File::read(Box::new(bytes))?;
        if file.header.base.is_some() {
            return Err(Unloaded::Base);
        }
        let growth = file.growth();
        let stream = Context::within(file.bytes, file.body.pools);
        let queries =
            file.body.queries.into_iter().map(|q| (q.name, Layers { base: None, own: Some(q.query) }));
        self.attach(Arc::new(stream), Some(growth), queries.collect(), None, Taking::Missing)
    }

    /// Loads the file of shared memos `base`, with `own`, the memos saved layered over it, as
    /// [`Db::load`] does a file alone. The memos saved next are layered over `base`.
    ///
    /// # Errors
    /// As [`Db::load`], and if `base` is not a file of shared memos or `own` is layered over
    /// another.
    pub fn load_over(
        &self,
        base: impl AsRef<[u8]> + Send + Sync + 'static,
        own: Option<Vec<u8>>,
    ) -> Result<usize, Unloaded> {
        self.layer(Box::new(base), own, Taking::Missing)
    }

    /// Loads `own` over `base`, taking what `taking` says of their memos.
    pub(super) fn layer(
        &self,
        base: Bytes,
        own: Option<Vec<u8>>,
        taking: Taking<'_>,
    ) -> Result<usize, Unloaded> {
        let _timer = LOAD.start();
        let base = File::read(base)?;
        let id = base.header.id.filter(|_| base.header.base.is_none()).ok_or(Unloaded::Base)?;
        let own = own.map(|own| File::read(Box::new(own))).transpose()?;
        if own.as_ref().is_some_and(|own| own.header.base != Some(id)) {
            return Err(Unloaded::Base);
        }
        let under = Arc::new(Context::within(base.bytes, base.body.pools));
        let (stream, growth, own) = match own {
            Some(own) => {
                let growth = own.growth();
                let stream = Context::over(under, own.bytes, own.body.pools);
                (stream, Some(growth), own.body.queries)
            }
            None => {
                (Context::over(under, Box::new(Box::<[u8]>::default()), Pools::default()), None, Vec::new())
            }
        };
        let mut own = own.into_iter().map(|q| (q.name, stream.place(q.query)));
        let mut queries = Vec::new();
        // The file layered over holds each query of the shared file at its position there.
        for q in base.body.queries {
            let layer = match own.next() {
                Some((name, blob)) if name == q.name => Some(blob),
                Some(_) => return Err(Unloaded::Decode(Malformed::Invariant)),
                None => None,
            };
            queries.push((q.name, Layers { base: Some(q.query), own: layer }));
        }
        queries.extend(own.map(|(name, blob)| (name, Layers { base: None, own: Some(blob) })));
        self.attach(Arc::new(stream), growth, queries, Some(id), taking)
    }

    /// Attaches the queries of `stream`, by name, to the tables of the queries registered.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    fn attach(
        &self,
        stream: Arc<Context>,
        growth: Option<Growth>,
        queries: Vec<(String, Layers)>,
        base: Option<Hash>,
        taking: Taking<'_>,
    ) -> Result<usize, Unloaded> {
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let by_name: std::collections::HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let queries = queries.into_iter().map(|(name, layers)| {
            let persisted = by_name.get(name.as_str()).copied();
            (persisted, StoredQuery::new(name, layers, persisted.map(|p| p.table(self))))
        });
        let (persisted, queries): (Vec<Option<&Arc<dyn Persisted>>>, Vec<_>) = queries.unzip();
        let store = Arc::new(Store { stream, growth, queries, base });
        self.store.set(Arc::clone(&store)).map_err(|_| Unloaded::Again)?;
        let attached = persisted.iter().enumerate().filter_map(|(at, p)| Some((at, (*p)?)));
        Ok(attached.map(|(at, p)| p.attach(self, &store, at, taking)).count())
    }
}

/// Which saved memos the keys a database holds already take.
#[derive(Clone, Copy)]
pub(crate) enum Taking<'a> {
    /// Those of keys holding none, each found by its value.
    Missing,
    /// None: the memos saved were written from those the keys hold, at the places `Numbers`
    /// gave them, so the keys take those places and their memos count as saved.
    Written(&'a Numbers),
}

/// The bytes of a saved file, in memory or mapped.
type Bytes = Owned;

/// A saved file, its header and body read.
struct File {
    header: Saved<Blob>,
    body: Loaded,
    bytes: Bytes,
}

impl File {
    fn read(bytes: Bytes) -> Result<Self, Unloaded> {
        let header = Saved::header((*bytes).as_ref())?;
        if header.build.is_none() || header.build != Build::identity() {
            return Err(Unloaded::Build);
        }
        let body: Loaded = header.body.read((*bytes).as_ref()).map_err(Unloaded::Decode)?;
        // Each query is saved once, so that its positions name its own memos.
        let mut names = HashSet::new();
        if !body.queries.iter().all(|q: &Named<Blob>| names.insert(q.name.as_str())) {
            return Err(Unloaded::Decode(Malformed::Invariant));
        }
        Ok(Self { header, body, bytes })
    }

    fn growth(&self) -> Growth {
        let size = u64::try_from(self.header.body.len).expect("a body fits in 64 bits");
        Growth { size, compacted: self.header.compacted }
    }
}
