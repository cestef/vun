//! The types of the locals walked, which typing finds.

use vun_hir::tree::{BinderId, ExprId, Hir};
use vun_hir::visit;
use vun_sema::typed::Typed;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::ExprRef;

use super::Walk;

impl Walk<'_, '_, '_> {
    /// Whether typing gives `binder` a type holding one integer: what a `mut` local initialised by
    /// a literal takes, so that any other value written to it traps.
    pub(super) fn pinned(&self, binder: BinderId) -> bool {
        let ty = self.typed.as_ref().and_then(|typed| typed.binders.get(&binder));
        ty.and_then(|ty| ty.interval()).is_some_and(|range| range.is_singleton())
    }

    /// The function literal `expr`, walked with its typed body: that of an instantiation binding
    /// no parameter, so a generic function's locals of a type that depends on one have none.
    pub(super) fn function(&mut self, hir: &Hir, expr: ExprId) {
        let literal = ExprRef::new(self.env.file.clone(), hir, expr);
        self.typed = self.env.cx.get::<Typed>(&Instantiation::plain(literal));
        visit::walk_expr(self, hir, expr);
        self.typed = None;
    }
}
