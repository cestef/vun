//! The names a package's files see from outside themselves.

use std::collections::HashMap;

use overt_build::layout::SEPARATOR;
use overt_diag::{ChildKind, DiagCx, Diagnostic, Msg, errors};
use overt_intern::Symbol;
use overt_span::Span;

use super::{Dep, DepSource, Namespace, PackageKey, Unfound};
use crate::Session;

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
}

impl Session {
    /// The scope of the files of `package`, built once.
    pub fn scope(&mut self, package: PackageKey) -> &Scope {
        if !self.packages.scopes.contains_key(&package) {
            let mut names = HashMap::new();
            self.bind_deps(package, &mut names);
            self.bind_prelude(package, &mut names);
            self.packages.scopes.insert(package, Scope { names });
        }
        &self.packages.scopes[&package]
    }

    fn bind_deps(&mut self, package: PackageKey, names: &mut HashMap<Symbol, Outer>) {
        let p = &self.packages.list[&package];
        names.extend(p.deps.iter().enumerate().map(|(i, d)| (d.name, Outer::Dep(i))));
        names.extend(p.name.map(|name| (name, Outer::Package(package))));
    }

    /// The package of the dependency `index` of `package`, loaded once; a missing one, or a
    /// stored one whose contents do not match its hash, is reported each time it is needed.
    pub fn dep(&mut self, package: PackageKey, index: usize) -> Option<PackageKey> {
        let dep = self.packages.list[&package].deps[index].clone();
        let Some(found) = self.package_at(&self.dep_dir(&dep.source)) else {
            self.missing_dep(&dep);
            return None;
        };
        match (dep.source, found) {
            (DepSource::Stored(expected), PackageKey::Content(actual)) if expected != actual => {
                let name = self.interner.resolve(dep.name);
                let msg =
                    Msg::new().text("the stored contents of ").code(name).text(" do not match its hash");
                let note = Msg::new().text("expected ").code(expected.to_string()).text(", found ");
                let d = Diagnostic::error(errors::PACKAGE_HASH_MISMATCH, msg)
                    .child(ChildKind::Note, note.code(actual.to_string()));
                self.report(dep.span, d);
                None
            }
            _ => Some(found),
        }
    }

    fn bind_prelude(&mut self, package: PackageKey, names: &mut HashMap<Symbol, Outer>) {
        let span = self.packages.list[&package].prelude_span;
        for entry in self.packages.list[&package].prelude.clone() {
            let path: Vec<Symbol> = entry.split(SEPARATOR).map(|s| self.interner.intern(s)).collect();
            match self.namespace_at(package, &path) {
                Ok(ns) => {
                    for name in self.exports(&ns) {
                        names.entry(name).or_insert_with(|| Outer::Prelude(ns.clone()));
                    }
                }
                Err(Unfound::Dep) => {}
                Err(Unfound::Root | Unfound::Absent) => {
                    let d = Self::not_namespace(&entry);
                    self.report(span, d);
                }
            }
        }
    }

    fn missing_dep(&mut self, dep: &Dep) {
        let msg = Msg::new().text("cannot find dependency ").code(self.interner.resolve(dep.name));
        let dir = self.dep_dir(&dep.source).display().to_string();
        let d = Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg.text(" at ").code(dir));
        let d = match dep.source {
            DepSource::Dir(_) => d,
            DepSource::Stored(hash) => {
                let help = Msg::new().code(hash.to_string()).text(" is fetched by ").code("overt pkg fetch");
                d.child(ChildKind::Help, help)
            }
        };
        self.report(dep.span, d);
    }

    /// That `path` names no namespace.
    pub(super) fn not_namespace(path: &str) -> Diagnostic {
        let msg = Msg::new().code(path).text(" is not a namespace of this package or a dependency");
        Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg)
    }

    pub(super) fn report(&mut self, span: Option<Span>, d: Diagnostic) {
        match span {
            Some(span) => {
                DiagCx { diag: &mut self.diag, origins: &mut self.origins }.error_at(span, d);
            }
            None => {
                self.diag.error(d);
            }
        }
    }
}
