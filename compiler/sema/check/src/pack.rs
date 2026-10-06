//! A pack under the C convention (spec 07, section 5): its items are the variadic arguments of a
//! C function, `...`, which Vun passes and never reads, each as C's default promotions leave it.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};
use vun_sema_ty::item::DeclRef;

use super::Checker;
use vun_sema::callable::{Promoted, Variadic};
use vun_sema::decl::DeclType;

impl Checker<'_, '_, '_> {
    /// Reports a C function declared by `decl` whose pack C could not pass: of a function with a
    /// body, or of an item C promotes.
    pub(super) fn pack(&mut self, hir: &Hir, decl: DeclId) {
        let key = DeclRef::new(self.env.file.clone(), hir, decl);
        let Ok(declared) = self.env.cx.get::<DeclType>(&key) else { return };
        let Some(signature) = declared.ty.signature() else { return };
        if signature.params.last().and_then(|p| p.packed()).is_none() || !signature.is_c() {
            return;
        }
        let body = hir.decls[decl].value.is_some_and(|v| matches!(hir.exprs[v].kind, ExprKind::Fn { .. }));
        let origin = key.decl.origin;
        if body {
            let msg = Msg::new().text("a C function declared with a body cannot take variadic arguments");
            let help =
                "declare it without a body, as a function C provides, or take the arguments as a slice";
            let note = "C reads variadic arguments through `va_list`, which Vun does not provide";
            self.diag
                .error(Diagnostic::error(errors::ABI_VARIADIC, msg).at(origin, None).help(help).note(note));
        } else if let Some(ty) = signature.promoted() {
            self.diag.error(Promoted(ty).report(origin));
        }
    }
}
