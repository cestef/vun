//! Operators at compile time (spec 02, section 4): `a op b` calls the function of the
//! operator's `lang.ops` interface for the type of `a`, as a run-time operator does, with the
//! `$` arguments it leaves out inferred from the operands' types.

use vun_desugar::{OPERATORS, PREFIX_OPERATORS};
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_items::Item;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use super::Env;
use crate::callable::Callable;
use crate::member::MemberValue;
use crate::typeck::omitted::{Plan, Unifier};

/// An operand at compile time: its value, of the type typing found for it where it did, which a
/// string literal's value does not tell (`"ab"` is a `&static [2]u8`, not of its bytes alone).
pub struct Operand {
    pub value: Value,
    pub ty: Option<Ty>,
}

impl Operand {
    /// The type passed: the one typing found, reached through its borrows, or else the value's.
    fn ty(&self) -> Option<Ty> {
        self.ty.map(|t| t.through_borrows()).or_else(|| self.value.ty())
    }
}

impl Env<'_, '_> {
    /// `left op right` at compile time.
    ///
    /// # Errors
    /// Returns why it has no value here: an operator without an interface, operands whose type
    /// has no impl of it, or what the function called returns.
    pub fn binary(&self, op: TokenKind, left: Operand, right: Operand) -> Result<Value, Unevaluated> {
        let row = OPERATORS.iter().find(|row| row.token == op).ok_or(Unevaluated::Unsupported)?;
        self.operate(row.function, vec![left, right])
    }

    /// `op operand` at compile time, for an operator of the table `prefix_operators`.
    ///
    /// # Errors
    /// As [`Self::binary`].
    pub fn prefix(&self, op: TokenKind, operand: Operand) -> Result<Value, Unevaluated> {
        let row = PREFIX_OPERATORS.iter().find(|row| row.token == op).ok_or(Unevaluated::Unsupported)?;
        self.operate(row.function, vec![operand])
    }

    /// The call of `function`, a requirement of an interface, for the type of the first of
    /// `operands`, the others after it, each passed as its parameter asks.
    fn operate(&self, function: Item, operands: Vec<Operand>) -> Result<Value, Unevaluated> {
        let mut ty = operands.first().and_then(Operand::ty).ok_or(Unevaluated::Unsupported)?;
        // Operands typing found no types for hold what their values show: the left one's type is
        // widened to hold the others, as an array's items are: `"reset_" ++ name` operates on
        // bytes, not on the letters of `reset_`.
        if operands.iter().all(|o| o.ty.is_none()) {
            for other in operands.iter().skip(1).filter_map(Operand::ty) {
                ty = Self::widened(ty, other);
            }
        }
        let (interface, name) = (function.parent().ok_or(Unevaluated::Absent)?, function.symbol());
        let method = self.implemented_by(ty, interface, name).ok_or(Unevaluated::Unsupported)?;
        let f = Callable::of(self.cx.get::<MemberValue>(&(method.owner, name))?)
            .ok_or(Unevaluated::Unsupported)?;
        let plan = Plan::of(self, &f, operands.len()).ok_or(Unevaluated::Unsupported)?;
        let modes = f.modes(self).unwrap_or_default();
        // A compile-time borrow is its value, so the type passed is the value's, borrowed in the
        // mode its parameter is written with.
        let types: Vec<Option<Ty>> = operands
            .iter()
            .zip(&plan.slots)
            .map(|(o, &slot)| match modes.get(slot).copied().flatten() {
                Some(mode) => TyKind::borrowed_as(mode, o.ty()?),
                None => o.ty(),
            })
            .collect();
        let inferred = Unifier::infer(self, &f, &plan, &types).ok_or(Unevaluated::Unsupported)?;
        let mut args: Vec<Option<Value>> = vec![None; plan.slots.len() + plan.omitted.len()];
        for (&slot, operand) in plan.slots.iter().zip(operands) {
            args[slot] = Some(operand.value);
        }
        for (&slot, value) in plan.omitted.iter().zip(inferred) {
            args[slot] = value;
        }
        let args = args.into_iter().collect::<Option<Vec<_>>>().ok_or(Unevaluated::Unsupported)?;
        self.apply(f.value(), args)
    }

    /// `expr` as an operand: its value, of the type typing found.
    pub(super) fn operand(&self, expr: ExprId) -> Result<Operand, Unevaluated> {
        Ok(Operand { value: self.eval(expr)?, ty: self.type_of(expr) })
    }

    /// The value of `left op right`. A form typed by its context on the right has the type of
    /// the left operand, where typing did not give it one: `a == .b`.
    pub(super) fn infix(&self, op: TokenKind, left: ExprId, right: ExprId) -> Result<Value, Unevaluated> {
        let left = self.operand(left)?;
        let ty = self.type_of(right);
        let expected = ty.or(left.ty).or_else(|| left.value.ty());
        let right = Operand { value: self.eval_as(right, expected)?, ty };
        self.binary(op, left, right)
    }

    /// `ty` widened to hold the values of `other`: their join, or for a sequence, the same
    /// sequence of the items of both; `ty` itself when they have nothing in common.
    fn widened(ty: Ty, other: Ty) -> Ty {
        let items = || {
            let (len, item) = ty.as_array()?;
            Some(TyKind::array(len.clone(), item.join(&*other.items()?)?))
        };
        ty.join(&other).or_else(items).unwrap_or(ty)
    }
}
