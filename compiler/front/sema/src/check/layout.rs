//! Layout attributes (spec 05, section 6): the fields of a `#[layout.bits(uN)]` struct take
//! exactly N bits.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};

use super::Checker;
use crate::Primitive;
use crate::decl::DeclValue;
use crate::item::DeclRef;
use crate::layout::Layouts;
use crate::member::Members;
use crate::primitives::layout::Layout;
use crate::ty::Value;

impl Checker<'_, '_, '_> {
    /// Reports the struct declared by `decl` when it is `#[layout.bits(uN)]` and its fields do
    /// not take N bits.
    pub(super) fn bit_layout(&mut self, hir: &Hir, decl: DeclId) {
        let d = &hir.decls[decl];
        let Some(value) = d.value.filter(|&v| matches!(hir.exprs[v].kind, ExprKind::Struct(_))) else {
            return;
        };
        let cx = self.env.cx;
        let Some(args) = self.env.attribute(&d.attrs, Primitive::Layout(Layout::bits)) else { return };
        let [backing] = args.as_slice() else { return };
        let (Some(backing), Ok(Value::Type(ty))) =
            (backing.as_type(), cx.get::<DeclValue>(&DeclRef::new(self.env.file.clone(), hir, decl)))
        else {
            return;
        };
        let Some(members) = cx.get::<Members>(&ty) else { return };
        let fields = members.fields.iter().map(|f| Layouts::bits(cx, *f.ty.as_ref().ok()?));
        let (Some(taken), Some(width)) = (fields.sum::<Option<u64>>(), Layouts::bits(cx, backing)) else {
            return;
        };
        if taken != width {
            let msg = Msg::new()
                .text("the fields take ")
                .num(i128::from(taken))
                .text(" bits, not the ")
                .num(i128::from(width))
                .text(" of ")
                .code(backing.to_string());
            self.diag
                .error(Diagnostic::error(errors::LAYOUT_BITS_WIDTH, msg).at(hir.exprs[value].origin, None));
        }
    }
}
