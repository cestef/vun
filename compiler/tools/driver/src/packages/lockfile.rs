//! The project's lockfile (spec 12, section 4): written from the selection, and checked
//! against it, pointing at each line that records something else.

use std::collections::HashSet;
use std::path::Path;

use vun_build::lock::{self, Lock};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fs::Edit;
use vun_span::{ByteRange, SourceMap, Span};

use vun_hash::Hash;
use vun_intern::Symbol;

use vun_trace::Activity;

use super::{Fetched, PackageKey, Selection};
use crate::Session;

/// Finding the project around the root file, fetching what it depends on, and writing its
/// lockfile, as timed.
const LOCATE: Activity = Activity::stage("locate");
const FETCH: Activity = Activity::stage("fetch");
const LOCK: Activity = Activity::stage("lock");

impl Session {
    /// Writes the lockfile of the project in `dir` when its selection differs from it, registers
    /// the project as a root of the store, and returns the selection. A partial or conflicting
    /// selection writes nothing; each absent package is reported. A project without a lockfile
    /// gets one once it selects something.
    pub fn write_lock(&mut self, dir: &Path, project: PackageKey) -> Option<Lock> {
        let Selection { lock: selection, absent } = self.select(project).ok()?;
        if !absent.is_empty() {
            for dep in &absent {
                let _ = self.dep_package(dep);
            }
            return None;
        }
        let path = dir.join(lock::FILE);
        if selection.is_empty() && !path.exists() {
            return Some(selection);
        }
        let text = selection.to_string();
        let unchanged = std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str());
        // The lockfile is what keeps the project's entries in the shared store.
        let written = if unchanged { Ok(()) } else { path.put(&text) };
        self.packages.wrote(&path);
        if let Err(e) = written.and_then(|()| self.store.register(dir)) {
            self.diag.error(Diagnostic::unwritable(&path, &e));
        }
        Some(selection)
    }

    /// Makes the project around the file at `path` ready to compile, as cargo does before
    /// building: fetches the dependencies the store lacks and records the selection in the
    /// lockfile. Returns what each fetch did; nothing for a file outside a project.
    pub fn prepare(&mut self, path: &Path) -> Vec<(Symbol, Hash, Fetched)> {
        let found = {
            let _timer = LOCATE.start();
            let around = path.parent().and_then(|dir| self.package_around(dir));
            around.map(|(dir, key)| self.project_of(&dir, key))
        };
        let Some((dir, project)) = found else { return Vec::new() };
        let fetched = {
            let _timer = FETCH.start();
            self.fetch(project)
        };
        if !self.failed() {
            let _timer = LOCK.start();
            self.write_lock(&dir, project);
        }
        fetched
    }

    /// The lockfile of `root`, if it reads.
    pub(crate) fn recorded_lock(&self, root: PackageKey) -> Option<Lock> {
        let path = self.packages.list[&root].dir.join(lock::FILE);
        std::fs::read_to_string(path).ok()?.parse().ok()
    }

    /// Reports a lockfile of `root` that does not read, at what does not, or that does not record
    /// `selection`, at its first line the selection does not have, labeling the others and noting
    /// each line it lacks.
    pub(super) fn check_lock(&mut self, root: PackageKey, selection: &Lock) {
        let path = self.packages.list[&root].dir.join(lock::FILE);
        if !path.is_file() {
            return;
        }
        let Some(file) = self.load(&path) else { return };
        let text = self.text(file).to_owned();
        let recorded = self.declarations(file, |decls, cx| Lock::read(decls, file, cx));
        if recorded.as_ref().is_none_or(|r| r == selection) {
            return;
        }
        let expected = selection.to_string();
        let wanted: HashSet<&str> = expected.lines().collect();
        let present: HashSet<&str> = text.lines().collect();
        let source = self.sources.file(file);
        let stale: Vec<ByteRange> = source
            .lines()
            .filter(|(line, _)| !line.trim().is_empty() && !wanted.contains(line))
            .map(|(line, range)| {
                // An entry is labeled from its first character, past the indentation.
                let indent = u32::try_from(line.len() - line.trim_start().len()).unwrap_or_default();
                ByteRange::new(range.lo + indent, range.hi)
            })
            .collect();
        let end = u32::try_from(text.len()).expect("source files are smaller than 4 GiB");
        let mut stale = stale.into_iter().map(|range| Span { file, range });
        let first = stale.next().unwrap_or(Span::new(file, end, end));

        let msg = Msg::new().code(path.display().to_string()).text(" does not record the selected versions");
        let mut d = Diagnostic::error(errors::PACKAGE_STALE_LOCK, msg);
        for span in stale {
            d = d.label(self.origins.source(span), None);
        }
        for line in expected.lines().filter(|line| !present.contains(line)) {
            d = d.child(ChildKind::Note, Msg::new().text("the selection requires ").code(line));
        }
        if let Some(check) = &self.check {
            d = d.child(ChildKind::Help, Msg::new().text("rewrite it with ").code(check.as_str()));
        }
        self.report(Some(first), d);
    }
}
