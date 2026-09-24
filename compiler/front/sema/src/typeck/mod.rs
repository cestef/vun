//! Typing expressions (spec 02 and 04): the types of parameters and locals come from their
//! annotations, names of declarations have their declared types, and integer arithmetic is typed
//! by interval arithmetic. A form not typed yet has no type, and checks stay silent about it.
//!
//! Function bodies are typed one at a time ([`Typed`]); the value of a declaration outside a body
//! is typed where it is checked, and by [`crate::decl::DeclType`] where it is named.

mod block;
mod call;
mod control;
mod expect;
mod expected;
mod infer;
mod pattern;
mod sequence;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use overt_diag::DiagCtxt;
use overt_hir::tree::{BinderId, BlockId, DeclId, Entry, ExprId, ExprKind, Hir, NsId, PatKind};
use overt_hir::visit::Visitor;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::env::Env;
use crate::eval::Instantiation;
use crate::ty::{Ty, TyKind, Value};

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
    /// Each method named through a value, with the struct declaring it and its name.
    pub methods: BTreeMap<ExprId, (Ty, Symbol)>,
    pub diag: DiagCtxt,
    pub exprs: usize,
}

/// Counts the expressions of a body, as the typer reaches them: not into nested functions,
/// quotes or namespaces, which are typed on their own.
struct Exprs(usize);

impl Visitor for Exprs {
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        self.0 += 1;
        if !matches!(hir.exprs[expr].kind, ExprKind::Fn { .. } | ExprKind::Quote(_)) {
            overt_hir::visit::walk_expr(self, hir, expr);
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
        let env = Env::inside(cx, &i.f, i.args.iter().cloned())?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, returns, body } = &hir.exprs[i.f.literal.expr.id].kind else {
            return None;
        };
        let mut typer = Typer::new(&env, Stage::Run);
        for (index, param) in params.iter().enumerate() {
            let Entry::One(param) = param else { continue };
            let (Some(pat), Some(ty)) = (param.pat, param.ty) else { continue };
            let PatKind::Bind(binder) = &hir.pats[pat].kind else { continue };
            // A `$` parameter bound by the instantiation has the type of its value alone.
            let bound = i.args.get(index).and_then(Option::as_ref).and_then(Value::ty);
            let Some(ty) = bound.or_else(|| typer.ty(ty)) else { continue };
            typer.bind(*binder, ty);
        }
        let result = match returns {
            Some(r) => typer.ty(*r),
            None => Some(TyKind::unit()),
        };
        typer.result = result;
        let found = typer.block_value(*body, result);
        if let (Some(found), Some(result)) = (found, result) {
            let at = hir.blocks[*body].tail.map_or(hir.blocks[*body].origin, |t| hir.exprs[t].origin);
            typer.expect(at, found, result, *returns);
        }
        let mut exprs = Exprs(0);
        exprs.block(hir, *body);
        Some(Arc::new(TypedBody {
            types: typer.types.into_iter().collect(),
            methods: typer.methods.into_iter().collect(),
            diag: typer.diag,
            exprs: exprs.0,
        }))
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
    /// When the code typed runs: at compile time outside a body, or under `$`.
    stage: Stage,
    types: HashMap<ExprId, Known>,
    locals: HashMap<BinderId, Known>,
    /// Member expressions naming a declaration through a value: `p.norm` in `p.norm()`, whose
    /// call passes `p` first (spec 01, section 4); each with the struct declaring it, the type's
    /// own or a witness, and its name.
    methods: HashMap<ExprId, (Ty, Symbol)>,
    /// The type expected of an expression where its context says, for forms typed by it:
    /// `.name`, `.{ }` (spec 01, section 2).
    expected: HashMap<ExprId, Ty>,
    /// The result type of the function typed, which `return` values are checked against.
    result: Option<Ty>,
    pub diag: DiagCtxt,
}

impl<'e, 'a, 'db> Typer<'e, 'a, 'db> {
    #[must_use]
    pub fn new(env: &'e Env<'a, 'db>, stage: Stage) -> Self {
        Self {
            env,
            stage,
            types: HashMap::new(),
            locals: HashMap::new(),
            methods: HashMap::new(),
            expected: HashMap::new(),
            result: None,
            diag: DiagCtxt::new(),
        }
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
