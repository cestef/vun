//! Calls at compile time: a function value, which the evaluator runs, or a primitive; `a.f(b)`
//! on a value `a` of type `T` is `T.f(a, b)` (spec 01, section 4).

use super::passing::Passed;
use super::{Env, Unevaluated};
use crate::eval::{Call, Instance};
use crate::member::{Found, Members};
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
        let mut receiver = None;
        let callee = match &self.parsed.hir.exprs[callee].kind {
            ExprKind::Member { object, name: Name::Plain(name) } => {
                let ty = self.type_of(*object);
                let object = self.eval(*object)?;
                // A member of a namespace or of a type is a declaration, never a method of the
                // namespace or of `type`.
                let declared = matches!(object, Value::Namespace(_) | Value::Type(_));
                let ty = if declared { None } else { object.ty().or(ty) };
                // A field holds a value, which the call calls with no receiver.
                let field = ty
                    .and_then(|t| self.cx.get::<Members>(&t))
                    .is_some_and(|m| matches!(m.find(*name), Found::Field(_)));
                match ty {
                    Some(_) if field => self.member(object, *name)?,
                    // Through borrows, as `.` goes: a compile-time borrow is its value. A type
                    // lacking the member, or not known to have it, passes the search on; a
                    // member that fails to evaluate fails the call.
                    Some(ty) => {
                        receiver = Some(object);
                        let mut types = std::iter::successors(Some(ty), |t| t.borrowed());
                        let found = types.find_map(|t| match self.member(Value::Type(t), *name) {
                            Err(Unevaluated::Absent | Unevaluated::Unsupported) => None,
                            found => Some(found),
                        });
                        found.ok_or(Unevaluated::Unsupported)??
                    }
                    None => self.member(object, *name)?,
                }
            }
            _ => self.eval(callee)?,
        };
        self.call_value(callee, receiver, args, inferred)
    }

    /// `callee(args)`, passed as [`Self::passing`] reads them: the receiver `receiver` of
    /// `a.f(b)` when there is one, and the `$` arguments left out, `inferred` by parameter.
    pub(super) fn call_value(
        &self,
        callee: Value,
        mut receiver: Option<Value>,
        args: &[Entry<Arg>],
        inferred: &[(usize, Value)],
    ) -> Result<Value, Unevaluated> {
        let passing = self.passing(Some(&callee), receiver.is_some(), args, inferred)?;
        let values = passing
            .into_iter()
            .map(|passed| match passed {
                Passed::Receiver => receiver.take().ok_or(Unevaluated::Unsupported),
                Passed::Inferred(value) => Ok(value),
                Passed::Code(expr) => Ok(Value::Code(self.code(expr))),
                Passed::Known { expr, ty } | Passed::Run { expr, ty } => self.eval_as(expr, ty),
            })
            .collect::<Result<_, _>>()?;
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
