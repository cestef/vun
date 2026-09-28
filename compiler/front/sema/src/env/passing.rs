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
    /// The arguments a pack gathers, the tuple its parameter receives, of type `ty` where known
    /// (spec 01, section 4).
    Pack { exprs: Vec<ExprId>, ty: Option<Ty> },
}

impl Passed {
    /// The type each argument of a pack of type `ty` is passed as: its item of the tuple.
    #[must_use]
    pub fn item(ty: Option<Ty>, at: usize) -> Option<Ty> {
        ty?.as_tuple()?.get(at).copied()
    }
}

impl Env<'_, '_> {
    /// What a call of `callee`, when it is known, passes to each of its parameters: the receiver,
    /// when it has one, the written arguments `args`, and the `$` arguments left out, `inferred`
    /// by parameter, in their places; the written arguments from `pack` on, as typing found
    /// them, gathered into the tuple of the last parameter.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for an argument written by name.
    pub fn passing(
        &self,
        callee: Option<&Value>,
        receiver: bool,
        args: &[Entry<Arg>],
        inferred: &[(usize, Value)],
        pack: Option<usize>,
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
        let written = |arg: &Entry<Arg>| match arg {
            Entry::One(Arg { name: None, value }) => Ok(*value),
            _ => Err(Unevaluated::Unsupported),
        };
        let (fixed, packed) = args.split_at(pack.unwrap_or(args.len()).min(args.len()));
        for arg in fixed {
            fill(&mut passed);
            let expr = written(arg)?;
            let slot = passed.len();
            let ty = params.get(slot).copied().flatten();
            passed.push(if quotes.get(slot) == Some(&true) {
                Passed::Code(expr)
            } else if comptime.get(slot) == Some(&true) {
                Passed::Known { expr, ty }
            } else {
                Passed::Run { expr, ty }
            });
        }
        if pack.is_some() {
            fill(&mut passed);
            let exprs = packed.iter().map(written).collect::<Result<_, _>>()?;
            passed.push(Passed::Pack { exprs, ty: params.get(passed.len()).copied().flatten() });
        }
        fill(&mut passed);
        Ok(passed)
    }
}
