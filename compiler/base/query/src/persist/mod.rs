//! Memos that outlive the process (spec 13, section 2): the computed memos of the queries
//! registered with [`Db::persist`] or [`Db::trace`], with what they read, in the encoding of
//! `vun-codec`. A loaded memo is verified before its first use, like any memo from an earlier
//! revision, so a stale one is recomputed and never trusted. A query traced without its values
//! still saves its memos' reads and fingerprints: a loaded one whose reads hold stands for its
//! value in verifying its dependents, and the value is computed only if it is read.

mod query;
mod saved;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_codec::{Blob, Context, Decode, Encode, Writer};
use vun_par::Parallel;
use vun_trace::Activity;

pub(crate) use self::query::Persisted;
use self::query::{Persisting, Reads};
pub use self::saved::Unloaded;
use self::saved::{FORMAT, Loaded, Saved};
use super::Query;
use super::db::Db;

/// The steps of loading, as timed.
const DECODE: Activity = Activity::stage("load memos: decode");
const KEYS: Activity = Activity::stage("load memos: keys");
const RESTORE: Activity = Activity::stage("load memos: restore");

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

    /// The computed memos of every persisted query whose reads are all of persisted queries. The
    /// stream extends the one loaded, if any, so that values never read are written as they
    /// were read.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let positions = (0..).zip(&persisted).map(|(at, p)| (p.table(self), at)).collect();
        let mut w = self.stream.get().map_or_else(Writer::new, Writer::extending);
        let queries = persisted.iter().map(|p| p.save(self, &mut w, &positions)).collect();
        Saved { format: FORMAT, pools: w.into_pools(), queries }.encoded()
    }

    /// Whether a memo was computed that the memos loaded did not hold, or none were: what
    /// [`Db::save`] would write differs from what was loaded.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.stream.get().is_none() || self.changed.load(Ordering::Relaxed)
    }

    /// Loads saved memos as unverified, returning how many were restored. Their values are read
    /// when first needed. Memos of queries not registered, or that read one, are skipped.
    ///
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format, or if memos were loaded
    /// already.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    pub fn load(&self, bytes: Vec<u8>) -> Result<usize, Unloaded> {
        let format = Context::default().front::<u32>(&bytes).map_err(Unloaded::Decode)?;
        if format != FORMAT {
            return Err(Unloaded::Format { found: format });
        }
        let timer = DECODE.start();
        let saved = Loaded::decoded(&bytes).map_err(Unloaded::Decode)?;
        drop(timer);
        let stream = Context::within(bytes.into_boxed_slice(), saved.pools);
        self.stream.set(stream).map_err(|_| Unloaded::Again)?;
        let stream = self.stream.get().expect("set above");
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let queries: Vec<_> =
            saved.queries.into_iter().map(|q| (by_name.get(q.name.as_str()).copied(), q)).collect();
        // Every key is read on its own, in parallel, so that a large table is spread too: the
        // stream's pools and the tables are shared.
        let timer = KEYS.start();
        let keys: Vec<(usize, Blob)> = queries
            .iter()
            .enumerate()
            .filter(|(_, (p, _))| p.is_some())
            .flat_map(|(at, (_, q))| q.keys.iter().map(move |&key| (at, key)))
            .collect();
        let slots = keys.par_map(|&(at, key)| queries[at].0.and_then(|p| p.slot(self, stream, key)));
        let mut reads: Vec<_> = queries.iter().map(|(p, _)| p.map(|p| (p.table(self), Vec::new()))).collect();
        for (&(at, _), slot) in keys.iter().zip(slots) {
            reads[at].as_mut().expect("a registered query").1.push(slot);
        }
        let reads = Reads(reads);
        drop(timer);
        let _timer = RESTORE.start();
        let restored = queries.into_iter().zip(&reads.0).filter_map(|((p, q), read)| {
            let (_, slots) = read.as_ref()?;
            Some(p?.restore(self, q.memos, slots, &reads))
        });
        Ok(restored.sum())
    }
}
