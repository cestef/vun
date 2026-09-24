//! Typing expressions (spec 02 and 04): the types of parameters and locals come from their
//! annotations, names of declarations have their declared types, and integer arithmetic is typed
//! by interval arithmetic. A form not typed yet has no type, and checks stay silent about it.
//!
//! Function bodies are typed one at a time ([`Typed`]); the value of a declaration outside a body
//! is typed where it is checked, and by [`crate::decl::DeclType`] where it is named.

mod block;
mod call;
mod closure;
mod control;
mod early_return;
mod expect;
mod expected;
mod infer;
mod loops;
mod methods;
mod omitted;

pub use loops::Iteration;
mod pattern;
mod prefix;
mod range;
mod safety;
mod sequence;
mod staged;
mod suspend;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use overt_diag::DiagCtxt;
use overt_hir::tree::{BinderId, BlockId, DeclId, ExprId, ExprKind, Hir, NsId, PatId, Staged};
use overt_hir::visit::Visitor;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::env::Env;
use crate::eval::Instantiation;
use crate::ty::{Ty, Value};

/// When a value is known: at compile time, or only at run time. Only run-time integers are
/// capped at 128 bits (spec 04, rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    Compile,
    Run,
}

/// A typed expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Known {
    pub ty: Ty,
    pub stage: Stage,
}

/// A typed function body: the type of each expression typed, what typing reports, and how many
/// expressions the body holds, nested functions and quotes apart.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct TypedBody {
    pub types: BTreeMap<ExprId, Known>,
    /// The type each local of the body holds, steps' included.
    pub binders: BTreeMap<BinderId, Ty>,
    /// Each method named through a value, with the struct declaring it and its name.
    pub methods: BTreeMap<ExprId, (Ty, Symbol)>,
    /// The body of each step of a `$for`, typed with its binder bound (spec 03, section 1).
    pub steps: BTreeMap<Step, Arc<TypedBody>>,
    /// How each `for p in e` drives `e`, by `e`.
    pub iterations: BTreeMap<ExprId, Iteration>,
    /// The `$` arguments each call leaves out, by parameter, as inferred.
    pub inferred: BTreeMap<ExprId, Vec<(usize, Value)>>,
    pub diag: DiagCtxt,
    pub exprs: usize,
}

impl TypedBody {
    /// How many expressions are typed, in the body or in some step of it.
    #[must_use]
    pub fn typed_count(&self) -> usize {
        let mut typed = BTreeSet::new();
        self.collect_typed(&mut typed);
        typed.len()
    }

    fn collect_typed(&self, typed: &mut BTreeSet<ExprId>) {
        typed.extend(self.types.keys().copied());
        for step in self.steps.values() {
            step.collect_typed(typed);
        }
    }
}

/// One step of a `$for` in a body: the `$for`, by its pattern, and the item bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Step {
    pub at: PatId,
    pub index: usize,
}

/// Counts the expressions of a body, as the typer reaches them: not into nested functions or
/// namespaces, which are typed on their own, but into closures, typed as part of the body.
struct Exprs(usize);

impl Visitor for Exprs {
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        self.0 += 1;
        match &hir.exprs[expr].kind {
            ExprKind::Fn { .. } => {}
            ExprKind::Closure { func, .. } => {
                overt_hir::visit::walk_expr(self, hir, expr);
                overt_hir::visit::walk_expr(self, hir, *func);
            }
            _ => overt_hir::visit::walk_expr(self, hir, expr),
        }
    }
}

/// The typed body of an instantiation of a function literal; `None` for a file the driver did
/// not read. A generic body is typed for each instantiation its callers reach, with its `$`
/// parameters bound, and what that reports is reported at the generic definition.
pub struct Typed;

impl Query for Typed {
    const NAME: &'static str = "typed";
    type Key = Instantiation;
    type Value = Option<Arc<TypedBody>>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        if let Some(enclosing) = i.enclosing(cx) {
            return cx.get::<Self>(&enclosing);
        }
        let env = Env::inside(cx, &i.f, i.args.iter().cloned())?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { body, .. } = &hir.exprs[i.f.literal.expr.id].kind else { return None };
        let mut typer = Typer::new(&env, Stage::Run);
        typer.function(i.f.literal.expr.id, &i.args);
        let mut exprs = Exprs(0);
        exprs.block(hir, *body);
        Some(Arc::new(typer.finish(exprs.0)))
    }

    /// A body that reaches itself through calls is typed on its way in; the inner reach sees
    /// nothing.
    fn cycle(_: &Cx<'_>, _: &Instantiation, _: &Cycle) -> Self::Value {
        None
    }
}

/// Types the expressions of one body, or one declaration outside a body, in source order.
pub struct Typer<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    /// The environment of the `$for` step typed, if any: `env` under its bindings.
    step: Option<Env<'a, 'db>>,
    /// When the code typed runs: at compile time outside a body, or under `$`.
    stage: Stage,
    types: HashMap<ExprId, Known>,
    locals: HashMap<BinderId, Known>,
    /// Member expressions naming a declaration through a value: `p.norm` in `p.norm()`, whose
    /// call passes `p` first (spec 01, section 4); each with the struct declaring it, the type's
    /// own or a witness, and its name.
    methods: HashMap<ExprId, (Ty, Symbol)>,
    /// The typed steps of the `$for`s typed so far.
    steps: BTreeMap<Step, Arc<TypedBody>>,
    iterations: BTreeMap<ExprId, Iteration>,
    inferred: BTreeMap<ExprId, Vec<(usize, Value)>>,
    /// The type expected of an expression where its context says, for forms typed by it:
    /// `.name`, `.{ }` (spec 01, section 2).
    expected: HashMap<ExprId, Ty>,
    /// The result type of the function typed, which `return` values are checked against.
    result: Option<Ty>,
    /// The annotation the result type is written as, which reports name it by.
    written: Option<ExprId>,
    /// The loops around the expression typed, innermost last, which `break` values join.
    loops: Vec<loops::Loop>,
    /// What the coroutine function typed yields, which `yield` values are checked against.
    yields: Option<Ty>,
    /// The coroutine function literal typed, whose frame may not contain itself.
    framing: Option<ExprId>,
    /// How many `unsafe` expressions enclose the expression typed: where unchecked operations
    /// are vouched for (spec 07, section 4).
    vouched: usize,
    pub diag: DiagCtxt,
}

impl<'e, 'a, 'db> Typer<'e, 'a, 'db> {
    #[must_use]
    pub fn new(env: &'e Env<'a, 'db>, stage: Stage) -> Self {
        Self {
            env,
            step: None,
            stage,
            types: HashMap::new(),
            locals: HashMap::new(),
            methods: HashMap::new(),
            steps: BTreeMap::new(),
            iterations: BTreeMap::new(),
            inferred: BTreeMap::new(),
            expected: HashMap::new(),
            result: None,
            written: None,
            loops: Vec::new(),
            yields: None,
            framing: None,
            vouched: 0,
            diag: DiagCtxt::new(),
        }
    }

    /// What this typer found, over a body of `exprs` expressions.
    #[must_use]
    pub fn finish(self, exprs: usize) -> TypedBody {
        TypedBody {
            types: self.types.into_iter().collect(),
            binders: self.locals.into_iter().map(|(b, k)| (b, k.ty)).collect(),
            methods: self.methods.into_iter().collect(),
            steps: self.steps,
            iterations: self.iterations,
            inferred: self.inferred,
            diag: self.diag,
            exprs,
        }
    }

    /// The environment typed in: the file's, or a `$for` step's.
    fn env(&self) -> &Env<'a, 'db> {
        self.step.as_ref().unwrap_or(self.env)
    }

    fn hir(&self) -> &'e Hir {
        &self.env.parsed.hir
    }

    /// Records `binder` as holding values of type `ty`, known when the binder is.
    fn bind(&mut self, binder: BinderId, ty: Ty) {
        let stage = if self.hir().binders[binder].comptime { Stage::Compile } else { self.stage };
        self.locals.insert(binder, Known { ty, stage });
    }

    /// Types `expr` and its parts.
    pub fn typed(&mut self, expr: ExprId) -> Option<Known> {
        self.expr(self.hir(), expr);
        self.types.get(&expr).copied()
    }

    /// Types `expr` where a value of type `expected` is wanted, if one is.
    pub fn typed_as(&mut self, expr: ExprId, expected: Option<Ty>) -> Option<Known> {
        if let Some(expected) = expected {
            self.expected.insert(expr, expected);
        }
        self.typed(expr)
    }

    /// The type the context of `expr` expects, if it says.
    fn expected(&self, expr: ExprId) -> Option<Ty> {
        self.expected.get(&expr).copied()
    }
}

impl Visitor for Typer<'_, '_, '_> {
    /// Members of a nested namespace are checked on their own, not as part of this body.
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn staged<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        self.unroll(hir, staged, entry);
    }

    fn block(&mut self, _: &Hir, block: BlockId) {
        self.block_value(block, None);
    }

    /// `x := e` takes the type of `e`; `x: T = e` checks `e` against `T` and takes `T`.
    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        let d = &hir.decls[decl];
        let annotated = d.ty.and_then(|t| self.ty(t));
        let found = d.value.and_then(|v| self.typed_as(v, annotated));
        if let (Some(found), Some(expected), Some(value)) = (found, annotated, d.value) {
            self.expect(hir.exprs[value].origin, found, expected, d.ty);
        }
        self.pattern(d.pat, annotated.or(found.map(|k| k.ty)), None);
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let Some(known) = self.infer(hir, expr) {
            self.types.insert(expr, known);
        }
    }
}
