//! Calls of the program's functions and of the closures passed to them, with the arguments
//! they take.

use super::Draw;
use crate::ir::{Binding, Bound, Expr, FnId, LocalId, Place, Role, Stmt};
use crate::ty::Ty;

impl Draw<'_> {
    /// A call of an earlier function whose result `keep` holds, if the program can afford it.
    pub(super) fn call(&mut self, depth: usize, keep: &dyn Fn(&Ty) -> bool) -> Option<Expr> {
        let affordable: Vec<FnId> = self
            .program
            .functions
            .iter()
            .filter(|(_, f)| f.role == Role::Plain)
            .filter(|(_, f)| f.params > 0 && (!f.generics.is_empty() || keep(&f.result)))
            .filter(|(_, f)| {
                self.scope.steps.saturating_add(f.steps.saturating_mul(self.scope.runs)) < self.config.steps
            })
            .map(|(id, _)| id)
            .collect();
        if affordable.is_empty() {
            return None;
        }
        let id = *self.entropy.pick(&affordable);
        self.call_of(id, depth, keep)
    }

    /// A call of the function `id`, if its result `keep` holds.
    pub(super) fn call_of(&mut self, id: FnId, depth: usize, keep: &dyn Fn(&Ty) -> bool) -> Option<Expr> {
        let function = &self.program.functions[id];
        let (params, result, steps) = (function.signature(), function.result.clone(), function.steps);
        let bounds = function.generics.clone();
        // A callable's type is that of the closure made for it, set where the argument is.
        let mut generics: Vec<Ty> = bounds
            .iter()
            .map(|bound| match bound {
                Bound::Any => self.type_argument(),
                Bound::Call(..) => Ty::Unit,
            })
            .collect();
        let result = result.subst(&generics);
        if !keep(&result) {
            return None;
        }
        let seen = self.scope.visible.len();
        let mut lent = Vec::new();
        let mut before = Vec::new();
        let lends = params.iter().any(|p| matches!(p, Ty::Ref(..)));
        let mut args = Vec::new();
        for param in &params {
            if let Ty::Param(at) = param
                && let Bound::Call(taken, given) = &bounds[*at]
            {
                let closure = self.callable(taken, given, depth)?;
                generics[*at] = closure.ty.clone();
                args.push(if lends {
                    let local = self.scope.bind(closure.ty.clone(), false);
                    let ty = closure.ty.clone();
                    before.push(Stmt::Let { local, init: closure, binding: Binding::Inferred });
                    Expr::local(local, &ty)
                } else {
                    closure
                });
                continue;
            }
            let param = param.subst(&generics);
            args.push(match &param {
                Ty::Ref(mutable, inner) => self.lend(*mutable, inner, depth, &mut lent, &mut before),
                _ if lends => {
                    // Read before anything is lent, as a borrowed local may not be read.
                    let init = self.expr(&param, depth);
                    let local = self.scope.bind(param.clone(), false);
                    before.push(Stmt::Let { local, init, binding: Binding::Annotated });
                    Expr::local(local, &param)
                }
                _ => self.expr(&param, depth),
            });
        }
        self.scope.steps = self.scope.steps.saturating_add(steps.saturating_mul(self.scope.runs));
        let call = Expr::call(id, generics, &params, &result, args).ok()?;
        self.scope.visible.truncate(seen);
        Some(if before.is_empty() { call } else { Expr::block(before, call) })
    }

    /// A call of the parameter of the function being made that is bound to give a type `want`
    /// accepts.
    pub(super) fn called(&mut self, want: &Ty, depth: usize) -> Option<Expr> {
        let callable: Vec<(usize, Vec<Ty>, Ty)> = self
            .bounds
            .iter()
            .enumerate()
            .filter_map(|(at, bound)| match bound {
                Bound::Call(taken, given) if want.accepts(given) => Some((at, taken.clone(), given.clone())),
                _ => None,
            })
            .collect();
        if callable.is_empty() {
            return None;
        }
        let (at, taken, given) = self.entropy.pick(&callable).clone();
        let held = self.scope.find(|local| local.ty == Ty::Param(at));
        let callee = Expr::local(*held.first()?, &Ty::Param(at));
        let args = taken.iter().map(|ty| self.expr(ty, depth)).collect();
        self.scope.step();
        Expr::call_bound(callee, &taken, &given, args).ok()
    }

    /// A borrow of a value of type `inner`, of a local that holds one or of one made for it.
    /// Nothing lent mutably is lent twice, nor lent shared besides.
    fn lend(
        &mut self,
        mutable: bool,
        inner: &Ty,
        depth: usize,
        lent: &mut Vec<(LocalId, bool)>,
        before: &mut Vec<Stmt>,
    ) -> Expr {
        let free = |id: &LocalId| !lent.iter().any(|(held, by_mut)| held == id && (*by_mut || mutable));
        let writable = self.writable();
        let held: Vec<(LocalId, bool)> = self
            .scope
            .visible
            .iter()
            .copied()
            .filter(free)
            .filter_map(|id| {
                let ty = &self.scope.locals[id].ty;
                match ty {
                    Ty::Ref(by_mut, held) if **held == *inner && (*by_mut || !mutable) => {
                        (!mutable || writable.contains(&(id, true))).then_some((id, true))
                    }
                    _ if ty == inner => (!mutable || writable.contains(&(id, false))).then_some((id, false)),
                    _ => None,
                }
            })
            .collect();
        let (local, deref) = if !held.is_empty() && self.entropy.chance(3, 4) {
            *self.entropy.pick(&held)
        } else {
            let init = self.expr(inner, depth);
            let local = self.scope.bind(inner.clone(), mutable);
            before.push(Stmt::Let { local, init, binding: Binding::Annotated });
            (local, false)
        };
        lent.push((local, mutable));
        let root = self.scope.locals[local].ty.clone();
        let (place, ty) = Place::new(local, &root, deref, Vec::new()).expect("a whole local is a place");
        Expr::borrow(mutable, place, ty)
    }
}

impl Draw<'_> {}
