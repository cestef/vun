//! Patterns and `match` (spec 01, section 6): a pattern is checked against the type it matches,
//! and gives its binders the types of the parts they bind. Through a borrow, binders borrow the
//! parts in the same mode (decision 31). A `match` has the type all its arms can take.

use vun_hir::tree::{Arm, Entry, ExprId, ExprKind, Name, PatId, PatKind};

use vun_grammar::TokenKind;

use super::{Known, Operation, Typer};
use crate::Primitive;
use crate::member::{Found, Members};
use crate::ty::{Ty, TyKind, Value};

impl Typer<'_, '_, '_> {
    /// Checks `pat` against values of type `ty`, if known; `borrow`, when matching through a
    /// borrow, is the borrow type its binders take.
    pub(super) fn pattern(&mut self, pat: PatId, ty: Option<Ty>, borrow: Option<Primitive>) {
        let hir = self.hir();
        let origin = hir.pats[pat].origin;
        match &hir.pats[pat].kind {
            PatKind::Bind(binder) => {
                if let Some(ty) = ty {
                    let ty = borrow.map_or(ty, |b| TyKind::Primitive(b, vec![Value::Type(ty)]).intern());
                    self.bind(*binder, ty);
                }
            }
            PatKind::At { binder, pat } => {
                if let Some(ty) = ty {
                    let whole = borrow.map_or(ty, |b| TyKind::Primitive(b, vec![Value::Type(ty)]).intern());
                    self.bind(*binder, whole);
                }
                self.pattern(*pat, ty, borrow);
            }
            PatKind::Tuple(items) => {
                let parts = ty.and_then(|t| match &*t {
                    TyKind::Tuple(parts) if parts.len() == items.len() => Some(parts.clone()),
                    _ => None,
                });
                for (i, item) in items.iter().enumerate() {
                    if let Entry::One(item) = item {
                        self.pattern(*item, parts.as_ref().map(|p| p[i]), borrow);
                    }
                }
            }
            PatKind::Variant { name, args } => {
                let payload = match (name, ty) {
                    (Name::Plain(name), Some(ty)) => {
                        match self.env().cx.get::<Members>(&ty).map(|m| m.find(*name).into()) {
                            Some(Lookup::Variant(payload)) => Some(payload),
                            Some(Lookup::Absent) => {
                                self.no_member(origin, ty, *name);
                                None
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                for (i, arg) in args.iter().flatten().enumerate() {
                    if let Entry::One(arg) = arg {
                        let item = payload.as_ref().and_then(|p| p.get(i).copied().flatten());
                        self.pattern(*arg, item, borrow);
                    }
                }
            }
            PatKind::Or(alternatives) => {
                for &alternative in alternatives {
                    self.pattern(alternative, ty, borrow);
                }
            }
            // A computed name is known at compile time; what it binds is the declaration's.
            PatKind::Computed(name) => {
                self.typed(*name);
            }
            // A value pattern matches when `value == x`; a range when `lo <= x` and `hi > x`, or
            // `hi >= x` inclusive: operators on the value matched (spec 02, section 4).
            PatKind::Value(value) => {
                self.against(*value, ty);
                if let Some(ty) = ty {
                    self.tested(Operation::Test(pat), TokenKind::EqEq, *value, ty);
                }
            }
            PatKind::Range { lo, hi, inclusive } => {
                self.against(*lo, ty);
                if let Some(ty) = ty {
                    self.tested(Operation::Lower(pat), TokenKind::Le, *lo, ty);
                }
                if let Some(hi) = hi {
                    self.against(*hi, ty);
                    let op = if *inclusive { TokenKind::Ge } else { TokenKind::Gt };
                    if let Some(ty) = ty {
                        self.tested(Operation::Upper(pat), op, *hi, ty);
                    }
                }
            }
            PatKind::Struct { ty: written, fields } => {
                let ty = written.and_then(|t| self.ty(t)).or(ty);
                let members = ty.and_then(|t| self.env().cx.get::<Members>(&t));
                for (i, field) in fields.iter().enumerate() {
                    let Entry::One((name, field)) = field else { continue };
                    let found = members.as_ref().and_then(|m| match name {
                        Some(Name::Plain(name)) => match m.find(*name) {
                            Found::Field(f) => f.ty.as_ref().ok().copied(),
                            _ => None,
                        },
                        None => m.fields.get(i).and_then(|f| f.ty.as_ref().ok().copied()),
                        Some(_) => None,
                    });
                    self.pattern(*field, found, borrow);
                }
            }
            PatKind::Hole | PatKind::Rest(_) | PatKind::Error => {}
        }
    }

    /// `pat` tested against a value of type `matched`, looking through a borrow.
    pub(super) fn test(&mut self, pat: PatId, matched: Option<Ty>) {
        match matched.and_then(|t| t.borrow()) {
            Some((borrow, inner)) => self.pattern(pat, Some(inner), Some(borrow)),
            None => self.pattern(pat, matched, None),
        }
    }

    /// A value in a pattern, checked against the type matched.
    fn against(&mut self, value: ExprId, ty: Option<Ty>) {
        let found = self.typed_as(value, ty);
        if let (Some(found), Some(ty)) = (found, ty) {
            self.expect(self.hir().exprs[value].origin, found, ty, None);
        }
    }

    /// `match scrutinee { arms }`: each arm's pattern against the scrutinee, its guard as a
    /// condition, and its value where the match's own value is expected.
    pub(super) fn matching(&mut self, expr: ExprId, scrutinee: ExprId, arms: &[Entry<Arm>]) -> Option<Known> {
        let hir = self.hir();
        let matched = self.typed(scrutinee).map(|k| k.ty);
        let expected = self.expected(expr);
        let mut joined: Option<Option<Known>> = None;
        for arm in arms {
            let Entry::One(arm) = arm else { continue };
            self.test(arm.pat, matched);
            if let Some(guard) = arm.guard {
                self.logic(&[guard]);
            }
            let Some(value) = arm.value else { continue };
            let found = self.typed_as(value, expected);
            let found = self.fits(value, found, expected);
            let leaves = matches!(
                hir.exprs[value].kind,
                ExprKind::Return(_) | ExprKind::Break { .. } | ExprKind::Continue { .. }
            );
            if leaves {
                continue;
            }
            joined = Some(match (joined, found) {
                (None, found) => found,
                (Some(Some(sofar)), Some(found)) => self.join(value, sofar, found),
                _ => None,
            });
        }
        joined.flatten()
    }
}

/// What a variant pattern's name is in the type matched.
enum Lookup {
    /// A variant, with its payload's types where known.
    Variant(Vec<Option<Ty>>),
    Absent,
    Other,
}

impl From<Found<'_>> for Lookup {
    fn from(found: Found<'_>) -> Self {
        match found {
            Found::Variant(v) => Self::Variant(v.payload.iter().map(|p| p.as_ref().ok().copied()).collect()),
            Found::Absent | Found::Field(_) | Found::Decl(_) => Self::Absent,
            Found::Unknown => Self::Other,
        }
    }
}
