//! Integers as the spec types them (spec 04, section 1): every integer type is a range, and the
//! range of a result follows from the ranges of its operands, by interval arithmetic.
//!
//! This is the model's own: it does not call the compiler's (`vun-sema-int`), so that the ranges
//! the compiler finds are compared with another account of the rules.

mod arithmetic;
mod range;

pub use num_bigint::BigInt;
pub use range::{Range, Width};
