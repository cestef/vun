//! Values built from parts at compile time: struct literals (with field defaults), variants,
//! strings as byte arrays, and the parts read back: fields and items. A form typed by its
//! context, `.{ }` or `.name`, evaluates with the type expected of it, from its surroundings or
//! from typing.

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, List, Lit, Name};
use vun_intern::Symbol;

use super::operator::Operand;
use super::{Env, Unevaluated};
use crate::float::{Binary, Format};
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
            // Where a float type is expected, a float literal has it, and so do the operands of
            // the operators giving it (spec 04, section 2).
            (ExprKind::Lit(Lit::Float(text)), Some(ty)) if let Some(format) = Format::of(&ty) => {
                Binary::parse(text, format).map(Value::Float).ok_or(Unevaluated::Unsupported)
            }
            (ExprKind::Prefix { op, operand, .. }, Some(ty)) if Format::of(&ty).is_some() => {
                self.prefix(*op, self.float_operand(*operand, ty)?)
            }
            (ExprKind::Infix { op, left, right: Some(right) }, Some(ty)) if Format::of(&ty).is_some() => {
                self.binary(*op, self.float_operand(*left, ty)?, self.float_operand(*right, ty)?)
            }
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

    /// `expr` as an operand of the float type `ty`.
    fn float_operand(&self, expr: ExprId, ty: Ty) -> Result<Operand, Unevaluated> {
        Ok(Operand { value: self.eval_as(expr, Some(ty))?, ty: Some(ty) })
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
    /// from its default; for a record type, its fields; for a tuple type, its positions.
    pub(super) fn record(&self, ty: Ty, fields: &List<Arg>) -> Result<Value, Unevaluated> {
        // Fields a `$for` or `$if` makes are known once staged, which evaluation does not do.
        if fields.iter().any(|f| matches!(f, Entry::Staged(_))) {
            return Err(Unevaluated::Unsupported);
        }
        // `.{ x, y }` where a tuple is expected: its items, each as its position's type.
        if let TyKind::Tuple(types) = &*ty {
            if fields.len() != types.len() {
                return Err(Unevaluated::Unsupported);
            }
            let items = fields.iter().zip(types.iter()).map(|(field, &item)| match field {
                Entry::One(Arg { name: None, value }) => self.eval_as(*value, Some(item)),
                _ => Err(Unevaluated::Unsupported),
            });
            return items.collect::<Result<_, _>>().map(Value::Tuple);
        }
        // `.{ x, y }` where an array or slice is expected: an array of its items.
        if let Some(item) = ty.items() {
            let items = fields.iter().map(|field| match field {
                Entry::One(Arg { name: None, value }) => self.eval_as(*value, Some(item)),
                _ => Err(Unevaluated::Unsupported),
            });
            return items.collect::<Result<_, _>>().map(Value::Array);
        }
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
            return self.call_value(self.member(Value::Type(ty), name)?, None, args, &[], None);
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
