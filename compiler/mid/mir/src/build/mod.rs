//! Lowering a function literal's HIR into MIR (spec 13, section 3). Parameters and local
//! declarations become locals; any other name is a constant sema evaluates. A form not lowered
//! yet is [`Unbuilt`], at its origin, so that evaluation stops rather than guesses.

mod aggregate;
mod assign;
mod block;
mod call;
mod closure;
mod control;
mod defer;
mod early_return;
mod expand;
mod loops;
mod operator;
mod pattern;
mod place;
mod staged;
mod suspend;
mod temp;
mod value;

use std::collections::{HashMap, HashSet};
use std::panic::Location;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, PatKind};
use vun_index::{Idx, IndexVec};
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};
use vun_query::{Cx, Cycle, Query};
use vun_sema::env::Env;
use vun_sema::eval::{Instantiation, Staging};
use vun_sema::ty::Ty;
use vun_sema::typeck::{Step, Typed, TypedBody};
use vun_span::{SourceFiles, SourceMap};

use crate::Lowered;
use crate::place::Place;
use crate::{
    Block, BlockId, Body, Local, LocalDecl, Operand, Role, Rvalue, Stmt, Terminator, TerminatorKind,
};

/// A form MIR does not lower yet, by where it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unbuilt {
    pub at: OriginId,
    /// The check of the builder that stopped, which `vun debug mir` names.
    pub by: Check,
}

/// Where in the builder a check is, as `file:line:column`: the caller of [`Unbuilt::at`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Check {
    pub file: Symbol,
    pub line: u32,
    pub column: u32,
}

impl std::fmt::Display for Check {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

impl Unbuilt {
    #[must_use]
    #[track_caller]
    pub fn at(at: OriginId) -> Self {
        let caller = Location::caller();
        let by = Check { file: Symbol::intern(caller.file()), line: caller.line(), column: caller.column() };
        Self { at, by }
    }
}

/// The MIR of an instantiation of a function literal: generic code is lowered per
/// instantiation, with its `$` parameters bound (spec 13, section 3).
pub struct Mir;

impl Query for Mir {
    const NAME: &'static str = "mir";
    type Key = Instantiation;
    type Value = Result<Arc<Body>, Unbuilt>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        // What is evaluated while lowering, `$if` conditions and `$for` items, is typed as the
        // body is.
        let env = i.env(cx).expect("a function value's file is read");
        let name = OriginTable::global().declaration_name(i.f.literal.expr.origin);
        let _span =
            vun_trace::tracing::debug_span!("mir", f = name.as_ref().map_or("", |n| n.as_str())).entered();
        let built = Builder::lower(&env, i);
        match &built {
            Ok(body) => {
                vun_trace::tracing::debug!(blocks = body.blocks.len(), locals = body.locals.len(), "built");
            }
            Err(unbuilt) => {
                let (origins, sources) = (OriginTable::global(), SourceFiles::global());
                let at = sources.locate(origins.source_span(unbuilt.at)).to_string();
                vun_trace::tracing::debug!(%at, by = %unbuilt.by, "unbuilt");
            }
        }
        built.map(Arc::new)
    }

    fn cycle(_: &Cx<'_>, i: &Instantiation, _: &Cycle) -> Self::Value {
        Err(Unbuilt::at(i.f.literal.expr.origin))
    }
}

struct Builder<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    locals: IndexVec<Local, LocalDecl>,
    blocks: IndexVec<BlockId, Block>,
    /// The block statements are appended to.
    current: BlockId,
    /// The local each binder of the body is stored in.
    bound: HashMap<BinderId, Local>,
    /// The captured binders whose local holds a borrow of them.
    through: HashSet<BinderId>,
    /// Where the result is written before returning.
    ret: Local,
    /// The loops around the code lowered, innermost last.
    loops: Vec<loops::Loop>,
    /// What runs at the exits of the blocks around the code lowered, latest last.
    pending: Vec<defer::Pending>,
    /// The instantiation lowered, whose typed body gives the types of places.
    f: Instantiation,
    /// The environment of the `$for` step lowered, if any: `env` under its bindings.
    step: Option<Env<'a, 'db>>,
    /// The `$for` steps lowered, outermost first, which lead to their typed bodies.
    path: Vec<Step>,
}

impl<'a, 'db> Builder<'_, 'a, 'db> {
    fn lower(env: &Env<'_, '_>, f: &Instantiation) -> Result<Body, Unbuilt> {
        let hir = &env.parsed.hir;
        let literal = f.f.literal.expr.id;
        let origin = hir.exprs[literal].origin;
        let ExprKind::Fn { params, body, .. } = &hir.exprs[literal].kind else {
            return Err(Unbuilt::at(origin));
        };
        let mut locals = IndexVec::new();
        let ret = locals.push(LocalDecl { name: None, ty: None, role: Role::Held, origin });
        let mut b = Builder {
            env,
            locals,
            blocks: IndexVec::new(),
            current: Idx::new(0),
            bound: HashMap::new(),
            through: HashSet::new(),
            ret,
            loops: Vec::new(),
            pending: Vec::new(),
            f: f.clone(),
            step: None,
            path: Vec::new(),
        };
        b.current = b.fresh(origin);
        let mut args = b.captures(literal, origin)?;
        let signature = env.literal_signature(literal, &f.args).ok();
        let types = signature.as_ref().map(|s| s.params.clone()).unwrap_or_default();
        for (index, param) in params.iter().enumerate() {
            let Entry::One(param) = param else { return Err(Unbuilt::at(origin)) };
            let pat = param.pat.ok_or(Unbuilt::at(param.origin))?;
            let PatKind::Bind(binder) = hir.pats[pat].kind else { return Err(Unbuilt::at(param.origin)) };
            let local = b.bind(binder);
            b.locals[local].ty = types.get(index).copied();
            args.push(local);
        }
        let result = b.scope(*body, defer::Exit::Function)?;
        // The body ends where its value is written, or at the body.
        let end = hir.blocks[*body].tail.map_or(hir.blocks[*body].origin, |t| hir.exprs[t].origin);
        b.assign(Place::of(ret), Rvalue::Use(result), end);
        b.blocks[b.current].term = Terminator { kind: TerminatorKind::Return, origin: end };
        let mut body = Body { locals: b.locals, params: args, ret, blocks: b.blocks, origin };
        // A coroutine's body ends with what its frame gives when done, not the frame.
        let result = signature.map(|s| s.result);
        body.locals[ret].ty = if body.suspends() {
            result.and_then(|frame| env.frame_step(frame)).map(|(_, done)| done)
        } else {
            result
        };
        Ok(body)
    }

    /// The environment lowered in: the function's, or a `$for` step's.
    fn env(&self) -> &Env<'a, 'db> {
        self.step.as_ref().unwrap_or(self.env)
    }

    /// `expr`, evaluated by the call running the body, under the bindings of the step lowered.
    fn comptime(&self, expr: ExprId, expected: Option<Ty>) -> Rvalue {
        Rvalue::Comptime { expr, staged: self.stepped(), expected }
    }

    /// The code `expr`, written in the call running the body.
    fn quote(&mut self, expr: ExprId, origin: OriginId) -> Operand {
        self.temp(Rvalue::Quote { expr, staged: self.stepped() }, origin)
    }

    /// The bindings of the `$for` steps lowered.
    fn stepped(&self) -> Vec<Staging> {
        self.env().staged()[self.env.staged().len()..].to_vec()
    }

    /// The typed body of the code lowered: the instantiation's, or a `$for` step's in it.
    fn typed(&self) -> Option<Arc<TypedBody>> {
        let mut typed = self.env.cx.get::<Typed>(&self.f)?;
        for step in &self.path {
            typed = Arc::clone(typed.steps.get(step)?);
        }
        Some(typed)
    }

    /// A local for `binder`, of the type the typer found, which names in the body read from now
    /// on.
    fn bind(&mut self, binder: BinderId) -> Local {
        let b = &self.env.parsed.hir.binders[binder];
        let ty = self.typed().and_then(|t| t.binders.get(&binder).copied());
        let ty =
            ty.or_else(|| self.env.cx.get::<Typed>(&self.f).and_then(|t| t.binders.get(&binder).copied()));
        let local =
            self.locals.push(LocalDecl { name: Some(b.name), ty, role: Role::Held, origin: b.origin });
        self.bound.insert(binder, local);
        local
    }

    fn assign(&mut self, place: Place, value: Rvalue, origin: OriginId) {
        vun_trace::tracing::trace!(block = ?self.current, ?place, ?value, "statement");
        self.blocks[self.current].stmts.push(Stmt { place, value, origin });
    }

    /// The value of `expr`; a temporary holding it has its type, so that checks know its kind.
    fn operand(&mut self, expr: ExprId) -> Result<Operand, Unbuilt> {
        let operand = self.value(expr)?;
        if let Operand::Place(Place { local, projection }) = &operand
            && projection.is_empty()
            && self.locals[*local].name.is_none()
            && let Some(ty) = self.type_of(expr)
        {
            self.locals[*local].ty = Some(ty);
        }
        Ok(operand)
    }
}
