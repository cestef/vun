//! Memos that outlive the process (spec 13, section 2): computed memos of the queries registered
//! with [`Db::persist`], with their traces, in the encoding of `vun-codec`. A loaded memo is verified
//! before its first use, like any memo from an earlier revision, so a stale one is recomputed and
//! never trusted. Traces hold interface hashes, which do not depend on the process, so they stay
//! valid across processes.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use vun_codec::{Decode, Encode, Malformed, record};
use vun_hash::Hash;

use super::Query;
use super::cx::{Edge, Read};
use super::db::{Db, Memo};

/// A query whose memos are saved: its keys and values are data.
pub trait Persist: Query<Key: Encode + Decode, Value: Encode + Decode> {}

/// The version of the saved form; a file of another version is not read.
const FORMAT: u32 = 1;

struct Saved {
    format: u32,
    queries: Vec<SavedQuery>,
}
record!(Saved { format, queries });

/// The memos of one query, by its name.
struct SavedQuery {
    name: String,
    memos: Vec<SavedMemo>,
}
record!(SavedQuery { name, memos });

struct SavedMemo {
    key: Vec<u8>,
    value: Vec<u8>,
    hash: Hash,
    deps: Vec<SavedEdge>,
}
record!(SavedMemo { key, value, hash, deps });

/// A read in a trace: the query by name, its key and the interface hash it had.
struct SavedEdge {
    query: String,
    key: Vec<u8>,
    hash: Hash,
}
record!(SavedEdge { query, key, hash });

/// How the memos of one persisted query are written and read back.
#[derive(Clone, Copy)]
pub(crate) struct Codec {
    name: &'static str,
    save: fn(&Db) -> Vec<SavedMemo>,
    load: fn(&Db, &[SavedMemo]) -> usize,
    key: fn(&dyn Any) -> Vec<u8>,
    edge: fn(&SavedEdge) -> Option<Arc<dyn Edge>>,
}

/// Why saved memos could not be loaded.
#[derive(Debug)]
pub enum Unloaded {
    Decode(Malformed),
    /// Written in another version of the saved form.
    Format {
        found: u32,
    },
}

impl fmt::Display for Unloaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => f.write_str("the saved memos do not decode"),
            Self::Format { found } => write!(f, "the saved memos are in format {found}, not {FORMAT}"),
        }
    }
}

impl std::error::Error for Unloaded {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Format { .. } => None,
        }
    }
}

impl Codec {
    fn of<Q: Persist>() -> Self {
        Self {
            name: Q::NAME,
            save: |db| db.saved::<Q>(),
            load: |db, memos| db.restored::<Q>(memos),
            key: |key| key.downcast_ref::<Q::Key>().expect("an edge's key is its query's").encoded(),
            edge: |saved| {
                let key = Q::Key::decoded(&saved.key).ok()?;
                Some(Arc::new(Read::<Q> { key, hash: saved.hash }))
            },
        }
    }
}

impl Db {
    /// Registers `Q` as persisted: its computed memos are saved, and traces reading it can be.
    ///
    /// # Panics
    /// Panics if the registry was poisoned by a panicking query.
    pub fn persist<Q: Persist>(&self) {
        self.codecs.write().expect("query codecs").insert(TypeId::of::<Q>(), Codec::of::<Q>());
    }

    /// The computed memos of every persisted query whose traces read only persisted queries.
    ///
    /// # Panics
    /// Panics if the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let codecs: Vec<Codec> = self.codecs.read().expect("query codecs").values().copied().collect();
        let mut queries: Vec<_> =
            codecs.iter().map(|c| SavedQuery { name: c.name.to_owned(), memos: (c.save)(self) }).collect();
        queries.sort_by(|a, b| a.name.cmp(&b.name));
        Saved { format: FORMAT, queries }.encoded()
    }

    /// Loads saved memos as unverified, returning how many were restored. Memos of queries not
    /// registered, or whose traces read one, are skipped.
    ///
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format.
    ///
    /// # Panics
    /// Panics if the registry was poisoned by a panicking query.
    pub fn load(&self, bytes: &[u8]) -> Result<usize, Unloaded> {
        // The format leads, so that another version is named rather than misread.
        let format = u32::decode(&mut &bytes[..]).map_err(Unloaded::Decode)?;
        if format != FORMAT {
            return Err(Unloaded::Format { found: format });
        }
        let saved = Saved::decoded(bytes).map_err(Unloaded::Decode)?;
        let by_name = self.codecs_by_name();
        let restored =
            saved.queries.iter().filter_map(|q| Some((by_name.get(q.name.as_str())?.load)(self, &q.memos)));
        Ok(restored.sum())
    }

    fn codecs_by_name(&self) -> HashMap<&'static str, Codec> {
        self.codecs.read().expect("query codecs").values().map(|c| (c.name, *c)).collect()
    }

    /// The saved form of the computed memos of `Q` whose reads are all of persisted queries.
    fn saved<Q: Persist>(&self) -> Vec<SavedMemo> {
        let codecs = self.codecs.read().expect("query codecs").clone();
        let mut saved = Vec::new();
        for (key, memo) in self.memos::<Q>().into_iter().filter(|(_, m)| !m.input) {
            let deps: Option<Vec<_>> = memo.deps.iter().map(|d| Some((codecs.get(&d.query())?, d))).collect();
            let Some(deps) = deps else { continue };
            let deps = deps.into_iter().map(|(codec, d)| SavedEdge {
                query: codec.name.to_owned(),
                key: (codec.key)(d.key()),
                hash: d.hash(),
            });
            saved.push(SavedMemo {
                key: key.encoded(),
                value: memo.value.encoded(),
                hash: memo.hash,
                deps: deps.collect(),
            });
        }
        // Tables are hash maps; the saved form does not depend on their order.
        saved.sort_by(|a, b| a.key.cmp(&b.key));
        saved
    }

    /// Stores `memos` of `Q` as never verified, returning how many decoded.
    fn restored<Q: Persist>(&self, memos: &[SavedMemo]) -> usize {
        let by_name = self.codecs_by_name();
        let mut restored = 0;
        for saved in memos {
            let key = Q::Key::decoded(&saved.key).ok();
            let value = Q::Value::decoded(&saved.value).ok();
            let deps: Option<Vec<_>> =
                saved.deps.iter().map(|e| (by_name.get(e.query.as_str())?.edge)(e)).collect();
            let (Some(key), Some(value), Some(deps)) = (key, value, deps) else { continue };
            let memo = Memo::<Q> { value, hash: saved.hash, deps: deps.into(), verified: None, input: false };
            self.store(key, memo);
            restored += 1;
        }
        restored
    }
}
