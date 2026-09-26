//! How a call passes its arguments (spec 01, section 4; spec 03, section 4): the one reading of
//! a call's written arguments against its callee's parameters, which compile-time evaluation
//! and lowering both follow.

use vun_codec::{Decode, Encode};
use vun_hir::tree::{Arg, Entry, ExprId};

use super::{Env, Unevaluated};
use crate::callable::Callable;
use crate::ty::{Ty, Value};

/// What a call passes to one parameter.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Passed {
    /// The receiver of `a.f(b)`, passed as the method asks.
    Receiver,
    /// A `$` argument left out, as typing inferred it.
    Inferred(Value),
    /// The argument of a `Code(T)` parameter, passed as code.
    Code(ExprId),
    /// The argument of another `$` parameter: its value at compile time, as a value of the
    /// parameter's type `ty`, where known.
    Known { expr: ExprId, ty: Option<Ty> },
    /// A run-time argument, for a parameter of type `ty`, where known.
    Run { expr: ExprId, ty: Option<Ty> },
}

impl Env<'_, '_> {
    /// What a call of `callee`, when it is known, passes to each of its parameters: the receiver,
    /// when it has one, the written arguments `args`, and the `$` arguments left out, `inferred`
    /// by parameter, in their places.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for an argument written by name.
    pub fn passing(
        &self,
        callee: Option<&Value>,
        receiver: bool,
        args: &[Entry<Arg>],
        inferred: &[(usize, Value)],
    ) -> Result<Vec<Passed>, Unevaluated> {
        let callable = callee.cloned().and_then(Callable::of);
        let quotes = callable.as_ref().and_then(|c| c.quotes(self)).unwrap_or_default();
        let comptime = callable.as_ref().and_then(|c| c.comptime(self)).unwrap_or_default();
        let params = callable.as_ref().and_then(|c| c.types(self)).unwrap_or_default();
        let fill = |passed: &mut Vec<Passed>| {
            while let Some((_, value)) = inferred.iter().find(|(slot, _)| *slot == passed.len()) {
                passed.push(Passed::Inferred(value.clone()));
            }
        };
        let mut passed = Vec::new();
        fill(&mut passed);
        if receiver {
            passed.push(Passed::Receiver);
        }
        for arg in args {
            fill(&mut passed);
            let Entry::One(Arg { name: None, value: expr }) = arg else {
                return Err(Unevaluated::Unsupported);
            };
            let (slot, expr) = (passed.len(), *expr);
            let ty = params.get(slot).copied().flatten();
            passed.push(if quotes.get(slot) == Some(&true) {
                Passed::Code(expr)
            } else if comptime.get(slot) == Some(&true) {
                Passed::Known { expr, ty }
            } else {
                Passed::Run { expr, ty }
            });
        }
        fill(&mut passed);
        Ok(passed)
    }
}
