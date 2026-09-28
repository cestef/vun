//! Compile-time work splits in two (spec 03, section 7). Expansions, `$if` and `$for`, run while
//! a body is typed, since its types depend on what they generate. Evaluations, a `$e` whose value
//! typing does not need, such as `$assert(x)`, run once the body is typed, with its types, so that
//! the code values they pass evaluate where they are written.

use std::sync::Arc;

use vun_diag::{ChildKind, DiagCtxt, Diagnostic, Msg, errors};
use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, ExprId, ExprKind, Hir, PatId};
use vun_hir::visit::{Visitor, walk_expr, walk_pat};
use vun_origin::OriginId;
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
            if surely && let Some((read, local)) = Reads::local(&here, operand) {
                diag.error(Self::runtime(hir, local, hir.exprs[read].origin));
                continue;
            }
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

impl TypedBody {
    /// The report of a `$` form reading, at `origin`, the local `local`, which its body computes
    /// when it runs: what `$` asks for cannot wait for that.
    fn runtime(hir: &Hir, local: BinderId, origin: OriginId) -> Diagnostic {
        let name = hir.binders[local].name;
        let msg = Msg::new().code(name.as_str()).text(" is known only when the program runs");
        let help = Msg::new()
            .text("bind it with ")
            .code("$")
            .text(" to know it when compiling, or compute this without ")
            .code("$");
        Diagnostic::error(errors::COMPTIME_RUNTIME, msg).at(origin, None).child(ChildKind::Help, help)
    }
}

/// The names an expression reads, and the binders it introduces itself.
#[derive(Default)]
struct Reads {
    names: Vec<ExprId>,
    bound: Vec<BinderId>,
}

impl Reads {
    /// The first local `expr` reads that its body computes when it runs (bound without `$`, by
    /// the body rather than by `expr` itself or a namespace), and the name reading it.
    fn local(env: &Env<'_, '_>, expr: ExprId) -> Option<(ExprId, BinderId)> {
        let hir = &env.parsed.hir;
        let mut reads = Self::default();
        reads.expr(hir, expr);
        let resolved = &env.parsed.resolution.names;
        let mut binders = reads.names.iter().filter_map(|&e| match resolved[e] {
            Some(Res::Binder(b)) => Some((e, b)),
            _ => None,
        });
        binders.find(|&(_, b)| {
            !hir.binders[b].comptime && hir.member_decl(b).is_none() && !reads.bound.contains(&b)
        })
    }
}

impl Visitor for Reads {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let ExprKind::Name(_) = hir.exprs[expr].kind {
            self.names.push(expr);
        }
        walk_expr(self, hir, expr);
    }

    fn pat(&mut self, hir: &Hir, pat: PatId) {
        hir.binders(pat, &mut self.bound);
        walk_pat(self, hir, pat);
    }
}
