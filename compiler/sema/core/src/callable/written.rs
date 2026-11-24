//! Where a function's parameters are written, read: which are `$`, packs, quoted or borrowed.

use vun_desugar::Borrow;
use vun_grammar::TokenKind;
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, Param, PatKind};
use vun_primitives::Primitive;
use vun_primitives::meta::Meta;
use vun_sema_ty::{Ty, TyKind, Value};

use super::Written;

/// What a function ending in a pack declares whatever its calls bind: the parameters before the
/// pack that are no `$` ones, and the result. A call's `$` arguments only add the pack's items.
pub struct Fixed {
    pub params: Vec<Ty>,
    pub result: Ty,
}

impl Written<'_, '_> {
    /// Each parameter's binder and annotation.
    #[must_use]
    pub fn params(&self) -> Option<Vec<(Option<BinderId>, Option<ExprId>)>> {
        let hir = &self.env.parsed.hir;
        let (ExprKind::Fn { params, .. } | ExprKind::FnType { params, .. }) = &hir.exprs[self.expr].kind
        else {
            return None;
        };
        params
            .iter()
            .map(|p| {
                let Param { pat, ty, .. } = p.written()?;
                let binder = pat.and_then(|p| match hir.pats[p].kind {
                    PatKind::Bind(b) | PatKind::Rest(Some(b)) => Some(b),
                    _ => None,
                });
                Some((binder, *ty))
            })
            .collect()
    }

    /// The expression of the result type, when the function writes one.
    #[must_use]
    pub fn returns(&self) -> Option<ExprId> {
        let (ExprKind::Fn { returns, .. } | ExprKind::FnType { returns, .. }) =
            &self.env.parsed.hir.exprs[self.expr].kind
        else {
            return None;
        };
        *returns
    }

    /// The parameter gathering the arguments after the others, when it has one: the last, a
    /// rest pattern `..args: P` in a function literal, `..P` in a function type.
    #[must_use]
    pub fn pack(&self) -> Option<usize> {
        let hir = &self.env.parsed.hir;
        let (ExprKind::Fn { params, .. } | ExprKind::FnType { params, .. }) = &hir.exprs[self.expr].kind
        else {
            return None;
        };
        let last = params.len().checked_sub(1)?;
        let Some(Entry::One(Param { pat, ty, .. })) = params.iter().last() else { return None };
        let packed = match (pat, ty) {
            (Some(p), _) => matches!(hir.pats[*p].kind, PatKind::Rest(_)),
            (None, Some(t)) => matches!(hir.exprs[*t].kind, ExprKind::Prefix { op: TokenKind::DotDot, .. }),
            (None, None) => false,
        };
        packed.then_some(last)
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self) -> Option<Vec<bool>> {
        let hir = &self.env.parsed.hir;
        Some(self.params()?.iter().map(|(b, _)| b.is_some_and(|b| hir.binders[b].comptime)).collect())
    }

    /// Each parameter's type, where its annotation names no `$` parameter: known before a call
    /// is instantiated.
    #[must_use]
    pub fn types(&self) -> Option<Vec<Option<Ty>>> {
        let ty = |(_, ty): &(Option<BinderId>, Option<ExprId>)| self.env.eval((*ty)?).ok()?.as_type();
        Some(self.params()?.iter().map(ty).collect())
    }

    /// What it declares apart from its calls: the types of the run-time parameters before the pack,
    /// and the result. `None` when it ends in no pack, or a type it names depends on a `$` parameter.
    #[must_use]
    pub fn fixed(&self) -> Option<Fixed> {
        let (pack, comptime, types) = (self.pack()?, self.comptime()?, self.types()?);
        let params = types
            .into_iter()
            .zip(comptime)
            .take(pack)
            .filter(|&(_, comptime)| !comptime)
            .map(|(ty, _)| ty)
            .collect::<Option<_>>()?;
        let result = match self.returns() {
            Some(expr) => self.env.eval(expr).ok()?.as_type()?,
            None => TyKind::unit(),
        };
        Some(Fixed { params, result })
    }

    /// Whether each parameter receives its argument as code: those of type `lang.meta.Code(T)`,
    /// whatever `T` (spec 03, section 4).
    #[must_use]
    pub fn quotes(&self) -> Option<Vec<bool>> {
        let hir = &self.env.parsed.hir;
        let quotes = |(_, ty): &(Option<BinderId>, Option<ExprId>)| {
            let Some(ExprKind::Call { callee, .. }) = ty.map(|t| &hir.exprs[t].kind) else { return false };
            matches!(self.env.eval(*callee), Ok(Value::Primitive(Primitive::Meta(Meta::Code))))
        };
        Some(self.params()?.iter().map(quotes).collect())
    }

    /// The borrow mode each parameter's annotation writes, whatever the `$` parameters: known
    /// before a call is instantiated. `None` for a parameter not written as a borrow.
    #[must_use]
    pub fn modes(&self) -> Option<Vec<Option<Borrow>>> {
        let hir = &self.env.parsed.hir;
        let mode = |(_, ty): &(Option<BinderId>, Option<ExprId>)| match hir.exprs[(*ty)?].kind {
            ExprKind::Borrow { mode, .. } => mode,
            _ => None,
        };
        Some(self.params()?.iter().map(mode).collect())
    }
}
