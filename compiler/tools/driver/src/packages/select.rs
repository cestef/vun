//! Minimal version selection (spec 12, sections 3 and 4): every version any package of the graph
//! requires is a minimum, and the selected version of an identity is the largest of them. The
//! lockfile records every required version, from which the selection follows, so that the store
//! keeps what checking the project reads. There is no solver: selection is a function of the
//! manifests.

use std::collections::HashMap;

use vun_build::lock::{Lock, Locked};
use vun_build::manifest::{DepKind, Identity, Version};
use vun_diag::{ChildKind, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_span::Span;

use super::{Dep, DepSource, PackageKey, Replace};
use crate::Session;

/// What selection chose over the packages present.
#[derive(Clone)]
pub struct Selection {
    pub lock: Lock,
    /// Dependencies of the graph whose packages are not there as declared: what they require is
    /// unknown, so the lock is partial.
    pub absent: Vec<Dep>,
}

/// A required version and the dependency that requires it.
struct Requirement {
    locked: Locked,
    span: Option<Span>,
}

impl Session {
    /// The versions the graph of `project` requires, over the packages present; the absent ones
    /// are reported where they are needed.
    ///
    /// # Errors
    /// Returns the proof that a conflict is reported.
    pub fn select(&mut self, project: PackageKey) -> Result<Selection, ErrorGuaranteed> {
        let mut required_by: HashMap<(Identity, Version), Requirement> = HashMap::new();
        let mut entries = Vec::new();
        let mut absent = Vec::new();
        let mut failed = None;
        self.walk(project, |s, package, dep| {
            let loaded = match dep.kind {
                DepKind::Vun => s.found(dep).map_err(|_| absent.push(dep.clone())).ok(),
                DepKind::Foreign => None,
            };
            let identity = loaded.and_then(|k| s.packages.list[&k].release).map(|(i, _)| i);
            if let Some(replace) = identity.and_then(|i| s.replacement_in(project, i))
                && s.found(&replace.dep).is_err()
            {
                absent.push(replace.dep);
            }
            let DepSource::Stored { hash, version, .. } = dep.source else { return loaded };
            let via = (package != project).then(|| s.packages.list[&package].name).flatten();
            let via = via.map(|name| name.as_str().to_owned());
            let name = dep.name.as_str().to_owned();
            let required =
                Requirement { locked: Locked { name, version, identity, hash, via }, span: dep.span };
            match identity {
                None if dep.kind == DepKind::Foreign => entries.push(required.locked),
                None => {}
                Some(identity) => match required_by.get(&(identity, version)) {
                    Some(held) if held.locked.hash != hash => failed = Some(s.conflict(held, &required)),
                    Some(_) => {}
                    None => {
                        entries.push(required.locked.clone());
                        required_by.insert((identity, version), required);
                    }
                },
            }
            loaded
        });
        if let Some(reported) = failed {
            return Err(reported);
        }
        let replaced = self.packages.list[&project].replace.iter().map(|r| r.recorded.clone()).collect();
        Ok(Selection { lock: Lock::new(entries).replacing(replaced), absent })
    }

    /// What the project replaces `identity` with.
    pub(super) fn replacement(&self, identity: Identity) -> Option<Replace> {
        self.replacement_in(self.packages.root?, identity)
    }

    pub(super) fn replacement_in(&self, project: PackageKey, identity: Identity) -> Option<Replace> {
        self.packages.list[&project].replace.iter().find(|r| r.identity == identity).cloned()
    }

    /// What selection chose for `identity` in the project's graph, selected once; nothing outside
    /// a project. A complete selection is checked against the project's lockfile. A partial one
    /// yields to the lockfile, which recorded the whole graph, so that an absent package is
    /// reported where it is needed instead of another version being used in its place.
    pub(super) fn selected(&mut self, identity: Identity) -> Result<Option<Locked>, ErrorGuaranteed> {
        if self.packages.selection.is_none() {
            let Some(root) = self.packages.root else { return Ok(None) };
            let selection = self.select(root).map(|Selection { lock, absent }| {
                if absent.is_empty() {
                    self.check_lock(root, &lock);
                    lock
                } else {
                    self.recorded_lock(root).unwrap_or(lock)
                }
            });
            self.packages.selection = Some(selection);
        }
        let selection = self.packages.selection.as_ref().expect("selected above");
        Ok(selection.as_ref().map_err(|&reported| reported)?.selected(identity).cloned())
    }

    /// Two dependencies require one version of an identity with different contents: reported at
    /// the later one, labeling the earlier.
    fn conflict(&mut self, held: &Requirement, required: &Requirement) -> ErrorGuaranteed {
        let r = &required.locked;
        let msg = Msg::new().code(&r.name).text(" ").code(r.version.to_string());
        let note = |l: &Locked| {
            let by = match &l.via {
                Some(via) => Msg::new().code(via),
                None => Msg::new().text("the project"),
            };
            Msg::new().code(l.hash.to_string()).text(" required by ").append(by)
        };
        let d = Diagnostic::error(errors::PACKAGE_VERSION_CONFLICT, msg.text(" is required with two hashes"));
        let mut d = d.child(ChildKind::Note, note(&held.locked)).child(ChildKind::Note, note(r));
        if let Some(span) = held.span {
            let label = Msg::new().text("also required with ").code(held.locked.hash.to_string());
            d = d.label(self.origins.source(span), Some(label));
        }
        self.report(required.span, d)
    }
}
