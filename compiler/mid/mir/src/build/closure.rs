//! Closures (spec 01, section 4): a closure value holds its function and its captures, and its
//! function takes the captures before its parameters. A capture by borrow (`[&mut n]`) holds the
//! borrow, so the body reaches `n` through it.

use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, List};
use vun_origin::OriginId;
use vun_sema::ty::TyKind;

use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{Aggregate, Local, Operand, Rvalue};

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
                Entry::Staged(_) => Err(Unbuilt::at(origin)),
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
            let Entry::One(capture) = capture else { return Err(Unbuilt::at(origin)) };
            let (binder, borrow) = self.env.parsed.captured(*capture).ok_or(Unbuilt::at(origin))?;
            let local = self.bind(binder);
            // A capture by borrow holds the borrow, through which the body reaches the binder.
            if let Some(mode) = borrow {
                self.through.insert(binder);
                self.locals[local].ty = self.locals[local].ty.and_then(|t| TyKind::borrowed_as(mode, t));
            }
            params.push(local);
        }
        Ok(params)
    }

    /// The place a bound binder names: its local, or what a captured borrow points to.
    pub(super) fn binder_place(&self, binder: BinderId) -> Place {
        let place = Place::of(self.bound[&binder]);
        if self.through.contains(&binder) { place.project(Projection::Deref) } else { place }
    }
}
