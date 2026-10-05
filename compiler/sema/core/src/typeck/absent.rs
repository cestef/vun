//! What typing reports when what a use needs is not there: a name not visible where it is used, an
//! operator with no impl of its interface, and a member its type does not have.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_items::Item;
use vun_origin::OriginId;
use vun_sema_ty::Ty;
use vun_suggest::closest;

use super::{Known, Typer};
use crate::member::Members;
use crate::visible::Visible;

impl Typer<'_, '_, '_> {
    /// Reports `name`, used at `expr`, when its visibility `visible` does not reach there.
    pub(super) fn check_visible(&mut self, expr: ExprId, name: Symbol, visible: &Visible) {
        if self.env().sees(visible, expr) != Some(false) {
            return;
        }
        self.diag.error(self.env().not_visible(self.hir().exprs[expr].origin, name, visible));
    }

    /// The type of `operand.*` at `expr`: what the operand borrows or points to, or what its type
    /// reads through by its `deref`; reported when it has neither.
    pub(super) fn read_through(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        let known = self.typed(operand)?;
        self.raw_deref(expr, known.ty);
        let owned = || self.env().cx.get::<Members>(&known.ty)?.deref().map(|(_, ty)| ty);
        let Some(ty) = known.ty.pointee().or_else(owned) else {
            self.not_dereferenceable(expr, known.ty);
            return None;
        };
        Some(Known { ty, ..known })
    }

    /// Reports the read `.*` at `expr` of a value of type `ty`, which borrows or points to nothing
    /// and owns no value to read through.
    pub(super) fn not_dereferenceable(&mut self, expr: ExprId, ty: Ty) {
        let msg = Msg::new().text("cannot read through a value of type ").ty(ty.to_string());
        let help = Msg::new()
            .text("only a borrow, a pointer or a type with a ")
            .code("deref")
            .text(" can be read with ")
            .code(".*");
        let d = Diagnostic::error(errors::TYPES_NOT_DEREFERENCEABLE, msg)
            .at(self.hir().exprs[expr].origin, None)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    /// Reports the operator at `expr` as having no impl of its interface `interface` for `ty`,
    /// the type of its left operand.
    pub(super) fn no_impl(&mut self, expr: ExprId, interface: Item, ty: Ty) {
        let msg = Msg::new().text("no impl of ").code(interface.to_string()).text(" for ").ty(ty.to_string());
        let help = Msg::new()
            .text("declare one: a struct of its methods, with the attribute ")
            .code(vun_items::lang::r#impl.to_string())
            .text(" naming ")
            .code(interface.to_string())
            .text(" of ")
            .ty(ty.to_string());
        let d =
            Diagnostic::error(errors::TYPES_NO_IMPL, msg).at(self.hir().exprs[expr].origin, None).help(help);
        self.diag.error(d);
    }

    /// Reports the `$for` over `iter`, whose value `ty` has no items to step through.
    pub(super) fn no_items(&mut self, iter: ExprId) {
        let mut msg = Msg::new().text("a `$for` cannot step through ");
        msg = match self.typed(iter) {
            Some(known) => msg.text("a value of type ").ty(known.ty.to_string()),
            None => msg.text("this value"),
        };
        let help = Msg::new().text("it steps through the items of an array, of a tuple or of a range");
        let d =
            Diagnostic::error(errors::TYPES_NO_ITEMS, msg).at(self.hir().exprs[iter].origin, None).help(help);
        self.diag.error(d);
    }

    /// Reports `ty`, of the operand `expr` of a form driven by `interface`, as having no impl of
    /// it; a type whose impls its requirements give is left to them.
    pub(super) fn unimplemented(&mut self, expr: ExprId, interface: Option<Item>, ty: Ty) {
        if let Some(interface) = interface
            && !ty.through_borrows().is_required()
        {
            self.no_impl(expr, interface, ty);
        }
    }

    /// Reports `name`, used at `at` on the type `ty`, as a field, which values have.
    pub(super) fn field_of_type(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in the type ").ty(ty.to_string());
        let note = Msg::new().text("it is a field, which a value of the type has");
        self.diag
            .error(Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None).child(ChildKind::Note, note));
    }

    /// Reports `name`, used at `at`, as no member of `ty`.
    pub(super) fn no_member(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in ").ty(ty.to_string());
        let mut d = Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None);
        if let Some(members) = self.env().cx.get::<Members>(&ty.through_borrows()) {
            let fields = members.fields.iter().map(|f| f.name);
            let decls = members.decls.iter().map(|m| m.name);
            let names: Vec<Symbol> =
                fields.chain(decls).chain(members.variants.iter().map(|v| v.name)).collect();
            if let Some(near) = closest(name.as_str(), names.iter().map(|n| n.as_str())) {
                d = d.help(Msg::did_you_mean(near));
            }
        }
        self.diag.error(d);
    }
}
