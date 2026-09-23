//! The dependency graph of a project as its manifests declare it: every package reached once,
//! through the project's replacements. Selection, fetching and the lockfile walk it; the versions
//! selection chooses are among the declared ones, so the declared graph reaches all of them.

use std::collections::HashSet;

use super::{Dep, PackageKey};
use crate::Session;

impl Session {
    /// Visits every dependency of every package reachable from `project`, with its dependent.
    /// `visit` returns the package it loaded for the dependency, whose own dependencies are
    /// visited next, or those of its replacement when the project replaces its identity.
    pub(super) fn walk(
        &mut self,
        project: PackageKey,
        mut visit: impl FnMut(&mut Self, PackageKey, &Dep) -> Option<PackageKey>,
    ) {
        let mut seen = HashSet::new();
        let mut pending = vec![project];
        while let Some(package) = pending.pop() {
            if !seen.insert(package) {
                continue;
            }
            for dep in self.packages.list[&package].deps.clone() {
                let Some(loaded) = visit(self, package, &dep) else { continue };
                let identity = self.packages.list[&loaded].release.map(|(identity, _)| identity);
                let replaced = identity.and_then(|i| self.replacement_in(project, i));
                let walked = match replaced {
                    Some(replace) => self.found(&replace.dep).ok(),
                    None => Some(loaded),
                };
                pending.extend(walked);
            }
        }
    }
}
