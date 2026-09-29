//! Items reached by their path, such as `lang.ops.Eq`, which syntax desugars into (appendix B)
//! and primitives are declared at.

use vun_hir::db::{Namespace, Outer, Package, PackageId, Scope};
use vun_hir::tree::{ExprId, Name};
use vun_intern::Symbol;
use vun_items::Item;
use vun_query::{Cx, Query};

use super::{Env, Unevaluated};
use crate::decl::DeclValue;
use crate::item::{Declared, Site};
use crate::ty::Value;

impl Env<'_, '_> {
    /// What the path `object.name` leads to: the member `name` of the namespace `object` is.
    pub(super) fn member_site(&self, object: ExprId, name: &Name) -> Result<Site, Unevaluated> {
        let Value::Namespace(ns) = self.eval(object)? else { return Err(Unevaluated::Unsupported) };
        let name = self.name(name).ok_or(Unevaluated::Unsupported)?;
        self.cx.get::<Declared>(&(ns, name)).ok_or(Unevaluated::Absent)
    }

    /// The value of `item`, such as `lang.ops.Eq`, which syntax desugars into
    /// (appendix B): its first name is looked up outside the file, the rest as members.
    ///
    /// # Errors
    /// Returns why the item has no value here, such as a replaced `lang` that lacks it.
    pub fn item(&self, item: Item) -> Result<Value, Unevaluated> {
        match self.site(item) {
            Ok(site) => self.value_of(site),
            Err(_) => self.path(item.path().iter().map(|n| Symbol::intern(n))),
        }
    }

    /// The value at the binding path `names`, its first name looked up outside the file, the
    /// rest as members.
    ///
    /// # Errors
    /// Returns why the path leads to no value here.
    pub fn path(&self, names: impl IntoIterator<Item = Symbol>) -> Result<Value, Unevaluated> {
        let mut names = names.into_iter();
        let mut value = self.value_of(self.outer(names.next().ok_or(Unevaluated::Absent)?)?)?;
        for name in names {
            value = self.member(value, name)?;
        }
        Ok(value)
    }

    /// Where `item`, such as the namespace `lang.meta`, is declared: its first name is looked up outside the
    /// file, the rest in namespaces.
    ///
    /// # Errors
    /// Returns why it leads nowhere here.
    pub fn site(&self, item: Item) -> Result<Site, Unevaluated> {
        self.cx.get::<ItemSite>(&(self.file.package, item))
    }

    /// Where `name` leads outside the file: the package, a dependency, or the prelude.
    pub(super) fn outer(&self, name: Symbol) -> Result<Site, Unevaluated> {
        Self::outer_in(self.cx, &self.file.package, name)
    }

    /// Where `name` is declared as the files of `package` see it from outside themselves.
    fn outer_in(cx: &Cx<'_>, package: &PackageId, name: Symbol) -> Result<Site, Unevaluated> {
        let root = |package| Ok(Site::Namespace(Namespace { package, path: Vec::new() }));
        match cx.get::<Scope>(package).names.get(&name) {
            Some(Outer::Prelude(ns)) => cx.get::<Declared>(&(ns.clone(), name)).ok_or(Unevaluated::Absent),
            Some(Outer::Package(package)) => root(*package),
            Some(Outer::Dep(index)) => {
                let dep = &cx.get::<Package>(package).deps[*index];
                root(dep.package.clone().map_err(|_| Unevaluated::Absent)?)
            }
            None => Err(Unevaluated::Absent),
        }
    }

    /// The value of `site`.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn value_of(&self, site: Site) -> Result<Value, Unevaluated> {
        match site {
            // A `mut` binding is a static: what it holds is known at run time only.
            Site::Decl(decl) if decl.mutable(self.cx) => Err(Unevaluated::Runtime),
            Site::Decl(decl) => match self.cx.get::<DeclValue>(&decl) {
                Err(Unevaluated::Trapped(_)) => Err(Unevaluated::Failed),
                value => value,
            },
            Site::Namespace(ns) => Ok(Value::Namespace(ns)),
        }
    }
}

/// Where an item is declared as the files of a package see it: its first name looked up outside
/// them, the rest in namespaces. Found once per package, since syntax desugars into the same few
/// items everywhere.
pub struct ItemSite;

impl Query for ItemSite {
    const NAME: &'static str = "item_site";
    type Key = (PackageId, Item);
    type Value = Result<Site, Unevaluated>;

    fn compute(cx: &Cx<'_>, (package, item): &Self::Key) -> Self::Value {
        let mut names = item.path().iter().map(|n| Symbol::intern(n));
        let mut site = Env::outer_in(cx, package, names.next().ok_or(Unevaluated::Absent)?)?;
        for name in names {
            let Site::Namespace(ns) = site else { return Err(Unevaluated::Unsupported) };
            site = cx.get::<Declared>(&(ns, name)).ok_or(Unevaluated::Absent)?;
        }
        Ok(site)
    }
}
