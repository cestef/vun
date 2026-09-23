//! Minimal version selection (spec 12, sections 3 and 4): every version any package of the graph
//! requires is a minimum, and the selected version of an identity is the largest of them. The
//! lockfile records every required version, from which the selection follows, so that the store
//! keeps what checking the project reads. There is no solver: selection is a function of the
//! manifests.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use overt_build::lock::{self, Lock, Locked};
use overt_build::manifest::{DepKind, Identity, Version};
use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_fs::Edit;

use super::{DepSource, PackageKey, Replace};
use crate::Session;
use crate::commands::{FETCH_PATH, PROGRAM};

impl Session {
    /// The versions the graph of `project` requires, over the packages present as declared; the
    /// others are reported where their names are used, or by fetching.
    pub fn select(&mut self, project: PackageKey) -> Lock {
        let mut required_by: HashMap<(Identity, Version), Locked> = HashMap::new();
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut pending = vec![project];
        while let Some(package) = pending.pop() {
            if !seen.insert(package) {
                continue;
            }
            let via = (package != project).then(|| self.packages.list[&package].name).flatten();
            let via = via.map(|name| name.as_str().to_owned());
            for dep in self.packages.list[&package].deps.clone() {
                let loaded = (dep.kind == DepKind::Overt).then(|| self.found(&dep).ok()).flatten();
                // A replaced identity's graph is the replacement's.
                let identity = loaded.and_then(|k| self.packages.list[&k].release).map(|(i, _)| i);
                let replaced = identity.and_then(|i| self.replacement_in(project, i));
                let walked = match replaced {
                    Some(replace) => self.found(&replace.dep).ok(),
                    None => loaded,
                };
                pending.extend(walked);
                let DepSource::Stored { hash, version, .. } = dep.source else { continue };
                let identity = loaded.and_then(|k| self.packages.list[&k].release).map(|(i, _)| i);
                let name = dep.name.as_str().to_owned();
                let required = Locked { name, version, identity, hash, via: via.clone() };
                match identity {
                    None if dep.kind == DepKind::Foreign => entries.push(required),
                    None => {}
                    Some(identity) => match required_by.get(&(identity, version)) {
                        Some(held) if held.hash != hash => self.conflict(&held.clone(), &required),
                        Some(_) => {}
                        None => {
                            required_by.insert((identity, version), required.clone());
                            entries.push(required);
                        }
                    },
                }
            }
        }
        let replaced = self.packages.list[&project].replace.iter().map(|r| r.recorded.clone()).collect();
        Lock::new(entries).replacing(replaced)
    }

    /// What the project replaces `identity` with.
    pub(super) fn replacement(&self, identity: Identity) -> Option<Replace> {
        self.replacement_in(self.packages.root?, identity)
    }

    fn replacement_in(&self, project: PackageKey, identity: Identity) -> Option<Replace> {
        self.packages.list[&project].replace.iter().find(|r| r.identity == identity).cloned()
    }

    /// What selection chose for `identity` in the project's graph, which is selected once and
    /// checked against the project's lockfile.
    pub(super) fn selected(&mut self, identity: Identity) -> Option<Locked> {
        if self.packages.selection.is_none() {
            let root = self.packages.root?;
            let selection = self.select(root);
            self.check_lock(root, &selection);
            self.packages.selection = Some(selection);
        }
        self.packages.selection.as_ref()?.selected(identity).cloned()
    }

    /// Writes the lockfile of the project in `dir` when its selection differs from it, registers
    /// the project as a root of the store, and returns the selection.
    pub fn write_lock(&mut self, dir: &Path, project: PackageKey) -> Lock {
        let selection = self.select(project);
        let path = dir.join(lock::FILE);
        let text = selection.to_string();
        let unchanged = std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str());
        // The lockfile is what keeps the project's entries in the shared store.
        let written = if unchanged { Ok(()) } else { path.put(&text) };
        if let Err(e) = written.and_then(|()| self.store.register(dir)) {
            let msg = Msg::new().text("cannot write ").code(path.display().to_string());
            self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
        }
        selection
    }

    /// Reports a lockfile of `root` that does not record `selection`.
    fn check_lock(&mut self, root: PackageKey, selection: &Lock) {
        let path = self.packages.list[&root].dir.join(lock::FILE);
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let recorded = Lock::parse(&text);
        if recorded.as_ref().is_ok_and(|r| r == selection) {
            return;
        }
        let msg = Msg::new().code(path.display().to_string()).text(" does not record the selected versions");
        let fetch = PROGRAM.invocation(&FETCH_PATH).expect("fetch is a command");
        let help = Msg::new().text("rewrite it with ").code(fetch);
        let mut d = Diagnostic::error(errors::PACKAGE_STALE_LOCK, msg).child(ChildKind::Help, help);
        if let Err(e) = &recorded {
            d = d.caused_by(e);
        }
        self.diag.error(d);
    }

    /// Two packages require one version of an identity with different contents.
    fn conflict(&mut self, held: &Locked, required: &Locked) {
        let msg = Msg::new().code(&required.name).text(" ").code(required.version.to_string());
        let msg = msg.text(" is required with two hashes");
        let note = |l: &Locked| {
            let by = l.via.as_deref().unwrap_or("the project");
            Msg::new().code(l.hash.to_string()).text(" required by ").code(by)
        };
        let d = Diagnostic::error(errors::PACKAGE_VERSION_CONFLICT, msg);
        self.diag.error(d.child(ChildKind::Note, note(held)).child(ChildKind::Note, note(required)));
    }
}
