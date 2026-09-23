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
mod locate;
mod namespace;
mod rewrite;
mod scope;
mod select;
mod spread;
mod update;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use overt_build::layout::{Location, MANIFEST};
use overt_build::lock::{Lock, Replaced, Replacing};
use overt_build::manifest::{
    self, DEFAULT_DEPS, DEFAULT_PRELUDE, DepKind, Identity, Manifest, Source, Version,
};
use overt_hash::{Hash, Snapshot};
use overt_intern::Symbol;
use overt_span::{FileId, Span};

pub use add::{Addition, Unstated};
pub use fetch::Fetched;
pub use namespace::{Namespace, Unfound};
pub use scope::{Outer, Scope};
pub use spread::{FileScope, Spreads};
pub use update::Updated;

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
    /// The identities it replaces; only the project's apply.
    pub replace: Vec<Replace>,
}

/// An identity replaced everywhere in the graph, the dependency loaded in its place, and how the
/// lockfile records it.
#[derive(Clone)]
pub struct Replace {
    pub identity: Identity,
    pub dep: Dep,
    pub recorded: Replaced,
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

impl DepSource {
    /// Where a package declared in the manifest in `dir` comes from.
    fn of(dir: &Path, source: &Source) -> Self {
        match source {
            Source::Path(path) => Self::Dir(dir.join(path)),
            Source::Archive { hash, version, urls } => {
                Self::Stored { hash: *hash, version: *version, urls: urls.clone() }
            }
        }
    }
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
    fn with_defaults(mut self, library: &Path) -> Self {
        for &default in DEFAULT_DEPS {
            let name = Symbol::intern(default);
            if self.name != Some(name) && self.deps.iter().all(|d| d.name != name) {
                let source = DepSource::Dir(library.join(default));
                self.deps.push(Dep { name, source, kind: DepKind::Overt, span: None });
            }
        }
        self
    }

    /// Where its root namespace lives.
    fn root(&self) -> Option<Location> {
        self.name.map(|name| Location::root(&self.dir, name.as_str()))
    }
}

impl Session {
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
            let name = Some(Symbol::intern(&manifest.name));
            let replace = manifest.replace.iter().map(|r| Replace {
                identity: r.identity,
                dep: Dep {
                    name: Symbol::intern(&r.name),
                    source: DepSource::of(dir, &r.source),
                    kind: DepKind::Overt,
                    span: Some(r.span),
                },
                recorded: Replaced {
                    name: r.name.clone(),
                    identity: r.identity,
                    with: match &r.source {
                        Source::Path(path) => Replacing::Path(path.clone()),
                        Source::Archive { hash, .. } => Replacing::Stored(*hash),
                    },
                },
            });
            let replace = replace.collect();
            let deps = manifest.deps.iter().map(|d| Dep {
                name: Symbol::intern(&d.name),
                source: DepSource::of(dir, &d.source),
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
                replace,
            };
            let package = package.with_defaults(&self.library);
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
                replace: Vec::new(),
            };
            let package = package.with_defaults(&self.library);
            self.packages.list.insert(PackageKey::Loose, package);
        }
        PackageKey::Loose
    }
}
