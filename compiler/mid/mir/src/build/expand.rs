//! Calls with code arguments expand at the call site (spec 03, section 4): the callee's body,
//! instantiated with the code, is built into the caller's, and each `$c` splicing code that
//! reads what only the caller holds is built here, where the code is written.

use vun_hir::tree::ExprId;
use vun_index::{Idx, IndexVec};
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::comptime::Comptime;
use vun_sema::attributes::Attributed;
use vun_sema::callable::Parameters;
use vun_sema::env::Passed;
use vun_sema_ty::Value;
use vun_sema_ty::call::{Code, Func, Instantiation};
use vun_sema_ty::trap::Unevaluated;

use super::{Builder, Unbuilt};
use crate::concrete::Lowered;
use crate::place::Place;
use crate::remap::Remap;
use crate::{Aggregate, Body, Local, Mir, Operand, Rvalue, TerminatorKind};

impl Builder<'_, '_, '_> {
    /// `$c` at run time for the code `c` (spec 03, section 4): its value at compile time where it
    /// has one, and otherwise the code, a splice that the call site expanding this body builds
    /// where it is written, and runs when it does. A trap the code fires when computed at
    /// compile time is the exception: it is reported here, as the evaluation stopped. A function
    /// marked `#[comptime.deferred]` always splices the code.
    pub(super) fn spliced(&mut self, expr: ExprId, code: Code, origin: OriginId) -> Operand {
        if self.defers() {
            return self.temp(Rvalue::Splice { code }, origin);
        }
        match self.env().eval(expr) {
            Ok(value) => Operand::Const(value),
            Err(Unevaluated::Trapped(_)) => self.temp(self.comptime(expr, None), origin),
            Err(_) => self.temp(Rvalue::Splice { code }, origin),
        }
    }

    /// Whether the function built splices its code at run time only (`#[comptime.deferred]`).
    fn defers(&self) -> bool {
        let deferred = Primitive::Comptime(Comptime::deferred);
        let function = self.env().instance.as_ref().map(|i| &i.f);
        function.and_then(|f| f.attributes(self.env.cx)).is_some_and(|a| a.get(deferred).is_some())
    }

    /// The body a call of `f` passing `passing` expands into, every `$` argument known here:
    /// `None` when it has none to expand, such as a coroutine's, which the call constructs a
    /// frame of.
    pub(super) fn expansion(&self, f: &Func, passing: &[Passed]) -> Option<Body> {
        let cx = self.env.cx;
        let comptime = f.comptime(cx)?;
        let args = passing.iter().zip(&comptime).map(|(passed, &c)| match passed {
            _ if !c => Some(None),
            Passed::Inferred(value) => Some(Some(value.clone())),
            Passed::Code(expr) => Some(Some(Value::Code(self.env().code(*expr)))),
            Passed::Known { expr, ty } => self.env().eval_as(*expr, *ty).ok().map(Some),
            Passed::Receiver | Passed::Run { .. } | Passed::Pack { .. } => None,
        });
        let i = Instantiation::new(f.clone(), args.collect::<Option<_>>()?);
        let mut body = Body::clone(&*cx.get::<Mir>(&i).ok()?);
        body.evaluate(&i.env(cx)?).ok()?;
        // A closure's literal is evaluated in the callee's call, which the caller is not.
        let closes = body
            .blocks
            .iter()
            .flat_map(|(_, b)| &b.stmts)
            .any(|s| matches!(s.value, Rvalue::Aggregate { kind: Aggregate::Closure { .. }, .. }));
        (!body.suspends() && !closes).then_some(body)
    }

    /// Builds `body` here with its parameters bound to `args`: its value.
    pub(super) fn expand(
        &mut self,
        body: Body,
        args: Vec<Operand>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let locals: Vec<Local> = body.locals.iter().map(|(_, decl)| self.locals.push(decl.clone())).collect();
        let blocks: Vec<_> = body.blocks.iter().map(|(_, b)| self.fresh(b.term.origin)).collect();
        let remap = Remap { locals: IndexVec::from(locals), blocks: IndexVec::from(blocks) };
        for (&param, arg) in body.params.iter().zip(args) {
            self.assign(Place::of(remap.locals[param]), Rvalue::Use(arg), origin);
        }
        self.terminate(TerminatorKind::Goto(remap.blocks[Idx::new(0)]), origin);
        let after = self.fresh(origin);
        let ret = remap.locals[body.ret];
        for (id, block) in body.blocks.into_vec().into_iter().enumerate() {
            self.current = remap.blocks[Idx::new(id)];
            for mut stmt in block.stmts {
                remap.stmt(&mut stmt);
                match stmt.value {
                    Rvalue::Splice { code } if self.writes(&code) => {
                        let value = self.operand(code.expr.expr.id)?;
                        self.assign(stmt.place, Rvalue::Use(value), stmt.origin);
                    }
                    value => self.assign(stmt.place, value, stmt.origin),
                }
            }
            let mut term = block.term;
            remap.terminator(&mut term.kind);
            if term.kind == TerminatorKind::Return {
                term.kind = TerminatorKind::Goto(after);
            }
            self.blocks[self.current].term = term;
        }
        self.current = after;
        Ok(Operand::Place(Place::of(ret)))
    }

    /// Whether `code` is written here, under the bindings lowered: code this body builds.
    fn writes(&self, code: &Code) -> bool {
        code.expr.file == self.env.file && self.env().code(code.expr.expr.id) == *code
    }
}
