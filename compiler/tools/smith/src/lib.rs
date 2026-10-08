//! Programs made by chance, well typed by construction (spec 11): the generator draws from an
//! [`Entropy`], builds typed forms of the model ([`ir`]), and prints them as Vun source
//! through `vun_syntax::make` ([`lower`]). What the programs say is known, so what the compiler
//! does with them can be held against it.

#![allow(
    clippy::match_same_arms,
    reason = "a table lists every form with its weight, even where weights agree"
)]

pub mod draw;
pub mod entropy;
pub mod eval;
pub mod float;
pub mod int;
pub mod ir;
pub mod lower;
pub mod reduce;
pub mod ty;
pub mod value;

pub use entropy::Entropy;

#[cfg(test)]
mod tests;
