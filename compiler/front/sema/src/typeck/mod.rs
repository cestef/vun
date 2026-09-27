//! Typing expressions (spec 02 and 04): the types of parameters and locals come from their
//! annotations, names of declarations have their declared types, and integer arithmetic is typed
//! by interval arithmetic. A form not typed yet has no type, and checks stay silent about it.
//!
//! Function bodies are typed one at a time ([`Typed`]); the value of a declaration outside a body
//! is typed where it is checked, and by [`crate::decl::DeclType`] where it is named.

mod apply;
mod block;
mod call;
mod closure;
mod control;
mod early_return;
mod exhaust;
mod expect;
mod expected;
mod held;
mod infer;
mod loops;
mod methods;
mod names;
mod narrow;
pub(crate) mod omitted;
mod operator;

pub use loops::Iteration;
pub use operator::{Operation, Operator};
pub use reported::Reported;
use typed::Found;
pub use typed::{Step, Typed, TypedBody};
mod pattern;
mod prefix;
mod range;
mod record;
mod reported;
mod safety;
mod sequence;
mod staged;
mod suspend;
#[cfg(test)]
mod tests;
mod typed;

use vun_hash::Map;

use vun_codec::{Decode, Encode};
use vun_diag::DiagCtxt;
use vun_hir::tree::{BinderId, BlockId, DeclId, ExprId, Hir, NsId, Staged};
use vun_hir::visit::Visitor;

use crate::env::Env;
use crate::ty::Ty;

/// When a value is known: at compile time, or only at run time. Only run-time integers are
/// capped at 128 bits (spec 04, rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Stage {
    Compile,
    Run,
}

/// A typed expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Known {
    pub ty: Ty,
    pub stage: Stage,
}

/// Types the expressions of one body, or one declaration outside a body, in source order.
pub struct Typer<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    /// The environment of the `$for` step typed, if any: `env` under its bindings.
    step: Option<Env<'a, 'db>>,
    /// When the code typed runs: at compile time outside a body, or under `$`.
    stage: Stage,
    /// What typing the body, or the `$for` step typed, finds.
    found: Found,
    locals: Map<BinderId, Known>,
    /// The ranges immutable integer bindings are narrowed to where the expression typed is
    /// (spec 04, rule 4).
    narrowed: Map<BinderId, Ty>,
    /// The type expected of an expression where its context says, for forms typed by it:
    /// `.name`, `.{ }` (spec 01, section 2).
    expected: Map<ExprId, Ty>,
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
    /// How many `$if` and `$for` whose condition or items are not known here enclose the
    /// expression typed: code that may never run, whose `$e` is typed but not run.
    undecided: usize,
    pub diag: DiagCtxt,
}

impl<'e, 'a, 'db> Typer<'e, 'a, 'db> {
    #[must_use]
    pub fn new(env: &'e Env<'a, 'db>, stage: Stage) -> Self {
        Self {
            env,
            step: None,
            stage,
            found: Found::default(),
            locals: Map::default(),
            narrowed: Map::default(),
            expected: Map::default(),
            result: None,
            written: None,
            loops: Vec::new(),
            yields: None,
            framing: None,
            vouched: 0,
            undecided: 0,
            diag: DiagCtxt::new(),
        }
    }

    /// What this typer found, over a body of `exprs` expressions.
    #[must_use]
    pub fn finish(self, exprs: usize) -> TypedBody {
        let binders = self.locals.into_iter().map(|(b, k)| (b, k.ty)).collect();
        self.found.body(binders, self.diag, exprs)
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
        self.found.types.get(&expr).copied()
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
        // A local annotated with a type not known here has no type, not its value's.
        let ty = if d.ty.is_some() { annotated } else { found.map(|k| k.ty) };
        self.held(d, ty, found);
        self.pattern(d.pat, ty, None);
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let Some(known) = self.infer(hir, expr) {
            self.found.types.insert(expr, known);
        }
    }
}
