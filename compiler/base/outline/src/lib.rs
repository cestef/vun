//! Text laid out as an indented tree, one node per line, for dumps of every representation.

use std::fmt::{self, Write};

/// Lines indented by their depth in a tree.
#[derive(Default)]
pub struct Outline {
    out: String,
    depth: usize,
}

impl Outline {
    /// Indentation per tree level.
    pub const INDENT: usize = 2;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes one line at the current depth.
    pub fn line(&mut self, text: fmt::Arguments<'_>) {
        let indent = self.depth * Self::INDENT;
        let _ = writeln!(self.out, "{:indent$}{text}", "");
    }

    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }
}

/// A printer writing into an [`Outline`] while it walks a tree.
pub trait Outlined {
    fn outline(&mut self) -> &mut Outline;

    /// Writes a line, then what `children` writes one level deeper.
    fn node(&mut self, text: fmt::Arguments<'_>, children: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        let outline = self.outline();
        outline.line(text);
        outline.depth += 1;
        children(self);
        self.outline().depth -= 1;
    }
}

#[cfg(test)]
mod tests;
