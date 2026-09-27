//! The checks of unsafe code a program is built with (spec 07, section 7): an input the driver
//! sets from the program's build options, as it sets the target, which compile-time code reads as
//! `lang.check.current`, and what an operation marked `#[lang.check.by(kind, f)]` runs as under
//! them.

use vun_items::lang::check;
use vun_query::{Cx, Query};
use vun_target::Checks;

use crate::Primitive;
use crate::attributes::Attributes;
use crate::env::{Env, Unevaluated};
use crate::primitives::check::Check as Declared;
use crate::target::{Read, Variants as _};
use crate::ty::Value;

/// The checks of the compilation: none until the driver sets them.
pub struct ChecksOf;

impl Query for ChecksOf {
    const NAME: &'static str = "checks";
    const INPUT: bool = true;
    type Key = ();
    type Value = Checks;

    fn compute(_: &Cx<'_>, (): &()) -> Self::Value {
        Checks::default()
    }
}

impl Env<'_, '_> {
    /// `lang.check.current`: the checks of the compilation, as `lang.check.Kind`s.
    pub(crate) fn current_checks(&self) -> Result<Value, Unevaluated> {
        let Value::Type(ty) = self.item(check::Kind)? else { return Err(Unevaluated::Unsupported) };
        let checks = self.cx.get::<ChecksOf>(&());
        let kinds =
            checks.iter().map(|c| Value::Variant { ty, name: c.item().symbol(), payload: Vec::new() });
        Ok(Value::Array(kinds.collect()))
    }
}

impl Attributes<'_, '_> {
    /// What the item these are written on runs as under `checks`: the `f` of its
    /// `#[lang.check.by(kind, f)]` whose kind is checked, a function.
    #[must_use]
    pub fn checked(&self, checks: &Checks) -> Option<Value> {
        let args = self.get(Primitive::Check(Declared::by))?;
        let [kind, .., f] = args.as_slice() else { return None };
        let checked = vun_target::Check::read(kind).is_ok_and(|kind| checks.contains(kind));
        (checked && matches!(f, Value::Fn(_))).then(|| f.clone())
    }
}
