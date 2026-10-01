//! What a call expects of a function written as an argument (spec 01, section 4): the parameter it
//! is passed for is a function type, `f: (x: u8) -> bool`, or a `$` parameter the argument itself
//! binds, constrained by `lang.ops.Call((T), R)`, as `$F: Call((T), U), f: F` is. The types of
//! the parameters of the function written, and its result where the call says it, are those.

use vun_hir::resolve::Res;
use vun_hir::tree::{Entry, ExprKind, Param};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use super::Typer;
use crate::callable::{Callable, Written};
use crate::env::{Context, Env};

/// The parameter types and the result a parameter expects of a function written for it.
struct Expected {
    params: Vec<Result<Ty, Unevaluated>>,
    result: Option<Result<Ty, Unevaluated>>,
}

impl Typer<'_, '_, '_> {
    /// What the parameter at `slot` of `f` expects of a function written for it, if it says.
    /// `bound` gives the `$` parameters inferred so far, by position: the types of the
    /// parameters may name them, and the result is the one the call fixes whatever they turn out
    /// to be.
    pub(super) fn argument_context(
        &self,
        f: &Callable,
        slot: usize,
        bound: Vec<Option<Value>>,
    ) -> Option<Context> {
        let plain = f.written(self.env())?;
        let result = Self::expected_parts(&plain, slot)?.result;
        let known = match f {
            Callable::Fn(func) if bound.iter().any(Option::is_some) => {
                let env = Env::inside(self.env().cx, func, bound)?;
                Written { env, expr: func.literal.expr.id }
            }
            _ => plain,
        };
        let params = Self::expected_parts(&known, slot)?.params;
        Some(Context::passed(params, result.and_then(Result::ok)))
    }

    /// The parameter types and the result the parameter at `slot` of the function written in
    /// `written` expects of a function written for it.
    fn expected_parts(written: &Written<'_, '_>, slot: usize) -> Option<Expected> {
        let env = &written.env;
        let hir = &env.parsed.hir;
        let params = written.params()?;
        let annotation = params.get(slot)?.1?;
        // The annotation, or for a `$` parameter the annotation of that one.
        let shape = match hir.exprs[annotation].kind {
            ExprKind::Name(_) => {
                let Some(Res::Binder(binder)) = env.parsed.resolution.names[annotation] else { return None };
                params.iter().find(|(b, _)| *b == Some(binder))?.1?
            }
            _ => annotation,
        };
        match &hir.exprs[shape].kind {
            ExprKind::FnType { params, returns } => {
                let types = params.iter().map(|p| match p {
                    Entry::One(Param { ty: Some(t), .. }) => env.ty(*t),
                    _ => Err(Unevaluated::Unsupported),
                });
                Some(Expected { params: types.collect(), result: returns.map(|r| env.ty(r)) })
            }
            ExprKind::Call { callee, args } => {
                let call = env.item(vun_items::lang::ops::Call).ok()?;
                (env.eval(*callee).ok()? == call).then_some(())?;
                let [Entry::One(params), result] = args.as_slice() else { return None };
                // `(T)` is the one parameter `T`; `(A, B)`, or a tuple type, are those it holds.
                let types = match &hir.exprs[params.value].kind {
                    ExprKind::Tuple(items) => items
                        .iter()
                        .map(|item| match item {
                            Entry::One(item) => env.ty(*item),
                            Entry::Staged(_) => Err(Unevaluated::Unsupported),
                        })
                        .collect(),
                    _ => match env.eval(params.value).ok()?.as_type()? {
                        given if let TyKind::Tuple(types) = &*given => {
                            types.iter().copied().map(Ok).collect()
                        }
                        given => vec![Ok(given)],
                    },
                };
                let result = match result {
                    Entry::One(r) => Some(env.ty(r.value)),
                    Entry::Staged(_) => None,
                };
                Some(Expected { params: types, result })
            }
            _ => None,
        }
    }
}
