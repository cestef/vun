//! Packages written for tests through the manifest writer: manifests, published archives, and
//! checks from a fresh session.

#![allow(dead_code, reason = "each test binary uses part of the fixture")]

use std::path::{Path, PathBuf};

use overt_build::layout::MANIFEST;
use overt_build::manifest::{
    DEFAULT_PRELUDE, Dep, DepKind, EDITION, Identity, Manifest, Replacement, Source, Version,
};
use overt_build::store::Store;
use overt_diag::DiagId;
use overt_driver::Session;
use overt_fs::Scratch;
use overt_hash::Hash;
use overt_pkg::{Archive, Scheme};
use overt_span::{EXTENSION, SourceFiles, Span};

/// A scratch directory of packages.
pub(crate) struct Fixture {
    pub(crate) root: Scratch,
}

impl Fixture {
    /// A span for values written rather than read.
    fn unread() -> Span {
        Span::new(SourceFiles::new().add(MANIFEST, ""), 0, 0)
    }

    pub(crate) fn new(name: &str) -> Self {
        Self { root: Scratch::new(name) }
    }

    /// The manifest of a package whose files are its manifest and its root file.
    pub(crate) fn manifest(name: &str, id: u64, version: &str) -> Manifest {
        Manifest {
            name: name.into(),
            id,
            version: Version::parse(version).unwrap(),
            edition: EDITION,
            toolchain: None,
            include: vec![MANIFEST.into(), Path::new(name).with_extension(EXTENSION).display().to_string()],
            deps: Vec::new(),
            replace: Vec::new(),
            members: Vec::new(),
            prelude: vec![DEFAULT_PRELUDE.into()],
            prelude_span: Self::unread(),
        }
    }

    /// A dependency on the archive at `url` with tree hash `hash`.
    pub(crate) fn stored(name: &str, version: &str, url: &str, hash: Hash) -> Dep {
        let version = Version::parse(version).unwrap();
        let source = Source::Archive { version, hash, urls: vec![url.to_owned()] };
        Dep { name: name.into(), source, kind: DepKind::Overt, span: Self::unread() }
    }

    /// Replaces `identity` everywhere with the package in the directory `path`.
    pub(crate) fn replace(name: &str, identity: Identity, path: &str) -> Replacement {
        Replacement { name: name.into(), identity, source: Source::Path(path.into()), span: Self::unread() }
    }

    /// A dependency on the package in the directory `path`.
    pub(crate) fn local(name: &str, path: &str) -> Dep {
        Dep {
            name: name.into(),
            source: Source::Path(path.into()),
            kind: DepKind::Overt,
            span: Self::unread(),
        }
    }

    /// Writes the package of `manifest` into the directory `dir`, its root file holding `source`,
    /// and returns the directory.
    pub(crate) fn write(&self, dir: &str, manifest: &Manifest, source: &str) -> PathBuf {
        let dir = self.root.join(dir);
        self.root.write(dir.join(MANIFEST), manifest.source().unwrap());
        self.root.write(Self::file(&dir, manifest), source);
        dir
    }

    /// The root file of the package of `manifest` in `dir`.
    pub(crate) fn file(dir: &Path, manifest: &Manifest) -> PathBuf {
        dir.join(&manifest.name).with_extension(EXTENSION)
    }

    /// Publishes the package of `manifest` written in `dir` as an archive served from a `file://`
    /// URL, returning the URL and the tree hash.
    pub(crate) fn publish(&self, dir: &str, manifest: &Manifest) -> (String, Hash) {
        let (bytes, snapshot) = Archive::pack(&self.root.join(dir), &manifest.include).unwrap();
        let path = self.root.write(format!("{dir}{}", Archive::SUFFIX), bytes);
        (Scheme::file_url(&path), snapshot.root())
    }

    /// The store of this fixture, in place of the user's.
    pub(crate) fn store(&self) -> Store {
        Store::at(&self.root.join(Store::DIR))
    }

    /// A fresh session using this fixture's store.
    pub(crate) fn session(&self) -> Session {
        let mut session = Session::new();
        session.store = self.store();
        session
    }

    /// The diagnostics of checking `file` in a fresh session.
    pub(crate) fn check(&self, file: &Path) -> Vec<DiagId> {
        let mut session = self.session();
        let file = session.load(file).unwrap();
        session.check(file);
        session.diag.sorted().into_iter().map(|d| d.id.clone()).collect()
    }
}
