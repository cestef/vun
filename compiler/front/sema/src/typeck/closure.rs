//! Function bodies and closures (spec 01, section 4). A closure is an anonymous struct of its
//! captures, called through `lang.ops.Call` (decision 91); its body sees the binders it captures,
//! so it is typed as part of the body it is written in, and its own typed body is that one.

use std::mem::{replace, take};

use overt_hir::db::Parsed;
use overt_hir::resolve::Res;
use overt_hir::tree::{BlockId, Entry, ExprId, ExprKind, List, Param, PatKind};
use overt_intern::Symbol;
use overt_query::Cx;

use super::{Known, Typer};
use crate::eval::Instantiation;
use crate::item::ExprRef;
use crate::ty::{Closure, Signature, TyKind, Value};

impl Typer<'_, '_, '_> {
    /// Types a function literal's parameters and body, `bound` giving the value of each `$`
    /// parameter an instantiation binds, and gives its signature when every type is known. A
    /// `return` or a loop inside belongs to the function.
    pub(super) fn function(
        &mut self,
        params: &List<Param>,
        returns: Option<ExprId>,
        body: BlockId,
        bound: &[Option<Value>],
    ) -> Option<Signature> {
        let hir = self.hir();
        let mut types = Some(Vec::new());
        for (index, param) in params.iter().enumerate() {
            let ty = match param {
                Entry::One(Param { pat: Some(pat), ty: Some(ty), .. }) => {
                    // A `$` parameter bound by the instantiation has the type of its value alone.
                    let bound = bound.get(index).and_then(Option::as_ref).and_then(Value::ty);
                    let ty = bound.or_else(|| self.ty(*ty));
                    if let (PatKind::Bind(binder), Some(ty)) = (&hir.pats[*pat].kind, ty) {
                        self.bind(*binder, ty);
                    }
                    ty
                }
                _ => None,
            };
            match (&mut types, ty) {
                (Some(types), Some(ty)) => types.push(ty),
                _ => types = None,
            }
        }
        let result = match returns {
            Some(r) => self.ty(r),
            None => Some(TyKind::unit()),
        };
        let outer =
            (replace(&mut self.result, result), replace(&mut self.written, returns), take(&mut self.loops));
        let found = self.block_value(body, result);
        if let (Some(found), Some(result)) = (found, result) {
            let at = hir.blocks[body].tail.map_or(hir.blocks[body].origin, |t| hir.exprs[t].origin);
            self.expect(at, found, result, returns);
        }
        (self.result, self.written, self.loops) = outer;
        Some(Signature { params: types?, result: result? })
    }

    /// `[captures] (params) -> R { body }`: each capture typed here, the body typed as part of
    /// this one, and the closure's type.
    pub(super) fn closure(&mut self, expr: ExprId, captures: &List<ExprId>, func: ExprId) -> Option<Known> {
        let hir = self.hir();
        let mut typed = Some(Vec::new());
        for capture in captures {
            let known = match capture {
                Entry::One(c) => self.typed(*c).zip(self.captured(*c)),
                Entry::Staged(_) => None,
            };
            match (&mut typed, known) {
                (Some(typed), Some((known, name))) => typed.push((name, known.ty)),
                _ => typed = None,
            }
        }
        let ExprKind::Fn { params, returns, body } = &hir.exprs[func].kind else { return None };
        let signature = self.function(params, *returns, *body, &[])?;
        self.types.insert(func, Known { ty: signature.ty(), stage: self.stage });
        let closure = Closure {
            literal: ExprRef::new(self.env().file.clone(), hir, expr),
            instance: self.env().instance.clone(),
            captures: typed?,
            signature,
        };
        Some(Known { ty: TyKind::Closure(closure).intern(), stage: self.stage })
    }

    /// The name of the binder the capture `capture` (`x`, `&mut x`) captures.
    fn captured(&self, capture: ExprId) -> Option<Symbol> {
        let hir = self.hir();
        match &hir.exprs[capture].kind {
            ExprKind::Prefix { operand, .. } => self.captured(*operand),
            ExprKind::Name(_) => match self.env().parsed.resolution.names[capture] {
                Some(Res::Binder(b)) => Some(hir.binders[b].name),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Instantiation {
    /// For a closure's function, the instantiation of the function it is written in, whose
    /// typed body holds its own: the call it was evaluated in when that runs the enclosing
    /// function, or the plain one. `None` for any other function.
    pub(super) fn enclosing(&self, cx: &Cx<'_>) -> Option<Self> {
        let file = &self.f.literal.file;
        let parsed = cx.get::<Parsed>(file)?;
        let hir = &parsed.hir;
        let enclosing = hir.enclosing_function(hir.closure_of(self.f.literal.expr.id)?)?;
        match &self.f.within {
            Some(call) if call.f.literal.expr.id == enclosing => Self::of(cx, call),
            _ => Some(Self::plain(ExprRef::new(file.clone(), hir, enclosing))),
        }
    }
}
