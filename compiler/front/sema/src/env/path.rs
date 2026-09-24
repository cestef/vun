//! Items reached by their path, such as `lang.ops.Eq`, which syntax desugars into (appendix B)
//! and primitives are declared at.

use vun_intern::Symbol;
use vun_items::Item;

use super::{Env, Unevaluated};
use crate::item::{Declared, Site};
use crate::ty::Value;

impl Env<'_, '_> {
    /// The value of `item`, such as `lang.ops.Eq`, which syntax desugars into
    /// (appendix B): its first name is looked up outside the file, the rest as members.
    ///
    /// # Errors
    /// Returns why the item has no value here, such as a replaced `lang` that lacks it.
    pub fn item(&self, item: Item) -> Result<Value, Unevaluated> {
        let mut names = item.path().iter().map(|n| Symbol::intern(n));
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
}
