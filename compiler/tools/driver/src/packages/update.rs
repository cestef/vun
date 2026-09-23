//! Raising dependencies (`overt pkg update`, spec 12, section 3): each stored dependency is raised
//! to the newest release its identity's major has in the project's indexes. The release is
//! fetched from the index, verified against the hash the listing states, stored, and its entry in
//! `.deps` rewritten with that hash, so the index is trusted no more than any mirror.

use std::path::Path;

use overt_build::index::{Index, Listing};
use overt_build::layout::MANIFEST;
use overt_build::manifest::{self, DepKind, Source, Version};
use overt_build::store::Label;
use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_hash::Hash;
use overt_intern::Symbol;
use overt_pkg::{Failure, Scheme};

use super::{Dep, DepSource};
use crate::Session;

/// A dependency raised to a newer release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Updated {
    pub name: String,
    pub from: Version,
    pub to: Version,
    pub hash: Hash,
}

/// Why an index gave no listing.
#[derive(Debug)]
enum Unlisted {
    Read(Failure),
    /// The listing is not `version hash` lines.
    Malformed,
}

impl std::fmt::Display for Unlisted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(failure) => failure.fmt(f),
            Self::Malformed => f.write_str("the listing is not `version hash` lines"),
        }
    }
}

impl std::error::Error for Unlisted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(failure) => failure.source(),
            Self::Malformed => None,
        }
    }
}

impl Session {
    /// Raises the stored dependencies of the project in `dir`, returning what changed; failures
    /// are reported.
    pub fn update(&mut self, dir: &Path) -> Vec<Updated> {
        let path = dir.join(MANIFEST);
        let Some(file) = self.load(&path) else { return Vec::new() };
        let Some(project) = self.manifest(file) else { return Vec::new() };
        let mut raised: Vec<(manifest::Dep, Updated)> = Vec::new();
        for dep in project.deps.iter().filter(|d| d.kind == DepKind::Overt) {
            let Source::Archive { version, .. } = dep.source else { continue };
            let declared = Dep {
                name: Symbol::intern(&dep.name),
                source: DepSource::of(dir, &dep.source),
                kind: dep.kind,
                span: Some(dep.span),
            };
            let Some(key) = self.dep_package(&declared) else { continue };
            let Some((identity, _)) = self.packages.list[&key].release else { continue };
            for base in &project.index {
                let index = Index { base };
                let Some(listing) = self.listing(&index.listing(identity)) else { continue };
                let Some((to, hash)) = listing.newer(version) else { break };
                let url = index.archive(identity, to);
                let label = Label { name: &dep.name, version: to };
                let store = self.store.clone();
                if let Err(why) = self.stage(DepKind::Overt, &store, hash, &store.staging(hash), &url, label)
                {
                    let msg = Msg::new().text("cannot fetch ").code(&dep.name).text(" ").code(to.to_string());
                    self.diag.error(Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).caused_by(&why));
                    break;
                }
                let source = Source::Archive { version: to, hash, urls: vec![url] };
                let updated = Updated { name: dep.name.clone(), from: version, to, hash };
                raised.push((manifest::Dep { source, ..dep.clone() }, updated));
                break;
            }
        }
        if raised.is_empty() {
            return Vec::new();
        }
        let what = Msg::new().text("raise dependencies");
        let written = self.rewrite_manifest(file, &path, what, |decls, text| {
            raised.iter().map(|(dep, _)| dep.redeclare(decls, text)).collect()
        });
        if written { raised.into_iter().map(|(_, u)| u).collect() } else { Vec::new() }
    }

    /// The listing at `url`; one that cannot be read is reported.
    fn listing(&mut self, url: &str) -> Option<Listing> {
        let read = Scheme::get(url).map_err(Unlisted::Read).and_then(|bytes| {
            String::from_utf8(bytes).ok().and_then(|text| Listing::parse(&text)).ok_or(Unlisted::Malformed)
        });
        match read {
            Ok(listing) => Some(listing),
            Err(why) => {
                let msg = Msg::new().text("cannot read the index listing ").code(url);
                let help = Msg::new().text("indexes are declared in the manifest's ").code(".index");
                let d = Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).child(ChildKind::Help, help);
                self.diag.error(d.caused_by(&why));
                None
            }
        }
    }
}
