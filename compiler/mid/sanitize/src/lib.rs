//! Checks of unsafe code at run time (spec 07, section 7; spec 13, section 8.7): which checks a
//! program is built with (`ChecksOf`, an input the driver sets from `std.build.Options.checks`),
//! and the checked version a call runs as under them (`#[lang.check.by(kind, f)]`).

#[cfg(test)]
mod tests;

use vun_codec::{Decode, Encode};
use vun_items::Item;
use vun_items::lang::check_Kind as kind;
use vun_query::{Cx, Query};
use vun_sema::Primitive;
use vun_sema::attributes::Attributes;
use vun_sema::primitives::check::Check;
use vun_sema::target::{Read, Variants};
use vun_sema::ty::Value;

/// What a program may be built to check: `lang.check.Kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum Kind {
    Pointers,
    Allocations,
    Asm,
}

impl Variants for Kind {
    const VARIANTS: &'static [(Item, Self)] =
        &[(kind::pointers, Self::Pointers), (kind::allocations, Self::Allocations), (kind::asm, Self::Asm)];
}

/// The checks a program is built with, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Checks(Vec<Kind>);

impl Checks {
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = Kind>) -> Self {
        let mut kinds: Vec<Kind> = kinds.into_iter().collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self(kinds)
    }

    #[must_use]
    pub fn contains(&self, kind: Kind) -> bool {
        self.0.contains(&kind)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What something with `attributes` runs as under these checks: the `f` of its
    /// `#[lang.check.by(kind, f)]` whose kind is checked, a function.
    #[must_use]
    pub fn version(&self, attributes: &Attributes<'_, '_>) -> Option<Value> {
        let args = attributes.get(Primitive::Check(Check::by))?;
        let [kind, .., f] = args.as_slice() else { return None };
        let checked = Kind::read(kind).is_ok_and(|kind| self.contains(kind));
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
