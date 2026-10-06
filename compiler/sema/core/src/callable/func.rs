//! The parameters of a function literal as written, and whether an instantiation is of generic
//! code.

use vun_desugar::Borrow;
use vun_query::Cx;
use vun_sema_ty::call::{Func, Instantiation};

use super::Written;
use crate::env::Env;

/// A function literal's parameters, as written: which are `$` and how each is passed.
pub trait Parameters {
    /// Where this function's parameters are written: its literal, inside the call it was made in.
    #[must_use]
    fn written<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Written<'a, 'db>>;

    /// Whether each parameter is a `$` parameter; `None` when its file is not read.
    #[must_use]
    fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>>;

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    fn modes(&self, cx: &Cx<'_>) -> Option<Vec<Option<Borrow>>>;
}

impl Parameters for Func {
    fn written<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Written<'a, 'db>> {
        let env = Env::inside(cx, self, [])?;
        let expr = self.literal.expr.id;
        // A member of a witness that leaves its parameters' types out has them written by the
        // requirement it implements.
        if env.omits_param_types(expr)
            && let Some(Ok(asked)) = env.asked(expr)
        {
            return Some(asked);
        }
        Some(Written { env, expr })
    }

    fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        cx.get::<crate::fuel::Callee>(self).comptime
    }

    fn modes(&self, cx: &Cx<'_>) -> Option<Vec<Option<Borrow>>> {
        self.written(cx)?.modes()
    }
}

/// An instantiation as generic code or not.
pub trait Generic {
    /// Whether this is an instantiation of generic code: a function built in a compile-time
    /// call, or one with `$` parameters, all bound.
    fn generic(&self, cx: &Cx<'_>) -> bool;
}

impl Generic for Instantiation {
    fn generic(&self, cx: &Cx<'_>) -> bool {
        let Some(comptime) = self.f.comptime(cx) else { return false };
        let bound = comptime.iter().zip(self.args.iter()).all(|(&c, arg)| !c || arg.is_some());
        (self.f.within.is_some() || comptime.contains(&true)) && bound
    }
}
