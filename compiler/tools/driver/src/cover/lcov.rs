//! `vun cover --lcov`: what a run reached, in the line-oriented tracefile format of lcov (`SF`,
//! `FN`, `FNDA`, `DA`, `end_of_record`). A flag says a block ran, not how many times, so a count is
//! 0 or 1, and a function is named by where it begins, as `line:column`.

use std::fmt;

use super::{Reach, Source};

/// A [`Reach`] written as an lcov tracefile.
pub struct Lcov<'a>(pub &'a Reach);

impl fmt::Display for Lcov<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (path, source) in &self.0.files {
            writeln!(f, "TN:\nSF:{}", path.display())?;
            source.write(f)?;
            writeln!(f, "end_of_record")?;
        }
        Ok(())
    }
}

impl Source {
    fn write(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &(line, column) in self.functions.keys() {
            writeln!(f, "FN:{line},{line}:{column}")?;
        }
        for (&(line, column), &hit) in &self.functions {
            writeln!(f, "FNDA:{},{line}:{column}", u8::from(hit))?;
        }
        writeln!(f, "FNF:{}\nFNH:{}", self.functions.len(), Self::reached(&self.functions))?;
        for (line, &hit) in &self.lines {
            writeln!(f, "DA:{line},{}", u8::from(hit))?;
        }
        writeln!(f, "LF:{}\nLH:{}", self.lines.len(), Self::reached(&self.lines))
    }
}
