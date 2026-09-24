//! Operators (spec 02, section 4; appendix B): `a op b` calls the function the table
//! `operators` names, of the operator's `lang.ops` interface, that the witness of `a`'s type
//! gives; `op a` likewise by the table `prefix_operators`. The operands are passed as the
//! function's parameters ask, reached through their borrows as `.` reaches a value. Integers are
//! no exception: `lang.int.ops` holds their impls. A compound assignment, a value pattern and a
//! range pattern are operators too.

use vun_desugar::{OPERATORS, PREFIX_OPERATORS};
use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, PatId};
use vun_intern::Symbol;
use vun_items::Item;

use super::apply::{Argument, Callee, Passing};
use super::{Known, Stage, Typer};
use crate::callable::Callable;
use crate::eval::{Call, Instance};
use crate::member::{MemberType, MemberValue};
use crate::ty::{Ty, TyKind, Value};

/// Where an operator is written, by what it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    /// `a op b` or `op a`.
    Expr(ExprId),
    /// `x op= y`, by its place `x`.
    Assign(ExprId),
    /// A value pattern: the value matched `==` the pattern's.
    Test(PatId),
    /// A range pattern's lower end: `lo <=` the value matched.
    Lower(PatId),
    /// A range pattern's upper end: the value matched `<` or `<=` `hi`.
    Upper(PatId),
}

/// The function an operator calls: the struct declaring it and its name, and the `$` arguments
/// inferred for the call, by parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Operator {
    pub method: (Ty, Symbol),
    pub inferred: Vec<(usize, Value)>,
}

impl Typer<'_, '_, '_> {
    /// `left op right`, written at `expr`.
    pub(super) fn binary(
        &mut self,
        expr: ExprId,
        op: TokenKind,
        left: ExprId,
        right: ExprId,
    ) -> Option<Known> {
        let &(_, interface, function) = OPERATORS.iter().find(|(token, ..)| *token == op)?;
        self.operator(Operation::Expr(expr), expr, (interface, function), left, &[Argument::Expr(right)])
    }

    /// `op operand`, written at `expr`, for an operator of the table `prefix_operators`.
    pub(super) fn prefix(&mut self, expr: ExprId, op: TokenKind, operand: ExprId) -> Option<Known> {
        let &(_, interface, function) = PREFIX_OPERATORS.iter().find(|(token, ..)| *token == op)?;
        self.operator(Operation::Expr(expr), expr, (interface, function), operand, &[])
    }

    /// `x op= y`, whose operation belongs to the place `x` (spec 04, rule 9).
    pub(super) fn compound(&mut self, place: ExprId, op: TokenKind, value: ExprId) -> Option<Known> {
        let &(_, interface, function) = OPERATORS.iter().find(|(token, ..)| *token == op)?;
        self.operator(Operation::Assign(place), value, (interface, function), place, &[Argument::Expr(value)])
    }

    /// The test `at` of a pattern, `receiver op` the value matched, of type `matched`: a value
    /// pattern `v` is `v == x`, a range `lo..hi` is `lo <= x` and `hi > x`.
    pub(super) fn tested(
        &mut self,
        at: Operation,
        op: TokenKind,
        receiver: ExprId,
        matched: Ty,
    ) -> Option<Known> {
        let &(_, interface, function) = OPERATORS.iter().find(|(token, ..)| *token == op)?;
        self.operator(at, receiver, (interface, function), receiver, &[Argument::Matched(matched)])
    }

    /// The call of `function` of `interface` for the type of `receiver`, with the operands
    /// `args` after it. Its value is known when its operands are.
    fn operator(
        &mut self,
        at: Operation,
        site: ExprId,
        (interface, function): (Item, &str),
        receiver: ExprId,
        args: &[Argument],
    ) -> Option<Known> {
        let left = self.typed(receiver)?;
        let ty = left.ty.through_borrows();
        let function = Symbol::intern(function);
        let owner = if matches!(&*ty, TyKind::Param(_) | TyKind::Frame(_)) {
            // In a generic body, what the interface requires of its parameter.
            let Ok(Value::Interface(f)) = self.env().item(interface) else { return None };
            self.env().cx.get::<Call>(&Instance { f, args: vec![Value::Type(ty)] }).ok()?.as_type()?
        } else {
            let Some(method) = self.env().implemented_by(ty, interface, function) else {
                self.no_impl(site, interface, ty);
                return None;
            };
            self.check_visible(site, function, &method.visible);
            method.owner
        };
        // The witness's function, or a primitive it names: `add := lang.int.add`.
        let f = self.env().cx.get::<MemberValue>(&(owner, function)).ok().and_then(Callable::of);
        let signature =
            self.env().cx.get::<MemberType>(&(owner, function)).ok().and_then(|k| k.ty.callable());
        let callee = Callee { f, signature, receiver: Some(receiver), passing: Passing::Asked };
        let applied = self.apply(site, &callee, args)?;
        let written = args.iter().filter_map(|a| match a {
            Argument::Expr(e) => self.types.get(e),
            Argument::Matched(_) => None,
        });
        let stage = written.fold(left.stage, |s, k| s.max(k.stage));
        // A run-time intermediate needs a width (spec 04, rule 5).
        if let Some(range) = applied.known.ty.interval()
            && stage == Stage::Run
            && range.storage().is_none()
            && range.bits().is_some()
        {
            self.width_cap(site, &range);
            return None;
        }
        let inferred = applied.inferred.unwrap_or_default();
        self.operators.insert(at, Operator { method: (owner, function), inferred });
        Some(Known { stage, ..applied.known })
    }
}
