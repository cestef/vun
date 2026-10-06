//! Ranges (appendix B): `a..b`, `a..=b` and `a..` have type `lang.Bounds(T)`, `T` the integers
//! any range with such bounds can hold; where a `lang.Range` is expected, bounds are that range.

use vun_desugar::Form;
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_sema_int::Interval;
use vun_sema_ty::{TyKind, Value};

use super::{Known, Typer};

impl Typer<'_, '_, '_> {
    /// `lo op hi`, or `lo op` without an upper bound.
    pub(super) fn range(&mut self, op: TokenKind, lo: ExprId, hi: Option<ExprId>) -> Option<Known> {
        let lo = self.typed(lo);
        let hi = hi.map(|hi| self.typed(hi));
        let (lo, hi) = (lo?, hi.map_or(Some(None), |hi| hi.map(Some))?);
        let top = match hi {
            Some(hi) => Some(hi.ty.interval()?),
            None => None,
        };
        let items = match Interval::items(op, &lo.ty.interval()?, top.as_ref()) {
            Some(items) => TyKind::int(&items),
            None => self.env().item(vun_items::lang::never).ok()?.as_type()?,
        };
        let bounds = self.env().form(Form::Bounds).ok()?;
        let ty = self.env().apply(bounds, vec![Value::Type(items)]).ok()?.as_type()?;
        Some(Known { ty, stage: hi.map_or(lo.stage, |hi| lo.stage.max(hi.stage)) })
    }
}
