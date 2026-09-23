//! The demand-driven, incremental query engine.
//!
//! Design constraints (spec 13, section 2):
//! - the unit of work is a declaration or a function body, never an expression;
//! - every result has an interface hash, and dependents re-run only when it changed (early cutoff);
//! - results persist to disk between builds, with a bounded cache for bodies;
//! - evaluation is parallel, and results never depend on scheduling order.

/// A point in the history of inputs. Every input change creates a new revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A stable 128-bit hash of a query result's interface, used for early cutoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub u128);
