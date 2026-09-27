//! Checks of unsafe code at run time (spec 07, section 7; spec 13, section 8.7): which checks a
//! program is built with (`ChecksOf`, an input the driver sets from `std.build.Options.checks`),
//! the checked version a call runs as under them (`#[lang.check.by(kind, f)]`), and the link
//! symbols of the operations a runtime provides for them (`Operations`).

mod operations;
#[cfg(test)]
mod tests;

pub use operations::Operations;

/// The queries of the checks whose memos outlive the process: the checks, an input, by its trace.
pub const PERSISTED: &[fn(&vun_query::Db)] =
    &[vun_query::Db::trace::<ChecksOf>, vun_query::Db::persist::<Operations>];

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Query};
use vun_sema::Primitive;
use vun_sema::attributes::Attributes;
use vun_sema::primitives::check::Check as Declared;
use vun_sema::target::Read;
use vun_sema::ty::Value;
use vun_target::Check;

/// The checks a program is built with, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Checks(Vec<Check>);

impl Checks {
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = Check>) -> Self {
        let mut kinds: Vec<Check> = kinds.into_iter().collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self(kinds)
    }

    #[must_use]
    pub fn contains(&self, kind: Check) -> bool {
        self.0.contains(&kind)
    }

    pub fn iter(&self) -> impl Iterator<Item = Check> {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What something with `attributes` runs as under these checks: the `f` of its
    /// `#[lang.check.by(kind, f)]` whose kind is checked, a function.
    #[must_use]
    pub fn version(&self, attributes: &Attributes<'_, '_>) -> Option<Value> {
        let args = attributes.get(Primitive::Check(Declared::by))?;
        let [kind, .., f] = args.as_slice() else { return None };
        let checked = Check::read(kind).is_ok_and(|kind| self.contains(kind));
        (checked && matches!(f, Value::Fn(_))).then(|| f.clone())
    }
}

/// The checks of the compilation: an input the driver sets from the program's build options,
/// none until then.
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
