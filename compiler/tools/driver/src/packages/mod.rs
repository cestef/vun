//! Packages on disk: manifests, dependencies, the store and the lockfile (spec 12, sections 1 to
//! 4). Once a graph is loaded, it is published as query inputs (`vun_hir::db`), where
//! namespaces, scopes and name checks are computed.
//!
//! A file belongs to the package of the nearest enclosing `build.vun` that declares `package`; a
//! `build.vun` without it is an ordinary module, such as `std.build`.
//!
//! Everything is keyed by content: a package by the hash of its `include` tree, a manifest by the
//! hash of its text.

mod add;
mod deps;
mod fetch;
mod graph;
mod inputs;
mod locate;
mod lockfile;
mod rewrite;
mod select;
mod update;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_build::lock::{Lock, Replaced, Replacing};
use vun_build::manifest::{DEFAULT_DEPS, DEFAULT_PRELUDE, DepKind, Identity, Manifest, Source, Version};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_hash::{Hash, Snapshot};
use vun_intern::Symbol;
use vun_span::{FileId, PackageId, Span};
use vun_trace::tracing;
use vun_uri::Uri;

pub use add::{Addition, Unstated};
pub use fetch::Fetched;
use inputs::Sources;
pub use select::Selection;
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
    /// How its files are named in every process.
    pub id: PackageId,
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
    /// The text of its source files, as hashed.
    sources: Sources,
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
    Stored { hash: Hash, version: Version, urls: Vec<Uri> },
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
    /// Manifests by the hash of their text; `None` for a `build.vun` that declares no `package`.
    manifests: HashMap<Hash, Option<Manifest>>,
    /// The project: the package whose graph version selection runs over.
    pub root: Option<PackageKey>,
    /// Dependency directories whose package is not there as declared, with why.
    absent: HashMap<PathBuf, Diagnostic>,
    /// The package each directory looked at holds, if any: a session reads a directory's tree
    /// once, and again only after writing into it (`Packages::wrote`).
    seen: HashMap<PathBuf, Option<PackageKey>>,
    /// Packages whose query inputs are set.
    published: HashSet<PackageKey>,
    /// What selection chose for the project's graph, computed once.
    selection: Option<Result<Lock, ErrorGuaranteed>>,
}

impl Packages {
    /// Forgets what the directory holding `path` held, now that the session wrote `path`.
    pub(crate) fn wrote(&mut self, path: &Path) {
        if let Some(dir) = path.parent() {
            self.seen.remove(dir);
        }
    }
}

impl Package {
    /// Adds the default dependencies the package neither declares nor is, from `library`.
    fn with_defaults(mut self, library: &Path) -> Self {
        for &default in DEFAULT_DEPS {
            let name = Symbol::intern(default);
            if self.name != Some(name) && self.deps.iter().all(|d| d.name != name) {
                let source = DepSource::Dir(library.join(default));
                self.deps.push(Dep { name, source, kind: DepKind::Vun, span: None });
            }
        }
        self
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
        if let Some(&seen) = self.packages.seen.get(dir) {
            return seen;
        }
        let found = self.read_package(dir);
        self.packages.seen.insert(dir.to_path_buf(), found);
        found
    }

    /// The package whose manifest is in `dir`, read from the disk.
    fn read_package(&mut self, dir: &Path) -> Option<PackageKey> {
        let path = dir.join(MANIFEST);
        let manifest = self.manifest_at(&path.is_file().then_some(path)?)?;
        if dir.starts_with(self.store.dir())
            && let Some(span) = manifest.by_path().next()
        {
            let msg = Msg::new().text("a stored package names its dependencies by hash, not by path");
            self.report(Some(span), Diagnostic::error(errors::MANIFEST_INVALID_FIELD, msg));
            return None;
        }
        let taken = Snapshot::take(dir, &manifest.include)
            .map_err(|e| self.diag.error(Diagnostic::unreadable(dir, &e)))
            .ok()?;
        let key = PackageKey::Content(taken.snapshot.root());
        tracing::debug!(dir = %dir.display(), name = %manifest.name, files = taken.snapshot.files().count(), "package read");
        if !self.packages.list.contains_key(&key) {
            let name = Some(Symbol::intern(&manifest.name));
            let replace = manifest.replace.iter().map(|r| Replace {
                identity: r.identity,
                dep: Dep {
                    name: Symbol::intern(&r.name),
                    source: DepSource::of(dir, &r.source),
                    kind: DepKind::Vun,
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
            let identity = manifest.identity();
            let id = if dir.starts_with(self.store.dir()) {
                PackageId::Stored(taken.snapshot.root())
            } else {
                PackageId::Local {
                    name: Symbol::intern(&manifest.name),
                    id: identity.id,
                    version: manifest.version,
                }
            };
            let package = Package {
                id,
                name,
                dir: dir.to_path_buf(),
                deps: deps.collect(),
                prelude: manifest.prelude,
                prelude_span: Some(manifest.prelude_span),
                sources: Sources::of(&taken),
                snapshot: taken.snapshot,
                release: Some((identity, manifest.version)),
                replace,
            };
            let package = package.with_defaults(&self.library);
            self.packages.list.insert(key, package);
        }
        Some(key)
    }

    /// The manifest in the `build.vun` at `path`, read once per content.
    pub fn manifest_at(&mut self, path: &Path) -> Option<Manifest> {
        let read = |s: &mut Self, file| {
            s.declarations(file, |decls, cx| {
                Manifest::declared(decls).then(|| Manifest::read(decls, file, cx)).flatten()
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
                id: PackageId::Loose,
                name: None,
                dir: PathBuf::new(),
                deps: Vec::new(),
                prelude,
                prelude_span: None,
                snapshot: Snapshot::default(),
                sources: Sources::default(),
                release: None,
                replace: Vec::new(),
            };
            let package = package.with_defaults(&self.library);
            self.packages.list.insert(PackageKey::Loose, package);
        }
        PackageKey::Loose
    }
}
