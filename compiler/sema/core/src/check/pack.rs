//! A pack under the C convention (spec 07, section 5): its items are the variadic arguments of a
//! C function, `...`, which Vun passes and never reads, each as C's default promotions leave it.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};
use vun_primitives::abi::Abi;
use vun_sema_float::Format;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::item::DeclRef;

use super::Checker;
use crate::decl::DeclType;

impl Checker<'_, '_, '_> {
    /// Reports a C function declared by `decl` whose pack C could not pass: of a function with a
    /// body, or of an item C promotes.
    pub(super) fn pack(&mut self, hir: &Hir, decl: DeclId) {
        let key = DeclRef::new(self.env.file.clone(), hir, decl);
        let Ok(declared) = self.env.cx.get::<DeclType>(&key) else { return };
        let Some(signature) = declared.ty.signature() else { return };
        let Some(items) = signature.params.last().and_then(|p| p.packed()) else { return };
        if !signature.has(vun_primitives::Primitive::Abi(Abi::c)) {
            return;
        }
        let body = hir.decls[decl].value.is_some_and(|v| matches!(hir.exprs[v].kind, ExprKind::Fn { .. }));
        let items = items.as_tuple().map(<[_]>::to_vec).unwrap_or_default();
        let promoted = items.iter().find(|p| Format::of(p).is_some_and(|f| f != Format::F64));
        let msg = if body {
            Msg::new().text("a C function declared with a body cannot take variadic arguments")
        } else if let Some(ty) = promoted {
            let msg = Msg::new().text("C promotes a variadic ").ty(ty.to_string()).text(" to ");
            msg.code("f64").text(", which this item of the pack must be")
        } else {
            return;
        };
        let help = if body {
            "declare it without a body, as a function C provides, or take the arguments as a slice"
        } else {
            "declare the item as `f64`, which C passes"
        };
        let note = "C reads variadic arguments through `va_list`, which Vun does not provide";
        let d = Diagnostic::error(errors::ABI_VARIADIC, msg).at(key.decl.origin, None).help(help);
        self.diag.error(if body { d.note(note) } else { d });
    }
}
