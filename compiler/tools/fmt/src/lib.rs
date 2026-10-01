//! The formatter (spec 13, section 5). It changes whitespace only and keeps the line breaks the
//! author wrote; spaces within a line come from the grammar's tables and the role each token has
//! in the CST, indentation from delimiter groups and list entries, and the groups on a line past
//! `MAX_WIDTH` are broken. It has no options.

mod roles;
mod spacing;
#[cfg(test)]
mod tests;
mod wrap;
mod write;

use std::collections::BTreeSet;
use std::fmt;

pub use wrap::MAX_WIDTH;

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
        // Each pass breaks the outermost group of every line still too long; the file is done
        // once a pass has none left to break.
        let mut forced = BTreeSet::new();
        loop {
            let mut out = String::new();
            let trace = write::Writer::new(self, &mut out, &forced).file();
            if !wrap::explode(&trace, &mut forced) {
                return f.write_str(&out);
            }
        }
    }
}
