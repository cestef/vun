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

mod add;
mod fetch;
mod namespace;
mod scope;
mod select;
mod spread;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use overt_build::layout::{Location, MANIFEST};
use overt_build::lock::Lock;
use overt_build::manifest::{
    self, DEFAULT_DEPS, DEFAULT_PRELUDE, DepKind, Identity, Manifest, Source, Version,
};
use overt_diag::{Diagnostic, Msg, errors};
use overt_hash::{Hash, Snapshot};
use overt_intern::{Interner, Symbol};
use overt_span::{FileId, SourceMap, Span};

pub use add::{Addition, Unstated};
pub use fetch::Fetched;
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
    /// The hashes of its files, from which its key is computed; empty for [`PackageKey::Loose`].
    pub snapshot: Snapshot,
    /// Its identity across versions and its version; `None` for [`PackageKey::Loose`].
    pub release: Option<(Identity, Version)>,
}

/// A dependency: the name it is bound to, where it comes from, and where it is declared.
#[derive(Clone)]
pub struct Dep {
    pub name: Symbol,
    pub source: DepSource,
    pub kind: DepKind,
    /// `None` for a default dependency.
    pub span: Option<Span>,
}

#[derive(Clone)]
pub enum DepSource {
    /// A directory on disk.
    Dir(PathBuf),
    /// A package in the store, by its tree hash, with its declared version and the mirrors that
    /// serve it.
    Stored { hash: Hash, version: Version, urls: Vec<String> },
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
    /// The project: the package whose graph version selection runs over.
    pub root: Option<PackageKey>,
    /// What selection chose for the project's graph, computed once.
    selection: Option<Lock>,
}

impl Package {
    /// Adds the default dependencies the package neither declares nor is, from `library`.
    fn with_defaults(mut self, library: &Path, interner: &mut Interner) -> Self {
        for &default in DEFAULT_DEPS {
            let name = interner.intern(default);
            if self.name != Some(name) && self.deps.iter().all(|d| d.name != name) {
                let source = DepSource::Dir(library.join(default));
                self.deps.push(Dep { name, source, kind: DepKind::Overt, span: None });
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
    /// The package `file` belongs to. The first one found is the project, whose graph version
    /// selection runs over.
    pub fn package_of(&mut self, file: FileId) -> PackageKey {
        let path = PathBuf::from(&self.sources.file(file).path);
        let found = path.ancestors().skip(1).find_map(|dir| self.package_at(dir));
        let key = found.unwrap_or_else(|| self.loose());
        if self.packages.root.is_none() {
            self.packages.root = found;
        }
        key
    }

    /// The project around the current directory; its absence is reported.
    ///
    /// # Errors
    /// Returns an error if the current directory cannot be read.
    pub fn project(&mut self) -> std::io::Result<Option<(PathBuf, PackageKey)>> {
        let here = std::env::current_dir()?;
        let found = here.ancestors().find_map(|d| self.package_at(d).map(|p| (d.to_path_buf(), p)));
        if let Some((_, key)) = &found {
            self.packages.root = Some(*key);
        } else {
            let msg = Msg::new().text("no ").code(MANIFEST).text(" declaring ").code(manifest::PACKAGE);
            let msg = msg.text(" in this directory or above");
            self.diag.error(Diagnostic::error(errors::PACKAGE_NO_PROJECT, msg));
        }
        Ok(found)
    }

    /// Where a dependency is on disk; a stored one in the project's store.
    fn dep_dir(&self, source: &DepSource) -> PathBuf {
        match source {
            DepSource::Dir(dir) => dir.clone(),
            DepSource::Stored { hash, .. } => self.store.package(*hash),
        }
    }

    /// The package whose manifest is in `dir`, if there is one.
    pub fn package_at(&mut self, dir: &Path) -> Option<PackageKey> {
        let path = dir.join(MANIFEST);
        let manifest = self.manifest_at(&path.is_file().then_some(path)?)?;
        let snapshot = Snapshot::of(dir, &manifest.include);
        let key = PackageKey::Content(snapshot.root());
        if !self.packages.list.contains_key(&key) {
            let name = Some(self.interner.intern(&manifest.name));
            let deps = manifest.deps.iter().map(|d| Dep {
                name: self.interner.intern(&d.name),
                source: match &d.source {
                    Source::Path(path) => DepSource::Dir(dir.join(path)),
                    Source::Archive { hash, version, urls } => {
                        DepSource::Stored { hash: *hash, version: *version, urls: urls.clone() }
                    }
                },
                kind: d.kind,
                span: Some(d.span),
            });
            let release = Some((manifest.identity(), manifest.version));
            let package = Package {
                name,
                dir: dir.to_path_buf(),
                deps: deps.collect(),
                prelude: manifest.prelude,
                prelude_span: Some(manifest.prelude_span),
                snapshot,
                release,
            };
            let package = package.with_defaults(&self.library, &mut self.interner);
            self.packages.list.insert(key, package);
        }
        Some(key)
    }

    /// The manifest in the `build.ov` at `path`, read once per content.
    pub fn manifest_at(&mut self, path: &Path) -> Option<Manifest> {
        let read = |s: &mut Self, file| {
            s.declarations(file, |decls, cx| {
                manifest::declared(decls).then(|| manifest::read(decls, file, cx)).flatten()
            })
        };
        self.by_content(path, |p| &mut p.manifests, read).flatten()
    }

    /// What `derive` computes from the file at `path`, once per content: `cache` selects where
    /// results are kept by the hash of the text.
    fn by_content<V: Clone>(
        &mut self,
        path: &Path,
        cache: fn(&mut Packages) -> &mut HashMap<Hash, V>,
        derive: impl FnOnce(&mut Self, FileId) -> V,
    ) -> Option<V> {
        let text = self.read(path)?;
        let hash = Hash::of(text.as_bytes());
        if let Some(known) = cache(&mut self.packages).get(&hash) {
            return Some(known.clone());
        }
        let file = self.sources.add(path.display().to_string(), text);
        let value = derive(self, file);
        cache(&mut self.packages).insert(hash, value.clone());
        Some(value)
    }

    fn loose(&mut self) -> PackageKey {
        if !self.packages.list.contains_key(&PackageKey::Loose) {
            let prelude = vec![DEFAULT_PRELUDE.to_owned()];
            let package = Package {
                name: None,
                dir: PathBuf::new(),
                deps: Vec::new(),
                prelude,
                prelude_span: None,
                snapshot: Snapshot::default(),
                release: None,
            };
            let package = package.with_defaults(&self.library, &mut self.interner);
            self.packages.list.insert(PackageKey::Loose, package);
        }
        PackageKey::Loose
    }
}
