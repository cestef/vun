//! The names a package's files see from outside themselves.

use std::collections::HashMap;

use overt_build::layout::SEPARATOR;
use overt_diag::{ChildKind, DiagCx, Diagnostic, ErrorGuaranteed, Msg, errors};
use overt_intern::Symbol;
use overt_span::Span;

use overt_hash::{Change, Hash, Snapshot};

use super::{Dep, DepSource, Namespace, PackageKey, Unfound};
use crate::Session;
use crate::commands::{FETCH_PATH, PROGRAM};

/// Why the package of a dependency is not there as declared.
#[derive(Clone, Copy, Debug)]
pub(super) enum Absent {
    Missing,
    /// Stored, but its contents hash to `actual`.
    Mismatch {
        expected: Hash,
        actual: Hash,
    },
}

impl std::fmt::Display for Absent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("the package is not on disk"),
            Self::Mismatch { expected, actual } => {
                write!(f, "`{actual}` is stored where `{expected}` is declared")
            }
        }
    }
}

impl std::error::Error for Absent {}

/// What a name from outside a file refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outer {
    /// The package itself.
    Package(PackageKey),
    /// A dependency of the package, by its index in [`super::Package::deps`], loaded when needed.
    Dep(usize),
    /// A public member of this prelude namespace.
    Prelude(Namespace),
}

/// The package and its dependencies, then the prelude's names, the first listed prelude first; a
/// file's own names come before all of them.
pub struct Scope {
    pub names: HashMap<Symbol, Outer>,
    /// Set when a prelude namespace could not be read, which is reported: a name found nowhere
    /// may be one it provides.
    pub poisoned: Option<ErrorGuaranteed>,
}

impl Session {
    /// The scope of the files of `package`, built once.
    pub fn scope(&mut self, package: PackageKey) -> &Scope {
        if !self.packages.scopes.contains_key(&package) {
            let mut names = HashMap::new();
            self.bind_deps(package, &mut names);
            let poisoned = self.bind_prelude(package, &mut names);
            self.packages.scopes.insert(package, Scope { names, poisoned });
        }
        &self.packages.scopes[&package]
    }

    fn bind_deps(&mut self, package: PackageKey, names: &mut HashMap<Symbol, Outer>) {
        let p = &self.packages.list[&package];
        names.extend(p.deps.iter().enumerate().map(|(i, d)| (d.name, Outer::Dep(i))));
        names.extend(p.name.map(|name| (name, Outer::Package(package))));
    }

    /// The package of the dependency `index` of `package`: the version selection chose for its
    /// identity (spec 12, section 3), or the declared one. A missing package, or a stored one whose
    /// contents do not match its hash, is reported once.
    ///
    /// # Errors
    /// Returns the proof that why it cannot be loaded is reported.
    pub fn dep(&mut self, package: PackageKey, index: usize) -> Result<PackageKey, ErrorGuaranteed> {
        let dep = self.packages.list[&package].deps[index].clone();
        let declared = self.dep_package(&dep)?;
        let Some((identity, _)) = self.packages.list[&declared].release else { return Ok(declared) };
        if let Some(replace) = self.replacement(identity) {
            return self.dep_package(&replace.dep);
        }
        match self.selected(identity)? {
            Some(locked) if PackageKey::Content(locked.hash) != declared => {
                let source =
                    DepSource::Stored { hash: locked.hash, version: locked.version, urls: Vec::new() };
                self.dep_package(&Dep { source, ..dep })
            }
            _ => Ok(declared),
        }
    }

    /// The package `dep` declares, checked against its hash when it is stored; why not is
    /// reported once per package, however many dependents declare it.
    pub(super) fn dep_package(&mut self, dep: &Dep) -> Result<PackageKey, ErrorGuaranteed> {
        let dir = self.dep_dir(&dep.source);
        if let Some(&reported) = self.packages.absent.get(&dir) {
            return Err(reported);
        }
        let reported = match self.found(dep) {
            Ok(found) => return Ok(found),
            Err(Absent::Missing) => self.missing_dep(dep),
            Err(Absent::Mismatch { expected, actual }) => {
                let snapshot = &self.packages.list[&PackageKey::Content(actual)].snapshot;
                let d = self.mismatch(dep, expected, actual, snapshot);
                self.report(dep.span, d)
            }
        };
        self.packages.absent.insert(dir, reported);
        Err(reported)
    }

    /// The package `dep` declares, if it is there as declared.
    pub(super) fn found(&mut self, dep: &Dep) -> Result<PackageKey, Absent> {
        let found = self.package_at(&self.dep_dir(&dep.source)).ok_or(Absent::Missing)?;
        match (&dep.source, found) {
            (&DepSource::Stored { hash: expected, .. }, PackageKey::Content(actual))
                if expected != actual =>
            {
                Err(Absent::Mismatch { expected, actual })
            }
            _ => Ok(found),
        }
    }

    /// Binds the exports of the prelude's namespaces, returning whether one could not be read.
    fn bind_prelude(
        &mut self,
        package: PackageKey,
        names: &mut HashMap<Symbol, Outer>,
    ) -> Option<ErrorGuaranteed> {
        let mut poisoned = None;
        let span = self.packages.list[&package].prelude_span;
        for entry in self.packages.list[&package].prelude.clone() {
            let path: Vec<Symbol> = entry.split(SEPARATOR).map(Symbol::intern).collect();
            match self.namespace_at(package, &path) {
                Ok(ns) => {
                    for name in self.exports(&ns) {
                        names.entry(name).or_insert_with(|| Outer::Prelude(ns.clone()));
                    }
                }
                Err(Unfound::Dep(reported)) => poisoned = Some(reported),
                Err(Unfound::Root | Unfound::Absent) => {
                    let d = Self::not_namespace(&entry);
                    poisoned = Some(self.report(span, d));
                }
            }
        }
        poisoned
    }

    fn missing_dep(&mut self, dep: &Dep) -> ErrorGuaranteed {
        let msg = Msg::new().text("cannot find dependency ").code(dep.name.as_str());
        let dir = self.dep_dir(&dep.source).display().to_string();
        let d = Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg.text(" at ").code(dir));
        let d = match dep.source {
            DepSource::Dir(_) => d,
            DepSource::Stored { hash, .. } => {
                let fetch = PROGRAM.invocation(&FETCH_PATH).expect("fetch is a command");
                let help = Msg::new().code(hash.to_string()).text(" is fetched by ").code(fetch);
                d.child(ChildKind::Help, help)
            }
        };
        self.report(dep.span, d)
    }

    /// That the stored package of `dep` hashes to `actual`, naming the files changed since the
    /// snapshot recorded when it was fetched.
    fn mismatch(&self, dep: &Dep, expected: Hash, actual: Hash, current: &Snapshot) -> Diagnostic {
        let name = dep.name.as_str();
        let msg = Msg::new().text("the stored contents of ").code(name).text(" do not match its hash");
        let note =
            Msg::new().text("expected ").code(expected.to_string()).text(", found ").code(actual.to_string());
        let mut d = Diagnostic::error(errors::PACKAGE_HASH_MISMATCH, msg).child(ChildKind::Note, note);
        let recorded = std::fs::read_to_string(self.store.snapshot(expected)).ok();
        let recorded = recorded.and_then(|text| Snapshot::parse(&text));
        for (path, change) in recorded.map(|r| r.changes(current)).unwrap_or_default() {
            let what = match change {
                Change::Added => " was added",
                Change::Removed => " was removed",
                Change::Modified => " was modified",
                Change::Mode => " changed whether it is executable",
            };
            d = d.child(ChildKind::Note, Msg::new().code(path).text(what).text(" since it was fetched"));
        }
        d
    }

    /// That `path` names no namespace.
    pub(super) fn not_namespace(path: &str) -> Diagnostic {
        let msg = Msg::new().code(path).text(" is not a namespace of this package or a dependency");
        Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg)
    }

    pub(super) fn report(&mut self, span: Option<Span>, d: Diagnostic) -> ErrorGuaranteed {
        match span {
            Some(span) => DiagCx { diag: &mut self.diag, origins: &self.origins }.error_at(span, d),
            None => self.diag.error(d),
        }
    }
}
