//! Counts a function's attributes give (spec 03, section 7; spec 07, section 4): the step budget
//! of its compile-time calls, the default or the `n` of `#[comptime.fuel(n)]`, and its stack bound,
//! `#[stack.max(n)]`, which a declaration provided from outside states for what it provides. The attributes are primitives of `lang`, read as data.

use vun_query::Cx;

use crate::Primitive;
use crate::env::Env;
use crate::eval::Func;
use crate::item::DeclRef;
use crate::primitives::comptime::Comptime;
use crate::primitives::stack::Stack;

/// The steps a call may take, its own and those of the calls it makes, when its function sets
/// no budget.
pub const FUEL: u64 = 1 << 24;

impl Func {
    /// The step budget of a call of this function.
    #[must_use]
    pub fn fuel(&self, cx: &Cx<'_>) -> u64 {
        self.count(cx, Primitive::Comptime(Comptime::fuel)).unwrap_or(FUEL)
    }

    /// The most bytes of stack a call of this function may take, `#[stack.max(n)]`, if bound.
    #[must_use]
    pub fn stack_max(&self, cx: &Cx<'_>) -> Option<u64> {
        self.count(cx, Primitive::Stack(Stack::max))
    }

    /// The count the attribute `attr` gives on the declaration whose value is this function's
    /// literal, as `n` of `#[comptime.fuel(n)]`, if it has one.
    fn count(&self, cx: &Cx<'_>, attr: Primitive) -> Option<u64> {
        let env = Env::of(cx, &self.literal.file)?;
        env.count(&env.attrs_of(self.literal.expr.id), attr)
    }
}

impl DeclRef {
    /// The most bytes of stack a call of what this declaration provides from outside takes, as
    /// its `#[stack.max(n)]` declares: trusted, as its signature is (spec 07, section 4).
    #[must_use]
    pub fn stack_max(&self, cx: &Cx<'_>) -> Option<u64> {
        let env = Env::of(cx, &self.file)?;
        env.count(&env.parsed.hir.decls[self.decl.id].attrs, Primitive::Stack(Stack::max))
    }
}
