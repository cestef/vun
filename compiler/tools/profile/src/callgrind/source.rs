//! Where a piece of code comes from, read off the path callgrind names for it.

use std::fmt;
use std::path::{Component, Path};

use vun_intern::Symbol;

/// The directory the compiler's crates are under, then their group and their own directory.
const WORKSPACE: &str = "compiler";
/// The directory of the registry's sources, then an index and `<crate>-<version>`.
const REGISTRY: &str = "registry";
/// The directory of the toolchain's libraries, under `/rustc/<hash>/`.
const TOOLCHAIN: &str = "library";
const TOOLCHAIN_ROOT: &str = "rustc";

/// The crate or object a cost belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// A crate of this workspace: `compiler/<group>/<name>`.
    Workspace { group: Symbol, name: Symbol },
    /// A crate from the registry, without its version.
    Registry(Symbol),
    /// A library of the toolchain: `core`, `alloc`, `std`.
    Toolchain(Symbol),
    /// Anything else: the object file that holds the code (the C library, the loader).
    System(Symbol),
}

impl Source {
    /// The source of code that debug information places in `file`, within `object`.
    #[must_use]
    pub fn of(file: &str, object: &str) -> Self {
        let parts: Vec<&str> = Path::new(file)
            .components()
            .filter_map(|c| if let Component::Normal(part) = c { part.to_str() } else { None })
            .collect();
        if let Some(at) = parts.iter().position(|&p| p == WORKSPACE)
            && let [group, name, _, ..] = parts[at + 1..]
        {
            return Self::Workspace { group: Symbol::intern(group), name: Symbol::intern(name) };
        }
        if let Some(at) = parts.iter().position(|&p| p == REGISTRY)
            && let Some(versioned) = parts.get(at + 3)
        {
            return Self::Registry(Symbol::intern(Self::unversioned(versioned)));
        }
        if parts.first() == Some(&TOOLCHAIN_ROOT)
            && let Some(at) = parts.iter().position(|&p| p == TOOLCHAIN)
            && let Some(library) = parts.get(at + 1)
        {
            return Self::Toolchain(Symbol::intern(library));
        }
        let name = Path::new(object).file_name().and_then(|n| n.to_str()).unwrap_or(object);
        Self::System(Symbol::intern(name))
    }

    /// `blake3-1.8.7` without its version: the part before the last `-` that a digit follows.
    fn unversioned(directory: &str) -> &str {
        directory
            .rmatch_indices('-')
            .find(|(at, _)| directory[at + 1..].starts_with(|c: char| c.is_ascii_digit()))
            .map_or(directory, |(at, _)| &directory[..at])
    }

    /// Whether the code is this project's own.
    #[must_use]
    pub const fn is_workspace(self) -> bool {
        matches!(self, Self::Workspace { .. })
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace { group, name } => write!(f, "{}/{}", group.as_str(), name.as_str()),
            Self::Registry(name) | Self::Toolchain(name) | Self::System(name) => f.write_str(name.as_str()),
        }
    }
}
