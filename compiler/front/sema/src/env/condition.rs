//! Conditions: the `lang` type of `if` heads and of `and`, `or` and `!`, and the value on which
//! a condition holds, both found through the table `forms` (appendix B). Comparisons produce
//! conditions, so their values are built here.

use vun_desugar::{FORMS, Form};
use vun_grammar::TokenKind;
use vun_hir::tree::{BlockId, Cond, ExprId, ExprKind};

use super::{Env, Unevaluated};
use crate::member::Members;
use crate::ty::{Ty, Value};

impl Env<'_, '_> {
    /// The item the syntactic form `form` means.
    ///
    /// # Errors
    /// Returns why the item has no value here.
    pub fn form(&self, form: Form) -> Result<Value, Unevaluated> {
        let (_, item) = FORMS.iter().find(|(f, _)| *f == form).ok_or(Unevaluated::Absent)?;
        self.item(*item)
    }

    /// The type of conditions.
    ///
    /// # Errors
    /// Returns why it has no value here.
    pub fn condition(&self) -> Result<Ty, Unevaluated> {
        self.form(Form::Condition)?.as_type().ok_or(Unevaluated::Unsupported)
    }

    /// Whether the condition `value` holds.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for a value that is no condition.
    pub fn holds(&self, value: &Value) -> Result<bool, Unevaluated> {
        let truth = self.form(Form::Truth)?;
        match value {
            Value::Variant { ty, .. } if *ty == self.condition()? => Ok(*value == truth),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The condition that holds when `holds` is: the truth, or the condition type's other
    /// value.
    ///
    /// # Errors
    /// Returns why the condition type or its values are not known here.
    pub fn decide(&self, holds: bool) -> Result<Value, Unevaluated> {
        let truth = self.form(Form::Truth)?;
        if holds {
            return Ok(truth);
        }
        let Value::Variant { ty, name, .. } = &truth else { return Err(Unevaluated::Unsupported) };
        let members = self.cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let mut others = members.variants.iter().filter(|v| v.name != *name);
        match (others.next(), others.next()) {
            (Some(other), None) => Ok(Value::Variant { ty: *ty, name: other.name, payload: Vec::new() }),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `left op right` at compile time, comparisons included.
    ///
    /// # Errors
    /// As [`Value::binary`].
    pub fn binary(&self, op: TokenKind, left: &Value, right: &Value) -> Result<Value, Unevaluated> {
        match Value::compare(op, left, right) {
            Some(holds) => self.decide(holds),
            None => Value::binary(op, left, right),
        }
    }

    /// `op operand` at compile time, `!` on conditions included.
    ///
    /// # Errors
    /// As [`Value::unary`].
    pub fn unary(&self, op: TokenKind, operand: &Value) -> Result<Value, Unevaluated> {
        match op {
            TokenKind::Bang => self.decide(!self.holds(operand)?),
            _ => Value::unary(op, operand),
        }
    }

    /// The branch the `if` expression `expr` takes when its conditions are known here: `$if` as
    /// a value selects its branch at compile time (spec 03, section 1), `else if` included.
    ///
    /// # Errors
    /// Returns why a condition has no value here; a test condition (`if p = e`) is not selected.
    pub fn select(&self, mut expr: ExprId) -> Result<Selected, Unevaluated> {
        let hir = &self.parsed.hir;
        loop {
            let ExprKind::If { cond: Cond::Expr(cond), then, otherwise } = &hir.exprs[expr].kind else {
                return Err(Unevaluated::Unsupported);
            };
            if self.holds(&self.eval(*cond)?)? {
                return Ok(Selected::Block(*then));
            }
            match otherwise {
                None => return Ok(Selected::Unit),
                Some(e) if matches!(hir.exprs[*e].kind, ExprKind::If { .. }) => expr = *e,
                Some(e) => return Ok(Selected::Expr(*e)),
            }
        }
    }
}

/// The branch a `$if` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected {
    Block(BlockId),
    /// An `else` that is not a block, or not a block written as one.
    Expr(ExprId),
    /// No `else`, and no condition held: `()`.
    Unit,
}
