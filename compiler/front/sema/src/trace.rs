//! How much of its code a program's table describes (spec 07, section 3): an input the driver
//! sets from `--trace` and the program's build options, which compile-time code reads as
//! `lang.trace.current`.

use vun_items::lang::{trace, trace_Detail as detail};
use vun_query::{Cx, Query};
use vun_target::Detail;

use crate::env::{Env, Unevaluated};
use crate::target::Variants;
use crate::ty::Value;

/// The detail of the compilation: every call until the driver sets it.
pub struct TraceOf;

impl Query for TraceOf {
    const NAME: &'static str = "trace";
    const INPUT: bool = true;
    type Key = ();
    type Value = Detail;

    fn compute(_: &Cx<'_>, (): &()) -> Self::Value {
        Detail::default()
    }
}

impl Variants for Detail {
    const VARIANTS: &'static [(vun_items::Item, Self)] =
        &[(detail::off, Self::Off), (detail::sites, Self::Sites), (detail::calls, Self::Calls)];
}

impl Env<'_, '_> {
    /// `lang.trace.current`: the detail of the compilation, as a `lang.trace.Detail`.
    pub(crate) fn current_trace(&self) -> Result<Value, Unevaluated> {
        let Value::Type(ty) = self.item(trace::Detail)? else { return Err(Unevaluated::Unsupported) };
        let detail = self.cx.get::<TraceOf>(&()).item().symbol();
        Ok(Value::Variant { ty, name: detail, payload: Vec::new() })
    }
}
