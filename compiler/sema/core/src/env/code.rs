//! Code values at compile time (spec 03, section 4): a quote or a `Code(T)` argument is code
//! written here, and splicing it with `$` evaluates it where it was written.

use std::sync::Arc;

use vun_hir::db::{FileRef, FileText};
use vun_hir::tree::{ExprId, ExprKind};
use vun_origin::{OriginId, OriginTable, Via};
use vun_query::Cx;
use vun_sema_int::{Extended, Integer, Interval};
use vun_sema_ty::Value;
use vun_sema_ty::call::{Code, Func, Instantiation, Staging};
use vun_sema_ty::item::{ExprRef, Node};
use vun_sema_ty::trap::Unevaluated;
use vun_span::Span;
use vun_syntax::lexer;

use super::Env;
use crate::callable::Parameters;
use crate::eval::Instantiate;
use crate::typeck::Typed;

impl<'a, 'db> Env<'a, 'db> {
    /// Where the bytes `bytes` of the string literal the code `code` writes are written: a part
    /// of the code's origin (spec 10, section 3). The code's own origin when it writes no string
    /// literal, or when the bytes are empty or past it.
    #[must_use]
    pub fn origin_within(&self, code: &Code, bytes: Option<&Interval>) -> OriginId {
        let origin = code.expr.expr.origin;
        bytes.and_then(|b| self.literal_part(origin, &code.expr.file, b)).unwrap_or(origin)
    }

    fn literal_part(&self, origin: OriginId, file: &FileRef, bytes: &Interval) -> Option<OriginId> {
        let origins = OriginTable::global();
        let span = origins.source_span(origin);
        let text = self.cx.get::<FileText>(file)?;
        let located = lexer::located_string(span.range.of(&text))?;
        let at = |e: &Extended| {
            let i = e.finite().and_then(Integer::to_i128).and_then(|i| usize::try_from(i).ok())?;
            located.get(i)
        };
        let (first, last) = (at(bytes.lo())?, at(bytes.hi())?);
        let lo = span.range.lo + u32::try_from(first.1.start).ok()?;
        let hi = span.range.lo + u32::try_from(last.1.end).ok()?;
        Some(origins.derive(origin, Span::new(span.file, lo, hi), Via::Part))
    }

    /// The code `expr`, written here: it keeps the call evaluated in and every binder bound, so
    /// its names mean what they mean here.
    #[must_use]
    pub fn code(&self, expr: ExprId) -> Code {
        let hir = &self.parsed.hir;
        let mut bound: Vec<_> = self
            .bindings
            .iter()
            .map(|(&b, value)| Staging {
                binder: Node { id: b, origin: hir.binders[b].origin },
                value: value.clone(),
            })
            .collect();
        bound.sort_by_key(|s| s.binder);
        Code {
            expr: ExprRef::new(self.file.clone(), hir, expr),
            within: self.instance.clone().map(Arc::new),
            bound: Arc::from(bound),
        }
    }

    /// The environment `code` was written in, with the types typing found there: the call's
    /// instantiation, or the function literal holding it outside any call.
    #[must_use]
    pub fn quoted(cx: &'a Cx<'db>, code: &Code) -> Option<Self> {
        let env = if let Some(call) = &code.within {
            call.running(cx)?
        } else {
            let env = Self::of(cx, &code.expr.file)?;
            let hir = &env.parsed.hir;
            let site = hir
                .enclosing_function(code.expr.expr.id)
                .map(|f| ExprRef::new(code.expr.file.clone(), hir, f));
            let types = site.and_then(|f| cx.get::<Typed>(&Instantiation::plain(f)));
            env.typed_by(types.map(|t| Arc::clone(&t.found)))
        };
        Some(env.staging(&code.bound))
    }

    /// `$value`: code is evaluated where it was written; any other value is itself.
    ///
    /// # Errors
    /// Returns why the code has no value at compile time.
    pub fn splice(&self, value: Value) -> Result<Value, Unevaluated> {
        let Value::Code(code) = value else { return Ok(value) };
        let env = Self::quoted(self.cx, &code).ok_or(Unevaluated::Absent)?;
        match &env.parsed.hir.exprs[code.expr.expr.id].kind {
            // A quote's value is that of its body.
            ExprKind::Quote(block) => env.block_value(*block),
            _ => env.eval(code.expr.expr.id),
        }
    }
}

/// Which of a function's parameters take code.
pub trait Quoting {
    /// Whether each parameter of this function receives its argument as code: see
    /// [`crate::callable::Written::quotes`]. `None` when its file is not read.
    #[must_use]
    fn quotes(&self, cx: &Cx<'_>) -> Option<Vec<bool>>;
}

impl Quoting for Func {
    fn quotes(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        self.written(cx)?.quotes()
    }
}
