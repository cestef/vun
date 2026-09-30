//! Where typing gave up, told where a value it left without a type is needed.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{ExprId, Hir, NsId};
use vun_hir::visit::{self, Visitor};
use vun_origin::OriginId;

use super::{GaveUp, TypedBody};

impl TypedBody {
    /// Why the value written at `at` has no type though typing reported nothing: the first form
    /// under it whose rule gave up, its parts typed. Nothing when no form was found there.
    #[must_use]
    pub fn gave_up_under(&self, hir: &Hir, at: OriginId) -> Option<Diagnostic> {
        let (value, _) = hir.exprs.iter().find(|(_, e)| e.origin == at)?;
        let mut under = Under { typed: self, found: None };
        under.expr(hir, value);
        let (form, why) = under.found?;
        let form = &hir.exprs[form];
        let msg =
            Msg::new().text("typing gave this ").code(form.kind.name()).text(" no type and reported nothing");
        let d = Diagnostic::error(errors::BACKEND_UNLOWERED, msg).at(form.origin, Some(why.msg()));
        Some(
            d.note("a compiler bug: a form typing cannot type is reported as an error").help(
                Msg::new()
                    .code("vun debug dump types")
                    .text(" marks each form typing gave up on, and ")
                    .code("VUN_LOG=vun_sema=debug")
                    .text(" logs each as it happens"),
            ),
        )
    }

    /// Why typing gave up on `expr`, in the body or in some step of it, if it did.
    fn gave_up_on(&self, expr: ExprId) -> Option<&GaveUp> {
        self.gave_up.get(&expr).or_else(|| self.steps.values().find_map(|step| step.gave_up_on(expr)))
    }
}

impl GaveUp {
    /// Why, as a report says.
    #[must_use]
    pub fn msg(&self) -> Msg {
        match self {
            Self::Form => Msg::new().text("its parts are typed; the rule for the form gave up"),
            Self::Unevaluated(why) => {
                Msg::new().text("the type of what it names has no value here: ").text(why.why())
            }
            Self::Members(ty) => {
                Msg::new().text("the members of ").ty(ty.to_string()).text(" are not all known here")
            }
        }
    }
}

/// Finds the first form typing gave up on under an expression, in source order.
struct Under<'t> {
    typed: &'t TypedBody,
    found: Option<(ExprId, &'t GaveUp)>,
}

impl Visitor for Under<'_> {
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if self.found.is_some() {
            return;
        }
        if let Some(why) = self.typed.gave_up_on(expr) {
            self.found = Some((expr, why));
            return;
        }
        visit::walk_expr(self, hir, expr);
    }
}
