//! Tree-sitter grammars and queries as typed values.
//!
//! A [`Grammar`] is written as the `grammar.js` the tree-sitter CLI reads, and its [`Config`] as
//! `tree-sitter.json`; a [`Query`] as the `.scm` files editors read. A [`Tree`] is read back from
//! what `tree-sitter parse` prints, so a grammar's trees can be compared with another parser's. A
//! [`Release`] names the revision of a grammar's repository that editors build.

mod config;
mod grammar;
mod query;
mod release;
mod rule;
#[cfg(test)]
mod tests;
mod tree;

pub use config::Config;
pub use grammar::Grammar;
pub use query::{Pattern, Query};
pub use release::Release;
pub use rule::{Assoc, Rule};
pub use tree::{Malformed, Node, Point, Tree};
