//! Calls, closures and loops that give a value.

use super::Untyped;
use crate::ir::{Closure, Expr, FnId, Kind, Label, Stmt};
use crate::ty::Ty;

impl Expr {
    /// # Errors
    /// Returns why the arguments do not fit the parameters.
    pub fn call(
        function: FnId,
        generics: Vec<Ty>,
        params: &[Ty],
        result: &Ty,
        args: Vec<Self>,
    ) -> Result<Self, Untyped> {
        let params: Vec<Ty> = params.iter().map(|p| p.subst(&generics)).collect();
        let fits = params.len() == args.len() && params.iter().zip(&args).all(|(p, a)| p.accepts(&a.ty));
        let ty = result.subst(&generics);
        fits.then_some(Self { ty, kind: Kind::Call(function, generics, args) }).ok_or(Untyped::Mismatch)
    }

    /// `lang.flow.tail(function, args)`, which gives what a call of the function does.
    ///
    /// # Errors
    /// Returns why the arguments do not fit the parameters.
    pub fn tail_call(function: FnId, params: &[Ty], result: &Ty, args: Vec<Self>) -> Result<Self, Untyped> {
        let fits = params.len() == args.len() && params.iter().zip(&args).all(|(p, a)| p.accepts(&a.ty));
        fits.then(|| Self { ty: result.clone(), kind: Kind::TailCall(function, args) })
            .ok_or(Untyped::Mismatch)
    }

    /// A loop that runs `body` until a `break` with a value, which has type `ty`.
    #[must_use]
    pub fn forever(label: Label, body: Vec<Stmt>, ty: Ty) -> Self {
        Self { ty, kind: Kind::Forever(label, body) }
    }

    /// A closure written, of the type its values have.
    #[must_use]
    pub fn closure(closure: Closure, params: Vec<Ty>) -> Self {
        let result = closure.result.clone().unwrap_or_else(|| closure.body.ty.clone());
        let ty = Ty::Closure(closure.id, params, Box::new(result));
        Self { ty, kind: Kind::Closure(Box::new(closure)) }
    }

    /// `callee(args)` of a value whose type is a parameter bound to take `params` and give `result`.
    ///
    /// # Errors
    /// Returns why the callee is no parameter or the arguments do not fit.
    pub fn call_bound(callee: Self, params: &[Ty], result: &Ty, args: Vec<Self>) -> Result<Self, Untyped> {
        if !matches!(callee.ty, Ty::Param(_)) {
            return Err(Untyped::Operand);
        }
        Self::accepts(params, &args)?;
        Ok(Self { ty: result.clone(), kind: Kind::CallClosure(Box::new(callee), args) })
    }

    /// `callee(args)` of a closure.
    ///
    /// # Errors
    /// Returns why the callee is no closure or the arguments do not fit it.
    pub fn call_closure(callee: Self, args: Vec<Self>) -> Result<Self, Untyped> {
        let Ty::Closure(_, params, result) = &callee.ty else { return Err(Untyped::Operand) };
        Self::accepts(params, &args)?;
        let ty = (**result).clone();
        Ok(Self { ty, kind: Kind::CallClosure(Box::new(callee), args) })
    }
}
