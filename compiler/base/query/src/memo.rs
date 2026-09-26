//! A memoized result: the value, its interface hash, the trace of what computing it read, and the
//! revision it was last known current in, which any thread verifying it records.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vun_hash::Hash;

use super::cx::Edge;
use super::{Query, Revision};

pub(crate) struct Memo<Q: Query> {
    pub(crate) value: Q::Value,
    pub(crate) hash: Hash,
    pub(crate) deps: Box<[Arc<dyn Edge>]>,
    /// None for a loaded memo until it is verified.
    pub(crate) verified: Verified,
    /// Set with [`Db::set`](super::Db::set) rather than computed.
    pub(crate) input: bool,
}

impl<Q: Query> Memo<Q> {
    pub(crate) fn input(value: Q::Value, hash: Hash, revision: Revision) -> Self {
        Self { value, hash, deps: Box::new([]), verified: Verified::at(revision), input: true }
    }

    /// A memo computed, reading `deps`; known current in `revision`, or in none when loaded.
    pub(crate) fn computed(
        value: Q::Value,
        hash: Hash,
        deps: Box<[Arc<dyn Edge>]>,
        verified: Verified,
    ) -> Self {
        Self { value, hash, deps, verified, input: false }
    }

    /// Whether it holds in `revision` without looking at its trace.
    pub(crate) fn current(&self, revision: Revision) -> bool {
        self.input || self.verified.get() == Some(revision)
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
