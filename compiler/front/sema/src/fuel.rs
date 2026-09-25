//! Step budgets of compile-time calls (spec 03, section 7): the default, or the `n` of
//! `#[comptime.fuel(n)]` on the declaration of the function called. The attribute is the
//! primitive `lang.comptime.fuel`, read as data.

use vun_query::Cx;

use crate::Primitive;
use crate::eval::Func;
use crate::primitives::comptime::Comptime;

/// The steps a call may take, its own and those of the calls it makes, when its function sets
/// no budget.
pub const FUEL: u64 = 1 << 24;

impl Func {
    /// The step budget of a call of this function.
    #[must_use]
    pub fn fuel(&self, cx: &Cx<'_>) -> u64 {
        self.attributes(cx).and_then(|a| a.count(Primitive::Comptime(Comptime::fuel))).unwrap_or(FUEL)
    }
}
