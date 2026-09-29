//! Choosing among kinds that may spell the same source: which is preferred (`preferred`).

use crate::{NodeKind, PREFERRED};

impl NodeKind {
    /// How many kinds are preferred to this one, one before another: the longest chain of kinds
    /// listed before it in rows of `preferred`.
    #[must_use]
    pub fn rank(self) -> usize {
        let before = PREFERRED.iter().filter_map(|row| Some(&row[..row.iter().position(|&k| k == self)?]));
        before.flatten().map(|k| k.rank() + 1).max().unwrap_or(0)
    }
}
