//! What a call of a function reads of it, computed once per function (`Callee`): which of its
//! parameters are `$` ones, and its step budget (spec 03, section 7): the default, or the `n` of
//! `#[comptime.fuel(n)]` on the declaration of the function called. The attribute is the
//! primitive `lang.comptime.fuel`, read as data.

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::eval::Func;
use crate::primitives::comptime::Comptime;

/// The steps a call may take, its own and those of the calls it makes, when its function sets
/// no budget.
pub const FUEL: u64 = 1 << 24;

/// A function as its calls read it: every call of it evaluated at compile time asks.
pub struct Callee;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Called {
    /// Whether each parameter is a `$` parameter; `None` when its file is not read.
    pub comptime: Option<Vec<bool>>,
    pub fuel: u64,
}

impl Query for Callee {
    const NAME: &'static str = "callee";
    type Key = Func;
    type Value = Called;

    fn compute(cx: &Cx<'_>, f: &Func) -> Called {
        let comptime = f.written(cx).and_then(|w| w.comptime());
        let fuel =
            f.attributes(cx).and_then(|a| a.count(Primitive::Comptime(Comptime::fuel))).unwrap_or(FUEL);
        Called { comptime, fuel }
    }

    fn cycle(_: &Cx<'_>, _: &Func, _: &Cycle) -> Called {
        Called { comptime: None, fuel: FUEL }
    }
}

impl Func {
    /// The step budget of a call of this function.
    #[must_use]
    pub fn fuel(&self, cx: &Cx<'_>) -> u64 {
        cx.get::<Callee>(self).fuel
    }
}
