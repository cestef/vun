//! `_` as a whole value (spec 01, section 3): provided from outside, which only `lang` (its
//! primitives), a declaration with a `link` attribute (the linker) and an interface's
//! requirements (each impl) may say; anywhere else it is `binding.hole_value`.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_primitives::link::Link;
use vun_sema_ty::Value;
use vun_sema_ty::item::Site;

use super::Checker;

impl Checker<'_, '_, '_> {
    /// Reports the declaration `decl` when its value is `_` where nothing provides it.
    pub(super) fn hole(&mut self, hir: &Hir, decl: DeclId) {
        let d = &hir.decls[decl];
        let Some(value) = d.value else { return };
        if !matches!(hir.exprs[value].kind, ExprKind::Hole) || self.interfaces > 0 || self.in_lang() {
            return;
        }
        let linked = Link::ALL.iter().any(|&l| self.env.attribute(&d.attrs, Primitive::Link(l)).is_some());
        if linked {
            return;
        }
        let msg = Msg::new()
            .code("_")
            .text(" as a value means provided from outside, and nothing provides it here");
        let help = Msg::new()
            .text("give the value, or name what provides it with a ")
            .code("link")
            .text(" attribute");
        let diagnostic = Diagnostic::error(errors::BINDING_HOLE_VALUE, msg)
            .at(hir.exprs[value].origin, None)
            .child(vun_diag::ChildKind::Help, help);
        self.diag.error(diagnostic);
    }

    /// Whether the file checked is in the package of `lang`, which declares the primitives.
    fn in_lang(&self) -> bool {
        match self.env.site(Lang::NAMESPACE) {
            Ok(Site::Namespace(ns)) => ns.package == self.env.file.package,
            _ => false,
        }
    }

    /// Whether `callee` is `lang.interface`, whose argument's struct holds requirements.
    pub(super) fn is_interface(&self, callee: vun_hir::tree::ExprId) -> bool {
        matches!(self.env.eval(callee), Ok(Value::Primitive(Primitive::Lang(Lang::interface))))
    }
}
