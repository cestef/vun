//! Checking reuse: a memo the engine reuses without computing it, as one whose reads still hold or
//! one loaded from saved queries, must be what computing it again gives. Under
//! [`Reuse::Check`] each such memo is computed again, kept nowhere, and compared with, so that a
//! value whose fingerprint misses what it depends on, a read not recorded, or a key naming
//! different things in two revisions fails where it is reused instead of giving a wrong result.

use std::fmt;

use super::Cx;
use super::frame::Standing;
use super::why::Why;
use crate::Query;
use crate::memo::Memo;
use crate::table::{AnyTable, Slot, Table};
use crate::tally::Event;

/// Whether memos reused without being computed are trusted, or computed again to check them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reuse {
    #[default]
    Trust,
    Check,
}

impl Reuse {
    /// The variable asking for [`Reuse::Check`] with `1`.
    pub const VAR: &'static str = "VUN_QUERY_CHECK";

    /// What the environment asks for.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(Self::VAR).as_deref() {
            Ok("1") => Self::Check,
            _ => Self::Trust,
        }
    }
}

/// A reused memo computing it again contradicts.
#[derive(Debug)]
pub struct Unsound {
    pub query: &'static str,
    pub key: String,
    pub differs: Differs,
}

/// What of a reused memo computing it again gives otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Differs {
    /// Its fingerprint: what dependents compared it by.
    Fingerprint,
    /// Its value, under an equal fingerprint: the fingerprint misses part of what the value is.
    Value,
    /// What it read: verifying ahead found it to hold, verifying in order to have changed.
    Settled,
}

impl fmt::Display for Unsound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.differs {
            Differs::Fingerprint => "computing it again changes its fingerprint",
            Differs::Value => "computing it again changes its value under an equal fingerprint",
            Differs::Settled => "verifying it in order finds a read changed",
        };
        write!(f, "memo {}({}) was reused, but {what}", self.query, self.key)
    }
}

impl std::error::Error for Unsound {}

impl Cx<'_> {
    /// Computes `memo`, found to hold at `slot`, again and compares; a query computing on this
    /// thread already is left to the cycle it is part of.
    ///
    /// # Panics
    /// With [`Unsound`] when computing it again differs.
    #[inline(never)]
    pub(super) fn check<Q: Query>(&self, table: &Table<Q>, slot: Slot, memo: &Memo<Q>) {
        if self.computing.borrow().contains_key(&(table.id, slot)) {
            return;
        }
        table.counts.add(Event::Checked);
        let entered = self.enter(table.id, slot, Why::Checked);
        let value = Q::compute(self, table.key(slot));
        let (_, standing) = entered.leave();
        if standing == Standing::Provisional {
            return;
        }
        let differs = if Q::interface(&value) != memo.fingerprint {
            Some(Differs::Fingerprint)
        } else if memo.value(self.db).is_some_and(|reused| *reused != value) {
            Some(Differs::Value)
        } else {
            None
        };
        if let Some(differs) = differs {
            Self::unsound(table, slot, differs);
        }
    }

    /// Fails with [`Unsound`] for the memo at `slot` of `table`.
    ///
    /// # Panics
    /// Always.
    pub(super) fn unsound<Q: Query>(table: &Table<Q>, slot: Slot, differs: Differs) -> ! {
        let unsound = Unsound { query: Q::NAME, key: AnyTable::show(table, slot), differs };
        panic!("{unsound}");
    }
}
