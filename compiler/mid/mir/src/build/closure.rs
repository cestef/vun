//! Closures (spec 01, section 4): a closure value holds its function and its captures, and its
//! function takes the captures before its parameters. A capture by borrow (`[&mut n]`) holds the
//! borrow, so the body reaches `n` through it.

use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, List};
use vun_origin::OriginId;

use super::{Builder, Unbuilt};
use crate::body::{Aggregate, Local, Operand, Rvalue};
use crate::place::{Place, Projection};

impl Builder<'_, '_, '_> {
    /// `[captures] (params) { body }`: the closure value.
    pub(super) fn closure(
        &mut self,
        captures: &List<ExprId>,
        func: ExprId,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let parts = captures
            .iter()
            .map(|c| match c {
                Entry::One(c) => self.operand(*c),
                Entry::Staged(_) => Err(Unbuilt { at: origin }),
            })
            .collect::<Result<_, _>>()?;
        let kind = Aggregate::Closure { literal: func, staged: self.stepped() };
        Ok(self.temp(Rvalue::Aggregate { kind, parts }, origin))
    }

    /// When the function lowered is a closure's, a parameter for each capture, in order.
    pub(super) fn captures(&mut self, literal: ExprId, origin: OriginId) -> Result<Vec<Local>, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let Some(closure) = hir.closure_of(literal) else { return Ok(Vec::new()) };
        let ExprKind::Closure { captures, .. } = &hir.exprs[closure].kind else { return Ok(Vec::new()) };
        let mut params = Vec::new();
        for capture in captures {
            let Entry::One(capture) = capture else { return Err(Unbuilt { at: origin }) };
            let binder = self.captured(*capture).ok_or(Unbuilt { at: origin })?;
            if matches!(hir.exprs[*capture].kind, ExprKind::Prefix { .. }) {
                self.through.insert(binder);
            }
            params.push(self.bind(binder));
        }
        Ok(params)
    }

    /// The place a bound binder names: its local, or what a captured borrow points to.
    pub(super) fn binder_place(&self, binder: BinderId) -> Place {
        let place = Place::of(self.bound[&binder]);
        if self.through.contains(&binder) { place.project(Projection::Deref) } else { place }
    }

    /// The binder the capture `capture` (`x`, `&mut x`) captures.
    fn captured(&self, capture: ExprId) -> Option<BinderId> {
        match &self.env.parsed.hir.exprs[capture].kind {
            ExprKind::Prefix { operand, .. } => self.captured(*operand),
            ExprKind::Name(_) => match self.env.parsed.resolution.names[capture] {
                Some(Res::Binder(b)) => Some(b),
                _ => None,
            },
            _ => None,
        }
    }
}
