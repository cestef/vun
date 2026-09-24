//! Demand-driven type checking interleaved with compile-time evaluation.
//!
//! Types are values, so checking a declaration may require evaluating compile-time code, and
//! evaluating it may require checking more declarations. Both happen on demand, through queries.

mod apply;
pub mod check;
pub mod decl;
pub mod env;
pub mod eval;
pub mod fuel;
pub mod int;
pub mod item;
pub mod layout;
pub mod member;
mod ops;
#[rustfmt::skip]
pub mod primitives;
mod reflect;
pub mod target;
pub mod trap;
pub mod ty;
pub mod typeck;
pub mod visible;
pub mod witness;

pub use primitives::Primitive;

#[cfg(test)]
mod tests;
