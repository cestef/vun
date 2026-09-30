//! A target's path in the package `targets`, `<os>.<arch>.<abi>`, as `--target` writes it.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_grammar::PATH_SEPARATOR;
use vun_intern::Symbol;
use vun_items::Item;

/// The names leading to a target inside the package `targets`: `linux.x86_64.kernel`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct TargetPath(pub Vec<Symbol>);

impl TargetPath {
    /// The path written as `--target` takes it.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self(text.split(PATH_SEPARATOR).map(Symbol::intern).collect())
    }

    /// The path of the target a handle of `vun_items::targets` names.
    #[must_use]
    pub fn of(target: Item) -> Self {
        Self(target.path().iter().skip(1).map(|&name| Symbol::intern(name)).collect())
    }

    /// The names from the package's own: `targets.linux.x86_64.kernel`.
    pub fn from_package(&self) -> impl Iterator<Item = Symbol> + '_ {
        std::iter::once(vun_items::targets.symbol()).chain(self.0.iter().copied())
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.0.iter().map(|n| n.as_str()).collect();
        f.write_str(&names.join(PATH_SEPARATOR))
    }
}
