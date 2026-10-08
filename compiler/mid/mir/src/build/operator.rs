//! Operators (spec 02, section 4): a call of the function typing found for each, integers'
//! included. The operands are passed as the function's parameters ask, each reached through
//! its borrows, then borrowed in the mode its parameter is written with, or read.

use vun_desugar::Access;
use vun_hir::tree::{Arg, Entry, ExprId};
use vun_origin::OriginId;
use vun_sema::callable::Callable;
use vun_sema::member::MemberValue;
use vun_sema::typed::Operation;
use vun_sema_ty::Ty;

use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{Aggregate, Operand, Rvalue};

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
        receiver: Side,
        args: Vec<Side>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = move || Unbuilt::at(origin);
        let operator = self.typed().and_then(|t| t.operators.get(&at).cloned()).ok_or_else(unbuilt)?;
        let f = self
            .env
            .cx
            .get::<MemberValue>(&operator.method)
            .ok()
            .and_then(|v| Callable::of(self.env(), v))
            .ok_or_else(unbuilt)?;
        let modes = f.modes(self.env()).unwrap_or_default();
        let mut values: Vec<Operand> = Vec::new();
        let fill = |values: &mut Vec<Operand>| {
            while let Some((_, value)) = operator.inferred.iter().find(|(slot, _)| *slot == values.len()) {
                values.push(Operand::Const(value.clone()));
            }
        };
        fill(&mut values);
        let sides: Vec<Side> = std::iter::once(receiver).chain(args).collect();
        let last = sides.len() - 1;
        for (at, side) in sides.into_iter().enumerate() {
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
            // A number lent to the call is copied first when an operand is still to be built, since
            // the later operands run after it is read, not while it is lent.
            let place = match mode {
                Some(mode)
                    if at < last
                        && mode.access() == Access::Shared
                        && place.ty(self.env.cx, &self.locals).is_some_and(|t| t.is_number()) =>
                {
                    Place::of(self.temp_local(Rvalue::Use(Operand::Place(place)), origin))
                }
                _ => place,
            };
            let value = match mode {
                Some(mode) => self.temp(Rvalue::Ref { mode, place }, origin),
                None => self.temp(Rvalue::Use(Operand::Place(place)), origin),
            };
            values.push(value);
            fill(&mut values);
        }
        let call = Rvalue::Call { callee: Operand::Const(f.value()), args: values };
        Ok(self.typed_temp(call, Some(operator.result), origin))
    }

    /// `f(a, b)` on a value that is no function, typed at `call`: its witness's `call(&mut f,
    /// (a, b))`, the arguments gathered into a tuple (spec 02, section 4).
    pub(super) fn call_through(
        &mut self,
        call: ExprId,
        callee: ExprId,
        args: &[Entry<Arg>],
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let parts = args
            .iter()
            .map(|arg| match arg {
                Entry::One(Arg { name: None, value }) => self.operand(*value),
                _ => Err(Unbuilt::at(origin)),
            })
            .collect::<Result<_, _>>()?;
        let tuple = Place::of(self.temp_local(Rvalue::Aggregate { kind: Aggregate::Tuple, parts }, origin));
        let ty = tuple.ty(self.env.cx, &self.locals);
        self.operator(Operation::Expr(call), Side::Expr(callee), vec![Side::Matched(tuple, ty)], origin)
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
