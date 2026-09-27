//! Compile-time work splits in two (spec 03, section 7). Expansions, `$if` and `$for`, run while
//! a body is typed, since its types depend on what they generate. Evaluations, a `$e` whose value
//! typing does not need, such as `$assert(x)`, run once the body is typed, with its types, so that
//! the code values they pass evaluate where they are written.

use std::sync::Arc;

use vun_diag::DiagCtxt;
use vun_hir::tree::ExprKind;
use vun_query::{Cx, Cycle, Query};

use super::{Typed, TypedBody};
use crate::env::Env;
use crate::eval::Instantiation;

/// What typing an instantiation reports, and what its evaluations report run after it; `None`
/// for a file the driver did not read.
pub struct Reported;

impl Query for Reported {
    const NAME: &'static str = "reported";
    type Key = Instantiation;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        if let Some(enclosing) = i.enclosing(cx) {
            return cx.get::<Self>(&enclosing);
        }
        let typed = cx.get::<Typed>(i)?;
        let env = Env::inside(cx, &i.f, i.args.iter().cloned())?.abstracted(i.f.literal.expr.id);
        let mut diag = typed.diag.clone();
        typed.evaluate(&env, &mut diag);
        Some(Arc::new(diag))
    }

    fn cycle(_: &Cx<'_>, _: &Instantiation, _: &Cycle) -> Self::Value {
        None
    }
}

impl TypedBody {
    /// Runs the evaluations of this body typed in `env`, and those of its steps, reporting in
    /// `diag` what stops one ([`crate::env::Unevaluated::report`]).
    pub fn evaluate(self: &Arc<Self>, env: &Env<'_, '_>, diag: &mut DiagCtxt) {
        let here = env.clone().staging(&self.staged).typed_by(Some(Arc::clone(self)));
        let hir = &here.parsed.hir;
        // A body typing reported errors in has forms its evaluation cannot be told apart from.
        let clean = self.diag.error_count() == 0;
        for &expr in &self.evaluations {
            let ExprKind::Prefix { operand, .. } = hir.exprs[expr].kind else { continue };
            let surely = clean && self.types.contains_key(&operand);
            if let Err(stopped) = here.eval(operand)
                && let Some(d) = stopped.report(hir.exprs[expr].origin, surely)
            {
                diag.error(d);
            }
        }
        for step in self.steps.values() {
            step.evaluate(env, diag);
        }
    }
}
