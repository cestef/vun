//! Places in expressions: names of locals, fields, items and `.*`, read, written and borrowed.
//! `.` and indexing look through borrows (spec 01, section 4), so the typed body decides where a
//! dereference goes.

use vun_grammar::TokenKind;
use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::env::Hop;
use vun_sema::member::Members;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Role, Rvalue};
use crate::place::{Place, Projection};

impl Builder<'_, '_, '_> {
    /// The place `expr` denotes: a local, or a projection of a place. Any other expression is
    /// computed into a temporary, which is then its place.
    pub(super) fn place(&mut self, expr: ExprId) -> Result<Place, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.exprs[expr].origin;
        match &hir.exprs[expr].kind {
            ExprKind::Name(_) => match self.env.parsed.resolution.names[expr] {
                Some(Res::Binder(b)) if self.bound.contains_key(&b) => Ok(self.binder_place(b)),
                Some(Res::Binder(b)) if !hir.binders[b].comptime => {
                    let place = self.held(expr, origin)?;
                    self.locals[place.local].role = Role::Static;
                    Ok(place)
                }
                _ => self.held(expr, origin),
            },
            ExprKind::Member { object, name } if self.env().eval(expr).is_err() => {
                if let Some(at) = self.env().position(name) {
                    return Ok(self.through_borrows(*object)?.project(Projection::Position(at)));
                }
                let name = self.env().name(name).ok_or(Unbuilt { at: origin })?;
                Ok(self.reached(*object, name)?.project(Projection::Field(name)))
            }
            ExprKind::Index { object, index } => {
                // An integer index names one item; bounds name the run of items they cover.
                let typed = self.typed().ok_or(Unbuilt { at: origin })?;
                let known = typed.types.get(index).map(|k| k.ty);
                if !known.is_some_and(|t| t.interval().is_some() || t.bounds().is_some()) {
                    return Err(Unbuilt { at: origin });
                }
                let place = self.through_borrows(*object)?;
                let index = self.place(*index)?;
                let index = if index.projection.is_empty() {
                    index.local
                } else {
                    self.temp_local(Rvalue::Use(Operand::Place(index)), origin)
                };
                Ok(place.project(Projection::Index(index)))
            }
            // Through a struct owning its target, `.*` goes through its `#[kind.owns]` field.
            ExprKind::Postfix { op: TokenKind::DotStar, operand } => {
                let ty = self.typed().and_then(|t| t.types.get(operand).map(|k| k.ty));
                let owner = ty.and_then(|t| self.env.cx.get::<Members>(&t)?.deref());
                let place = self.place(*operand)?;
                let place = match owner {
                    Some((field, _)) => place.project(Projection::Field(field)),
                    None => place,
                };
                Ok(place.project(Projection::Deref))
            }
            _ => self.held(expr, origin),
        }
    }

    /// The value of the place `expr`, copied out of it; a path to a constant is its value. A run
    /// of items reached through a borrow, `xs[a..b]`, is borrowed again in the same mode.
    pub(super) fn read(&mut self, expr: ExprId) -> Result<Operand, Unbuilt> {
        if let Ok(value) = self.env().eval(expr) {
            return Ok(Operand::Const(value));
        }
        let origin = self.env.parsed.hir.exprs[expr].origin;
        let place = self.place(expr)?;
        let run = matches!(self.env.parsed.hir.exprs[expr].kind, ExprKind::Index { .. });
        let borrow = self.type_of(expr).and_then(|t| t.borrowing());
        match borrow {
            Some((mode, ..)) if run => {
                let mode = (!mode.is_empty()).then(|| Symbol::intern(mode));
                Ok(self.temp(Rvalue::Ref { mode, place }, origin))
            }
            _ => Ok(self.temp(Rvalue::Use(Operand::Place(place)), origin)),
        }
    }

    /// `&mode operand`: a borrow of the place `operand`, or, for a type, the borrow type.
    pub(super) fn borrow(
        &mut self,
        expr: ExprId,
        mode: Option<Symbol>,
        operand: ExprId,
    ) -> Result<Operand, Unbuilt> {
        if let Ok(value) = self.env().eval(expr) {
            return Ok(Operand::Const(value));
        }
        let origin = self.env.parsed.hir.exprs[expr].origin;
        let place = self.place(operand)?;
        Ok(self.temp(Rvalue::Ref { mode, place }, origin))
    }

    /// The place `object`, dereferenced once for each borrow its type is.
    fn through_borrows(&mut self, object: ExprId) -> Result<Place, Unbuilt> {
        let mut place = self.place(object)?;
        let mut ty = self.typed().and_then(|t| t.types.get(&object).map(|k| k.ty));
        while let Some(pointee) = ty.and_then(|t| t.borrowed()) {
            place = place.project(Projection::Deref);
            ty = Some(pointee);
        }
        Ok(place)
    }

    /// The place `object`, followed through the borrows and owners `.name` looks through.
    pub(super) fn reached(&mut self, object: ExprId, name: Symbol) -> Result<Place, Unbuilt> {
        let mut place = self.place(object)?;
        let Some(ty) = self.typed().and_then(|t| t.types.get(&object).map(|k| k.ty)) else {
            return Ok(place);
        };
        for hop in self.env().reach(ty, name).0 {
            if let Hop::Owner(field) = hop {
                place = place.project(Projection::Field(field));
            }
            place = place.project(Projection::Deref);
        }
        Ok(place)
    }

    /// A temporary holding the value of `expr`.
    fn held(&mut self, expr: ExprId, origin: OriginId) -> Result<Place, Unbuilt> {
        let value = self.operand(expr)?;
        Ok(Place::of(self.temp_local(Rvalue::Use(value), origin)))
    }
}
