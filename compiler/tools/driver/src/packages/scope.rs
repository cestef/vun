//! The names a package's files see from outside themselves.

use std::collections::HashMap;

use overt_build::layout::SEPARATOR;
use overt_diag::{DiagCx, Diagnostic, Msg, errors};
use overt_intern::Symbol;
use overt_span::Span;

use super::{Dep, Namespace, PackageKey, Unfound};
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

    /// The package of the dependency `index` of `package`, loaded once; a missing one is
    /// reported each time it is needed.
    pub fn dep(&mut self, package: PackageKey, index: usize) -> Option<PackageKey> {
        let dep = self.packages.list[&package].deps[index].clone();
        let found = dep.dir.as_deref().and_then(|dir| self.package_at(dir));
        if found.is_none() {
            self.missing_dep(&dep);
        }
        found
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
        let msg = match &dep.dir {
            Some(dir) => msg.text(" at ").code(dir.display().to_string()),
            None => msg.text(" in the local store"),
        };
        self.report(dep.span, Diagnostic::error(errors::PACKAGE_MISSING_DEP, msg));
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
