//! Raising dependencies (`vun pkg update`, spec 12, section 3): each stored dependency is raised
//! to the newest release its identity's major has in the project's indexes. The release is
//! fetched from the index, verified against the hash the listing states, stored, and its entry in
//! `.deps` rewritten with that hash. The index is trusted to name a release, as `vun pkg add`
//! trusts an archive on first use; the release must declare the identity and version it is
//! listed under.

use std::path::Path;

use vun_build::index::{Index, Listing};
use vun_build::layout::MANIFEST;
use vun_build::manifest::{self, DepKind, Identity, Source, Version};
use vun_build::store::Label;
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fetch::Failure;
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_uri::Uri;

use super::fetch::Rejected;
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
        for dep in project.deps.iter().filter(|d| d.kind == DepKind::Vun) {
            let Source::Archive { version, .. } = dep.source else { continue };
            let declared = Dep {
                name: Symbol::intern(&dep.name),
                source: DepSource::of(&self.library, dir, &dep.source),
                kind: dep.kind,
                span: Some(dep.span),
            };
            let Ok(key) = self.dep_package(&declared) else { continue };
            let Some((identity, _)) = self.packages.list[&key].release else { continue };
            for base in &project.index {
                let index = Index { base };
                let Some(listing) = self.listing(&index.listing(identity)) else { continue };
                let Some((to, hash)) = listing.newer(version) else { break };
                let url = index.archive(identity, to);
                let label = Label { name: &dep.name, version: to };
                if let Err(why) = self.release(identity, to, hash, &url, label) {
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

    /// Stores the release `version` of `identity` from `url`, checking it is what it is listed as.
    fn release(
        &mut self,
        identity: Identity,
        version: Version,
        hash: Hash,
        url: &Uri,
        label: Label<'_>,
    ) -> Result<(), Rejected> {
        let store = self.store.clone();
        let staging = store.stage(hash).map_err(Rejected::Store)?;
        self.stage(DepKind::Vun, &store, hash, &staging, url, label)?;
        let stored = self.package_at(&store.package(hash));
        let declared = stored.and_then(|key| self.packages.list[&key].release);
        match declared {
            Some(release) if release == (identity, version) => Ok(()),
            _ => Err(Rejected::Release { identity, version }),
        }
    }

    /// The listing at `url`; one that cannot be read is reported.
    fn listing(&mut self, url: &Uri) -> Option<Listing> {
        let read = self.schemes.get(url).map_err(Unlisted::Read).and_then(|bytes| {
            String::from_utf8(bytes).ok().and_then(|text| Listing::parse(&text)).ok_or(Unlisted::Malformed)
        });
        match read {
            Ok(listing) => Some(listing),
            Err(why) => {
                let msg = Msg::new().text("cannot read the index listing ").code(url.as_str());
                let help = Msg::new().text("indexes are declared in the manifest's ").code(".index");
                let d = Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).child(ChildKind::Help, help);
                self.diag.error(d.caused_by(&why));
                None
            }
        }
    }
}
