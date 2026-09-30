//! Where typing gave up, told where a value it left without a type is needed.

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{ExprId, Hir, NsId};
use vun_hir::visit::{self, Visitor};
use vun_origin::OriginId;
use vun_sema_ty::Ty;
use vun_sema_ty::trap::Unevaluated;

use super::{Found, GaveUp, Known, TypedBody, Typer};

/// What typing may leave without a type in a body with no gap in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Leaves {
    /// Nothing: a body that is built, its `$` parameters bound.
    Nothing,
    /// The forms reading a `$` parameter the body leaves unbound: a generic body, typed apart
    /// from its instantiations, which type them.
    Unbound,
}

impl TypedBody {
    /// Why the value at `at`, written there or bound to the local declared there, has no type
    /// though typing reported nothing: the first form under it typing gave up on silently in a
    /// body that `leaves` that. Nothing when no form was found there.
    #[must_use]
    pub fn gave_up_under(&self, hir: &Hir, at: OriginId, leaves: Leaves) -> Option<Diagnostic> {
        let written = hir.exprs.iter().find(|(_, e)| e.origin == at).map(|(e, _)| e);
        let bound = || {
            let (binder, _) = hir.binders.iter().find(|(_, b)| b.origin == at)?;
            hir.decls[hir.decl_of(binder)?].value
        };
        let value = written.or_else(bound)?;
        let silent = Silent(self, leaves);
        let (form, why) = silent.first_under(hir, value)?;
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

    /// Why typing left `expr` without a type, as an evaluation needing that type says: the
    /// evaluation typing gave up for under it, or a form typing does not type yet.
    #[must_use]
    pub fn unevaluated(&self, hir: &Hir, expr: ExprId) -> Unevaluated {
        match self.first_under(hir, expr) {
            Some((_, GaveUp::Unevaluated(why))) => why.clone(),
            _ => Unevaluated::Unsupported,
        }
    }
}

impl GaveUp {
    /// Whether typing said nothing of it in a body that `leaves` that: no error reported
    /// explains it, and it is not left to the instantiations of a generic body, as a form
    /// reading a `$` parameter the body leaves unbound is.
    #[must_use]
    pub fn silent(&self, leaves: Leaves) -> bool {
        match self {
            Self::Reported => false,
            Self::Unevaluated(Unevaluated::Unbound) => leaves == Leaves::Nothing,
            _ => true,
        }
    }

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
            Self::Reported => Msg::new().text("it depends on a form typing reported an error for"),
        }
    }
}

impl Typer<'_, '_, '_> {
    /// The type of `value`, typed as `known`, that a pattern matches, or why it has none: why
    /// typing gave up under it, or the error typing reported there.
    pub(super) fn matched(&self, value: ExprId, known: Option<Known>) -> Result<Ty, GaveUp> {
        let why =
            || self.found.first_under(self.hir(), value).map_or(GaveUp::Reported, |(_, why)| why.clone());
        known.map(|k| k.ty).ok_or_else(why)
    }
}

/// Where typing gave up, by form.
trait GaveUps {
    fn gave_up_on(&self, expr: ExprId) -> Option<&GaveUp>;

    /// The first form under `expr`, in source order, that typing gave up on, and why.
    fn first_under<'t>(&'t self, hir: &Hir, expr: ExprId) -> Option<(ExprId, &'t GaveUp)> {
        let mut under = Under { gave_ups: self, found: None };
        under.expr(hir, expr);
        under.found
    }
}

impl GaveUps for Found {
    fn gave_up_on(&self, expr: ExprId) -> Option<&GaveUp> {
        self.gave_up.get(&expr)
    }
}

/// In the body or in some step of it.
impl GaveUps for TypedBody {
    fn gave_up_on(&self, expr: ExprId) -> Option<&GaveUp> {
        self.gave_up.get(&expr).or_else(|| self.steps.values().find_map(|step| step.gave_up_on(expr)))
    }
}

/// The forms of a typed body, or of some step of it, typing gave up on silently, in a body
/// that leaves what the second says.
struct Silent<'t>(&'t TypedBody, Leaves);

impl GaveUps for Silent<'_> {
    fn gave_up_on(&self, expr: ExprId) -> Option<&GaveUp> {
        self.0.gave_up_on(expr).filter(|why| why.silent(self.1))
    }
}

/// Finds the first form typing gave up on under an expression, in source order.
struct Under<'t, G: ?Sized> {
    gave_ups: &'t G,
    found: Option<(ExprId, &'t GaveUp)>,
}

impl<G: GaveUps + ?Sized> Visitor for Under<'_, G> {
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if self.found.is_some() {
            return;
        }
        if let Some(why) = self.gave_ups.gave_up_on(expr) {
            self.found = Some((expr, why));
            return;
        }
        visit::walk_expr(self, hir, expr);
    }
}
