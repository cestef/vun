//! Temporaries: locals holding what a form computes, typed from what they hold.

use vun_origin::OriginId;
use vun_sema::eval::Instantiation;
use vun_sema::ty::{Ty, TyKind, Value};

use super::Builder;
use crate::place::Place;
use crate::{Aggregate, Local, LocalDecl, Lowered, Operand, Role, Rvalue};

impl Builder<'_, '_, '_> {
    /// `value` in a new temporary.
    pub(super) fn temp(&mut self, value: Rvalue, origin: OriginId) -> Operand {
        Operand::Place(Place::of(self.temp_local(value, origin)))
    }

    /// `value` in a new temporary of type `ty`, what typing found for it.
    pub(super) fn typed_temp(&mut self, value: Rvalue, ty: Ty, origin: OriginId) -> Operand {
        let local = self.temp_local(value, origin);
        self.locals[local].ty = Some(ty);
        Operand::Place(Place::of(local))
    }

    /// A new temporary local holding `value`.
    pub(super) fn temp_local(&mut self, value: Rvalue, origin: OriginId) -> Local {
        let ty = self.rvalue_ty(&value);
        let local = self.locals.push(LocalDecl { name: None, ty, role: Role::Held, origin });
        self.assign(Place::of(local), value, origin);
        local
    }

    /// The type of what `value` computes, where its operands' types give it; a temporary holding
    /// an expression's value takes the expression's type instead.
    fn rvalue_ty(&self, value: &Rvalue) -> Option<Ty> {
        let operand = |o: &Operand| match o {
            Operand::Place(place) => place.ty(self.env.cx, &self.locals),
            Operand::Const(value) => value.ty(),
        };
        match value {
            Rvalue::Use(o) => operand(o),
            Rvalue::Not { .. } | Rvalue::IsVariant { .. } => self.env.condition().ok(),
            Rvalue::Resume { step, .. } => Some(*step),
            Rvalue::End { .. } => Some(TyKind::unit()),
            Rvalue::Ref { mode, place } => TyKind::borrowed_as(*mode, place.ty(self.env.cx, &self.locals)?),
            Rvalue::Aggregate {
                kind: Aggregate::Struct { ty, .. } | Aggregate::Variant { ty, .. }, ..
            } => Some(*ty),
            Rvalue::Call { callee: Operand::Const(Value::Fn(f)), args } => {
                Some(Instantiation::called(self.env.cx, f, args).signature(self.env.cx).ok()?.result)
            }
            Rvalue::Aggregate { kind: Aggregate::Tuple, parts } => {
                Some(TyKind::Tuple(parts.iter().map(operand).collect::<Option<_>>()?).intern())
            }
            _ => None,
        }
    }
}
