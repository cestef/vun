//! Functions and the program of them.

use super::Draw;
use super::scope::Scope;
use crate::ir::{Bound, Expr, Function, Role, Stmt};
use crate::ty::Ty;

impl Draw<'_> {
    /// Every function of the program: helpers calling the helpers before them, then the roots
    /// calling any.
    pub(super) fn functions(&mut self) {
        for _ in 0..self.config.helpers {
            match self.entropy.below(6) {
                0 | 1 => {
                    self.generic();
                    continue;
                }
                2 => {
                    self.higher();
                    continue;
                }
                _ => {}
            }
            if self.entropy.chance(1, 8) {
                self.iterator();
                continue;
            }
            let params =
                (0..self.entropy.between(1, self.config.params.max(1))).map(|_| self.param()).collect();
            let result = self.ty(1);
            self.function(Vec::new(), params, result);
        }
        for _ in 0..self.config.roots {
            let result = self.ty(2);
            let root = self.function(Vec::new(), Vec::new(), result);
            self.program.roots.push(root);
        }
        let generic: Vec<_> =
            self.program.functions.iter().filter(|(_, f)| !f.generics.is_empty()).map(|(id, _)| id).collect();
        for id in generic {
            self.instance(id);
        }
    }

    /// The type of a parameter: a value, or a borrow of one.
    fn param(&mut self) -> Ty {
        let ty = self.ty(1);
        if self.entropy.chance(1, 3) { Ty::Ref(self.entropy.chance(1, 2), Box::new(ty)) } else { ty }
    }

    /// A function over some types: each is the type of a parameter, alone or in a part of one, and
    /// the result is made of them as well, so that all it can do with a value of one is pass it on.
    fn generic(&mut self) {
        let generics = self.entropy.between(1, 2);
        let bounds = vec![Bound::Any; generics];
        let mut params: Vec<Ty> = (0..generics).map(Ty::Param).collect();
        for _ in 0..self.entropy.between(0, 2) {
            let at = self.entropy.below(generics);
            params.push(self.shaped(at, generics));
        }
        for _ in 0..self.entropy.between(0, 2) {
            params.push(self.param());
        }
        let at = self.entropy.below(generics);
        let result = self.shaped(at, generics);
        self.function(bounds, params, result);
    }

    /// A function over a callable, which the call infers from the closure it is given: it takes
    /// and gives values of types of their own, and calls it where it needs one.
    fn higher(&mut self) {
        let taken = (0..self.entropy.between(0, 2)).map(|_| self.ty(1)).collect();
        let bound = Bound::Call(taken, self.ty(1));
        let mut params = vec![Ty::Param(0)];
        for _ in 0..self.entropy.between(0, 2) {
            params.push(self.param());
        }
        let result = self.ty(1);
        self.function(vec![bound], params, result);
    }

    /// A type made of the `at`th parameter, and perhaps another.
    fn shaped(&mut self, at: usize, generics: usize) -> Ty {
        let other = Ty::Param(self.entropy.below(generics));
        let this = Ty::Param(at);
        match self.entropy.below(5) {
            0 => Ty::Tuple(vec![this, other]),
            1 => Ty::Array(self.entropy.between(1, 3), Box::new(this)),
            2 => Ty::option(this),
            3 => Ty::Tuple(vec![this, self.ty(1)]),
            _ => this,
        }
    }

    /// A type to give a generic function for one of its parameters: one of the function being made.
    pub(super) fn type_argument(&mut self) -> Ty {
        let any: Vec<usize> = (0..self.bounds.len()).filter(|&at| self.bounds[at] == Bound::Any).collect();
        if !any.is_empty() && !self.scope.inside_closure && self.entropy.chance(1, 3) {
            Ty::Param(*self.entropy.pick(&any))
        } else {
            self.ty(1)
        }
    }

    /// A root that calls the generic function `id`, which nothing else would build.
    fn instance(&mut self, id: crate::ir::FnId) {
        self.scope = Scope::default();
        self.spent = 0;
        self.bounds.clear();
        self.result = None;
        let depth = self.config.depth;
        let Some(tail) = self.call_of(id, depth, &|_| true) else { return };
        let scope = std::mem::take(&mut self.scope);
        let root = self.program.functions.push(Function {
            locals: scope.locals,
            params: 0,
            generics: Vec::new(),
            role: Role::Plain,
            result: tail.ty.clone(),
            body: Vec::new(),
            tail,
            steps: scope.steps.saturating_add(1),
        });
        self.program.roots.push(root);
    }

    fn function(&mut self, generics: Vec<Bound>, params: Vec<Ty>, result: Ty) -> crate::ir::FnId {
        self.make(generics, params, result, Role::Plain)
    }

    /// An iterator function over some values, which yields values of a type of its own.
    fn iterator(&mut self) {
        let params = (0..self.entropy.between(0, 2)).map(|_| self.ty(1)).collect();
        let item = self.ty(1);
        self.make(Vec::new(), params, item, Role::Iterator);
    }

    fn make(&mut self, generics: Vec<Bound>, params: Vec<Ty>, result: Ty, role: Role) -> crate::ir::FnId {
        self.scope = Scope::default();
        self.spent = 0;
        self.bounds.clone_from(&generics);
        self.result = (role == Role::Plain).then(|| result.clone());
        self.scope.yields = (role == Role::Iterator).then(|| result.clone());
        let count = params.len();
        let received = params.clone();
        for ty in params {
            self.scope.bind(ty, false);
        }
        let mut body = self.block();
        let tail = match role {
            Role::Plain => self.finish(&result, &received),
            Role::Iterator => {
                body.insert(0, Stmt::Yield(self.expr(&result, self.config.depth)));
                Expr::unit()
            }
        };
        let scope = std::mem::take(&mut self.scope);
        self.program.functions.push(Function {
            locals: scope.locals,
            params: count,
            generics,
            role,
            result,
            body,
            tail,
            steps: scope.steps.saturating_add(1),
        })
    }

    /// The value a function ends with: a call it jumps to, or an expression.
    fn finish(&mut self, result: &Ty, received: &[Ty]) -> Expr {
        if self.entropy.chance(1, 5)
            && let Some(call) = self.tail_call(result, received)
        {
            return call;
        }
        self.expr(result, self.config.depth)
    }

    /// `lang.flow.tail(f, args)` of an earlier plain function that gives `result`, takes nothing
    /// it would have to borrow from here and nothing in memory the caller did not receive (spec
    /// 09: the arguments of a tail call reuse the memory the caller received).
    fn tail_call(&mut self, result: &Ty, received: &[Ty]) -> Option<Expr> {
        let callees: Vec<crate::ir::FnId> = self
            .program
            .functions
            .iter()
            .filter(|(_, f)| f.role == Role::Plain && f.generics.is_empty() && f.result == *result)
            .filter(|(_, f)| f.signature().iter().all(|p| !p.holds_borrow() && !matches!(p, Ty::Closure(..))))
            .filter(|(_, f)| Ty::fit_received(&f.signature(), received))
            .filter(|(_, f)| self.scope.steps.saturating_add(f.steps) < self.config.steps)
            .map(|(id, _)| id)
            .collect();
        if callees.is_empty() {
            return None;
        }
        let id = *self.entropy.pick(&callees);
        let (params, steps) = (self.program.functions[id].signature(), self.program.functions[id].steps);
        let args = params.iter().map(|p| self.expr(p, self.config.depth)).collect();
        self.scope.steps = self.scope.steps.saturating_add(steps);
        Expr::tail_call(id, &params, result, args).ok()
    }
}
