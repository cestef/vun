//! Packages on disk: manifests, dependencies, namespaces and preludes (spec 01, section 8, and
//! spec 12, sections 1 to 3).
//!
//! A file belongs to the package of the nearest enclosing `build.ov` that declares `package`; a
//! `build.ov` without it is an ordinary module, such as `std.build`. Every file of a package sees
//! the package under its own name, each dependency under its declared name, and the public names
//! of the prelude's namespaces.
//!
//! Everything is keyed by content: a package by the hash of its `include` tree, a manifest and a
//! file's declarations by the hash of their text, a scope by its package.

mod namespace;
mod scope;
mod spread;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use overt_build::layout::{Location, MANIFEST};
use overt_build::manifest::{self, DEFAULT_DEPS, DEFAULT_PRELUDE, Manifest, Source};
use overt_hash::Hash;
use overt_intern::{Interner, Symbol};
use overt_span::{FileId, SourceMap, Span};

pub use namespace::{Namespace, Unfound};
pub use scope::{Outer, Scope};
pub use spread::{FileScope, Spreads};

use crate::Session;

/// The library shipped with the toolchain, where default dependencies are found. Until the
/// launcher installs toolchains (spec 12, section 6), it is the repository's `lib/`.
pub const LIBRARY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../lib");

/// Identifies a package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackageKey {
    /// The hash of its `include` tree.
    Content(Hash),
    /// Files outside any package: the default dependencies and prelude, and nothing else.
    Loose,
}

pub struct Package {
    /// `None` for [`PackageKey::Loose`].
    pub name: Option<Symbol>,
    pub dir: PathBuf,
    pub deps: Vec<Dep>,
    /// Namespace paths, such as `std.prelude`.
    pub prelude: Vec<String>,
    /// Where a manifest declares the prelude.
    pub prelude_span: Option<Span>,
}

/// A dependency: the name it is bound to, its directory, and where it is declared.
#[derive(Clone)]
pub struct Dep {
    pub name: Symbol,
    /// `None` for an archive the local store does not hold yet.
    pub dir: Option<PathBuf>,
    /// `None` for a default dependency.
    pub span: Option<Span>,
}

#[derive(Default)]
pub struct Packages {
    pub list: HashMap<PackageKey, Package>,
    /// Manifests by the hash of their text; `None` for a `build.ov` that declares no `package`.
    manifests: HashMap<Hash, Option<Manifest>>,
    /// What files declare, by the hash of their text.
    declared: HashMap<Hash, namespace::Declared>,
    /// Namespaces whose exports are being read, against spreads that loop.
    expanding: HashSet<Location>,
    pub(crate) scopes: HashMap<PackageKey, Scope>,
}

impl Package {
    /// Adds the default dependencies the package neither declares nor is, from `library`.
    fn with_defaults(mut self, library: &Path, interner: &mut Interner) -> Self {
        for &default in DEFAULT_DEPS {
            let name = interner.intern(default);
            if self.name != Some(name) && self.deps.iter().all(|d| d.name != name) {
                self.deps.push(Dep { name, dir: Some(library.join(default)), span: None });
            }
        }
        self
    }

    /// Where its root namespace lives.
    fn root(&self, interner: &Interner) -> Option<Location> {
        self.name.map(|name| Location::root(&self.dir, interner.resolve(name)))
    }
}

impl Session {
    /// The package `file` belongs to.
    pub fn package_of(&mut self, file: FileId) -> PackageKey {
        let path = PathBuf::from(&self.sources.file(file).path);
        let found = path.ancestors().skip(1).find_map(|dir| self.package_at(dir));
        found.unwrap_or_else(|| self.loose())
    }

    /// The package whose manifest is in `dir`, if there is one.
    pub fn package_at(&mut self, dir: &Path) -> Option<PackageKey> {
        let path = dir.join(MANIFEST);
        let manifest = self.manifest_at(&path.is_file().then_some(path)?)?;
        let key = PackageKey::Content(Hash::tree(dir, &manifest.include));
        if !self.packages.list.contains_key(&key) {
            let name = Some(self.interner.intern(&manifest.name));
            let deps = manifest.deps.iter().map(|d| Dep {
                name: self.interner.intern(&d.name),
                dir: match &d.source {
                    Source::Path(path) => Some(dir.join(path)),
                    Source::Archive { .. } => None,
                },
                span: Some(d.span),
            });
            let package = Package {
                name,
                dir: dir.to_path_buf(),
                deps: deps.collect(),
                prelude: manifest.prelude,
                prelude_span: Some(manifest.prelude_span),
            };
            let package = package.with_defaults(&self.library, &mut self.interner);
            self.packages.list.insert(key, package);
        }
        Some(key)
    }

    /// The manifest in the `build.ov` at `path`, read once per content.
    fn manifest_at(&mut self, path: &Path) -> Option<Manifest> {
        let text = self.read(path)?;
        let hash = Hash::of(text.as_bytes());
        if let Some(known) = self.packages.manifests.get(&hash) {
            return known.clone();
        }
        let file = self.sources.add(path.display().to_string(), text);
        let manifest = self.declarations(file, |decls, cx| {
            manifest::declared(decls).then(|| manifest::read(decls, file, cx)).flatten()
        });
        self.packages.manifests.insert(hash, manifest.clone());
        manifest
    }

    fn loose(&mut self) -> PackageKey {
        if !self.packages.list.contains_key(&PackageKey::Loose) {
            let prelude = vec![DEFAULT_PRELUDE.to_owned()];
            let package =
                Package { name: None, dir: PathBuf::new(), deps: Vec::new(), prelude, prelude_span: None };
            let package = package.with_defaults(&self.library, &mut self.interner);
            self.packages.list.insert(PackageKey::Loose, package);
        }
        PackageKey::Loose
    }
}
