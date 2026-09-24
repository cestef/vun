//! Items reached by their path, such as `lang.ops.Eq`, which syntax desugars into (appendix B)
//! and primitives are declared at.

use vun_grammar::PATH_SEPARATOR;
use vun_intern::Symbol;

use super::{Env, Unevaluated};
use crate::item::{Declared, Site};
use crate::ty::Value;

impl Env<'_, '_> {
    /// The value of the item at `path`, such as `lang.ops.Eq`, which syntax desugars into
    /// (appendix B): its first name is looked up outside the file, the rest as members.
    ///
    /// # Errors
    /// Returns why the item has no value here, such as a replaced `lang` that lacks it.
    pub fn item(&self, path: &str) -> Result<Value, Unevaluated> {
        let mut names = path.split(PATH_SEPARATOR).map(Symbol::intern);
        let mut value = self.value_of(self.outer(names.next().ok_or(Unevaluated::Absent)?)?)?;
        for name in names {
            value = self.member(value, name)?;
        }
        Ok(value)
    }

    /// Where the path `path`, such as `lang.meta`, leads: its first name is looked up outside the
    /// file, the rest in namespaces.
    ///
    /// # Errors
    /// Returns why it leads nowhere here.
    pub fn site(&self, path: &str) -> Result<Site, Unevaluated> {
        let mut names = path.split(PATH_SEPARATOR).map(Symbol::intern);
        let mut site = self.outer(names.next().ok_or(Unevaluated::Absent)?)?;
        for name in names {
            let Site::Namespace(ns) = site else { return Err(Unevaluated::Unsupported) };
            site = self.cx.get::<Declared>(&(ns, name)).ok_or(Unevaluated::Absent)?;
        }
        Ok(site)
    }
}
