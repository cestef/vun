//! Memos that outlive the process (spec 13, section 2): computed memos of the queries registered
//! with [`Db::persist`], with their traces, written as facet-postcard. A loaded memo is verified
//! before its first use, like any memo from an earlier revision, so a stale one is recomputed and
//! never trusted. Traces hold interface hashes, which do not depend on the process, so they stay
//! valid across processes.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use facet::Facet;
use overt_hash::Hash;

use super::Query;
use super::cx::{Edge, Read};
use super::db::{Db, Memo};

/// A query whose memos are saved: its keys and values are data.
pub trait Persist: Query<Key: Facet<'static>, Value: Facet<'static>> {}

/// The version of the saved form; a file of another version is not read.
const FORMAT: u32 = 1;

#[derive(Facet)]
struct Saved {
    format: u32,
    queries: Vec<SavedQuery>,
}

/// The memos of one query, by its name.
#[derive(Facet)]
struct SavedQuery {
    name: String,
    memos: Vec<SavedMemo>,
}

#[derive(Facet)]
struct SavedMemo {
    key: Vec<u8>,
    value: Vec<u8>,
    hash: Hash,
    deps: Vec<SavedEdge>,
}

/// A read in a trace: the query by name, its key and the interface hash it had.
#[derive(Facet)]
struct SavedEdge {
    query: String,
    key: Vec<u8>,
    hash: Hash,
}

/// How the memos of one persisted query are written and read back.
#[derive(Clone, Copy)]
pub(crate) struct Codec {
    name: &'static str,
    save: fn(&Db) -> Result<Vec<SavedMemo>, Unsaved>,
    load: fn(&Db, &[SavedMemo]) -> usize,
    key: fn(&dyn Any) -> Result<Vec<u8>, Unsaved>,
    edge: fn(&SavedEdge) -> Option<Arc<dyn Edge>>,
}

/// Why memos could not be saved.
#[derive(Debug)]
pub enum Unsaved {
    Encode(facet_postcard::SerializeError),
}

/// Why saved memos could not be loaded.
#[derive(Debug)]
pub enum Unloaded {
    Decode(facet_postcard::DeserializeError),
    /// Written in another version of the saved form.
    Format {
        found: u32,
    },
}

impl fmt::Display for Unsaved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(_) => f.write_str("a memo cannot be encoded"),
        }
    }
}

impl std::error::Error for Unsaved {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
        }
    }
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
            key: |key| {
                let key = key.downcast_ref::<Q::Key>().expect("an edge's key is its query's");
                facet_postcard::to_vec(key).map_err(Unsaved::Encode)
            },
            edge: |saved| {
                let key = facet_postcard::from_slice::<Q::Key>(&saved.key).ok()?;
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
    /// # Errors
    /// Returns an error if a key or a value cannot be encoded.
    ///
    /// # Panics
    /// Panics if the registry was poisoned by a panicking query.
    pub fn save(&self) -> Result<Vec<u8>, Unsaved> {
        let codecs: Vec<Codec> = self.codecs.read().expect("query codecs").values().copied().collect();
        let mut queries = Vec::new();
        for codec in codecs {
            queries.push(SavedQuery { name: codec.name.to_owned(), memos: (codec.save)(self)? });
        }
        queries.sort_by(|a, b| a.name.cmp(&b.name));
        facet_postcard::to_vec(&Saved { format: FORMAT, queries }).map_err(Unsaved::Encode)
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
        let saved: Saved = facet_postcard::from_slice(bytes).map_err(Unloaded::Decode)?;
        if saved.format != FORMAT {
            return Err(Unloaded::Format { found: saved.format });
        }
        let by_name = self.codecs_by_name();
        let restored =
            saved.queries.iter().filter_map(|q| Some((by_name.get(q.name.as_str())?.load)(self, &q.memos)));
        Ok(restored.sum())
    }

    fn codecs_by_name(&self) -> HashMap<&'static str, Codec> {
        self.codecs.read().expect("query codecs").values().map(|c| (c.name, *c)).collect()
    }

    /// The saved form of the computed memos of `Q` whose reads are all of persisted queries.
    fn saved<Q: Persist>(&self) -> Result<Vec<SavedMemo>, Unsaved> {
        let codecs = self.codecs.read().expect("query codecs").clone();
        let mut saved = Vec::new();
        for (key, memo) in self.memos::<Q>().into_iter().filter(|(_, m)| !m.input) {
            let deps: Option<Vec<_>> = memo.deps.iter().map(|d| Some((codecs.get(&d.query())?, d))).collect();
            let Some(deps) = deps else { continue };
            let deps = deps.into_iter().map(|(codec, d)| {
                Ok(SavedEdge { query: codec.name.to_owned(), key: (codec.key)(d.key())?, hash: d.hash() })
            });
            saved.push(SavedMemo {
                key: facet_postcard::to_vec(&key).map_err(Unsaved::Encode)?,
                value: facet_postcard::to_vec(&memo.value).map_err(Unsaved::Encode)?,
                hash: memo.hash,
                deps: deps.collect::<Result<_, _>>()?,
            });
        }
        // Tables are hash maps; the saved form does not depend on their order.
        saved.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(saved)
    }

    /// Stores `memos` of `Q` as never verified, returning how many decoded.
    fn restored<Q: Persist>(&self, memos: &[SavedMemo]) -> usize {
        let by_name = self.codecs_by_name();
        let mut restored = 0;
        for saved in memos {
            let key = facet_postcard::from_slice::<Q::Key>(&saved.key).ok();
            let value = facet_postcard::from_slice::<Q::Value>(&saved.value).ok();
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
