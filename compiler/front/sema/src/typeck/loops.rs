//! Loops (spec 01, section 6): `for { }`, `for cond { }`, `for p = e { }` and `for p in e { }`.
//! `break v` gives a loop its value; a loop that ends by its head has `()` besides. `for p in e`
//! binds `p` to each item, found through the table `iteration` (appendix B): the witness of
//! `lang.ops.Iterate` for the type of `e` names the coroutine interface of its iterator, whose
//! `resume` result carries each item in its variant `yielded`.

use vun_desugar::ITERATION;
use vun_hir::tree::{BlockId, Cond, ExprId, ExprKind, ForHead};
use vun_intern::Symbol;

use super::{Known, Stage, Typer};
use crate::eval::{Call, Instance};
use crate::member::{Found, MemberType, MemberValue, Members};
use crate::ty::{Ty, TyKind, Value};
use crate::witness::Witnesses;

/// A loop being typed: its label, and the join of the values it ends with so far.
pub(super) struct Loop {
    label: Option<Symbol>,
    value: Ends,
}

/// The values a loop ends with so far.
enum Ends {
    /// None yet: a loop without a head that no `break` leaves never ends.
    Never,
    With(Known),
    /// Values that disagree, or one not typed.
    Unknown,
}

impl Typer<'_, '_, '_> {
    /// `label: for head { body }`, its label `None` when it has none.
    pub(super) fn looping(&mut self, label: Option<Symbol>, head: &ForHead, body: BlockId) -> Option<Known> {
        let ends = Ends::With(Known { ty: TyKind::unit(), stage: Stage::Compile });
        let value = match head {
            ForHead::Forever => Ends::Never,
            ForHead::While(Cond::Expr(cond)) => {
                self.logic(&[*cond]);
                ends
            }
            ForHead::While(Cond::Test { pat, value }) => {
                let matched = self.typed(*value);
                self.test(*pat, matched.map(|k| k.ty));
                ends
            }
            ForHead::In { pat, iter } => {
                let driven = self.typed(*iter).and_then(|k| self.iteration(k.ty));
                if let Some((iteration, _)) = driven {
                    self.iterations.insert(*iter, iteration);
                }
                self.pattern(*pat, driven.map(|(_, item)| item), None);
                ends
            }
        };
        self.loops.push(Loop { label, value });
        self.block_value(body, None);
        match self.loops.pop()?.value {
            Ends::With(known) => Some(known),
            Ends::Never | Ends::Unknown => None,
        }
    }

    /// `label: body`, a labeled loop.
    pub(super) fn labeled(&mut self, expr: ExprId, label: Symbol, body: ExprId) -> Option<Known> {
        let hir = self.hir();
        let ExprKind::For { head, body: block } = &hir.exprs[body].kind else {
            vun_hir::visit::walk_expr(self, hir, expr);
            return None;
        };
        let known = self.looping(Some(label), head, *block);
        if let Some(known) = known {
            self.types.insert(body, known);
        }
        known
    }

    /// `break label value`: the value joins those of the loop it leaves.
    pub(super) fn leave(&mut self, at: ExprId, label: Option<Symbol>, value: Option<ExprId>) {
        let found = match value {
            Some(value) => self.typed(value),
            None => Some(Known { ty: TyKind::unit(), stage: Stage::Compile }),
        };
        let target = self.loops.iter().rposition(|l| label.is_none() || l.label == label);
        let Some(target) = target else { return };
        let sofar = std::mem::replace(&mut self.loops[target].value, Ends::Unknown);
        let joined = match (sofar, found) {
            (Ends::Never, Some(found)) => Some(found),
            (Ends::With(sofar), Some(found)) => self.join(value.unwrap_or(at), sofar, found),
            _ => None,
        };
        self.loops[target].value = joined.map_or(Ends::Unknown, Ends::With);
    }

    /// The witness of the interface `item`, such as `lang.ops.Iterate`, for `ty`.
    pub(super) fn witness_of(&self, item: vun_items::Item, ty: Ty) -> Option<Ty> {
        let Ok(Value::Interface(interface)) = self.env().item(item) else { return None };
        let witnesses = self.env().cx.get::<Witnesses>(&(self.env().file.clone(), ty));
        witnesses
            .iter()
            .find(|w| w.decl.contract.as_ref().is_some_and(|c| c.interface == interface))
            .map(|w| w.ty)
    }

    /// How `for p in e` drives `e` of type `ty`, and the type of the items it binds, looking
    /// through a borrow when the borrow itself is not iterated.
    pub(super) fn iteration(&self, ty: Ty) -> Option<(Iteration, Ty)> {
        let &(interface, _, iterator, resume, ..) = ITERATION.first()?;
        let &(.., item, _) = ITERATION.first()?;
        let witness =
            self.witness_of(interface, ty).or_else(|| self.witness_of(interface, ty.borrowed()?))?;
        let member = (witness, Symbol::intern(iterator));
        let Ok(Value::Interface(coroutine)) = self.env().cx.get::<MemberValue>(&member) else { return None };
        let requirements = self
            .env()
            .cx
            .get::<Call>(&Instance { f: coroutine, args: vec![Value::Type(ty)] })
            .ok()?
            .as_type()?;
        let resumed = self.env().cx.get::<MemberType>(&(requirements, Symbol::intern(resume))).ok()?;
        let step = resumed.ty.signature()?.result;
        let item = match self.env().cx.get::<Members>(&step)?.find(Symbol::intern(item)) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied()?,
            _ => return None,
        };
        Some((Iteration { witness, step }, item))
    }
}

/// How `for p in e` drives `e` (the table `iteration`): the witness of `lang.ops.Iterate` whose
/// `iter` makes the iterator, and the type its `resume` gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Iteration {
    pub witness: Ty,
    pub step: Ty,
}
