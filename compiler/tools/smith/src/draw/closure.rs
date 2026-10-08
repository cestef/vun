//! Closures: written, bound and called in one block, so that what they borrow is not used
//! besides (spec 01, section 4).

use super::Draw;
use crate::ir::{Binding, Capture, Capturing, Closure, Expr, Local, LocalId, Stmt};
use crate::ty::Ty;

impl Draw<'_> {
    /// `{ arguments; mut c := [captures] (params) { body }; c(arguments) }`, of type `want`, or of
    /// an integer where none is asked for.
    pub(super) fn closed(&mut self, want: Option<&Ty>, depth: usize) -> Option<Expr> {
        // A closure sees only what it captures, and a type parameter's values are the arguments.
        if self.scope.inside_closure || want.is_some_and(Ty::has_param) {
            return None;
        }
        let below = depth.saturating_sub(1);
        let seen = self.scope.visible.len();
        let mut block = Vec::new();
        let param_tys: Vec<Ty> = (0..self.entropy.between(0, 2)).map(|_| self.ty(1)).collect();
        // The arguments are read first, as what the closure borrows may not be read after.
        let mut args = Vec::new();
        for ty in &param_tys {
            let init = self.expr(ty, below);
            let local = self.scope.bind(ty.clone(), false);
            block.push(Stmt::Let { local, init, binding: Binding::Annotated });
            args.push(Expr::local(local, ty));
        }
        let captures = self.captures(true);
        let params: Vec<LocalId> = param_tys
            .iter()
            .map(|ty| self.scope.locals.push(Local { ty: ty.clone(), mutable: false }))
            .collect();
        let body = self.closure_body(&captures, &params, want, below);
        // A function written beside no call that says its result gives `()`, so it is written.
        let result = Some(match want {
            Some(ty) => ty.clone(),
            None => self.wider(&body.ty),
        });
        let id = self.closures;
        self.closures += 1;
        let made = Expr::closure(Closure { id, captures, params, result, body }, param_tys);
        let local = self.scope.locals.push(Local { ty: made.ty.clone(), mutable: true });
        block.push(Stmt::Let { local, init: made.clone(), binding: Binding::Inferred });
        let call = Expr::call_closure(Expr::local(local, &made.ty), args).ok()?;
        self.scope.visible.truncate(seen);
        Some(Expr::block(block, call))
    }

    /// A closure written as an argument, taking `taken` and giving `result`, that holds copies.
    pub(super) fn callable(&mut self, taken: &[Ty], result: &Ty, depth: usize) -> Option<Expr> {
        if self.scope.inside_closure {
            return None;
        }
        let captures = self.captures(false);
        let params: Vec<LocalId> =
            taken.iter().map(|ty| self.scope.locals.push(Local { ty: ty.clone(), mutable: false })).collect();
        let body = self.closure_body(&captures, &params, Some(result), depth.saturating_sub(1));
        let id = self.closures;
        self.closures += 1;
        let closure = Closure { id, captures, params, result: Some(result.clone()), body };
        Some(Expr::closure(closure, taken.to_vec()))
    }

    /// What the closure holds of the locals around it: a few, each a copy, or a borrow of the
    /// local, which a borrow may be only where it can be written.
    fn captures(&mut self, borrow: bool) -> Vec<Capture> {
        let writable: Vec<LocalId> =
            self.writable().into_iter().filter(|(_, deref)| !deref).map(|(id, _)| id).collect();
        let held: Vec<LocalId> = self.scope.find(|local| !local.ty.holds_borrow());
        let mut captures: Vec<Capture> = Vec::new();
        for _ in 0..self.entropy.between(0, 3) {
            if held.is_empty() {
                break;
            }
            let local = *self.entropy.pick(&held);
            if captures.iter().any(|c| c.local == local) {
                continue;
            }
            let how = match if borrow { self.entropy.below(3) } else { 0 } {
                0 => Capturing::Copy,
                1 => Capturing::Shared,
                _ if writable.contains(&local) => Capturing::Mut,
                _ => Capturing::Copy,
            };
            captures.push(Capture { local, how });
        }
        captures
    }

    /// The body: what it may read and write is what it captures and takes.
    fn closure_body(
        &mut self,
        captures: &[Capture],
        params: &[LocalId],
        want: Option<&Ty>,
        depth: usize,
    ) -> Expr {
        let outer = std::mem::take(&mut self.scope.visible);
        let frozen = self.scope.frozen.len();
        let loops = std::mem::take(&mut self.scope.loops);
        let labels = std::mem::take(&mut self.scope.labels);
        let exits = std::mem::take(&mut self.scope.exits);
        let yields = self.scope.yields.take();
        self.scope.inside_closure = true;
        self.scope.visible = captures.iter().map(|c| c.local).chain(params.iter().copied()).collect();
        self.scope.frozen.extend(captures.iter().filter(|c| c.how != Capturing::Mut).map(|c| c.local));
        let stmts = self.block();
        let tail = match want {
            Some(ty) => self.expr(ty, depth),
            None => self.int(depth),
        };
        self.scope.inside_closure = false;
        self.scope.loops = loops;
        self.scope.labels = labels;
        self.scope.exits = exits;
        self.scope.yields = yields;
        self.scope.frozen.truncate(frozen);
        self.scope.visible = outer;
        if stmts.is_empty() { tail } else { Expr::block(stmts, tail) }
    }
}
