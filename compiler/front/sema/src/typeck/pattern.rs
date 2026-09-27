//! Patterns and `match` (spec 01, section 6): a pattern is checked against the type it matches,
//! and gives its binders the types of the parts they bind. Through a borrow, binders borrow the
//! parts in the same mode (decision 31). A `match` has the type all its arms can take.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{Arm, BinderId, Entry, ExprId, ExprKind, Joined, Name, PatId, PatKind};

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
            // A rest binds what its tuple pattern gives it, the items it stands for.
            PatKind::Bind(binder) | PatKind::Rest(Some(binder)) => self.bind_matched(*binder, ty, borrow),
            PatKind::At { binder, pat } => {
                self.bind_matched(*binder, ty, borrow);
                self.pattern(*pat, ty, borrow);
            }
            PatKind::Tuple(items) => {
                let items_of = ty.and_then(|t| match &*t {
                    TyKind::Tuple(parts) => Some(self.tuple_items(pat, t, items, parts)),
                    _ => None,
                });
                for (i, item) in items.iter().enumerate() {
                    if let Entry::One(item) = item {
                        self.pattern(
                            *item,
                            items_of.as_ref().and_then(|p| p.get(i).copied().flatten()),
                            borrow,
                        );
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
                self.alternatives(alternatives);
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
                        Some(Name::Plain(name)) => {
                            if let Found::Field(f) = m.find(*name) {
                                f.ty.as_ref().ok().copied()
                            } else {
                                self.no_member(hir.pats[*field].origin, ty?, *name);
                                None
                            }
                        }
                        None => m.fields.get(i).and_then(|f| f.ty.as_ref().ok().copied()),
                        Some(_) => None,
                    });
                    self.pattern(*field, found, borrow);
                }
            }
            PatKind::Hole | PatKind::Rest(None) | PatKind::Error => {}
        }
    }

    /// Gives `binder` the type of the values of `ty` it binds, or a borrow of them when matching
    /// through the borrow `borrow`.
    fn bind_matched(&mut self, binder: BinderId, ty: Option<Ty>, borrow: Option<Primitive>) {
        if let Some(ty) = ty {
            self.bind(binder, borrow.map_or(ty, |b| TyKind::Primitive(b, vec![Value::Type(ty)]).intern()));
        }
    }

    /// The type each item of the tuple pattern `pat` matches in a tuple of type `ty` with the
    /// items `parts`: in order before a `..`, from the end after it, and for the `..` the tuple
    /// of those it stands for; a pattern with more items, or with other items and no `..`, is
    /// reported and matches none.
    fn tuple_items(&mut self, pat: PatId, ty: Ty, items: &[Entry<PatId>], parts: &[Ty]) -> Vec<Option<Ty>> {
        let hir = self.hir();
        let is_rest = |item: &Entry<PatId>| matches!(item, Entry::One(p) if matches!(hir.pats[*p].kind, PatKind::Rest(_)));
        let rest = items.iter().position(is_rest);
        let written = items.len() - usize::from(rest.is_some());
        if written > parts.len() || (rest.is_none() && written != parts.len()) {
            let msg = Msg::new()
                .text("this pattern has ")
                .num(u64::try_from(written).unwrap_or(u64::MAX))
                .text(" items, and the tuple of type ")
                .ty(ty.to_string())
                .text(" has ")
                .num(u64::try_from(parts.len()).unwrap_or(u64::MAX));
            self.diag.error(Diagnostic::error(errors::TYPES_MISMATCH, msg).at(hir.pats[pat].origin, None));
            return Vec::new();
        }
        let Some(rest) = rest else { return parts.iter().copied().map(Some).collect() };
        let after = items.len() - rest - 1;
        let skipped = TyKind::Tuple(parts[rest..parts.len() - after].to_vec()).intern();
        let (before, tail) = (&parts[..rest], &parts[parts.len() - after..]);
        before
            .iter()
            .copied()
            .map(Some)
            .chain([Some(skipped)])
            .chain(tail.iter().copied().map(Some))
            .collect()
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
            let mut holds = Vec::new();
            if let Some(guard) = arm.guard {
                self.logic(&[guard]);
                holds = self.facts(guard).holds;
            }
            let Some(value) = arm.value else { continue };
            let found = self.narrowing(&holds, |t| t.typed_as(value, expected));
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
        self.exhaustive(hir.exprs[expr].origin, matched, arms);
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

impl Typer<'_, '_, '_> {
    /// Reports each name an alternative binds with another type than the first alternative;
    /// names bound in one alternative only are the resolver's to report.
    fn alternatives(&mut self, alternatives: &[PatId]) {
        let hir = self.hir();
        for joined in hir.joined(alternatives) {
            let Joined::Paired { first, other } = joined else { continue };
            let (Some(expected), Some(found)) = (self.locals.get(&first), self.locals.get(&other)) else {
                continue;
            };
            if expected.ty != found.ty {
                let msg = Msg::new()
                    .code(hir.binders[other].name.as_str())
                    .text(" is ")
                    .ty(found.ty.to_string())
                    .text(" here but ")
                    .ty(expected.ty.to_string())
                    .text(" in the first alternative");
                let d = Diagnostic::error(errors::BINDING_UNEQUAL_ALTERNATIVES, msg);
                self.diag.error(d.at(hir.binders[other].origin, None));
            }
        }
    }
}
