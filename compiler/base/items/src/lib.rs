//! The declarations of the toolchain's packages as typed handles: `lang::Option`,
//! `lang::int::unsigned`, `std::build_Build::new`. `cargo gen` writes a module per namespace and
//! per type with members from `lib/lang` and `lib/std`, so a handle exists exactly when its
//! declaration does, and Rust code never names one by a path in a string.

#[rustfmt::skip]
pub mod lang;
#[rustfmt::skip]
pub mod std;

/// The package `lang`, whose namespaces and declarations are in the module of the same name.
#[allow(non_upper_case_globals, reason = "handles are spelled as their declarations")]
pub const lang: Item = Item::new(&["lang"]);
/// The package `std`.
#[allow(non_upper_case_globals, reason = "handles are spelled as their declarations")]
pub const std: Item = Item::new(&["std"]);

use ::std::fmt;

use vun_grammar::PATH_SEPARATOR;

/// A declaration, by its binding path: the package, its namespaces, then its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    path: &'static [&'static str],
}

impl Item {
    /// Made by the generated modules only, from declarations that exist.
    #[must_use]
    pub(crate) const fn new(path: &'static [&'static str]) -> Self {
        Self { path }
    }

    /// Its binding path, from the package's name to its own.
    #[must_use]
    pub const fn path(self) -> &'static [&'static str] {
        self.path
    }

    /// Its own name, the path's last segment.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.path.last().copied().unwrap_or_default()
    }
}

impl fmt::Display for Item {
    /// The path as written: `lang.mem.Ptr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path.join(PATH_SEPARATOR))
    }
}
