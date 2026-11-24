use std::collections::BTreeSet;

use super::Counts;

/// A group whose count is not its baseline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Drift {
    pub(crate) group: String,
    pub(crate) counted: u32,
    pub(crate) allowed: u32,
}

impl Drift {
    /// Whether the group has more comments than its baseline allows; otherwise fewer, and the
    /// baseline is lowered to follow.
    pub(crate) const fn above(&self) -> bool {
        self.counted > self.allowed
    }
}

impl Counts {
    /// The groups whose count is not the baseline's, in group order; a group the baseline does
    /// not name is allowed none.
    pub(crate) fn drift(&self, baseline: &Self) -> Vec<Drift> {
        let groups: BTreeSet<&String> = self.0.keys().chain(baseline.0.keys()).collect();
        let at = |counts: &Self, group: &String| counts.0.get(group).copied().unwrap_or(0);
        let drifts = groups.into_iter().map(|g| Drift {
            group: g.clone(),
            counted: at(self, g),
            allowed: at(baseline, g),
        });
        drifts.filter(|d| d.counted != d.allowed).collect()
    }
}
