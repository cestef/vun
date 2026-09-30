//! Workspaces (spec 12, section 5): a root package listing packages below it as its members
//! (`.members`). The members are one project: the root's graph is theirs together, and they share
//! the root's lockfile.

use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;

use super::PackageKey;
use crate::Session;

impl Session {
    /// The directories of the members `key` lists; none for a package that roots no workspace.
    pub fn members(&mut self, key: PackageKey) -> Vec<PathBuf> {
        if key == PackageKey::Loose {
            return Vec::new();
        }
        let dir = self.packages.list[&key].dir.clone();
        let listed = self.manifest_at(&dir.join(MANIFEST)).map(|m| m.members).unwrap_or_default();
        listed.iter().map(|member| dir.join(member)).collect()
    }

    /// The packages of the members `key` lists, skipping a member that holds none.
    pub(super) fn member_packages(&mut self, key: PackageKey) -> Vec<PackageKey> {
        self.members(key).iter().filter_map(|dir| self.package_at(dir)).collect()
    }

    /// The project the package `key` in `dir` is part of: the workspace listing it as a member,
    /// else the package itself.
    pub(crate) fn project_of(&mut self, dir: &Path, key: PackageKey) -> (PathBuf, PackageKey) {
        let Ok(own) = dir.canonicalize() else { return (dir.to_path_buf(), key) };
        for above in own.ancestors().skip(1) {
            let Some(root) = self.package_at(above) else { continue };
            let members = self.members(root);
            if members.iter().any(|member| member.canonicalize().is_ok_and(|m| m == own)) {
                return (above.to_path_buf(), root);
            }
        }
        (dir.to_path_buf(), key)
    }
}
