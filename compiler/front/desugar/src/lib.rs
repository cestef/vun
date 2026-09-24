//! What syntax desugars into (appendix B): the operators' interfaces, the forms, type prefixes,
//! borrows with their place transitions, markers, calls, early returns and iteration, generated
//! from `lib/lang/lang/syntax/desugar.vun` by `cargo gen`. Apart from `vun-grammar`, which the
//! parser and the generator are built from, so that no table can keep the generator from building
//! (decision 132).

#[rustfmt::skip]
mod generated;

pub use generated::*;

/// Separates the words of a borrow mode in the table `borrows`: `temp mut`.
pub const MODE_SEPARATOR: &str = " ";
