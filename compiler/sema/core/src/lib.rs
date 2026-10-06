//! Demand-driven type checking interleaved with compile-time evaluation.
//!
//! Types are values, so checking a declaration may require evaluating compile-time code, and
//! evaluating it may require checking more declarations. Both happen on demand, through queries.

mod apply;
pub mod asm;
pub mod attributes;
pub mod callable;
pub mod checks;
pub mod cover;
pub mod decl;
pub mod env;
pub mod eval;
pub mod fuel;
pub mod gate;
pub mod installed;
pub mod kinds;
pub mod layout;
pub mod link;
pub mod member;
pub mod omitted;
mod persisted;
pub mod reflect;
pub mod site;
pub mod target;
pub mod testing;
pub mod thread;
pub mod trace;
pub mod typed;
pub mod value;
pub mod visible;
pub mod witness;

pub use apply::Apply;
pub use persisted::PERSISTED;

#[cfg(test)]
mod tests;
