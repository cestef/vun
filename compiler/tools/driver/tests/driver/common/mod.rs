//! Packages written for tests through the manifest writer: manifests, published archives, and
//! checks from a fresh session.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_build::layout::MANIFEST;
use vun_build::manifest::{
    DEFAULT_PRELUDE, Dep, DepKind, EDITION, Identity, Manifest, Replacement, Source, Version,
};
use vun_build::store::Store;
use vun_diag::DiagId;
use vun_driver::Session;
use vun_fetch::{Format, Scheme};
use vun_fs::Scratch;
use vun_hash::Hash;
use vun_query::Db;
use vun_span::{EXTENSION, SourceFiles, Span};

/// Where the tests of one build share the libraries' memos, so that `lang` and `std` are
/// computed once per target and set of checks rather than in every session.
const SHARED: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/memos");

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
            index: Vec::new(),
            members: Vec::new(),
            prelude: vec![DEFAULT_PRELUDE.into()],
            prelude_span: Self::unread(),
        }
    }

    /// A dependency on the archive at `url` with tree hash `hash`.
    pub(crate) fn stored(name: &str, version: &str, url: &str, hash: Hash) -> Dep {
        let version = Version::parse(version).unwrap();
        let source = Source::Archive { version, hash, urls: vec![url.to_owned()] };
        Dep { name: name.into(), source, kind: DepKind::Vun, span: Self::unread() }
    }

    /// Replaces `identity` everywhere with the package in the directory `path`.
    pub(crate) fn replace(name: &str, identity: Identity, path: &str) -> Replacement {
        Replacement { name: name.into(), identity, source: Source::Path(path.into()), span: Self::unread() }
    }

    /// A dependency on the package in the directory `path`.
    pub(crate) fn local(name: &str, path: &str) -> Dep {
        Dep { name: name.into(), source: Source::Path(path.into()), kind: DepKind::Vun, span: Self::unread() }
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
        let (bytes, snapshot) =
            Format::PUBLISHED.pack(&self.root.join(dir), &manifest.include, &manifest.release()).unwrap();
        let path = self.root.write(format!("{dir}{}", Format::PUBLISHED.suffix()), bytes);
        (Scheme::file_url(&path), snapshot.root())
    }

    /// The store of this fixture, in place of the user's.
    pub(crate) fn store(&self) -> Store {
        Store::at(&self.root.join(Store::DIR))
    }

    /// A fresh session using this fixture's store, starting from the libraries' memos that the
    /// tests of this build share (`SHARED`); `VUN_QUERY_CHECK=1` checks every memo it reuses
    /// against computing it again.
    pub(crate) fn session(&self) -> Session {
        // Logs, where `VUN_LOG` asks for them, as the command line gives them.
        vun_trace::Config::from_env().install();
        let mut session = Session::sharing(&Arc::new(Db::new()));
        session.store = self.store();
        session.shared = Some(PathBuf::from(SHARED));
        session
    }

    /// The diagnostics of checking `file` in a fresh session.
    pub(crate) fn check(&self, file: &Path) -> Vec<DiagId> {
        self.reported(file).into_iter().map(|(id, _)| id).collect()
    }

    /// The ids and messages of checking `file` in a fresh session.
    pub(crate) fn reported(&self, file: &Path) -> Vec<(DiagId, String)> {
        let mut session = self.session();
        let file = session.load(file).unwrap();
        session.check(file);
        session
            .diag
            .sorted(session.origins)
            .into_iter()
            .map(|d| (d.id.clone(), d.message.to_string()))
            .collect()
    }
}
