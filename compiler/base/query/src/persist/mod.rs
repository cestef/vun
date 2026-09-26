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

use vun_codec::{Context, Decode, Encode, Writer};

pub(crate) use self::query::Persisted;
use self::query::{Persisting, Reads};
pub use self::saved::Unloaded;
use self::saved::{FORMAT, Saved};
use super::Query;
use super::db::Db;

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

    /// # Panics
    /// If the registry was poisoned by a panicking query.
    fn register(&self, query: Arc<dyn Persisted>) {
        let mut persisted = self.persisted.write().expect("persisted queries");
        persisted.retain(|p| p.name() != query.name());
        persisted.push(query);
    }

    /// The computed memos of every persisted query whose reads are all of persisted queries.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let positions = (0..).zip(&persisted).map(|(at, p)| (p.table(self), at)).collect();
        let mut w = Writer::pooling(self);
        let queries = persisted.iter().map(|p| p.save(self, &mut w, &positions)).collect();
        Saved { format: FORMAT, pools: w.into_pools(), queries }.encoded()
    }

    /// Loads saved memos as unverified, returning how many were restored. Memos of queries not
    /// registered, or that read one, are skipped.
    ///
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    pub fn load(&self, bytes: &[u8]) -> Result<usize, Unloaded> {
        let format = Context::default().front::<u32>(bytes).map_err(Unloaded::Decode)?;
        if format != FORMAT {
            return Err(Unloaded::Format { found: format });
        }
        let saved = Saved::decoded(bytes).map_err(Unloaded::Decode)?;
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let by_name: HashMap<_, _> = persisted.iter().map(|p| (p.name(), p)).collect();
        let registered: Vec<_> = saved.queries.iter().map(|q| by_name.get(q.name.as_str())).collect();
        let cx = Context::new(&saved.pools, self);
        let reads = Reads(
            saved
                .queries
                .iter()
                .zip(&registered)
                .map(|(q, p)| p.map(|p| (p.table(self), p.slots(self, &cx, &q.keys))))
                .collect(),
        );
        let restored = saved.queries.iter().zip(&registered).zip(&reads.0).filter_map(|((q, p), read)| {
            let (_, slots) = read.as_ref()?;
            Some((*p)?.restore(self, &cx, &q.memos, slots, &reads))
        });
        Ok(restored.sum())
    }
}
