//! Declaring a new dependency (`vun pkg add`, spec 12, section 3): a directory is declared by
//! its path, and an archive is fetched once, trusted on first use, stored, and declared with the
//! hash it has now, so that every later fetch is checked against it.

use std::fmt;
use std::path::Path;

use vun_build::layout::MANIFEST;
use vun_build::manifest::{self, DepKind, Manifest, Source, Version};
use vun_build::store::{Label, Staging};
use vun_diag::{Diagnostic, Msg, errors};
use vun_fetch::Mirror;
use vun_fs::Relative;
use vun_hash::{Hash, Snapshot};
use vun_span::Span;
use vun_uri::Uri;

use super::fetch::Rejected;
use crate::Session;

/// What `vun pkg add` is asked to declare: a directory or an archive's URL, and the name and
/// version to use when the package does not state them.
pub struct Addition<'a> {
    pub source: &'a str,
    pub name: Option<&'a str>,
    pub version: Option<Version>,
}

/// What a new dependency lacks to be declared; its package has no manifest stating it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unstated {
    Name,
    Version,
}

impl fmt::Display for Unstated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Name => "its name",
            Self::Version => "its version",
        })
    }
}

impl std::error::Error for Unstated {}

/// Where a new dependency was found, and its manifest when it has one.
struct Found {
    at: Place,
    kind: DepKind,
    manifest: Option<Manifest>,
}

enum Place {
    /// A directory, by its path from the project.
    Path(String),
    /// An archive unpacked under staging, by where it was fetched; it is stored once declared.
    Archive { url: Uri, staged: Staged },
}

/// An unpacked archive awaiting its move into the store; dropped undeclared, it is removed.
struct Staged {
    staging: Staging,
    snapshot: Snapshot,
}

impl Session {
    /// Declares a dependency in the manifest of the project in `dir`, returning it; `Err` names
    /// what the package does not state, and other failures are reported.
    ///
    /// # Errors
    /// Returns what the addition must supply because the package does not state it.
    pub fn add(&mut self, dir: &Path, addition: &Addition<'_>) -> Result<Option<manifest::Dep>, Unstated> {
        let path = dir.join(MANIFEST);
        let Some(file) = self.load(&path) else { return Ok(None) };
        let Some(project) = self.manifest(file) else { return Ok(None) };
        let found = if Path::new(addition.source).is_dir() {
            self.local(dir, addition.source)
        } else {
            self.archive(addition.source)
        };
        let Some(Found { at, kind, manifest: stated }) = found else { return Ok(None) };
        let name = addition.name.map(str::to_owned).or_else(|| stated.as_ref().map(|m| m.name.clone()));
        let name = name.ok_or(Unstated::Name)?;
        let (source, unstored) = match at {
            Place::Archive { url, staged } => {
                let version = addition.version.or_else(|| stated.as_ref().map(|m| m.version));
                let version = version.ok_or(Unstated::Version)?;
                (Source::Archive { version, hash: staged.snapshot.root(), urls: vec![url] }, Some(staged))
            }
            Place::Path(path) => (Source::Path(path), None),
        };
        let dep = manifest::Dep { name, source, kind, span: Span::new(file, 0, 0) };
        if let Some(existing) = project.deps.iter().find(|d| d.name == dep.name) {
            let msg = Msg::new().code(&dep.name).text(" is already a dependency");
            self.report(Some(existing.span), Diagnostic::error(errors::PACKAGE_DUPLICATE_DEP, msg));
            return Ok(None);
        }
        if let (Some(staged), Source::Archive { version, .. }) = (&unstored, &dep.source) {
            let label = Label { name: &dep.name, version: *version };
            if let Err(why) = Self::commit(&self.store, &staged.staging, &staged.snapshot, label) {
                let msg = Msg::new().text("cannot store ").code(&dep.name);
                self.diag.error(Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).caused_by(&why));
                return Ok(None);
            }
        }
        let what = Msg::new().text("declare ").code(&dep.name);
        if !self.rewrite_manifest(file, &path, what, |decls, text| Ok(vec![dep.declare(decls, text)?])) {
            return Ok(None);
        }
        Ok(Some(dep))
    }

    /// A package in the directory `source`, declared by its path from the project in `dir`.
    fn local(&mut self, dir: &Path, source: &str) -> Option<Found> {
        let target = Path::new(source);
        let manifest_path = target.join(MANIFEST);
        let manifest = manifest_path.is_file().then(|| self.manifest_at(&manifest_path)).flatten();
        let Some(manifest) = manifest else {
            let msg = Msg::new().text("no ").code(MANIFEST).text(" declaring ").code(manifest::PACKAGE);
            let msg = msg.text(" in ").code(source);
            self.diag.error(Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg));
            return None;
        };
        let Some(relative) = target.relative_to(dir) else {
            let msg = Msg::new().text("cannot reach ").code(source).text(" from the project");
            self.diag.error(Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg));
            return None;
        };
        let at = Place::Path(relative.to_string_lossy().into_owned());
        Some(Found { at, kind: DepKind::Vun, manifest: Some(manifest) })
    }

    /// The archive at `url`, unpacked under staging; an archive without a manifest is foreign.
    /// A URL no scheme reads, such as `http://`, is refused before anything is staged.
    fn archive(&mut self, url: &str) -> Option<Found> {
        let uri = Uri::parse(url).map_err(Rejected::Url);
        let fetched =
            uri.and_then(|uri| Ok((Mirror::of(&uri, &self.schemes).map_err(Rejected::Mirror)?, uri)));
        let fetched = fetched.and_then(|(mirror, uri)| {
            // Its contents, and so its hash, are unknown until it is unpacked.
            let staging = self.store.stage(Hash::of(url.as_bytes())).map_err(Rejected::Store)?;
            let unpacked = Self::unpack(&staging, &mirror)?;
            let manifest_path = unpacked.join(MANIFEST);
            let manifest = manifest_path.is_file().then(|| self.manifest_at(&manifest_path)).flatten();
            let kind = if manifest.is_some() { DepKind::Vun } else { DepKind::Foreign };
            let snapshot =
                Snapshot::of(&unpacked, &self.include(kind, &unpacked)).map_err(Rejected::Unpacked)?;
            let at = Place::Archive { url: uri, staged: Staged { staging, snapshot } };
            Ok(Found { at, kind, manifest })
        });
        match fetched {
            Ok(found) => Some(found),
            Err(why) => {
                let msg = Msg::new().text("cannot fetch ").code(url);
                self.diag.error(Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).caused_by(&why));
                None
            }
        }
    }
}
