//! Memos that outlive the process (spec 13, section 2): computed memos of the queries registered
//! with [`Db::persist`], with what they read, in the encoding of `vun-codec`. A read is saved by
//! query name and encoded key, since slots are the process's own. A loaded memo is verified
//! before its first use, like any memo from an earlier revision, so a stale one is recomputed and
//! never trusted.

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use vun_codec::{Decode, Encode, Malformed, record};

use super::Query;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::{Dep, Memo, Verified};
use super::table::{AnyTable, Slot, Table};

/// A query whose memos are saved: its keys and values are data.
pub trait Persist: Query<Key: Encode + Decode, Value: Encode + Decode> {}

/// The version of the saved form; a file of another version is not read.
const FORMAT: u32 = 2;

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
    fingerprint: Fingerprint,
    deps: Vec<SavedDep>,
}
record!(SavedMemo { key, value, fingerprint, deps });

/// A read: the query by name, its key and the fingerprint it had.
struct SavedDep {
    query: String,
    key: Vec<u8>,
    fingerprint: Fingerprint,
}
record!(SavedDep { query, key, fingerprint });

/// How the memos of one persisted query are written and read back.
#[derive(Clone, Copy)]
pub(crate) struct Codec {
    name: &'static str,
    save: fn(&Db) -> Vec<SavedMemo>,
    load: fn(&Db, &[SavedMemo]) -> usize,
    key: fn(&dyn AnyTable, Slot) -> Vec<u8>,
    dep: fn(&Db, &SavedDep) -> Option<Dep>,
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
            save: Db::saved::<Q>,
            load: Db::restored::<Q>,
            key: |table, slot| {
                let table: &Table<Q> = table.as_any().downcast_ref().expect("a table of its query");
                table.entry(slot).key.encoded()
            },
            dep: |db, saved| {
                let table = db.table::<Q>();
                let slot = table.slot(&Q::Key::decoded(&saved.key).ok()?);
                Some(Dep { table: table.id, slot, fingerprint: saved.fingerprint })
            },
        }
    }
}

impl Db {
    /// Registers `Q` as persisted: its computed memos are saved, and memos reading it can be.
    ///
    /// # Panics
    /// Panics if the registry was poisoned by a panicking query.
    pub fn persist<Q: Persist>(&self) {
        self.codecs.write().expect("query codecs").insert(TypeId::of::<Q>(), Codec::of::<Q>());
    }

    /// The computed memos of every persisted query that read only persisted queries.
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
    /// registered, or that read one, are skipped.
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

    /// The saved form of the computed memos of `Q` that read only persisted queries.
    fn saved<Q: Persist>(&self) -> Vec<SavedMemo> {
        let codecs = self.codecs.read().expect("query codecs").clone();
        let saved_dep = |dep: &Dep| {
            let table = self.any_table(dep.table);
            let codec = codecs.get(&table.query())?;
            Some(SavedDep {
                query: codec.name.to_owned(),
                key: (codec.key)(table, dep.slot),
                fingerprint: dep.fingerprint,
            })
        };
        let mut saved: Vec<_> = self
            .table::<Q>()
            .memos()
            .filter(|(_, memo)| !memo.input)
            .filter_map(|(key, memo)| {
                Some(SavedMemo {
                    key: key.encoded(),
                    value: memo.value.encoded(),
                    fingerprint: memo.fingerprint,
                    deps: memo.deps.iter().map(saved_dep).collect::<Option<_>>()?,
                })
            })
            .collect();
        // Slots are in the order keys were asked for; the saved form does not depend on it.
        saved.sort_by(|a, b| a.key.cmp(&b.key));
        saved
    }

    /// Stores `memos` of `Q` as never verified, returning how many decoded.
    fn restored<Q: Persist>(&self, memos: &[SavedMemo]) -> usize {
        let by_name = self.codecs_by_name();
        let table = self.table::<Q>();
        let restored = memos.iter().filter_map(|saved| {
            let key = Q::Key::decoded(&saved.key).ok()?;
            let value = Q::Value::decoded(&saved.value).ok()?;
            let deps: Box<[Dep]> = saved
                .deps
                .iter()
                .map(|d| (by_name.get(d.query.as_str())?.dep)(self, d))
                .collect::<Option<_>>()?;
            let memo = Memo::<Q>::computed(value, saved.fingerprint, deps, Verified::default());
            table.entry(table.slot(&key)).memo.store(Some(Arc::new(memo)));
            Some(())
        });
        restored.count()
    }
}
