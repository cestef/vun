//! `vun cover --lcov`: what a run reached, in the line-oriented tracefile format of lcov (`SF`,
//! `FN`, `FNDA`, `DA`, `end_of_record`): a function is named by the path of its declaration, else
//! by where it begins, as `line:column`.

use std::fmt;

use super::reach::{Function, Start};
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
        let named = |(line, column): Start, function: &Function| {
            function.name.clone().unwrap_or_else(|| format!("{line}:{column}"))
        };
        for (&start @ (line, _), function) in &self.functions {
            writeln!(f, "FN:{line},{}", named(start, function))?;
        }
        for (&start, function) in &self.functions {
            writeln!(f, "FNDA:{},{}", function.calls, named(start, function))?;
        }
        writeln!(f, "FNF:{}\nFNH:{}", self.functions.len(), self.functions_called())?;
        for (line, count) in &self.lines {
            writeln!(f, "DA:{line},{count}")?;
        }
        writeln!(f, "LF:{}\nLH:{}", self.lines.len(), self.lines_hit())
    }
}
