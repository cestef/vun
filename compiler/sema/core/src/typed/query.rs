//! The queries typing each instantiation and declaration, through the installed typist.

use std::sync::Arc;

use vun_hir::db::Parsed;
use vun_hir::tree::{DeclId, ExprId, ExprKind};
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::ExprRef;

use super::{Found, TypedBody};
use crate::env::Env;
use crate::eval::Instantiate;
use crate::installed::Installed;

/// What the typing of a body, a declaration or an expression asks of the typer.
pub trait Typist: Send + Sync {
    /// The body of the function literal `literal` typed in `env`, its `$` parameters bound to
    /// `args`.
    fn function(&self, env: &Env<'_, '_>, literal: ExprId, args: &[Option<Value>]) -> TypedBody;

    /// The value of the declaration `decl` typed in `env`, outside a body.
    fn declaration(&self, env: &Env<'_, '_>, decl: DeclId) -> TypedBody;

    /// What typing the expression `expr` finds, compile-time code outside a body.
    fn expression(&self, env: &Env<'_, '_>, expr: ExprId) -> Arc<Found>;
}

/// The installed typist, set once by the driver.
pub type Typists = Installed<dyn Typist>;

/// The typed body of an instantiation of a function literal; `None` for a file the driver did
/// not read. A generic body is typed for each instantiation its callers reach, with its `$`
/// parameters bound, and what that reports is reported at the generic definition.
pub struct Typed;

impl Query for Typed {
    const NAME: &'static str = "typed";
    type Key = Instantiation;
    type Value = Option<Arc<TypedBody>>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        if let Some(enclosing) = i.enclosing(cx) {
            return cx.get::<Self>(&enclosing);
        }
        let env = Env::inside(cx, &i.f, i.args.iter().cloned())?.abstracted(i.f.literal.expr.id);
        let hir = &env.parsed.hir;
        let ExprKind::Fn { .. } = &hir.exprs[i.f.literal.expr.id].kind else { return None };
        let typist = cx.db().resource::<Typists>().get().cloned()?;
        Some(Arc::new(typist.function(&env, i.f.literal.expr.id, &i.args)))
    }

    /// A body that reaches itself through calls is typed on its way in; the inner reach sees
    /// nothing.
    fn cycle(_: &Cx<'_>, _: &Instantiation, _: &Cycle) -> Self::Value {
        None
    }
}

/// The instantiation a closure's body is typed within.
pub trait Enclosing: Sized {
    /// For a closure's function, the instantiation of the function it is written in, whose
    /// typed body holds its own: the one it was evaluated in when that runs the enclosing
    /// function, or else the enclosing function evaluated where the closure was, binding
    /// nothing. `None` for any other function.
    fn enclosing(&self, cx: &Cx<'_>) -> Option<Self>;
}

impl Enclosing for Instantiation {
    fn enclosing(&self, cx: &Cx<'_>) -> Option<Self> {
        let file = &self.f.literal.file;
        let parsed = cx.get::<Parsed>(file)?;
        let hir = &parsed.hir;
        let enclosing = hir.enclosing_function(hir.closure_of(self.f.literal.expr.id)?)?;
        match &self.f.within {
            Some(within) if within.f.literal.expr.id == enclosing => within.body(cx),
            _ => {
                let literal = ExprRef::new(file.clone(), hir, enclosing);
                Some(Self::new(self.f.at(literal), Vec::new()))
            }
        }
    }
}
