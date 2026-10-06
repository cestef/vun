//! The formatter (spec 13, section 5). It changes whitespace only and keeps the line breaks the
//! author wrote; spaces within a line come from the grammar's tables and the role each token has
//! in the CST, indentation from delimiter groups and list entries, and the groups on a line past
//! the width are broken. The width is the one option.

mod roles;
mod spacing;
#[cfg(test)]
mod tests;
mod vun;
mod wrap;
mod write;

use std::collections::BTreeSet;
use std::fmt;

pub use vun::Vun;

use vun_formatter::WIDTH;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use roles::Roles;

/// A parsed file as the formatter lays it out; its `Display` is the formatted text.
pub struct Layout<'a> {
    tokens: &'a Tokens,
    source: &'a str,
    roles: Roles,
    width: usize,
}

impl<'a> Layout<'a> {
    /// The layout of `source`, lexed as `tokens` and parsed as `cst` without errors.
    #[must_use]
    pub fn new(tokens: &'a Tokens, cst: &Cst, source: &'a str) -> Self {
        Self { tokens, source, roles: Roles::of(tokens, cst), width: WIDTH }
    }

    /// This layout with lines of at most `width` columns, where they can be broken, in place of
    /// [`vun_formatter::WIDTH`].
    #[must_use]
    pub fn width(mut self, width: usize) -> Self {
        self.width = width;
        self
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
            if !wrap::explode(&trace, &mut forced, self.width) {
                return f.write_str(&out);
            }
        }
    }
}
