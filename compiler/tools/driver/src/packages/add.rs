//! Declaring a new dependency (`overt pkg add`, spec 12, section 3): a directory is declared by
//! its path, and an archive is fetched once, trusted on first use, stored, and declared with the
//! hash it has now, so that every later fetch is checked against it.

use std::fmt;
use std::path::Path;

use overt_build::layout::MANIFEST;
use overt_build::manifest::{self, DepKind, Manifest, Source, Version};
use overt_diag::{Diagnostic, Edit as TextEdit, Msg, errors};
use overt_fs::{Edit, Relative};
use overt_hash::{Hash, Snapshot};
use overt_span::Span;

use crate::Session;

/// What `overt pkg add` is asked to declare: a directory or an archive's URL, and the name and
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
    /// A stored archive, by the tree hash it has now and where it was fetched.
    Archive { hash: Hash, url: String },
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
        let source = match at {
            Place::Archive { hash, url } => {
                let version = addition.version.or_else(|| stated.as_ref().map(|m| m.version));
                Source::Archive { version: version.ok_or(Unstated::Version)?, hash, urls: vec![url] }
            }
            Place::Path(path) => Source::Path(path),
        };
        let dep = manifest::Dep { name, source, kind, span: Span::new(file, 0, 0) };
        if let Some(existing) = project.deps.iter().find(|d| d.name == dep.name) {
            let msg = Msg::new().code(&dep.name).text(" is already a dependency");
            self.report(Some(existing.span), Diagnostic::error(errors::PACKAGE_DUPLICATE_DEP, msg));
            return Ok(None);
        }
        let text = self.text(file).to_owned();
        match self.declarations(file, |decls, _| dep.declare(decls, &text)) {
            Ok(edit) => {
                if let Err(e) = path.put(TextEdit::apply(&[edit], &text)) {
                    let msg = Msg::new().text("cannot write ").code(path.display().to_string());
                    self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
                    return Ok(None);
                }
            }
            Err(e) => {
                let msg = Msg::new().text("cannot declare ").code(&dep.name).text(" in this manifest");
                let d = Diagnostic::error(errors::MANIFEST_UNEDITABLE, msg).caused_by(&e);
                self.report(Some(project.prelude_span), d);
                return Ok(None);
            }
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
        let relative = target.relative_to(dir)?;
        let at = Place::Path(relative.to_string_lossy().into_owned());
        Some(Found { at, kind: DepKind::Overt, manifest: Some(manifest) })
    }

    /// The archive at `url`, fetched into the store; an archive without a manifest is foreign.
    fn archive(&mut self, url: &str) -> Option<Found> {
        let store = self.store();
        // Its contents, and so its hash, are unknown until it is unpacked.
        let staging = store.staging(Hash::of(url.as_bytes()));
        let fetched = Self::unpack(&staging, url).and_then(|unpacked| {
            let manifest_path = unpacked.join(MANIFEST);
            let manifest = manifest_path.is_file().then(|| self.manifest_at(&manifest_path)).flatten();
            let kind = if manifest.is_some() { DepKind::Overt } else { DepKind::Foreign };
            let snapshot = Snapshot::of(&unpacked, &self.include(kind, &unpacked));
            Self::commit(&store, &staging, &snapshot)?;
            let at = Place::Archive { hash: snapshot.root(), url: url.to_owned() };
            Ok(Found { at, kind, manifest })
        });
        match fetched {
            Ok(found) => Some(found),
            Err(why) => {
                let _ = staging.remove();
                let msg = Msg::new().text("cannot fetch ").code(url);
                self.diag.error(Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).caused_by(&why));
                None
            }
        }
    }
}
