//! Workspaces (spec 12, section 5): a root package listing packages below it as its members
//! (`.members`). The members are one project: the root's graph is theirs together, and they share
//! the root's lockfile and work directory.

use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_fs::Tree;

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

    /// The directory of the project the file at `path` is part of: its package's, or its
    /// workspace's root for a member's file; outside any package, the file's own directory.
    pub fn project_dir(&mut self, path: &Path) -> PathBuf {
        let dir = path.parent().unwrap_or(Path::new(""));
        let around = self.package_around(dir);
        around.map_or_else(|| dir.to_path_buf(), |(found, key)| self.project_of(&found, key).0)
    }

    /// The work directory of the file at `path` (`vun_fs::WORK`): the one of its project, where
    /// its builds, saved queries and views are kept.
    pub fn work(&mut self, path: &Path) -> PathBuf {
        self.project_dir(path).join(vun_fs::WORK)
    }

    /// The project the package `key` in `dir` is part of: the workspace listing it as a member,
    /// else the package itself.
    pub(crate) fn project_of(&mut self, dir: &Path, key: PackageKey) -> (PathBuf, PackageKey) {
        let Ok(own) = dir.resolved() else { return (dir.to_path_buf(), key) };
        for above in own.ancestors().skip(1) {
            let Some(root) = self.package_at(above) else { continue };
            let members = self.members(root);
            if members.iter().any(|member| member.resolved().is_ok_and(|m| m == own)) {
                return (above.to_path_buf(), root);
            }
        }
        (dir.to_path_buf(), key)
    }
}
