//! Operators at compile time (spec 02, section 4): `a op b` calls the function of the
//! operator's `lang.ops` interface for the type of `a`, as a run-time operator does, with the
//! `$` arguments it leaves out inferred from the operands' types.

use vun_desugar::{OPERATORS, PREFIX_OPERATORS};
use vun_grammar::TokenKind;
use vun_intern::Symbol;
use vun_items::Item;

use super::{Env, Unevaluated};
use crate::callable::Callable;
use crate::member::MemberValue;
use crate::ty::{Ty, TyKind, Value};
use crate::typeck::omitted::{Plan, Unifier};

impl Env<'_, '_> {
    /// `left op right` at compile time.
    ///
    /// # Errors
    /// Returns why it has no value here: an operator without an interface, operands whose type
    /// has no impl of it, or what the function called returns.
    pub fn binary(&self, op: TokenKind, left: Value, right: Value) -> Result<Value, Unevaluated> {
        let &(_, interface, function) =
            OPERATORS.iter().find(|(token, ..)| *token == op).ok_or(Unevaluated::Unsupported)?;
        self.operate(interface, function, vec![left, right])
    }

    /// `op operand` at compile time, for an operator of the table `prefix_operators`.
    ///
    /// # Errors
    /// As [`Self::binary`].
    pub fn prefix(&self, op: TokenKind, operand: Value) -> Result<Value, Unevaluated> {
        let &(_, interface, function) =
            PREFIX_OPERATORS.iter().find(|(token, ..)| *token == op).ok_or(Unevaluated::Unsupported)?;
        self.operate(interface, function, vec![operand])
    }

    /// The call of `function` of `interface` for the type of the first of `operands`, the
    /// others after it, each passed as its parameter asks.
    fn operate(&self, interface: Item, function: &str, operands: Vec<Value>) -> Result<Value, Unevaluated> {
        let ty = operands.first().and_then(Value::ty).ok_or(Unevaluated::Unsupported)?;
        let name = Symbol::intern(function);
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
            .map(|(v, &slot)| match modes.get(slot).copied().flatten() {
                Some(mode) => TyKind::borrowed_as(mode, v.ty()?),
                None => v.ty(),
            })
            .collect();
        let inferred = Unifier::infer(self, &f, &plan, &types).ok_or(Unevaluated::Unsupported)?;
        let mut args: Vec<Option<Value>> = vec![None; plan.slots.len() + plan.omitted.len()];
        for (&slot, value) in plan.slots.iter().zip(operands) {
            args[slot] = Some(value);
        }
        for (&slot, value) in plan.omitted.iter().zip(inferred) {
            args[slot] = value;
        }
        let args = args.into_iter().collect::<Option<Vec<_>>>().ok_or(Unevaluated::Unsupported)?;
        self.apply(f.value(), args)
    }
}
