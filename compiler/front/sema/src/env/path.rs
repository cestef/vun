//! Items reached by their path, such as `lang.ops.Eq`, which syntax desugars into (appendix B)
//! and primitives are declared at.

use vun_intern::Symbol;
use vun_items::Item;

use super::{Env, Unevaluated};
use crate::decl::DeclValue;
use crate::item::{Declared, Site};
use crate::ty::Value;

impl Env<'_, '_> {
    /// The value of `item`, such as `lang.ops.Eq`, which syntax desugars into
    /// (appendix B): its first name is looked up outside the file, the rest as members.
    ///
    /// # Errors
    /// Returns why the item has no value here, such as a replaced `lang` that lacks it.
    pub fn item(&self, item: Item) -> Result<Value, Unevaluated> {
        self.path(item.path().iter().map(|n| Symbol::intern(n)))
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
        let mut names = item.path().iter().map(|n| Symbol::intern(n));
        let mut site = self.outer(names.next().ok_or(Unevaluated::Absent)?)?;
        for name in names {
            let Site::Namespace(ns) = site else { return Err(Unevaluated::Unsupported) };
            site = self.cx.get::<Declared>(&(ns, name)).ok_or(Unevaluated::Absent)?;
        }
        Ok(site)
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
