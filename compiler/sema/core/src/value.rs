//! The value of an expression when compiling computes it, wherever it is written: what an editor
//! shows of a `$` form or of any expression known at compile time.

use std::sync::Arc;

use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{DeclRef, ExprRef};
use vun_sema_ty::trap::Unevaluated;

use crate::decl::TypedDecl;
use crate::env::Env;
use crate::typeck::Typed;

/// The value of an expression: in the body of the function holding it, typed plain, or else in
/// the value of the member declaration holding it, as that declaration's value is evaluated.
pub struct ExprValue;

impl Query for ExprValue {
    const NAME: &'static str = "expr_value";
    type Key = ExprRef;
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, e: &ExprRef) -> Self::Value {
        let env = Env::of(cx, &e.file).ok_or(Unevaluated::Absent)?;
        let hir = &env.parsed.hir;
        if let Some(function) = hir.enclosing_function(e.expr.id) {
            let i = Instantiation::plain(ExprRef::new(e.file.clone(), hir, function));
            let typed = cx.get::<Typed>(&i).ok_or(Unevaluated::Absent)?;
            let inside = Env::inside(cx, &i.f, i.args.iter().cloned()).ok_or(Unevaluated::Absent)?;
            let here = inside.abstracted(function).staging(&typed.staged);
            return here.typed_by(Some(Arc::clone(&typed.found))).eval(e.expr.id);
        }
        let member = hir.enclosing_member(e.expr.id).ok_or(Unevaluated::Absent)?;
        let types = cx.get::<TypedDecl>(&(DeclRef::new(e.file.clone(), hir, member), None, Vec::new()));
        env.typed_by(types.map(|t| Arc::clone(&t.found))).eval(e.expr.id)
    }

    fn cycle(_: &Cx<'_>, _: &ExprRef, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
