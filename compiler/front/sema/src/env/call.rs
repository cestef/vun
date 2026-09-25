//! Calls at compile time: a function value, which the evaluator runs, or a primitive; `a.f(b)`
//! on a value `a` of type `T` is `T.f(a, b)` (spec 01, section 4).

use super::{Env, Unevaluated};
use crate::eval::{Call, Instance};
use crate::ty::Value;
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Name};

impl Env<'_, '_> {
    /// The call `call` of a function value, which the evaluator runs, or of a primitive. `a.f(b)`
    /// on a value `a` of type `T` is `T.f(a, b)` (spec 01, section 4).
    pub(super) fn call(
        &self,
        call: ExprId,
        callee: ExprId,
        args: &[Entry<Arg>],
    ) -> Result<Value, Unevaluated> {
        let origin = self.parsed.hir.exprs[call].origin;
        self.called(callee, args, self.inferred(call)).map_err(|u| u.through(origin))
    }

    /// `callee(args)`, the `$` arguments left out `inferred`.
    fn called(
        &self,
        callee: ExprId,
        args: &[Entry<Arg>],
        inferred: &[(usize, Value)],
    ) -> Result<Value, Unevaluated> {
        let mut values = Vec::new();
        let callee = match &self.parsed.hir.exprs[callee].kind {
            ExprKind::Member { object, name: Name::Plain(name) } => {
                let ty = self.type_of(*object);
                let object = self.eval(*object)?;
                // A member of a namespace or of a type is a declaration, never a method of the
                // namespace or of `type`.
                let declared = matches!(object, Value::Namespace(_) | Value::Type(_));
                let ty = if declared { None } else { object.ty().or(ty) };
                match ty {
                    // Through borrows, as `.` goes: a compile-time borrow is its value.
                    Some(ty) => {
                        values.push(object);
                        let mut types = std::iter::successors(Some(ty), |t| t.borrowed());
                        types
                            .find_map(|t| self.member(Value::Type(t), *name).ok())
                            .ok_or(Unevaluated::Unsupported)?
                    }
                    None => self.member(object, *name)?,
                }
            }
            _ => self.eval(callee)?,
        };
        self.call_value(callee, values, args, inferred)
    }

    /// `callee(values..., args)`: the arguments evaluated after `values`, which a receiver fills,
    /// and the `$` arguments left out, `inferred` by parameter, in their places.
    pub(super) fn call_value(
        &self,
        callee: Value,
        mut values: Vec<Value>,
        args: &[Entry<Arg>],
        inferred: &[(usize, Value)],
    ) -> Result<Value, Unevaluated> {
        let fill = |values: &mut Vec<Value>| {
            while let Some((_, value)) = inferred.iter().find(|(slot, _)| *slot == values.len()) {
                values.push(value.clone());
            }
        };
        // A `Code(T)` parameter receives its argument as code; an argument typed by its context,
        // such as `.{ ... }`, is evaluated as its parameter's type where that type is known.
        let (quotes, params) = match &callee {
            Value::Fn(f) => {
                let signature = Env::inside(self.cx, f, [])
                    .and_then(|env| env.literal_signature(f.literal.expr.id, &[]).ok());
                (f.quotes(self.cx).unwrap_or_default(), signature.map(|s| s.params).unwrap_or_default())
            }
            _ => (Vec::new(), Vec::new()),
        };
        for arg in args {
            fill(&mut values);
            match arg {
                Entry::One(Arg { name: None, value }) if quotes.get(values.len()) == Some(&true) => {
                    values.push(Value::Code(self.code(*value)));
                }
                Entry::One(Arg { name: None, value }) => {
                    values.push(self.eval_as(*value, params.get(values.len()).copied())?);
                }
                _ => return Err(Unevaluated::Unsupported),
            }
        }
        fill(&mut values);
        self.apply(callee, values)
    }

    /// `callee(args)` at compile time: a function value, which the evaluator runs, or a
    /// primitive.
    ///
    /// # Errors
    /// Returns why the call has no value.
    pub fn apply(&self, callee: Value, args: Vec<Value>) -> Result<Value, Unevaluated> {
        match callee {
            // An interface applied to a type is its requirement struct (spec 02, section 3).
            Value::Fn(f) | Value::Interface(f) => self.cx.get::<Call>(&Instance { f, args }),
            Value::Closure { f, captures } => {
                self.cx.get::<Call>(&Instance { f, args: captures.into_iter().chain(args).collect() })
            }
            Value::Primitive(p) => p.apply(self, &args),
            Value::Constructor { ty, name } => Ok(Value::Variant { ty, name, payload: args }),
            // A link symbol runs at run time only.
            Value::Linked(_) => Err(Unevaluated::Runtime),
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
