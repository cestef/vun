//! Expressions of a type asked for, and the reads of locals and calls that give one.

use super::Draw;
use crate::int::Range;
use crate::ir::{Bound, Expr};
use crate::ty::{Nominal, Ty};

impl Draw<'_> {
    /// An expression whose type `want` accepts.
    pub(super) fn expr(&mut self, want: &Ty, depth: usize) -> Expr {
        self.spent += 1;
        if self.spent > self.config.nodes {
            return self.value_of(want);
        }
        if self.entropy.chance(1, 6)
            && let Some(local) = self.local_of(&|ty| want.accepts(ty))
        {
            return local;
        }
        let below = depth.saturating_sub(1);
        if depth > 0
            && self.entropy.chance(1, 6)
            && self.bounds.iter().any(|b| matches!(b, Bound::Call(..)))
            && let Some(called) = self.called(want, below)
        {
            return called;
        }
        if depth > 0
            && self.entropy.chance(1, 8)
            && let Some(matched) = self.matched(Some(want), below)
        {
            return matched;
        }
        if depth > 0
            && self.entropy.chance(1, 10)
            && let Some(closed) = self.closed(Some(want), below)
        {
            return closed;
        }
        if depth > 0
            && self.entropy.chance(1, 14)
            && let Some(propagated) = self.propagated(want, below)
        {
            return propagated;
        }
        if depth > 0
            && self.entropy.chance(1, 12)
            && let Some(looped) = self.forever(want, below)
        {
            return looped;
        }
        match want {
            Ty::Int(range) => {
                let made = self.int(depth);
                self.coerced(made, range)
            }
            Ty::Bool => self.bool(depth),
            Ty::Float(format) => self.float(*format, depth),
            Ty::Unit | Ty::Ref(..) | Ty::Closure(..) => Expr::unit(),
            Ty::Param(_) => self.parameter(want),
            Ty::Tuple(parts) => Expr::tuple(parts.iter().map(|p| self.expr(p, below)).collect()),
            Ty::Array(len, item) if self.entropy.chance(1, 3) => Expr::repeat(self.expr(item, below), *len),
            Ty::Struct(_, fields) => {
                let fields = fields.iter().map(|f| self.expr(f, below)).collect();
                Expr::structure(want, fields).expect("fields of the types a struct holds make it")
            }
            Ty::Enum(Nominal::Option, variants)
                if matches!(variants[1][0], Ty::Int(_)) && self.entropy.chance(1, 3) =>
            {
                let Ty::Int(to) = variants[1][0].clone() else { return self.int_literal() };
                let operand = self.int(below);
                Expr::try_to(operand, to).expect("an integer is tried")
            }
            Ty::Enum(_, variants) => {
                let variant = self.entropy.below(variants.len());
                let payload = variants[variant].iter().map(|p| self.expr(p, below)).collect();
                Expr::variant(want, variant, payload)
                    .expect("a payload of the types a variant holds makes it")
            }
            Ty::Array(len, item) => {
                let items: Vec<Expr> = (0..*len).map(|_| self.expr(item, below)).collect();
                Expr::array(items.clone()).unwrap_or_else(|e| {
                    panic!("{e:?} {want:?} {:?}", items.iter().map(|i| &i.ty).collect::<Vec<_>>())
                })
            }
        }
    }

    /// `operand?` of type `want`, where the function gives an option or a result to return from.
    fn propagated(&mut self, want: &Ty, depth: usize) -> Option<Expr> {
        if self.scope.inside_closure || want.has_param() || matches!(want, Ty::Ref(..) | Ty::Closure(..)) {
            return None;
        }
        let Some(Ty::Enum(nominal, given)) = self.result.clone() else { return None };
        let held = match nominal {
            Nominal::Option => Ty::option(want.clone()),
            Nominal::Result => Ty::Enum(Nominal::Result, vec![vec![want.clone()], given[1].clone()]),
            Nominal::Declared(_) => return None,
        };
        let operand = self.expr(&held, depth);
        let result = self.result.clone()?;
        Expr::propagate(operand, &result).ok()
    }

    /// A read of a parameter of the type `ty`, of which a generic function has at least one.
    fn parameter(&mut self, ty: &Ty) -> Expr {
        self.local_of(&|held| held == ty).expect("a generic function has a parameter of each type")
    }

    /// A read of a visible local whose type `keep` holds.
    pub(super) fn local_of(&mut self, keep: &dyn Fn(&Ty) -> bool) -> Option<Expr> {
        let found =
            self.scope.find(|local| keep(&local.ty) || matches!(&local.ty, Ty::Ref(_, inner) if keep(inner)));
        (!found.is_empty()).then(|| {
            let id = *self.entropy.pick(&found);
            let local = Expr::local(id, &self.scope.locals[id].ty);
            match &local.ty {
                Ty::Ref(..) => Expr::deref(local).expect("a reference refers"),
                _ => local,
            }
        })
    }

    /// A part of an aggregate local, of a field or an item, whose type `keep` holds.
    pub(super) fn part(&mut self, depth: usize, keep: &dyn Fn(&Ty) -> bool) -> Option<Expr> {
        let found = self.scope.find(|local| Self::holds(&local.ty, keep));
        if found.is_empty() {
            return None;
        }
        let id = *self.entropy.pick(&found);
        let whole = Expr::local(id, &self.scope.locals[id].ty);
        self.project(whole, depth, keep)
    }

    /// Whether a part of `ty`, at any depth, is one `keep` holds.
    fn holds(ty: &Ty, keep: &dyn Fn(&Ty) -> bool) -> bool {
        match ty {
            Ty::Tuple(parts) | Ty::Struct(_, parts) => parts.iter().any(|p| keep(p) || Self::holds(p, keep)),
            Ty::Array(_, item) => keep(item) || Self::holds(item, keep),
            _ => false,
        }
    }

    /// A part of `whole`, then of that, until one `keep` holds.
    fn project(&mut self, whole: Expr, depth: usize, keep: &dyn Fn(&Ty) -> bool) -> Option<Expr> {
        let part = match &whole.ty {
            Ty::Tuple(parts) | Ty::Struct(_, parts) => {
                let candidates: Vec<usize> =
                    (0..parts.len()).filter(|&i| keep(&parts[i]) || Self::holds(&parts[i], keep)).collect();
                let at = *self.entropy.pick(&candidates);
                Expr::field(whole, at).ok()?
            }
            Ty::Array(len, item) if keep(item) || Self::holds(item, keep) => {
                let index = self.int(depth);
                let index = self.coerced(index, &Range::new(0, len - 1));
                Expr::index(whole, index).ok()?
            }
            _ => return None,
        };
        if keep(&part.ty) && (!Self::holds(&part.ty, keep) || self.entropy.chance(2, 3)) {
            Some(part)
        } else {
            self.project(part, depth, keep)
        }
    }

    /// A value of `ty` made of no further expressions: what a function over its size is made of.
    fn value_of(&mut self, ty: &Ty) -> Expr {
        match ty {
            Ty::Int(range) => Expr::int(self.literal_in(range)),
            Ty::Bool => Expr::bool(self.entropy.chance(1, 2)),
            Ty::Param(_) => self.parameter(ty),
            Ty::Tuple(parts) => Expr::tuple(parts.iter().map(|p| self.value_of(p)).collect()),
            Ty::Array(len, item) => Expr::repeat(self.value_of(item), *len),
            Ty::Struct(_, fields) => {
                let fields = fields.iter().map(|f| self.value_of(f)).collect();
                Expr::structure(ty, fields).expect("fields of the types a struct holds make it")
            }
            Ty::Enum(_, variants) => {
                let variant = self.entropy.below(variants.len());
                let payload = variants[variant].iter().map(|p| self.value_of(p)).collect();
                Expr::variant(ty, variant, payload).expect("a payload of the types a variant holds makes it")
            }
            _ => Expr::sample(ty),
        }
    }
}
