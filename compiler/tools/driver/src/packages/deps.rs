//! Loading dependencies: the package of each, checked against its hash, as selection chose it;
//! why one cannot be loaded is a diagnostic, reported where the dependency is needed.

use vun_diag::{ChildKind, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_span::Span;

use vun_hash::{Change, Hash, Snapshot};

use super::{Dep, DepSource, PackageKey};
use crate::Session;
use crate::commands::{CHECK_PATH, PROGRAM};

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

/// Why a dependency's package cannot be loaded.
pub(super) enum Unfetched {
    /// It is not there as declared: the diagnostic saying so, not yet reported.
    Absent(Box<Diagnostic>),
    /// Reported while the graph was selected, such as a version conflict.
    Reported(ErrorGuaranteed),
}

impl Session {
    /// The package of the dependency `index` of `package`: the version selection chose for its
    /// identity (spec 12, section 3), or the declared one. A missing package, or a stored one whose
    /// contents do not match its hash, is reported once.
    ///
    /// # Errors
    /// Returns the proof that why it cannot be loaded is reported.
    pub fn dep(&mut self, package: PackageKey, index: usize) -> Result<PackageKey, ErrorGuaranteed> {
        self.dep_unreported(package, index).map_err(|why| match why {
            Unfetched::Absent(d) => self.diag.error_once(*d),
            Unfetched::Reported(reported) => reported,
        })
    }

    /// [`Self::dep`], with why it cannot be loaded returned rather than reported, for whoever
    /// needs it.
    pub(super) fn dep_unreported(
        &mut self,
        package: PackageKey,
        index: usize,
    ) -> Result<PackageKey, Unfetched> {
        let dep = self.packages.list[&package].deps[index].clone();
        let declared = self.absence(&dep).map_err(Unfetched::Absent)?;
        let Some((identity, _)) = self.packages.list[&declared].release else { return Ok(declared) };
        if let Some(replace) = self.replacement(identity) {
            return self.absence(&replace.dep).map_err(Unfetched::Absent);
        }
        match self.selected(identity).map_err(Unfetched::Reported)? {
            Some(locked) if PackageKey::Content(locked.hash) != declared => {
                let source =
                    DepSource::Stored { hash: locked.hash, version: locked.version, urls: Vec::new() };
                self.absence(&Dep { source, ..dep }).map_err(Unfetched::Absent)
            }
            _ => Ok(declared),
        }
    }

    /// The package `dep` declares, checked against its hash when it is stored; why not is
    /// reported once per package, however many dependents declare it.
    pub(super) fn dep_package(&mut self, dep: &Dep) -> Result<PackageKey, ErrorGuaranteed> {
        self.absence(dep).map_err(|d| self.diag.error_once(*d))
    }

    /// The package `dep` declares, or why it is not there: one diagnostic per package directory,
    /// so that reporting it again records nothing new.
    fn absence(&mut self, dep: &Dep) -> Result<PackageKey, Box<Diagnostic>> {
        let dir = self.dep_dir(&dep.source);
        if let Some(d) = self.packages.absent.get(&dir) {
            return Err(Box::new(d.clone()));
        }
        let d = match self.found(dep) {
            Ok(found) => return Ok(found),
            Err(Absent::Missing) => self.missing_dep(dep),
            Err(Absent::Mismatch { expected, actual }) => {
                let snapshot = &self.packages.list[&PackageKey::Content(actual)].snapshot;
                self.mismatch(dep, expected, actual, snapshot)
            }
        };
        let d = self.located(dep.span, d);
        self.packages.absent.insert(dir, d.clone());
        Err(Box::new(d))
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
    fn missing_dep(&self, dep: &Dep) -> Diagnostic {
        let msg = Msg::new().text("cannot find dependency ").code(dep.name.as_str());
        let dir = self.dep_dir(&dep.source).display().to_string();
        let d = Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg.text(" at ").code(dir));
        match dep.source {
            DepSource::Dir(_) => d,
            DepSource::Stored { hash, .. } => {
                let check = PROGRAM.invocation(&CHECK_PATH).expect("check is a command");
                let help = Msg::new().code(hash.to_string()).text(" is fetched by ").code(check);
                d.child(ChildKind::Help, help)
            }
        }
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

    pub(super) fn report(&mut self, span: Option<Span>, d: Diagnostic) -> ErrorGuaranteed {
        let d = self.located(span, d);
        self.diag.error(d)
    }

    /// `d` with its primary label at `span`, when there is one.
    fn located(&self, span: Option<Span>, d: Diagnostic) -> Diagnostic {
        match span {
            Some(span) => d.at(self.origins.source(span), None),
            None => d,
        }
    }
}
