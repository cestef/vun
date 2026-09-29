//! Places in expressions: names of locals, fields, items and `.*`, read, written and borrowed.
//! `.` and indexing look through borrows (spec 01, section 4), so the typed body decides where a
//! dereference goes.

use vun_desugar::Borrow;
use vun_grammar::TokenKind;
use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::env::Hop;
use vun_sema::member::Members;
use vun_sema::thread::PerThread;
use vun_sema_int::Interval;
use vun_sema_ty::item::Site;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{TyKind, Value};

use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{Local, LocalDecl, Operand, Role, Rvalue};

impl Builder<'_, '_, '_> {
    /// The place `expr` denotes: a local, or a projection of a place. Any other expression is
    /// computed into a temporary, which is then its place.
    pub(super) fn place(&mut self, expr: ExprId) -> Result<Place, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.exprs[expr].origin;
        match &hir.exprs[expr].kind {
            ExprKind::Name(_) => match self.env.parsed.resolution.names[expr] {
                // A `$` binder is a value, not storage: its place holds a copy, of the type of
                // that value alone.
                Some(Res::Binder(b)) if self.bound.contains_key(&b) && !hir.binders[b].comptime => {
                    Ok(self.binder_place(b))
                }
                // A `mut` top-level binding is a static, starting as its initial value.
                _ if let Ok(Site::Decl(decl)) = self.env().resolve(expr)
                    && decl.mutable(self.env.cx) =>
                {
                    Ok(Place::of(self.global(Role::StaticMut(decl), expr, origin)))
                }
                Some(Res::Binder(b)) if !hir.binders[b].comptime => match self.env().resolve(expr) {
                    Ok(Site::Decl(decl)) => Ok(Place::of(self.global(Role::Static(decl), expr, origin))),
                    _ => self.held(expr, origin),
                },
                _ => self.held(expr, origin),
            },
            // A path to a top-level binding reaches its storage, as its name does: a `mut` one's,
            // and one with a storage per thread, whose value is its own thread's.
            ExprKind::Member { .. }
                if let Ok(Site::Decl(decl)) = self.env().resolve(expr)
                    && (decl.mutable(self.env.cx) || decl.per_thread(self.env.cx)) =>
            {
                let role = if decl.mutable(self.env.cx) { Role::StaticMut(decl) } else { Role::Static(decl) };
                Ok(Place::of(self.global(role, expr, origin)))
            }
            ExprKind::Member { object, name } if self.env().eval(expr).is_err() => {
                if let Some(at) = self.env().position(name) {
                    return Ok(self.through_borrows(*object)?.project(Projection::Position(at)));
                }
                let name = self.env().name(name).ok_or(Unbuilt::at(origin))?;
                Ok(self.reached(*object, name)?.project(Projection::Field(name)))
            }
            ExprKind::Index { object, index } => {
                // An integer index names one item; bounds name the run of items they cover.
                let typed = self.typed().ok_or(Unbuilt::at(origin))?;
                let known = typed.types.get(index).map(|k| k.ty);
                if !known.is_some_and(|t| t.interval().is_some() || t.bounds().is_some()) {
                    return Err(Unbuilt::at(origin));
                }
                let place = self.through_borrows(*object)?;
                let constant = self.env().eval(*index);
                let index = self.place(*index)?;
                let index = if index.projection.is_empty() {
                    index.local
                } else {
                    self.temp_local(Rvalue::Use(Operand::Place(index)), origin)
                };
                // A temporary holding a constant has the type of that integer alone, which
                // proves it within the items where it is.
                if self.locals[index].name.is_none()
                    && let Ok(Value::Int(n)) = constant
                {
                    self.locals[index].ty = Some(TyKind::int(&Interval::singleton(n)));
                }
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
            Some((mode, ..)) if run => Ok(self.temp(Rvalue::Ref { mode, place }, origin)),
            _ => Ok(self.temp(Rvalue::Use(Operand::Place(place)), origin)),
        }
    }

    /// `&mode operand`: a borrow of the place `operand`, or, for a type, the borrow type. A
    /// borrow of a top-level binding reaches its one storage; a borrow of another constant holds
    /// the value.
    pub(super) fn borrow(&mut self, expr: ExprId, mode: Borrow, operand: ExprId) -> Result<Operand, Unbuilt> {
        let binding = matches!(self.env().resolve(operand), Ok(Site::Decl(_)))
            && self.type_of(expr).is_some_and(|t| t.borrowing().is_some());
        if !binding && let Ok(value) = self.env().eval(expr) {
            return Ok(Operand::Const(value));
        }
        let origin = self.env.parsed.hir.exprs[expr].origin;
        let place = self.place(operand)?;
        Ok(self.temp(Rvalue::Ref { mode, place }, origin))
    }

    /// The place `object`, dereferenced once for each borrow its type is.
    pub(super) fn through_borrows(&mut self, object: ExprId) -> Result<Place, Unbuilt> {
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
        // A run of items through a borrow, `xs[a..b]`, reads as a borrow of the run, whose place
        // is the run itself: the first borrow is already gone through.
        let run = place.ty(self.env.cx, &self.locals).is_some_and(|t| Some(t) == ty.borrowed());
        for (i, hop) in self.env().reach(ty, name).0.into_iter().enumerate() {
            if run && i == 0 && !matches!(hop, Hop::Owner(_)) {
                continue;
            }
            if let Hop::Owner(field) = hop {
                place = place.project(Projection::Field(field));
            }
            place = place.project(Projection::Deref);
        }
        Ok(place)
    }

    /// The storage of a top-level binding, `init` from the program's start: one local of the
    /// binding's type, shared by every mention of it, which nothing in the body writes first.
    fn global(&mut self, role: Role, expr: ExprId, origin: OriginId) -> Local {
        if let Some(&local) = self.globals.get(&role) {
            return local;
        }
        let ty = self.type_of(expr);
        let local = self.locals.push(LocalDecl { name: None, ty, role: role.clone(), origin });
        self.globals.insert(role, local);
        local
    }

    /// A temporary holding the value of `expr`.
    fn held(&mut self, expr: ExprId, origin: OriginId) -> Result<Place, Unbuilt> {
        let value = self.operand(expr)?;
        // A temporary holding the value already is its place.
        if let Operand::Place(place) = &value
            && place.projection.is_empty()
            && self.locals[place.local].name.is_none()
        {
            return Ok(place.clone());
        }
        Ok(Place::of(self.temp_local(Rvalue::Use(value), origin)))
    }

    /// The value of the name `expr`: a local read where it is written, a `$` binder's value, a
    /// constant, or what a `mut` top-level binding's storage holds.
    pub(super) fn named(&mut self, expr: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        let hir = &self.env.parsed.hir;
        match self.env.parsed.resolution.names[expr] {
            // A read of a local is a statement where it is written, which checks point at; a
            // read of a linear value moves it, so the temporary has the value's type.
            Some(Res::Binder(b)) if self.bound.contains_key(&b) => {
                let read = self.temp_local(Rvalue::Use(Operand::Place(self.binder_place(b))), origin);
                self.locals[read].ty = self.type_of(expr);
                Ok(Operand::Place(Place::of(read)))
            }
            // A `$` parameter of an enclosing function, bound by the call running the body.
            // A `$for` binder, or one bound by the instantiation, is a constant.
            Some(Res::Binder(b)) if hir.binders[b].comptime => match self.env().bound(b) {
                Some(value) => Ok(Operand::Const(value.clone())),
                None => Ok(self.temp(self.comptime(expr, None), origin)),
            },
            // A constant is its value; a `mut` binding's is read from its storage.
            _ => match self.env().eval(expr) {
                Ok(value) => Ok(Operand::Const(value)),
                Err(Unevaluated::Runtime) => self.read(expr),
                Err(why) => Err(Unbuilt::unevaluated(origin, why)),
            },
        }
    }
}
