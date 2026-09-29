//! `cargo xtask layers`: a crate depends only on crates of its own group or of an earlier one
//! (spec 13, section 9). A group is a directory under `compiler/`; the order is declared here once,
//! and cargo's metadata says what each crate depends on.

use std::collections::BTreeMap;
use std::path::Path;

use crate::cargo::{self, CargoFailed};

/// The groups by rank: a group may depend on those of a lower rank. Groups of one rank are peers
/// and depend on none of each other.
pub(crate) const RANKS: &[&[&str]] = &[
    &["base"],
    &["lang"],
    &["isa", "object"],
    &["front"],
    &["sema"],
    &["mid"],
    &["low"],
    &["asm"],
    &["back", "link", "render"],
    &["native", "pkg"],
    &["tools"],
];

/// The directory holding the groups, relative to the repository root.
const GROUPS: &str = "compiler";

const METADATA: [&str; 4] = ["metadata", "--no-deps", "--format-version", "1"];

/// A crate of the workspace and its group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) name: String,
    pub(crate) group: String,
}

impl Placed {
    /// Its group's rank, if the group is ranked.
    fn rank(&self) -> Option<usize> {
        RANKS.iter().position(|peers| peers.contains(&self.group.as_str()))
    }
}

/// A dependency breaking the order: from a crate on one of a later group or a peer, or a crate
/// in a group with no rank.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Breach {
    Upward { from: Placed, to: Placed },
    Unranked(Placed),
}

impl Breach {
    /// Every breach in the workspace at `root`, in crate order.
    ///
    /// # Errors
    /// Returns why cargo reported no metadata.
    pub(crate) fn all(root: &Path) -> Result<Vec<Self>, CargoFailed> {
        let metadata = cargo::output(root, &METADATA)?;
        Ok(Self::within(&metadata, &root.join(GROUPS)))
    }

    /// The breaches cargo's `metadata` shows among the crates under `groups`.
    pub(crate) fn within(metadata: &str, groups: &Path) -> Vec<Self> {
        let Ok(metadata) = vun_data::json::parse(metadata) else { return Vec::new() };
        let packages = metadata["packages"].items();
        let mut placed = BTreeMap::new();
        let mut depends = Vec::new();
        for package in packages {
            let (Some(name), Some(manifest)) = (package["name"].as_str(), package["manifest_path"].as_str())
            else {
                continue;
            };
            let Ok(inside) = Path::new(manifest).strip_prefix(groups) else { continue };
            let Some(group) = inside.components().next() else { continue };
            let group = group.as_os_str().to_string_lossy().into_owned();
            placed.insert(name.to_owned(), Placed { name: name.to_owned(), group });
            depends.extend(package["dependencies"].items().filter_map(|d| Some((name, d["name"].as_str()?))));
        }
        let mut breaches: Vec<Self> =
            placed.values().filter(|p| p.rank().is_none()).cloned().map(Self::Unranked).collect();
        for (from, to) in depends {
            let (Some(from), Some(to)) = (placed.get(from), placed.get(to)) else { continue };
            let (Some(a), Some(b)) = (from.rank(), to.rank()) else { continue };
            if from.group != to.group && b >= a {
                breaches.push(Self::Upward { from: from.clone(), to: to.clone() });
            }
        }
        breaches
    }
}
