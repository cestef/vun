//! Values built from parts at compile time: struct literals (with field defaults), variants,
//! strings as byte arrays, and the parts read back: fields and items. A form typed by its
//! context, `.{ }` or `.name`, evaluates with the type expected of it, from its surroundings or
//! from typing.

use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, List, Name};
use overt_intern::Symbol;

use super::{Env, Unevaluated};
use crate::int::Integer;
use crate::member::{Found, Members};
use crate::ty::{Nominal, Ty, TyKind, Value};

impl Env<'_, '_> {
    /// The value of `expr` where a value of type `expected` is wanted, if known.
    ///
    /// # Errors
    /// As [`Env::eval`].
    pub fn eval_as(&self, expr: ExprId, expected: Option<Ty>) -> Result<Value, Unevaluated> {
        let expected = expected.or_else(|| self.type_of(expr));
        match (&self.parsed.hir.exprs[expr].kind, expected) {
            (ExprKind::DotName(Name::Plain(name)), Some(ty)) => self.member(Value::Type(ty), *name),
            (ExprKind::StructLit { ty: None, fields }, Some(ty)) => self.record(ty, fields),
            (ExprKind::Call { callee, args }, Some(ty))
                if let ExprKind::DotName(Name::Plain(name)) = self.parsed.hir.exprs[*callee].kind =>
            {
                self.variant(ty, name, args)
            }
            // Without a type, these forms have no value.
            (ExprKind::DotName(_) | ExprKind::StructLit { ty: None, .. }, None) => {
                Err(Unevaluated::Unsupported)
            }
            (ExprKind::Call { callee, .. }, None)
                if matches!(self.parsed.hir.exprs[*callee].kind, ExprKind::DotName(_)) =>
            {
                Err(Unevaluated::Unsupported)
            }
            _ => self.eval(expr),
        }
    }

    /// `T{ .name = value, ... }`: every field of `ty` in declaration order, from the literal or
    /// from its default.
    pub(super) fn record(&self, ty: Ty, fields: &List<Arg>) -> Result<Value, Unevaluated> {
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &*ty else {
            return Err(Unevaluated::Unsupported);
        };
        let mut values = Vec::new();
        for field in &members.fields {
            let written = fields.iter().find_map(|f| match f {
                Entry::One(Arg { name: Some(Name::Plain(name)), value }) if *name == field.name => {
                    Some(*value)
                }
                _ => None,
            });
            let expected = field.ty.as_ref().ok().copied();
            let value = match (written, &field.default) {
                (Some(value), _) => self.eval_as(value, expected)?,
                (None, Some(default)) => {
                    let defining =
                        Env::defining(self.cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
                    defining.eval_as(default.expr.id, expected)?
                }
                (None, None) => return Err(Unevaluated::Unsupported),
            };
            values.push((field.name, value));
        }
        Ok(Value::Struct { ty, fields: values })
    }

    /// `.name(args)` of the enum type `ty`: the variant with its payload.
    fn variant(&self, ty: Ty, name: Symbol, args: &List<Arg>) -> Result<Value, Unevaluated> {
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        let Found::Variant(variant) = members.find(name) else { return Err(Unevaluated::Absent) };
        let payload = args
            .iter()
            .enumerate()
            .map(|(i, arg)| match arg {
                Entry::One(Arg { name: None, value }) => {
                    let expected = variant.payload.get(i).and_then(|t| t.as_ref().ok().copied());
                    self.eval_as(*value, expected)
                }
                _ => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<_, _>>()?;
        Ok(Value::Variant { ty, name, payload })
    }

    /// A string literal at compile time: its bytes.
    pub(super) fn string(text: &str) -> Value {
        Value::Array(text.bytes().map(|b| Value::Int(Integer::from(i128::from(b)))).collect())
    }

    /// `object[index]`.
    pub(super) fn index(&self, object: ExprId, index: ExprId) -> Result<Value, Unevaluated> {
        self.eval(object)?.item(&self.eval(index)?)
    }
}
