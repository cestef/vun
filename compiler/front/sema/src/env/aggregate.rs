//! Values built from parts at compile time: struct literals (with field defaults), variants,
//! strings as byte arrays, and the parts read back: fields and items. A form typed by its
//! context, `.{ }` or `.name`, evaluates with the type expected of it, from its surroundings or
//! from typing.

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, List, Name};
use vun_intern::Symbol;

use super::{Env, Unevaluated};
use crate::member::{Found, Members};
use crate::ops::Part;
use crate::ty::{Nominal, Ty, TyKind, Value};

impl Env<'_, '_> {
    /// The value of `expr` where a value of type `expected` is wanted, if known.
    ///
    /// # Errors
    /// As [`Env::eval`].
    pub fn eval_as(&self, expr: ExprId, expected: Option<Ty>) -> Result<Value, Unevaluated> {
        let expected = expected.or_else(|| self.type_of(expr));
        match (&self.parsed.hir.exprs[expr].kind, expected) {
            (ExprKind::DotName(Name::Plain(name)), Some(ty)) if Some(ty) != Value::Name(*name).ty() => {
                self.member(Value::Type(ty), *name)
            }
            (ExprKind::StructLit { ty: None, fields }, Some(ty)) => self.record(ty, fields),
            (ExprKind::Call { callee, args }, Some(ty))
                if let ExprKind::DotName(Name::Plain(name)) = self.parsed.hir.exprs[*callee].kind =>
            {
                self.constructed(ty, name, args)
            }
            // Without a type, `.name` is the name itself, and `.{ }` a record or a tuple of what it
            // holds.
            (ExprKind::DotName(Name::Plain(name)), _) => Ok(Value::Name(*name)),
            (ExprKind::StructLit { ty: None, fields }, None) => self.anonymous(fields),
            (ExprKind::DotName(_), None) => Err(Unevaluated::Unsupported),
            (ExprKind::Call { callee, .. }, None)
                if matches!(self.parsed.hir.exprs[*callee].kind, ExprKind::DotName(_)) =>
            {
                Err(Unevaluated::Unsupported)
            }
            _ => self.eval(expr),
        }
    }

    /// `.{ .a = x, .b = y }` or `.{ x, y }` with no type expected: a record of its fields, by
    /// the types of their values, or a tuple.
    fn anonymous(&self, fields: &List<Arg>) -> Result<Value, Unevaluated> {
        let mut named = Vec::new();
        let mut items = Vec::new();
        for field in fields {
            match field {
                Entry::One(Arg { name: Some(Name::Plain(name)), value }) => {
                    named.push((*name, self.eval(*value)?));
                }
                Entry::One(Arg { name: None, value }) => items.push(self.eval(*value)?),
                _ => return Err(Unevaluated::Unsupported),
            }
        }
        if named.is_empty() {
            return Ok(Value::Tuple(items));
        }
        if !items.is_empty() {
            return Err(Unevaluated::Unsupported);
        }
        named.sort_by_key(|(name, _)| name.as_str());
        let types = named.iter().map(|(n, v)| Some((*n, v.ty()?))).collect::<Option<Vec<_>>>();
        let ty = TyKind::Record(types.ok_or(Unevaluated::Unsupported)?).intern();
        Ok(Value::Struct { ty, fields: named })
    }

    /// `T{ .name = value, ... }`: every field of `ty` in declaration order, from the literal or
    /// from its default; for a record type, its fields.
    pub(super) fn record(&self, ty: Ty, fields: &List<Arg>) -> Result<Value, Unevaluated> {
        if let TyKind::Record(types) = &*ty {
            let values = types.iter().map(|(name, field)| {
                let written = fields.iter().find_map(|f| match f {
                    Entry::One(Arg { name: Some(Name::Plain(n)), value }) if n == name => Some(*value),
                    _ => None,
                });
                Ok((*name, self.eval_as(written.ok_or(Unevaluated::Unsupported)?, Some(*field))?))
            });
            return Ok(Value::Struct { ty, fields: values.collect::<Result<_, Unevaluated>>()? });
        }
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

    /// `.name(args)` where a `ty` is expected: its variant `name` with the payload, or a call of
    /// its constructor function `name`.
    fn constructed(&self, ty: Ty, name: Symbol, args: &List<Arg>) -> Result<Value, Unevaluated> {
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        let Found::Variant(variant) = members.find(name) else {
            return self.call_value(self.member(Value::Type(ty), name)?, Vec::new(), args);
        };
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

    /// `object[index]`.
    pub(super) fn index(&self, object: ExprId, index: ExprId) -> Result<Value, Unevaluated> {
        self.eval(object)?.part(&Part::Item(self.eval(index)?))
    }
}
