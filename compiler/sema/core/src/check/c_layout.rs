//! `#[layout.c]` structs (spec 05, section 6): laid out as the target's C compiler does, which is
//! the default layout on every target, so what the attribute adds is a promise to C code: each
//! field has a type both sides read the same way.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir, Member, each_entry};
use vun_primitives::Primitive;
use vun_primitives::abi::Abi;
use vun_primitives::lang::Lang;
use vun_primitives::layout::Layout;
use vun_primitives::mem::Mem;
use vun_sema_float::Format;
use vun_sema_int::Integer;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::{NominalKind, Ty, TyKind};

use super::Checker;
use crate::attributes::Attributed;
use crate::member::Members;

/// Integer widths C has fixed-width types for, in bits.
const C_WIDTHS: [u64; 4] = [8, 16, 32, 64];

impl Checker<'_, '_, '_> {
    /// Reports each field of the `#[layout.c]` struct declared by `decl` whose type C cannot
    /// represent, and such a struct without fields.
    pub(super) fn c_layout(&mut self, hir: &Hir, decl: DeclId) {
        let Some(value) = hir.decls[decl].value else { return };
        let ExprKind::Struct(ns) = hir.exprs[value].kind else { return };
        let attrs = self.env.attrs_of(value);
        if self.env.attribute(&attrs, Primitive::Layout(Layout::c)).is_none() {
            return;
        }
        let mut fields = Vec::new();
        each_entry(&hir.namespaces[ns].members, &mut |m| {
            if let Member::Field(field) = m {
                fields.push((field.ty, hir.exprs[field.ty].origin));
            }
        });
        if fields.is_empty() {
            let msg = Msg::new()
                .text("a ")
                .code("#[layout.c]")
                .text(" struct has no fields, and C has no empty struct");
            self.diag.error(Diagnostic::error(errors::LAYOUT_NOT_C, msg).at(hir.exprs[value].origin, None));
        }
        for (annotation, origin) in fields {
            let Ok(ty) = self.env.ty(annotation) else { continue };
            if self.c_type(ty) == Some(false) {
                let msg = Msg::new().text("C cannot represent ").code(ty.to_string()).text(" as a field");
                let help = Msg::new()
                    .text("use a full-width integer, ")
                    .code("bool")
                    .text(", a float, a raw pointer, an ")
                    .code("#[abi.c]")
                    .text(" function, an array of these or another ")
                    .code("#[layout.c]")
                    .text(" struct");
                let d = Diagnostic::error(errors::LAYOUT_NOT_C, msg)
                    .at(origin, None)
                    .child(vun_diag::ChildKind::Help, help);
                self.diag.error(d);
            }
        }
    }

    /// Whether C represents values of `ty` as they are; `None` when a part is not known here.
    fn c_type(&self, ty: Ty) -> Option<bool> {
        if let Some(range) = ty.interval() {
            let bits = range.storage()?.bits();
            let full = range.count()? == Integer::power_of_two(u32::try_from(bits).ok()?);
            return Some(full && C_WIDTHS.contains(&bits));
        }
        if self.env.condition().ok() == Some(ty) {
            return Some(true);
        }
        if let Some(format) = Format::of(&ty) {
            // C has no 16-bit float type.
            return Some(format != Format::F16);
        }
        match &*ty {
            TyKind::Primitive(Primitive::Mem(Mem::Ptr), _) => Some(true),
            TyKind::Primitive(Primitive::Lang(Lang::Fn), _) => {
                Some(ty.signature()?.has(Primitive::Abi(Abi::c)))
            }
            TyKind::Primitive(Primitive::Mem(Mem::Array), _) => self.c_type(ty.as_array()?.1),
            TyKind::Nominal(nominal) if nominal.kind == NominalKind::Struct => {
                Some(nominal.attributes(self.env.cx)?.get(Primitive::Layout(Layout::c)).is_some())
            }
            // `?*T`: one pointer, null for `.none` (the niche rule).
            TyKind::Nominal(nominal) if nominal.kind == NominalKind::Enum => {
                let members = self.env.cx.get::<Members>(&ty)?;
                let payloads: Vec<_> = members.variants.iter().map(|v| v.payload.as_slice()).collect();
                let pointer = |p: &Ty| {
                    matches!(&**p, TyKind::Primitive(Primitive::Mem(Mem::Ptr), _))
                        || p.signature().is_some_and(|s| s.has(Primitive::Abi(Abi::c)))
                };
                Some(match payloads.as_slice() {
                    [[], [Ok(p)]] | [[Ok(p)], []] => pointer(p),
                    _ => false,
                })
            }
            _ => Some(false),
        }
    }
}
