//! Function bodies and closures (spec 01, section 4). A closure is an anonymous struct of its
//! captures, called through `lang.ops.Call` (decision 91); its body sees the binders it captures,
//! so it is typed as part of the body it is written in, and its own typed body is that one.

use std::mem::{replace, take};

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::db::Parsed;
use vun_hir::tree::{Entry, ExprId, ExprKind, List, Param, PatKind};
use vun_query::Cx;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::item::ExprRef;
use vun_sema_ty::{Closure, Signature, TyKind, Value};

use super::{GaveUp, Known, Stage, Typer};
use crate::callable::Callable;
use crate::eval::Instantiate;

impl Typer<'_, '_, '_> {
    /// The function literal `expr` as a value: of its signature's type, or of its own when it has
    /// `$` parameters.
    pub(super) fn literal(&mut self, expr: ExprId) -> Option<Known> {
        let signature = self.env().literal_signature(expr, &[]).map(|s| s.ty());
        let ty = signature.or_else(|why| {
            let f = self.evaluating().eval(expr).ok().and_then(Callable::of);
            f.and_then(|f| f.generic(self.env())).ok_or(why)
        });
        self.known(expr, ty.map(|ty| Known { ty, stage: Stage::Compile }))
    }

    /// Types the function literal `literal`'s parameters and body, `bound` giving the value of
    /// each `$` parameter an instantiation binds, and gives its signature when every type is
    /// known. A `return` or a loop inside belongs to the function; a coroutine function's body
    /// yields what its frame yields and ends with what it returns.
    pub(super) fn function(&mut self, literal: ExprId, bound: &[Option<Value>]) -> Option<Signature> {
        let hir = self.hir();
        let ExprKind::Fn { params, returns, body } = &hir.exprs[literal].kind else { return None };
        let (returns, body) = (*returns, *body);
        let mut types = Some(Vec::new());
        for (index, param) in params.iter().enumerate() {
            let ty = match param {
                Entry::One(Param { pat: Some(pat), ty: Some(ty), .. }) => {
                    // A `$` parameter bound by the instantiation has the type of its value alone,
                    // where its annotation accepts that type: a compile-time `&[u8]` holds an
                    // array, whose borrow the annotation is.
                    let bound = bound.get(index).and_then(Option::as_ref).and_then(Value::ty);
                    let ty = match (bound, self.annotation(*ty)) {
                        (Some(bound), Ok(written)) if !written.accepts(&bound) => Ok(written),
                        (bound, written) => bound.map_or(written, Ok),
                    };
                    self.pattern(*pat, ty.clone(), None);
                    ty.ok()
                }
                _ => None,
            };
            let pat = param.written().and_then(|p| p.pat);
            // A pack gathers the arguments after the others: it comes last.
            if let Some(p) = pat.filter(|p| matches!(hir.pats[*p].kind, PatKind::Rest(_)))
                && index + 1 != params.len()
            {
                let msg = Msg::new().text("a pack parameter comes last");
                let note = "a pack gathers the arguments left after the other parameters take theirs";
                let d = Diagnostic::error(errors::TYPES_PACK_POSITION, msg)
                    .at(hir.pats[p].origin, None)
                    .note(note)
                    .help("move it after the other parameters");
                self.diag.error(d);
            }
            match (&mut types, ty.and_then(|ty| self.env().param_type(pat, ty).ok())) {
                (Some(types), Some(ty)) => types.push(ty),
                _ => types = None,
            }
        }
        let frame =
            hir.suspends(literal).then(|| self.env().literal_signature(literal, bound).ok()).flatten();
        // A coroutine's annotation names its interface, not the type its body ends with.
        let written = if frame.is_some() { None } else { returns };
        let (signature_result, result, yields) = if let Some(frame) = frame.map(|s| s.result) {
            let step = self.env().frame_step(frame);
            (Some(frame), step.map(|(_, r)| r).ok_or(GaveUp::Form), step.map(|(y, _)| y))
        } else {
            let result = match returns {
                Some(r) => self.annotation(r),
                None => Ok(TyKind::unit()),
            };
            (result.clone().ok(), result, None)
        };
        let outer = (
            self.result.replace(result.clone()),
            replace(&mut self.written, written),
            take(&mut self.loops),
            replace(&mut self.yields, yields),
            replace(&mut self.framing, yields.is_some().then_some(literal)),
        );
        let result = match hir.blocks[body].tail {
            Some(tail) => self.expecting(tail, result),
            None => result.ok(),
        };
        let found = self.block_value(body, result);
        if let (Some(found), Some(result)) = (found, result) {
            let at = hir.blocks[body].tail.map_or(hir.blocks[body].origin, |t| hir.exprs[t].origin);
            self.expect(at, found, result, written);
        }
        (self.result, self.written, self.loops, self.yields, self.framing) = outer;
        Some(Signature::new(types?, signature_result?))
    }

    /// `[captures] (params) -> R { body }`: each capture typed here, the body typed as part of
    /// this one, and the closure's type.
    pub(super) fn closure(&mut self, captures: &List<ExprId>, func: ExprId) -> Option<Known> {
        let hir = self.hir();
        let mut typed = Some(Vec::new());
        for capture in captures {
            let known = capture.written().and_then(|c| {
                let name = self.env().parsed.captured(*c).map(|(b, _)| hir.binders[b].name);
                self.typed(*c).zip(name)
            });
            match (&mut typed, known) {
                (Some(typed), Some((known, name))) => typed.push((name, known.ty)),
                _ => typed = None,
            }
        }
        let signature = self.function(func, &[])?;
        let known = Known { ty: signature.ty(), stage: self.stage };
        self.finding().types.insert(func, known);
        let closure = Closure { f: self.env().func(func), captures: typed?, signature };
        Some(Known { ty: TyKind::Closure(closure).intern(), stage: self.stage })
    }
}

/// The instantiation a closure's body is typed within.
pub(super) trait Enclosing: Sized {
    /// For a closure's function, the instantiation of the function it is written in, whose
    /// typed body holds its own: the one it was evaluated in when that runs the enclosing
    /// function, or else the enclosing function evaluated where the closure was, binding
    /// nothing. `None` for any other function.
    fn enclosing(&self, cx: &Cx<'_>) -> Option<Self>;
}

impl Enclosing for Instantiation {
    fn enclosing(&self, cx: &Cx<'_>) -> Option<Self> {
        let file = &self.f.literal.file;
        let parsed = cx.get::<Parsed>(file)?;
        let hir = &parsed.hir;
        let enclosing = hir.enclosing_function(hir.closure_of(self.f.literal.expr.id)?)?;
        match &self.f.within {
            Some(within) if within.f.literal.expr.id == enclosing => within.body(cx),
            _ => {
                let literal = ExprRef::new(file.clone(), hir, enclosing);
                Some(Self::new(Func { literal, ..self.f.clone() }, Vec::new()))
            }
        }
    }
}
