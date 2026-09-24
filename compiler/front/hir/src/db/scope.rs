//! The names a package's files see from outside themselves (spec 01, section 8): the package and
//! its dependencies, then the public names of the prelude's namespaces, the first listed first.

use std::collections::BTreeMap;
use std::sync::Arc;

use vun_diag::{DiagCtxt, DiagCx, ErrorGuaranteed};
use vun_grammar::PATH_SEPARATOR;
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::{Cx, Query};

use super::input::{Package, PackageId, Unloaded};
use super::layout::Namespace;
use super::namespace::{Exports, NamespaceAt, Unfound};

/// What a name from outside a file refers to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Outer {
    /// The package itself.
    Package(PackageId),
    /// A dependency, by its index among the package's.
    Dep(usize),
    /// A public member of this prelude namespace.
    Prelude(Namespace),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScopeData {
    pub names: BTreeMap<Symbol, Outer>,
    /// Set when a prelude namespace could not be read, which is reported: a name found nowhere may
    /// be one it provides.
    pub poisoned: Option<ErrorGuaranteed>,
    /// Prelude entries that name no namespace, and dependencies of the prelude that are absent.
    pub diag: DiagCtxt,
}

/// The scope of a package's files.
pub struct Scope;

impl Query for Scope {
    const NAME: &'static str = "scope";
    type Key = PackageId;
    type Value = Arc<ScopeData>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        let data = cx.get::<Package>(package);
        let mut scope = ScopeData::default();
        scope.names.extend(data.deps.iter().enumerate().map(|(i, d)| (d.name, Outer::Dep(i))));
        scope.names.extend(data.name.map(|name| (name, Outer::Package(*package))));
        for path in &data.prelude {
            match cx.get::<NamespaceAt>(&(*package, path.clone())) {
                Ok(ns) => {
                    for &name in cx.get::<Exports>(&ns).iter() {
                        scope.names.entry(name).or_insert_with(|| Outer::Prelude(ns.clone()));
                    }
                }
                Err(Unfound::Dep(Unloaded::Absent(reports))) => scope.poisoned = scope.diag.absorb(&reports),
                Err(Unfound::Dep(Unloaded::Reported)) => {}
                Err(Unfound::Root | Unfound::Absent) => {
                    let text: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
                    let d = Unfound::not_namespace(&text.join(PATH_SEPARATOR));
                    let reported = match data.prelude_span {
                        Some(span) => {
                            let origins = cx.db().resource::<OriginTable>();
                            DiagCx { diag: &mut scope.diag, origins: &origins }.error_at(span, d)
                        }
                        None => scope.diag.error(d),
                    };
                    scope.poisoned = Some(reported);
                }
            }
        }
        Arc::new(scope)
    }
}
