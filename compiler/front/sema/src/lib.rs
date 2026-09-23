//! Demand-driven type checking interleaved with compile-time evaluation.
//!
//! Types are values, so checking a declaration may require evaluating compile-time code, and
//! evaluating it may require checking more declarations. Both happen on demand, through queries.

pub mod check;
pub mod eval;
pub mod int;
pub mod layout;
#[rustfmt::skip]
pub mod primitives;
pub mod ty;

pub use primitives::Primitive;

#[cfg(test)]
mod tests;
