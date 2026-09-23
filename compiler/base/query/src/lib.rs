//! The demand-driven, incremental query engine (spec 13, section 2).
//!
//! A query maps a key to a value; the unit of work is a declaration or a function body, never an
//! expression. Each result is memoized with a verifying trace: the interface hash of every query
//! it read. In a later revision a memo is reused when every dependency, brought up to date, still
//! has the interface hash it had, so a dependency recomputed to an equal interface leaves its
//! dependents untouched (early cutoff). Traces hold content hashes, not revisions, so they stay
//! valid across processes once persisted. A memo is verified at most once per revision.
//!
//! Inputs are queries whose values are set with [`Db::set`]; every change of an input's
//! interface starts a new revision.

mod cx;
mod db;
#[cfg(test)]
mod tests;

use std::fmt;

use overt_hash::Hash;

pub use cx::{Cx, Cycle};
pub use db::Db;

/// A point in the history of inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A function from keys to values, memoized by the engine.
pub trait Query: 'static {
    /// Names the query in cycles and traces.
    const NAME: &'static str;
    type Key: Clone + Eq + std::hash::Hash + fmt::Debug + Send + Sync + 'static;
    type Value: Clone + std::hash::Hash + Send + Sync + 'static;

    /// Computes the value, reading other queries through `cx`. An input's is what a key that
    /// was never set holds.
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> Self::Value;

    /// What dependents see of the value; they re-run only when it changes. The whole value by
    /// default; a query whose dependents read part of it hashes that part.
    fn interface(value: &Self::Value) -> Hash {
        Hash::of_value(value)
    }

    /// The value of a query that depends on itself.
    ///
    /// # Panics
    /// By default, with the cycle, for queries where one is a bug.
    #[must_use]
    fn cycle(_: &Cx<'_>, _: &Self::Key, cycle: &Cycle) -> Self::Value {
        panic!("{cycle}")
    }
}
