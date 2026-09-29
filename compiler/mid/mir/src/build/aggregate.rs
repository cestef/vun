//! Values built from parts: struct literals (fields in declaration order, a missing one from its
//! default), variants, tuples, arrays and repeated items. A form typed by its context takes its type from the
//! typed body.

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, List, Name};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::env::Env;
use vun_sema::member::{Found, Members};
use vun_sema_ty::{Nominal, Ty, TyKind};

use super::{Builder, Unbuilt};
use crate::{Aggregate, Operand, Rvalue};

impl Builder<'_, '_, '_> {
    /// The MIR of `expr` when it builds a value from parts; `None` for any other form.
    pub(super) fn aggregate(&mut self, expr: ExprId) -> Option<Result<Operand, Unbuilt>> {
        let hir = &self.env.parsed.hir;
        let origin = hir.exprs[expr].origin;
        let unbuilt = move || Unbuilt::at(origin);
        Some(match &hir.exprs[expr].kind {
            ExprKind::Tuple(items) => self.parts(items, Aggregate::Tuple, origin),
            ExprKind::Array(items) => self.parts(items, Aggregate::Array, origin),
            ExprKind::Repeat { value, count } => {
                let (value, count) = (*value, *count);
                self.env().eval(count).map_err(|why| Unbuilt::unevaluated(origin, why)).and_then(|count| {
                    let operand = self.operand(value)?;
                    Ok(self.temp(Rvalue::Repeat { operand, count }, origin))
                })
            }
            ExprKind::StructLit { ty, fields } => {
                let ty = match ty {
                    Some(ty) => self.env().ty(*ty).ok(),
                    None => self.type_of(expr),
                };
                ty.ok_or_else(unbuilt).and_then(|ty| self.record(ty, fields, origin))
            }
            ExprKind::Call { callee, args } => {
                let ExprKind::DotName(Name::Plain(name)) = hir.exprs[*callee].kind else { return None };
                let ty = self.type_of(expr).ok_or_else(unbuilt);
                ty.and_then(|ty| self.variant(ty, name, args, origin))
            }
            ExprKind::DotName(Name::Plain(_)) => {
                let ty = self.type_of(expr).ok_or_else(unbuilt);
                ty.and_then(|ty| {
                    self.env().eval_as(expr, Some(ty)).map(Operand::Const).map_err(|_| unbuilt())
                })
            }
            _ => return None,
        })
    }

    /// The type the typed body found for `expr`.
    pub(super) fn type_of(&self, expr: ExprId) -> Option<Ty> {
        let typed = self.typed()?;
        typed.types.get(&expr).map(|k| k.ty)
    }

    /// A tuple or an array of `items`, in order.
    fn parts(&mut self, items: &List<ExprId>, kind: Aggregate, origin: OriginId) -> Result<Operand, Unbuilt> {
        let parts = items
            .iter()
            .map(|item| match item {
                Entry::One(item) => self.operand(*item),
                Entry::Staged(_) => Err(Unbuilt::at(origin)),
            })
            .collect::<Result<_, _>>()?;
        Ok(self.temp(Rvalue::Aggregate { kind, parts }, origin))
    }

    /// A value of the struct type `ty`: the fields written, in the order written, then the whole
    /// in declaration order, a missing field from its default.
    fn record(&mut self, ty: Ty, fields: &List<Arg>, origin: OriginId) -> Result<Operand, Unbuilt> {
        let unbuilt = move || Unbuilt::at(origin);
        // A structural literal: a tuple of its items, or a record of its fields by name.
        if let TyKind::Tuple(_) | TyKind::Record(_) = &*ty {
            let mut written = Vec::new();
            for field in fields {
                let Entry::One(Arg { name, value }) = field else { return Err(unbuilt()) };
                written.push((*name, self.operand(*value)?));
            }
            let kind = match &*ty {
                TyKind::Record(types) => {
                    let names: Vec<Symbol> = types.iter().map(|(n, _)| *n).collect();
                    written.sort_by_key(|(name, _)| match name {
                        Some(Name::Plain(n)) => names.iter().position(|m| m == n),
                        _ => None,
                    });
                    Aggregate::Struct { ty, fields: names }
                }
                _ => Aggregate::Tuple,
            };
            let parts = written.into_iter().map(|(_, part)| part).collect();
            return Ok(self.temp(Rvalue::Aggregate { kind, parts }, origin));
        }
        let members = self.env.cx.get::<Members>(&ty).ok_or_else(unbuilt)?;
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &*ty else { return Err(unbuilt()) };
        let mut written = Vec::new();
        for field in fields {
            let Entry::One(Arg { name: Some(Name::Plain(name)), value }) = field else {
                return Err(unbuilt());
            };
            written.push((*name, self.operand(*value)?));
        }
        let mut parts = Vec::new();
        for field in &members.fields {
            let part = if let Some(i) = written.iter().position(|(name, _)| *name == field.name) {
                written.swap_remove(i).1
            } else {
                let default = field.default.as_ref().ok_or_else(unbuilt)?;
                let defining = Env::defining(self.env.cx, literal, instance.as_ref()).ok_or_else(unbuilt)?;
                let expected = field.ty.as_ref().ok().copied();
                Operand::Const(defining.eval_as(default.expr.id, expected).map_err(|_| unbuilt())?)
            };
            parts.push(part);
        }
        let names = members.fields.iter().map(|f| f.name).collect();
        Ok(self.temp(Rvalue::Aggregate { kind: Aggregate::Struct { ty, fields: names }, parts }, origin))
    }

    /// `.name(args)` of the enum type `ty`.
    fn variant(
        &mut self,
        ty: Ty,
        name: Symbol,
        args: &List<Arg>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = move || Unbuilt::at(origin);
        let members = self.env.cx.get::<Members>(&ty).ok_or_else(unbuilt)?;
        let Found::Variant(_) = members.find(name) else { return Err(unbuilt()) };
        let parts = args
            .iter()
            .map(|arg| match arg {
                Entry::One(Arg { name: None, value }) => self.operand(*value),
                _ => Err(unbuilt()),
            })
            .collect::<Result<_, _>>()?;
        Ok(self.temp(Rvalue::Aggregate { kind: Aggregate::Variant { ty, name }, parts }, origin))
    }
}
