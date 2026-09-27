//! The formatter (spec 13, section 5). It changes whitespace only and keeps the line breaks the
//! author wrote; spaces within a line come from the grammar's tables and the role each token has
//! in the CST, indentation from delimiter groups and list entries. It has no options.

mod roles;
mod spacing;
#[cfg(test)]
mod tests;
mod write;

use std::fmt;

use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use roles::Roles;

/// A parsed file as the formatter lays it out; its `Display` is the formatted text.
pub struct Layout<'a> {
    tokens: &'a Tokens,
    source: &'a str,
    roles: Roles,
}

impl<'a> Layout<'a> {
    /// The layout of `source`, lexed as `tokens` and parsed as `cst` without errors.
    #[must_use]
    pub fn new(tokens: &'a Tokens, cst: &Cst, source: &'a str) -> Self {
        Self { tokens, source, roles: Roles::of(tokens, cst) }
    }
}

impl fmt::Display for Layout<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write::Writer::new(self, f).file()
    }
}
