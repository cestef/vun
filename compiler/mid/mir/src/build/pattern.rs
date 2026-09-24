//! Patterns (spec 01, section 6): a test branches to a failure block when the value does not
//! match, and binds the binders when it does. Through a borrow, binders borrow the parts in the
//! same mode (decision 31), as the typer types them. `match`, `if p = e`, `for p = e` and
//! declarations with a pattern are built on tests.

use vun_desugar::Borrow;
use vun_hir::tree::{Arm, BinderId, Entry, ExprId, Name, PatId, PatKind};
use vun_origin::OriginId;
use vun_sema::member::{Found, Members};
use vun_sema::ty::{Ty, TyKind};

use vun_sema::typeck::Operation;

use super::operator::Side;
use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{BlockId, Operand, Role, Rvalue, TerminatorKind};

/// How a pattern's binders hold what they bind.
#[derive(Clone, Copy)]
enum Binding {
    Copy,
    /// A borrow in the mode of the borrow matched through.
    Borrow(Borrow),
}

/// A value being matched: where it is, its type when the typed body knows it, and how binders
/// hold its parts.
pub(super) struct Matched {
    place: Place,
    ty: Option<Ty>,
    binding: Binding,
}

impl Matched {
    /// The value at `place`, of a type not known here, whose binders take copies.
    pub(super) fn copied(place: Place) -> Self {
        Self { place, ty: None, binding: Binding::Copy }
    }

    /// The part of this value `projection` reaches, of type `ty`.
    fn part(&self, projection: Projection, ty: Option<Ty>) -> Self {
        Self { place: self.place.clone().project(projection), ty, binding: self.binding }
    }
}

impl Builder<'_, '_, '_> {
    /// The value of `expr` matched by a test: through a borrow, the place borrowed, whose parts
    /// binders borrow; otherwise the value itself, taken out of its place, since matching by
    /// value consumes it.
    pub(super) fn scrutinee(&mut self, expr: ExprId) -> Result<Matched, Unbuilt> {
        let ty = self.type_of(expr);
        if let Some((mode, _, inner)) = ty.and_then(|t| t.borrowing()) {
            let place = self.place(expr)?.project(Projection::Deref);
            let binding = Binding::Borrow(mode);
            return Ok(Matched { place, ty: Some(inner), binding });
        }
        self.declared(expr)
    }

    /// The value of `expr` matched by a declaration's pattern, taken out of its place.
    pub(super) fn declared(&mut self, expr: ExprId) -> Result<Matched, Unbuilt> {
        let value = self.operand(expr)?;
        let origin = self.env.parsed.hir.exprs[expr].origin;
        let local = self.temp_local(Rvalue::Use(value), origin);
        self.locals[local].ty = self.type_of(expr);
        self.locals[local].role = Role::Unpacked;
        Ok(Matched { place: Place::of(local), ty: self.type_of(expr), binding: Binding::Copy })
    }

    /// Tests `matched` against `pat`: on in the current block when it matches, with the binders
    /// bound, and to `fail` when it does not.
    pub(super) fn test(&mut self, pat: PatId, matched: &Matched, fail: BlockId) -> Result<(), Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.pats[pat].origin;
        let unbuilt = Unbuilt { at: origin };
        match &hir.pats[pat].kind {
            PatKind::Hole | PatKind::Rest(None) => Ok(()),
            PatKind::Bind(binder) => {
                self.bind_to(*binder, matched, origin);
                Ok(())
            }
            PatKind::At { binder, pat } => {
                self.bind_to(*binder, matched, origin);
                self.test(*pat, matched, fail)
            }
            PatKind::Tuple(items) => {
                let parts = matched.ty.and_then(|t| match &*t {
                    TyKind::Tuple(parts) => Some(parts.clone()),
                    _ => None,
                });
                for (i, item) in items.iter().enumerate() {
                    let Entry::One(item) = item else { return Err(unbuilt) };
                    // A rest pattern last matches the remaining items.
                    if let PatKind::Rest(None) = hir.pats[*item].kind
                        && i + 1 == items.len()
                    {
                        continue;
                    }
                    let ty = parts.as_ref().and_then(|p| p.get(i).copied());
                    self.test(*item, &matched.part(Projection::Position(i), ty), fail)?;
                }
                Ok(())
            }
            PatKind::Variant { name: Name::Plain(name), args } => {
                let place = matched.place.clone();
                let cond = self.temp(Rvalue::IsVariant { place, variant: *name }, origin);
                self.guard(cond, fail, origin);
                let members = matched.ty.and_then(|t| self.env.cx.get::<Members>(&t));
                let payload = match members.as_deref().map(|m| m.find(*name)) {
                    Some(Found::Variant(v)) => v.payload.iter().map(|p| p.as_ref().ok().copied()).collect(),
                    _ => Vec::new(),
                };
                for (i, arg) in args.iter().flatten().enumerate() {
                    let Entry::One(arg) = arg else { return Err(unbuilt) };
                    let ty = payload.get(i).copied().flatten();
                    self.test(*arg, &matched.part(Projection::Payload(i), ty), fail)?;
                }
                Ok(())
            }
            PatKind::Struct { ty, fields } => {
                let ty = ty.and_then(|t| self.env().ty(t).ok()).or(matched.ty).ok_or(unbuilt)?;
                let members = self.env.cx.get::<Members>(&ty).ok_or(unbuilt)?;
                for (i, field) in fields.iter().enumerate() {
                    let Entry::One((name, field)) = field else { return Err(unbuilt) };
                    if let PatKind::Rest(None) = hir.pats[*field].kind {
                        continue;
                    }
                    let def = match name {
                        Some(Name::Plain(name)) => members.fields.iter().find(|f| f.name == *name),
                        None => members.fields.get(i),
                        Some(_) => None,
                    };
                    let def = def.ok_or(unbuilt)?;
                    let field_matched =
                        matched.part(Projection::Field(def.name), def.ty.as_ref().ok().copied());
                    self.test(*field, &field_matched, fail)?;
                }
                Ok(())
            }
            PatKind::Or(alternatives) => self.either(alternatives, matched, fail, origin),
            // The tests typing found: `value == x`, and `lo <= x` with `hi >= x` or `hi > x`.
            PatKind::Value(value) => {
                let found = Side::Matched(matched.place.clone(), matched.ty);
                let cond = self.operator(Operation::Test(pat), *value, vec![found], origin)?;
                self.guard(cond, fail, origin);
                Ok(())
            }
            PatKind::Range { lo, hi, .. } => {
                let found = || Side::Matched(matched.place.clone(), matched.ty);
                let cond = self.operator(Operation::Lower(pat), *lo, vec![found()], origin)?;
                self.guard(cond, fail, origin);
                if let Some(hi) = hi {
                    let cond = self.operator(Operation::Upper(pat), *hi, vec![found()], origin)?;
                    self.guard(cond, fail, origin);
                }
                Ok(())
            }
            PatKind::Variant { .. } | PatKind::Rest(Some(_)) | PatKind::Computed(_) | PatKind::Error => {
                Err(unbuilt)
            }
        }
    }

    /// `p | q`: each alternative in turn, the next tried when one fails. Alternatives that bind
    /// are not lowered yet: each binds its own binders, which names after it do not all reach.
    fn either(
        &mut self,
        alternatives: &[PatId],
        matched: &Matched,
        fail: BlockId,
        origin: OriginId,
    ) -> Result<(), Unbuilt> {
        let hir = &self.env.parsed.hir;
        let binds = alternatives.iter().any(|&p| {
            let mut binders = Vec::new();
            hir.binders(p, &mut binders);
            !binders.is_empty()
        });
        if binds {
            return Err(Unbuilt { at: origin });
        }
        let success = self.fresh(origin);
        for (i, &alternative) in alternatives.iter().enumerate() {
            let next = if i + 1 == alternatives.len() { fail } else { self.fresh(origin) };
            self.test(alternative, matched, next)?;
            self.terminate(TerminatorKind::Goto(success), origin);
            self.current = next;
        }
        self.current = success;
        Ok(())
    }

    /// Binds `binder` to the matched value: a copy, or a borrow in the mode matched through.
    fn bind_to(&mut self, binder: BinderId, matched: &Matched, origin: OriginId) {
        let place = matched.place.clone();
        let value = match matched.binding {
            Binding::Copy => Rvalue::Use(Operand::Place(place)),
            Binding::Borrow(mode) => Rvalue::Ref { mode, place },
        };
        let local = self.bind_scoped(binder);
        self.assign(Place::of(local), value, origin);
    }

    /// Goes on when `cond` holds, and to `fail` when it does not.
    pub(super) fn guard(&mut self, cond: Operand, fail: BlockId, origin: OriginId) {
        let then = self.fresh(origin);
        self.terminate(TerminatorKind::Branch { cond, then, otherwise: fail }, origin);
        self.current = then;
    }

    /// `match scrutinee { arms }`: each arm's test, then its guard, in order; past the last arm
    /// control never goes, since `match` is exhaustive (decision 29).
    pub(super) fn matching(
        &mut self,
        scrutinee: ExprId,
        arms: &[Entry<Arm>],
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let matched = self.scrutinee(scrutinee)?;
        let (result, join) = (self.result(origin), self.fresh(origin));
        for arm in arms {
            let Entry::One(arm) = arm else { return Err(Unbuilt { at: origin }) };
            let next = self.fresh(arm.origin);
            self.test(arm.pat, &matched, next)?;
            if let Some(guard) = arm.guard {
                let cond = self.operand(guard)?;
                self.guard(cond, next, arm.origin);
            }
            let value = match arm.value {
                Some(value) => self.operand(value)?,
                None => Self::unit(),
            };
            self.assign(Place::of(result), Rvalue::Use(value), arm.origin);
            self.terminate(TerminatorKind::Goto(join), arm.origin);
            self.current = next;
        }
        self.terminate(TerminatorKind::Unreachable, origin);
        self.current = join;
        Ok(Operand::Place(Place::of(result)))
    }
}
