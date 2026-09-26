//! A memoized result: the value, its fingerprint, what computing it read, and the revision it was
//! last known current in, which any thread verifying it records.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use vun_codec::{Blob, Context, Malformed};
use vun_trace::tracing;

use super::db::Db;
use super::fingerprint::Fingerprint;
use super::persist::SavedDep;
use super::table::{Slot, TableId};
use super::{Query, Revision};

pub(crate) struct Memo<Q: Query> {
    /// Unset for a memo loaded without its value, or whose saved value is not read yet.
    value: OnceLock<Q::Value>,
    /// The saved value of a loaded memo, read when first needed.
    pub(crate) saved: Option<Saved<Q>>,
    pub(crate) fingerprint: Fingerprint,
    /// Each memo read, once, in the order of the first reads.
    pub(crate) deps: Deps,
    /// None for a loaded memo until it is verified.
    pub(crate) verified: Verified,
    /// Set with [`Db::set`], or an input's value while unset, rather than computed.
    pub(crate) input: bool,
}

/// A value as saved, a place in the stream the database loaded, and how to read it.
pub(crate) struct Saved<Q: Query> {
    pub(crate) at: Blob,
    pub(crate) read: ReadValue<Q::Value>,
}

/// Reads a value from its place in a stream.
pub(crate) type ReadValue<V> = fn(&Context, Blob) -> Result<V, Malformed>;

/// A memo read while another was computed, with the fingerprint it had then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Dep {
    pub(crate) table: TableId,
    pub(crate) slot: Slot,
    pub(crate) fingerprint: Fingerprint,
}

impl<Q: Query> Memo<Q> {
    pub(crate) fn input(value: Q::Value, fingerprint: Fingerprint, revision: Revision) -> Self {
        let verified = Verified::at(revision);
        Self {
            value: OnceLock::from(value),
            saved: None,
            fingerprint,
            deps: Deps::Read(Box::new([])),
            verified,
            input: true,
        }
    }

    pub(crate) fn computed(
        value: Q::Value,
        fingerprint: Fingerprint,
        deps: Box<[Dep]>,
        revision: Revision,
    ) -> Self {
        let verified = Verified::at(revision);
        let deps = Deps::Read(deps);
        Self { value: OnceLock::from(value), saved: None, fingerprint, deps, verified, input: false }
    }

    /// A memo of another process, never verified in this one.
    pub(crate) fn loaded(saved: Option<Saved<Q>>, fingerprint: Fingerprint, deps: Box<[SavedDep]>) -> Self {
        let verified = Verified::default();
        let deps = Deps::Saved { saved: deps, read: OnceLock::new() };
        Self { value: OnceLock::new(), saved, fingerprint, deps, verified, input: false }
    }

    /// Whether it holds in `revision` without looking at what it read.
    pub(crate) fn current(&self, revision: Revision) -> bool {
        self.input || self.verified.get() == Some(revision)
    }

    /// Its value if it has one: computed, or saved and read now the first time. `None` when it
    /// was loaded without it, or its saved form does not read.
    pub(crate) fn value(&self, db: &Db) -> Option<&Q::Value> {
        if let Some(value) = self.value.get() {
            return Some(value);
        }
        let saved = self.saved.as_ref()?;
        match (saved.read)(&db.store.get()?.stream, saved.at) {
            Ok(value) => Some(self.value.get_or_init(|| value)),
            Err(malformed) => {
                tracing::debug!(query = Q::NAME, %malformed, "saved value not read");
                None
            }
        }
    }

    /// Its value if computed or read already, without reading it.
    pub(crate) fn known(&self) -> Option<&Q::Value> {
        self.value.get()
    }
}

/// What a memo read: as recorded, or as saved, positions in the file loaded that are brought to
/// slots of this process when first needed.
pub(crate) enum Deps {
    Read(Box<[Dep]>),
    Saved { saved: Box<[SavedDep]>, read: OnceLock<Option<Box<[Dep]>>> },
}

impl Deps {
    /// The memos read, by slot; `None` when one of them is not held by this process's queries.
    pub(crate) fn get(&self, db: &Db) -> Option<&[Dep]> {
        match self {
            Self::Read(deps) => Some(deps),
            Self::Saved { saved, read } => read
                .get_or_init(|| {
                    let store = db.store.get()?;
                    saved.iter().map(|&dep| store.dep(db, dep)).collect()
                })
                .as_deref(),
        }
    }
}

/// A revision, or none: `0` stands for none and `n + 1` for revision `n`.
#[derive(Default)]
pub(crate) struct Verified(AtomicU64);

impl Verified {
    pub(crate) fn at(revision: Revision) -> Self {
        Self(AtomicU64::new(revision.0 + 1))
    }

    pub(crate) fn get(&self) -> Option<Revision> {
        self.0.load(Ordering::Acquire).checked_sub(1).map(Revision)
    }

    pub(crate) fn set(&self, revision: Revision) {
        self.0.store(revision.0 + 1, Ordering::Release);
    }
}
