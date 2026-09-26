//! Memos that outlive the process (spec 13, section 2): the computed memos of the queries
//! registered with [`Db::persist`] or [`Db::trace`], with what they read, in the encoding of
//! `vun-codec`. A loaded memo is verified before its first use, like any memo from an earlier
//! revision, so a stale one is recomputed and never trusted. A query traced without its values
//! still saves its memos' reads and fingerprints: a loaded one whose reads hold stands for its
//! value in verifying its dependents, and the value is computed only if it is read.

mod census;
mod query;
mod saved;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use vun_codec::{Blob, Context, Decode, Encode, Writer};
use vun_par::Parallel;
use vun_trace::{Activity, tracing};

pub use self::census::{Census, Counted};
pub(crate) use self::query::Persisted;
use self::query::{Keys, Persisting, Reads};
pub(crate) use self::saved::Growth;
pub(crate) use self::saved::Saved;
pub use self::saved::Unloaded;
use self::saved::{Body, FORMAT, Loaded, SavedQuery};
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
    /// were read; once the body loaded has outgrown its size when last written whole, it is
    /// written whole, its values read to be written anew.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let positions = (0..).zip(&persisted).map(|(at, p)| (p.table(self), at)).collect();
        let growth = self.growth.get().copied().filter(|growth| !growth.outgrown());
        let extends = growth.and(self.stream.get());
        let mut w = extends.map_or_else(Writer::new, Writer::extending);
        let queries = persisted.iter().map(|p| p.save(self, &mut w, &positions, extends).encoded()).collect();
        let body = Body { pools: w.into_pools(), queries }.encoded();
        let size = u64::try_from(body.len()).expect("a body fits in 64 bits");
        let compacted = growth.map_or(size, |growth| growth.compacted);
        tracing::debug!(size, compacted, whole = extends.is_none(), "memos saved");
        Saved { format: FORMAT, compacted, body }.encoded()
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
        let header = Saved::<Blob>::decoded(&bytes).map_err(Unloaded::Decode)?;
        let saved: Loaded = header.body.read(&bytes).map_err(Unloaded::Decode)?;
        let queries = saved.queries.par_map(|q| q.read::<SavedQuery<Blob>>(&bytes));
        let queries = queries.into_iter().collect::<Result<Vec<_>, _>>().map_err(Unloaded::Decode)?;
        let size = u64::try_from(header.body.len).expect("a body fits in 64 bits");
        let growth = Growth { size, compacted: header.compacted };
        drop(timer);
        let stream = Context::within(bytes.into_boxed_slice(), saved.pools);
        self.stream.set(stream).map_err(|_| Unloaded::Again)?;
        self.growth.set(growth).expect("set with the stream");
        let stream = self.stream.get().expect("set above");
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let queries: Vec<_> =
            queries.into_iter().map(|q| (by_name.get(q.name.as_str()).copied(), q)).collect();
        // Every key is read on its own, in parallel, so that a large table is spread too: the
        // stream's pools and the tables are shared.
        let timer = KEYS.start();
        let keys: Vec<(usize, Blob)> = queries
            .iter()
            .enumerate()
            .filter(|(_, (p, _))| p.is_some())
            .flat_map(|(at, (_, q))| q.keys.iter().map(move |key| (at, key.key)))
            .collect();
        let slots = keys.par_map(|&(at, key)| queries[at].0.and_then(|p| p.slot(self, stream, key)));
        let mut slots = slots.into_iter();
        let reads = queries.iter().map(|(p, q)| {
            let fingerprints = q.keys.iter().map(|key| key.fingerprint).collect();
            let slots = slots.by_ref().take(q.keys.len()).collect();
            p.map(|p| Keys { table: p.table(self), slots, fingerprints })
        });
        let reads = Reads(reads.collect());
        drop(timer);
        let _timer = RESTORE.start();
        // On one thread: storing a memo pays the debts of every thread reading memos, and threads
        // storing at once contend for them.
        let reads = &reads;
        let restored = queries
            .iter()
            .zip(&reads.0)
            .filter_map(|((p, q), keys)| Some((p.as_ref()?, q, keys.as_ref()?)))
            .flat_map(|(p, q, keys)| q.memos.iter().map(move |memo| p.restore(self, memo, keys, reads)));
        Ok(restored.filter(|&restored| restored).count())
    }
}
