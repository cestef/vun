//! A pack under the C convention (spec 07, section 5): its items are the variadic arguments of a
//! C function, `...`, which Vun passes and never reads, each as C's default promotions leave it.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};
use vun_origin::OriginId;
use vun_sema_float::Format;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Signature, Ty};

use super::Checker;
use crate::decl::DeclType;

/// What a signature's pack is under the C convention.
pub(crate) trait Variadic {
    /// The first item of the pack that C promotes to a `double`, which a pack cannot pass as it
    /// is: its arguments are variadic ones only under the C convention. The backends extend an
    /// integer narrower than an `int`.
    fn promoted(&self) -> Option<Ty>;
}

impl Variadic for Signature {
    fn promoted(&self) -> Option<Ty> {
        let items = self.params.last().and_then(|p| p.packed())?;
        let items = items.as_tuple().filter(|_| self.is_c())?;
        items.iter().find(|p| Format::of(p).is_some_and(|f| f != Format::F64)).copied()
    }
}

/// An item of a pack that C promotes.
pub(crate) struct Promoted(pub(crate) Ty);

impl Promoted {
    pub(crate) fn report(&self, at: OriginId) -> Diagnostic {
        let msg = Msg::new().text("C promotes a variadic ").ty(self.0.to_string()).text(" to ");
        let msg = msg.code("f64").text(", which this item of the pack must be");
        Diagnostic::error(errors::ABI_VARIADIC, msg)
            .at(at, None)
            .help("declare the item as `f64`, which C passes")
    }
}

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
