//! A memoized result: the value, its fingerprint, what computing it read, and the revision it was
//! last known current in, which any thread verifying it records.

use std::sync::atomic::{AtomicU64, Ordering};

use super::fingerprint::Fingerprint;
use super::table::{Slot, TableId};
use super::{Query, Revision};

pub(crate) struct Memo<Q: Query> {
    /// `None` for a memo loaded without its value: it stands for its fingerprint, and the value
    /// is computed again when read.
    pub(crate) value: Option<Q::Value>,
    pub(crate) fingerprint: Fingerprint,
    /// Each memo read, once, in the order of the first reads.
    pub(crate) deps: Box<[Dep]>,
    /// None for a loaded memo until it is verified.
    pub(crate) verified: Verified,
    /// Set with [`Db::set`](super::Db::set) rather than computed.
    pub(crate) input: bool,
}

/// A memo read while another was computed, with the fingerprint it had then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Dep {
    pub(crate) table: TableId,
    pub(crate) slot: Slot,
    pub(crate) fingerprint: Fingerprint,
}

impl<Q: Query> Memo<Q> {
    pub(crate) fn input(value: Q::Value, fingerprint: Fingerprint, revision: Revision) -> Self {
        Self {
            value: Some(value),
            fingerprint,
            deps: Box::new([]),
            verified: Verified::at(revision),
            input: true,
        }
    }

    pub(crate) fn computed(
        value: Option<Q::Value>,
        fingerprint: Fingerprint,
        deps: Box<[Dep]>,
        verified: Verified,
    ) -> Self {
        Self { value, fingerprint, deps, verified, input: false }
    }

    /// Whether it holds in `revision` without looking at what it read.
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
