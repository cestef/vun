//! What a declaration binds, for the lints that follow case (spec 10, section 5).

use vun_hir::tree::{DeclId, ExprId};
use vun_sema_ty::item::ExprRef;

use super::Walk;
use crate::lint::literal::Literal;
use crate::lint::tables::Binds;

impl Walk<'_, '_, '_> {
    /// What the declaration `id` binds (spec 10, section 5), by how it is written: a type when
    /// annotated `type`, `lang.Interface` or a function type giving one, or when its value is a
    /// `struct` that is no namespace, an `enum`, a function type, an `interface` or a function
    /// literal giving a type, or when it leaves its annotation to the requirement it implements
    /// and that is one. Any other value, an alias of a type included, is annotated to bind one. A
    /// primitive, `= _`, binds what the annotation of its declaration says.
    pub(super) fn binds(&self, id: DeclId) -> Binds {
        let hir = self.hir();
        let d = &hir.decls[id];
        let literal = |e: ExprId| Literal::written(self.env.cx, &ExprRef::new(self.env.file.clone(), hir, e));
        let annotated = d.ty.and_then(literal).is_some_and(|l| l.gives_type());
        let written = d.value.and_then(literal).is_some_and(|l| l.is_type());
        let required = d.ty.is_none() && self.required_type(id);
        if annotated || written || required { Binds::Type } else { Binds::Value }
    }

    /// Whether the interface a witness implements requires a type of the member `id`, which it
    /// leaves unannotated (`Error := ();` in an impl of `Writer`, decision 470).
    fn required_type(&self, id: DeclId) -> bool {
        let asked = self.env.annotated(id).and_then(Result::ok);
        asked.is_some_and(|w| w.env.ty(w.expr).is_ok_and(|t| t.holds_types()))
    }

    /// Whether the annotation `expr` denotes `type` or `lang.Interface`: what binds a type.
    pub(super) fn types(&self, expr: ExprId) -> bool {
        self.env.ty(expr).is_ok_and(|t| t.holds_types())
    }
}
