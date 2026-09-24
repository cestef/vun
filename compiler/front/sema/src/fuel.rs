//! Step budgets of compile-time calls (spec 03, section 7): the default, or the `n` of
//! `#[comptime.fuel(n)]` on the declaration of the function called. The attribute is the
//! primitive `lang.comptime.fuel`, read as data.

use vun_query::Cx;

use crate::Primitive;
use crate::env::Env;
use crate::eval::Func;
use crate::primitives::comptime::Comptime;
use crate::ty::Value;

/// The steps a call may take, its own and those of the calls it makes, when its function sets
/// no budget.
pub const FUEL: u64 = 1 << 24;

impl Func {
    /// The step budget of a call of this function.
    #[must_use]
    pub fn fuel(&self, cx: &Cx<'_>) -> u64 {
        self.set_fuel(cx).unwrap_or(FUEL)
    }

    /// The budget `#[comptime.fuel(n)]` sets on the declaration whose value is this function's
    /// literal, if one does.
    fn set_fuel(&self, cx: &Cx<'_>) -> Option<u64> {
        let env = Env::of(cx, &self.literal.file)?;
        let attrs = env.attrs_of(self.literal.expr.id);
        let args = env.attribute(&attrs, Primitive::Comptime(Comptime::fuel))?;
        let [Value::Int(n)] = args.as_slice() else { return None };
        u64::try_from(n.to_i128()?).ok()
    }
}
