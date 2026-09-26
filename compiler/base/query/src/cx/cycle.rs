//! Queries that depend on themselves.

use std::fmt;

/// Queries that depend on themselves: each by name and key, from the one reached again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub path: Vec<(&'static str, String)>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("query cycle: ")?;
        for (i, (name, key)) in self.path.iter().enumerate() {
            if i > 0 {
                f.write_str(" then ")?;
            }
            write!(f, "{name}({key})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Cycle {}
