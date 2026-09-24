//! Conditions: the `lang` type of `if` heads and of `and`, `or` and `!`, and the value on which
//! a condition holds, both found through the table `forms` (appendix B). Comparisons produce
//! conditions, so their values are built here.

use overt_grammar::{FORMS, Form, TokenKind};

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
        self.item(item)
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
}
