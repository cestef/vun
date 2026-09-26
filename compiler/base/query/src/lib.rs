//! The demand-driven, incremental query engine (spec 13, section 2).
//!
//! A query maps a key to a value; the unit of work is a declaration or a function body, never an
//! expression. Each result is memoized with what computing it read: every memo, with the
//! fingerprint of its value then. In a later revision a memo is reused when everything it read,
//! brought up to date, still has the fingerprint it had, so a dependency recomputed to an equal
//! interface leaves its dependents untouched (early cutoff). A memo is verified at most once per
//! revision. Fingerprints depend on values alone, not on revisions, so memos stay valid across
//! processes once persisted.
//!
//! Each key gets a slot in its query's table the first time it is asked for; reads are recorded,
//! verified and compared by table and slot. Threads share a database without locks on the way to
//! a memo, and one computes each memo while the others needing it wait for it.
//!
//! Inputs are queries whose values are set with [`Db::set`]; every change of an input's
//! interface starts a new revision.

mod claim;
mod cx;
mod db;
mod fingerprint;
mod memo;
mod persist;
mod table;
#[cfg(test)]
mod tests;

use std::fmt;

pub use cx::{Cx, Cycle};
pub use db::Db;
pub use fingerprint::Fingerprint;
pub use persist::Unloaded;

/// A point in the history of inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A function from keys to values, memoized by the engine.
pub trait Query: 'static {
    /// Names the query in cycles and traces.
    const NAME: &'static str;
    /// Whether its values are set with [`Db::set`] rather than computed: an input's memos are
    /// never saved, and one not set holds what it computes until it is.
    const INPUT: bool = false;
    type Key: Clone + Eq + std::hash::Hash + fmt::Debug + Send + Sync + 'static;
    type Value: Clone + std::hash::Hash + Send + Sync + 'static;

    /// Computes the value, reading other queries through `cx`. An input's is what a key that
    /// was never set holds.
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> Self::Value;

    /// What dependents see of the value; they re-run only when it changes. The whole value by
    /// default; a query whose dependents read part of it fingerprints that part.
    fn interface(value: &Self::Value) -> Fingerprint {
        Fingerprint::of(value)
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

/// Declares a query: its documentation, visibility and name, then its context, key and value,
/// then the body computing it, as a function would be written.
///
/// ```ignore
/// query! {
///     /// The length of a file's text.
///     pub Length(cx, file: FileRef) -> usize {
///         cx.get::<FileText>(file).len()
///     }
/// }
/// ```
#[macro_export]
macro_rules! query {
    ($(#[$attr:meta])* $vis:vis $name:ident($cx:ident, $key:ident: $Key:ty) -> $Value:ty $body:block) => {
        $(#[$attr])*
        $vis struct $name;

        impl $crate::Query for $name {
            const NAME: &'static str = stringify!($name);
            type Key = $Key;
            type Value = $Value;

            fn compute($cx: &$crate::Cx<'_>, $key: &$Key) -> $Value $body
        }
    };
}
