//! Operators (spec 02, section 4): a call of the function typing found for each, integers'
//! included. The operands are passed as the function's parameters ask, each reached through
//! its borrows, then borrowed in the mode its parameter is written with, or read.

use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::callable::Callable;
use vun_sema::member::MemberValue;
use vun_sema::ty::Ty;
use vun_sema::typeck::Operation;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue};
use crate::place::{Place, Projection};

/// An operand of an operator: an expression, or the place a pattern matches, of its type.
pub(super) enum Side {
    Expr(ExprId),
    Matched(Place, Option<Ty>),
}

impl Builder<'_, '_, '_> {
    /// The operator typed at `at`: a call of its function with `receiver`, then `args`.
    pub(super) fn operator(
        &mut self,
        at: Operation,
        receiver: ExprId,
        args: Vec<Side>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        let operator = self.typed().and_then(|t| t.operators.get(&at).cloned()).ok_or(unbuilt)?;
        let f =
            self.env.cx.get::<MemberValue>(&operator.method).ok().and_then(Callable::of).ok_or(unbuilt)?;
        let modes = f.modes(self.env()).unwrap_or_default();
        let mut values: Vec<Operand> = Vec::new();
        let fill = |values: &mut Vec<Operand>| {
            while let Some((_, value)) = operator.inferred.iter().find(|(slot, _)| *slot == values.len()) {
                values.push(Operand::Const(value.clone()));
            }
        };
        fill(&mut values);
        for side in std::iter::once(Side::Expr(receiver)).chain(args) {
            let mode = modes.get(values.len()).copied().flatten();
            // An operand that is no borrow, passed by value, is read as any argument is.
            if let (None, Side::Expr(expr)) = (mode, &side)
                && self.type_of(*expr).is_some_and(|t| t.borrowed().is_none())
            {
                let value = self.operand(*expr)?;
                values.push(value);
                fill(&mut values);
                continue;
            }
            let place = match side {
                Side::Expr(expr) => self.through_borrows(expr)?,
                Side::Matched(place, ty) => Self::reach(place, ty),
            };
            let value = match mode {
                Some(mode) => {
                    let mode = (!mode.is_empty()).then(|| Symbol::intern(mode));
                    self.temp(Rvalue::Ref { mode, place }, origin)
                }
                None => self.temp(Rvalue::Use(Operand::Place(place)), origin),
            };
            values.push(value);
            fill(&mut values);
        }
        Ok(self.temp(Rvalue::Call { callee: Operand::Const(f.value()), args: values }, origin))
    }

    /// `place`, of type `ty`, followed through each borrow its type is.
    fn reach(mut place: Place, mut ty: Option<Ty>) -> Place {
        while let Some(pointee) = ty.and_then(|t| t.borrowed()) {
            place = place.project(Projection::Deref);
            ty = Some(pointee);
        }
        place
    }
}
