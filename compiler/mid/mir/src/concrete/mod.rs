//! Instantiations ready for code generation (spec 13, section 3): the MIR of a function with its
//! `$` parameters bound, each compile-time value it reads evaluated into a constant that flows
//! into the operands reading it, each operation the program's checks check running as its
//! checked version (section 8.7), each block marking its coverage when the program is covered
//! (spec 11, section 6), and every local typed.

use std::sync::Arc;

use vun_index::IndexVec;
use vun_primitives::Primitive;
use vun_primitives::lower::Lower;
use vun_query::{Cx, Cycle, Query};
use vun_sema::attributes::Attributed;
use vun_sema::callable::Callable;
use vun_sema::callable::Parameters;
use vun_sema::checks::ChecksOf;
use vun_sema::cover::CoverageOf;
use vun_sema::env::Env;
use vun_sema::target::TargetOf;
use vun_sema::typeck::Typed;
use vun_sema_ty::Value;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::trap::Unevaluated;

mod covered;
mod unready;

pub use unready::Unready;

use crate::{Aggregate, Body, Local, Mir, Operand, Rvalue, Unbuilt};

/// An instantiation's body as code generation reads it; see the module.
pub struct Concrete;

impl Query for Concrete {
    const NAME: &'static str = "concrete";
    type Key = Instantiation;
    type Value = Result<Arc<Body>, Unready>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        Self::ready(cx, i).map_err(|unready| unready.explained(cx, i))
    }

    fn cycle(_: &Cx<'_>, i: &Instantiation, _: &Cycle) -> Self::Value {
        Err(Unready::Unevaluated { at: i.f.literal.expr.origin, why: Unevaluated::Cycle })
    }
}

impl Concrete {
    fn ready(cx: &Cx<'_>, i: &Instantiation) -> Result<Arc<Body>, Unready> {
        let mut body = Body::clone(&*cx.get::<Mir>(i)?);
        // Only a call site expanding the body builds the code it splices.
        let splice =
            body.blocks.iter().flat_map(|(_, b)| &b.stmts).find(|s| matches!(s.value, Rvalue::Splice { .. }));
        if let Some(stmt) = splice {
            return Err(Unready::Unbuilt(Unbuilt::at(stmt.origin)));
        }
        let env = i
            .env(cx)
            .ok_or(Unready::Unevaluated { at: i.f.literal.expr.origin, why: Unevaluated::Absent })?;
        body.evaluate(&env)?;
        body.compile_time_arguments(&env)?;
        body.propagate(&env, &i.args);
        body.checked(&env, &cx.get::<ChecksOf>(&()), &i.f);
        body.covered(&env, cx.get::<CoverageOf>(&()), i);
        body.typed()?;
        body.verified();
        Ok(Arc::new(body))
    }
}

/// What lowering an instantiation's body needs of it.
pub trait Lowered {
    /// The environment its body is lowered in: its `$` parameters bound, its types those typing
    /// found.
    fn env<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Env<'a, 'db>>;

    /// What a call of `f` with `args` runs: `f`, bound to the constants passed for its `$`
    /// parameters.
    fn called(cx: &Cx<'_>, f: &Func, args: &[Operand]) -> Self;
}

impl Lowered for Instantiation {
    fn env<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Env<'a, 'db>> {
        Some(
            Env::inside(cx, &self.f, self.args.iter().cloned())?
                .typed_by(cx.get::<Typed>(self).map(|t| Arc::clone(&t.found))),
        )
    }

    fn called(cx: &Cx<'_>, f: &Func, args: &[Operand]) -> Self {
        let comptime = f.comptime(cx).unwrap_or_default();
        let bound = comptime.iter().zip(args).map(|(&c, arg)| match arg {
            Operand::Const(value) if c => Some(value.clone()),
            _ => None,
        });
        Self::new(f.clone(), bound.collect())
    }
}

impl Body {
    /// Evaluates each compile-time value in `env`: a `Comptime` value becomes a constant, a
    /// quote the code it holds.
    pub(crate) fn evaluate(&mut self, env: &Env<'_, '_>) -> Result<(), Unready> {
        for (_, block) in self.blocks.iter_mut() {
            for stmt in &mut block.stmts {
                let Some(value) = stmt.value.compile_time(env) else { continue };
                let value = value.map_err(|why| Unready::Unevaluated { at: stmt.origin, why })?;
                stmt.value = Rvalue::Use(Operand::Const(value));
            }
        }
        Ok(())
    }

    /// Whether every `$` argument of a call is known here: one reading a local the body computes
    /// when it runs is passed only to a compile-time call, which binds it, never to a call in
    /// code generated.
    fn compile_time_arguments(&self, env: &Env<'_, '_>) -> Result<(), Unready> {
        let constants = self.constants();
        let unknown = |operand: &Operand| match operand {
            Operand::Const(_) => false,
            Operand::Place(place) => {
                self.locals[place.local].name.is_some() || constants[place.local].is_none()
            }
        };
        for stmt in self.blocks.iter().flat_map(|(_, b)| &b.stmts) {
            let Rvalue::Call { callee: Operand::Const(callee), args } = &stmt.value else { continue };
            let Some(comptime) = Callable::of(callee.clone()).and_then(|c| c.comptime(env)) else { continue };
            if comptime.iter().zip(args).any(|(&c, arg)| c && unknown(arg)) {
                return Err(Unready::Unevaluated { at: stmt.origin, why: Unevaluated::Runtime });
            }
        }
        Ok(())
    }

    /// Replaces each read of a local holding a constant by that constant: a `$` parameter bound
    /// to `bound`'s value, or one of [`Body::constants`]; and folds the calls known when compiling; until nothing changes. A constant written
    /// into a local nothing reads, and no static, is dropped.
    fn propagate(&mut self, env: &Env<'_, '_>, bound: &[Option<Value>]) {
        loop {
            let mut constants = self.constants();
            for (&param, value) in self.params.iter().zip(bound) {
                constants[param].clone_from(value);
            }
            let constant = |local: Local| constants[local].clone();
            let mut changed = false;
            for (_, block) in self.blocks.iter_mut() {
                let operands = block.stmts.iter_mut().flat_map(|s| s.value.operands_mut());
                for operand in operands.chain(block.term.kind.operands_mut()) {
                    if let Operand::Place(place) = operand
                        && place.projection.is_empty()
                        && let Some(value) = constant(place.local)
                    {
                        *operand = Operand::Const(value);
                        changed = true;
                    }
                }
            }
            if !self.fold(env) && !changed {
                break;
            }
        }
        let read = self.read();
        for (_, block) in self.blocks.iter_mut() {
            block.stmts.retain(|s| {
                let unread = s.place.projection.is_empty()
                    && !read.contains(&s.place.local)
                    && !self.locals[s.place.local].role.is_static();
                !unread || !matches!(s.value, Rvalue::Use(Operand::Const(_)))
            });
        }
    }

    /// Rewrites the calls known when compiling: a variant's constructor builds the variant, a
    /// primitive whose parameters are all `$` ones, given constants, is its value, which nothing
    /// at run time changes, as is the integer a function of `$` parameters alone gives, and a
    /// primitive marked `lower.by(f)` is a call of `f`. Gives whether any changed.
    fn fold(&mut self, env: &Env<'_, '_>) -> bool {
        let mut changed = false;
        for (_, block) in self.blocks.iter_mut() {
            for stmt in &mut block.stmts {
                let Rvalue::Call { callee: Operand::Const(callee), args } = &stmt.value else { continue };
                let folded = match callee {
                    Value::Constructor { ty, name } => Some(Rvalue::Aggregate {
                        kind: Aggregate::Variant { ty: *ty, name: *name },
                        parts: args.clone(),
                    }),
                    Value::Primitive(p) => {
                        Self::known(env, callee, args).map(|v| Rvalue::Use(Operand::Const(v))).or_else(|| {
                            let f = Self::lowered_by(env, *p, args)?;
                            Some(Rvalue::Call { callee: Operand::Const(f), args: args.clone() })
                        })
                    }
                    // A function of `$` parameters alone, as typing took it: an integer it gives is
                    // its value, whatever runs.
                    Value::Fn(_) => Self::known(env, callee, args)
                        .filter(|v| matches!(v, Value::Int(_)))
                        .map(|v| Rvalue::Use(Operand::Const(v))),
                    _ => None,
                };
                if let Some(folded) = folded {
                    stmt.value = folded;
                    changed = true;
                }
            }
        }
        changed
    }

    /// The function the primitive `p` applied to `args` runs as where no instruction computes
    /// it: always (`lower.by`), or where the target's processor does not compute with a type
    /// one of its constant arguments names (`lower.soft`): a float it lacks, an integer wider
    /// than an address. A function, so that folding it again changes nothing.
    fn lowered_by(env: &Env<'_, '_>, p: Primitive, args: &[Operand]) -> Option<Value> {
        let attributes = env.primitive_decl(p).ok()?.attributes(env.cx)?;
        let function = |attribute| {
            attributes
                .get(Primitive::Lower(attribute))
                .and_then(|mut f| f.pop())
                .filter(|f| matches!(f, Value::Fn(_)))
        };
        if let Some(f) = function(Lower::by) {
            return Some(f);
        }
        let target = env.cx.get::<TargetOf>(&())?;
        let lacked = args.iter().any(|a| matches!(a, Operand::Const(value) if !target.computes(value)));
        if lacked { function(Lower::soft) } else { None }
    }

    /// The value the primitive or function `callee` gives for `args`, when every parameter of it is
    /// a `$` one and every argument a constant: a call typing knew when compiling
    /// ([`Callable::folds`]).
    fn known(env: &Env<'_, '_>, callee: &Value, args: &[Operand]) -> Option<Value> {
        if !Callable::of(callee.clone())?.folds(env) {
            return None;
        }
        let args = args
            .iter()
            .map(|a| match a {
                Operand::Const(v) => Some(v.clone()),
                Operand::Place(_) => None,
            })
            .collect::<Option<_>>()?;
        env.apply(callee.clone(), args).ok()
    }

    /// The constant each local holds throughout the body: written once, whole, with a constant,
    /// never borrowed, and no parameter, result or static. Ending a local's scope is no write.
    #[must_use]
    pub fn constants(&self) -> IndexVec<Local, Option<Value>> {
        let mut writes: IndexVec<Local, u32> = IndexVec::filled(0, self.locals.len());
        let mut constants: IndexVec<Local, Option<Value>> = IndexVec::filled(None, self.locals.len());
        let stmts = self.blocks.iter().flat_map(|(_, b)| &b.stmts);
        for stmt in stmts.filter(|s| !matches!(s.value, Rvalue::Dead)) {
            writes[stmt.place.local] += 1;
            // A borrow may write through, and an assembly block writes its outputs.
            if let Rvalue::Ref { place, .. } = &stmt.value {
                writes[place.local] += 1;
            }
            for (place, _) in stmt.value.outputs() {
                writes[place.local] += 1;
            }
            if let (true, Rvalue::Use(Operand::Const(value))) =
                (stmt.place.projection.is_empty(), &stmt.value)
            {
                constants[stmt.place.local] = Some(value.clone());
            }
        }
        for &local in self.params.iter().chain([&self.ret]) {
            writes[local] += 1;
        }
        for (local, constant) in constants.iter_mut() {
            if writes[local] != 1 || self.locals[local].role.is_static() {
                *constant = None;
            }
        }
        constants
    }

    /// The locals some statement or terminator reads (`Stmt::reads`, `TerminatorKind::reads`);
    /// the result is read by the return.
    #[must_use]
    pub fn read(&self) -> vun_hash::Set<Local> {
        let mut read = vun_hash::Set::from_iter([self.ret]);
        for (_, block) in self.blocks.iter() {
            read.extend(block.stmts.iter().flat_map(crate::Stmt::reads));
            read.extend(block.term.kind.reads());
        }
        read
    }

    /// Whether every local has a type.
    fn typed(&self) -> Result<(), Unready> {
        match self.locals.iter().find(|(_, l)| l.ty.is_none()) {
            Some((_, local)) => Err(Unready::Untyped { at: local.origin, said: None }),
            None => Ok(()),
        }
    }
}
