//! Patterns and `match` (spec 01, section 6): a pattern is checked against the type it matches,
//! and gives its binders the types of the parts they bind. Through a borrow, binders borrow the
//! parts in the same mode (decision 31). A `match` has the type all its arms can take.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::{Arm, BinderId, Entry, ExprId, Joined, Name, PatId, PatKind};
use vun_primitives::Primitive;
use vun_sema_ty::{Ty, TyKind, Value};

use super::{GaveUp, Known, Operation, Typer};
use crate::member::{Found, Members};

impl Typer<'_, '_, '_> {
    /// Checks `pat` against values of type `ty`, if known, or gives its binders why it is not;
    /// `borrow`, when matching through a borrow, is the borrow type its binders take.
    pub(super) fn pattern(&mut self, pat: PatId, ty: Result<Ty, GaveUp>, borrow: Option<Primitive>) {
        let hir = self.hir();
        let origin = hir.pats[pat].origin;
        match &hir.pats[pat].kind {
            // A rest binds what its tuple pattern gives it, the items it stands for.
            PatKind::Bind(binder) | PatKind::Rest(Some(binder)) => self.bind_matched(*binder, ty, borrow),
            PatKind::At { binder, pat } => {
                self.bind_matched(*binder, ty.clone(), borrow);
                self.pattern(*pat, ty, borrow);
            }
            PatKind::Tuple(items) => {
                let items_of = ty.and_then(|t| match &*t {
                    TyKind::Tuple(parts) => Ok(self.tuple_items(pat, t, items, parts)),
                    _ => Err(GaveUp::Form),
                });
                for (i, item) in items.iter().enumerate() {
                    if let Entry::One(item) = item {
                        // A pattern of more items than the tuple's is reported.
                        let matched = items_of.as_ref().map_err(Clone::clone);
                        let matched =
                            matched.and_then(|p| p.get(i).copied().flatten().ok_or(GaveUp::Reported));
                        self.pattern(*item, matched, borrow);
                    }
                }
            }
            PatKind::Variant { name, args } => {
                let payload = match (name, ty) {
                    (Name::Plain(name), Ok(ty)) => {
                        match self.env().cx.get::<Members>(&ty).map(|m| m.find(*name).into()) {
                            Some(Lookup::Variant(payload)) => Ok(payload),
                            Some(Lookup::Absent) => {
                                self.no_member(origin, ty, *name);
                                Err(GaveUp::Reported)
                            }
                            _ => Err(GaveUp::Members(ty)),
                        }
                    }
                    (_, Err(why)) => Err(why),
                    _ => Err(GaveUp::Form),
                };
                for (i, arg) in args.iter().flatten().enumerate() {
                    if let Entry::One(arg) = arg {
                        let item = payload.as_ref().map_err(Clone::clone);
                        let item = item.and_then(|p| p.get(i).cloned().unwrap_or(Err(GaveUp::Reported)));
                        self.pattern(*arg, item, borrow);
                    }
                }
            }
            PatKind::Or(alternatives) => {
                for &alternative in alternatives {
                    self.pattern(alternative, ty.clone(), borrow);
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
                let ty = ty.ok();
                self.against(*value, ty);
                if let Some(ty) = ty {
                    self.tested(Operation::Test(pat), TokenKind::EqEq, *value, ty);
                }
            }
            PatKind::Range { lo, hi, inclusive } => {
                let ty = ty.ok();
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
            PatKind::Struct { ty: written, fields } => self.struct_pattern(*written, fields, ty, borrow),
            PatKind::Hole | PatKind::Rest(None) | PatKind::Error => {}
        }
    }

    /// A struct pattern, of the type written or else `ty`: each field's pattern against that
    /// field's type, named or by position.
    fn struct_pattern(
        &mut self,
        written: Option<ExprId>,
        fields: &[Entry<(Option<Name>, PatId)>],
        ty: Result<Ty, GaveUp>,
        borrow: Option<Primitive>,
    ) {
        let hir = self.hir();
        let ty = written.and_then(|t| self.ty(t)).map_or(ty, Ok);
        let members = ty.clone().and_then(|t| self.env().cx.get::<Members>(&t).ok_or(GaveUp::Members(t)));
        for (i, field) in fields.iter().enumerate() {
            let Entry::One((name, field)) = field else { continue };
            let found = members.as_ref().map_err(Clone::clone).and_then(|m| match name {
                Some(Name::Plain(name)) => {
                    if let Found::Field(f) = m.find(*name) {
                        f.ty.clone().map_err(GaveUp::Unevaluated)
                    } else {
                        self.no_member(hir.pats[*field].origin, ty.clone()?, *name);
                        Err(GaveUp::Reported)
                    }
                }
                None => m
                    .fields
                    .get(i)
                    .map_or(Err(GaveUp::Reported), |f| f.ty.clone().map_err(GaveUp::Unevaluated)),
                Some(_) => Err(GaveUp::Form),
            });
            self.pattern(*field, found, borrow);
        }
    }

    /// Gives `binder` the type of the values of `ty` it binds, or a borrow of them when matching
    /// through the borrow `borrow`.
    fn bind_matched(&mut self, binder: BinderId, ty: Result<Ty, GaveUp>, borrow: Option<Primitive>) {
        match ty {
            Ok(ty) => {
                self.bind(
                    binder,
                    borrow.map_or(ty, |b| TyKind::Primitive(b, vec![Value::Type(ty)]).intern()),
                );
            }
            Err(why) => {
                self.locals.insert(binder, Err(why));
            }
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
            let count = |n: usize| i128::try_from(n).unwrap_or(i128::MAX);
            let msg = Msg::new()
                .text("this pattern has ")
                .counted(count(written), "item", "items")
                .text(", and the tuple of type ")
                .ty(ty.to_string())
                .text(" has ")
                .num(count(parts.len()));
            let help = if written > parts.len() {
                Msg::new().text("remove the items the tuple does not have")
            } else {
                Msg::new()
                    .text("match each item, or skip those left with ")
                    .code(TokenKind::DotDot.text().unwrap_or_default())
            };
            let d = Diagnostic::error(errors::TYPES_MISMATCH, msg).at(hir.pats[pat].origin, None).help(help);
            self.diag.error(d);
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
    pub(super) fn test(&mut self, pat: PatId, matched: Result<Ty, GaveUp>) {
        match matched.as_ref().ok().and_then(|t| t.borrow()) {
            Some((borrow, inner)) => self.pattern(pat, Ok(inner), Some(borrow)),
            None => self.pattern(pat, matched, None),
        }
    }

    /// A value in a pattern, checked against the type matched. An integer is an operand of the
    /// comparison the pattern stands for, as in an expression, so it need not fit the type.
    fn against(&mut self, value: ExprId, ty: Option<Ty>) {
        let found = self.typed_as(value, ty);
        if let (Some(found), Some(ty)) = (found, ty)
            && (found.ty.interval().is_none() || ty.interval().is_none())
        {
            self.expect(self.hir().exprs[value].origin, found, ty, None);
        }
    }

    /// `match scrutinee { arms }`: each arm's pattern against the scrutinee, its guard as a
    /// condition, and its value where the match's own value is expected.
    pub(super) fn matching(&mut self, expr: ExprId, scrutinee: ExprId, arms: &[Entry<Arm>]) -> Option<Known> {
        let hir = self.hir();
        let known = self.typed(scrutinee);
        let (bound, matched) = (self.matched(scrutinee, known), known.map(|k| k.ty));
        let expected = self.expected(expr);
        let mut joined: Option<Option<Known>> = None;
        for arm in arms {
            let Entry::One(arm) = arm else { continue };
            self.test(arm.pat, bound.clone());
            let mut holds = Vec::new();
            if let Some(guard) = arm.guard {
                self.logic(&[guard]);
                holds = self.facts(guard).holds;
            }
            let Some(value) = arm.value else { continue };
            let arm_expected = self.expected_of(expr, value, Some);
            let found = self.narrowing(&holds, |t| t.typed_as(value, arm_expected));
            let found = self.fits(value, found, expected);
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
    /// A variant, with its payload's types, or why each is not known.
    Variant(Vec<Result<Ty, GaveUp>>),
    Absent,
    Other,
}

impl From<Found<'_>> for Lookup {
    fn from(found: Found<'_>) -> Self {
        match found {
            Found::Variant(v) => {
                Self::Variant(v.payload.iter().map(|p| p.clone().map_err(GaveUp::Unevaluated)).collect())
            }
            Found::Absent | Found::Field(_) | Found::Decl(_) | Found::Spread(_) => Self::Absent,
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
            let (Some(Ok(expected)), Some(Ok(found))) = (self.locals.get(&first), self.locals.get(&other))
            else {
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
                let name = hir.binders[other].name;
                let first_ty =
                    Msg::new().code(name.as_str()).text(" is ").ty(expected.ty.to_string()).text(" here");
                let help = Msg::new()
                    .text("bind ")
                    .code(name.as_str())
                    .text(" to values of one type in every alternative");
                let d = Diagnostic::error(errors::BINDING_UNEQUAL_ALTERNATIVES, msg)
                    .at(hir.binders[other].origin, None)
                    .label(hir.binders[first].origin, Some(first_ty))
                    .help(help);
                self.diag.error(d);
            }
        }
    }
}
