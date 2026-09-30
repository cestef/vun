//! Typing expressions (spec 02 and 04): the types of parameters and locals come from their
//! annotations, names of declarations have their declared types, and integer arithmetic is typed
//! by interval arithmetic. A form not typed yet has no type, and checks stay silent about it.
//!
//! Function bodies are typed one at a time ([`Typed`]); the value of a declaration outside a body
//! is typed where it is checked, and by [`crate::decl::DeclType`] where it is named.

mod apply;
mod asm;
mod block;
mod call;
mod closure;
mod control;
mod early_return;
mod exhaust;
mod expect;
mod expected;
mod gave_up;
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
pub use typed::{Found, GaveUp, Step, Typed, TypedBody};
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

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::DiagCtxt;
use vun_hash::Map;
use vun_hir::tree::{BinderId, BlockId, DeclId, ExprId, Hir, NsId, Staged};
use vun_hir::visit::Visitor;
use vun_sema_ty::Ty;
use vun_sema_ty::trap::Unevaluated;

use crate::env::Env;

/// When a value is known: at compile time, or only at run time. Only run-time integers are
/// capped at 128 bits (spec 04, rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Stage {
    Compile,
    Run,
}

impl Stage {
    /// When it runs, as dumps and editors show it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Compile => "compile",
            Self::Run => "run",
        }
    }
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
    found: Arc<Found>,
    /// Each local bound so far, with its type or why it has none.
    locals: Map<BinderId, Result<Known, GaveUp>>,
    /// The ranges immutable integer bindings are narrowed to where the expression typed is
    /// (spec 04, rule 4).
    narrowed: Map<BinderId, Ty>,
    /// The type expected of an expression where its context says, for forms typed by it:
    /// `.name`, `.{ }` (spec 01, section 2); or why the context cannot say, as an annotation
    /// whose type is not known here.
    expected: Map<ExprId, Result<Ty, GaveUp>>,
    /// The result type of the function typed, which `return` values are checked against, or why
    /// it is not known here.
    result: Option<Result<Ty, GaveUp>>,
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
    /// How many `$if` and `$for` whose condition or items are not known here, and `$` forms,
    /// enclose the expression typed: code that does not run on its own, whose `$e` is typed but
    /// not run.
    undecided: usize,
    /// How many times typing gave up, reporting nothing: a form is where a gap starts when none
    /// under it did.
    gave_ups: usize,
    /// How many evaluations typing asked for read a `$` parameter the generic body typed leaves
    /// unbound: a form giving up after one is left to the body's instantiations.
    unbound: usize,
    pub diag: DiagCtxt,
}

impl<'e, 'a, 'db> Typer<'e, 'a, 'db> {
    #[must_use]
    pub fn new(env: &'e Env<'a, 'db>, stage: Stage) -> Self {
        Self {
            env,
            step: None,
            stage,
            found: Arc::default(),
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
            gave_ups: 0,
            unbound: 0,
            diag: DiagCtxt::new(),
        }
    }

    /// What this typer found, over a body of `exprs` expressions.
    #[must_use]
    pub fn finish(self, exprs: usize) -> TypedBody {
        let binders = self.locals.into_iter().filter_map(|(b, k)| Some((b, k.ok()?.ty))).collect();
        self.found.body(binders, self.diag, exprs)
    }

    /// What typing found so far, to add to: shared with an evaluation only while it runs, so
    /// this copies nothing but then.
    fn finding(&mut self) -> &mut Found {
        Arc::make_mut(&mut self.found)
    }

    /// The environment typed in: the file's, or a `$for` step's.
    fn env(&self) -> &Env<'a, 'db> {
        self.step.as_ref().unwrap_or(self.env)
    }

    fn hir(&self) -> &'e Hir {
        &self.env.parsed.hir
    }

    /// Records `binder` as holding values of type `ty`, known when the binder is: at compile
    /// time for a `$` local, and for a local declared with a value existing then only.
    fn bind(&mut self, binder: BinderId, ty: Ty) {
        let fixed = self.env().declared(binder).is_some() && self.env().compile_only_type(ty);
        let stage = if self.hir().binders[binder].comptime || fixed { Stage::Compile } else { self.stage };
        self.locals.insert(binder, Ok(Known { ty, stage }));
    }

    /// Types `expr` and its parts.
    pub fn typed(&mut self, expr: ExprId) -> Option<Known> {
        self.expr(self.hir(), expr);
        self.found.types.get(&expr).copied()
    }

    /// Types `expr` where a value of type `expected` is wanted, if one is.
    pub fn typed_as(&mut self, expr: ExprId, expected: Option<Ty>) -> Option<Known> {
        if let Some(expected) = expected {
            self.expected.insert(expr, Ok(expected));
        }
        self.typed(expr)
    }

    /// The type of `expr` as `found` says, or none, recording why.
    fn known(&mut self, expr: ExprId, found: Result<Known, Unevaluated>) -> Option<Known> {
        found.map_err(|why| self.give_up(expr, GaveUp::Unevaluated(why))).ok()
    }

    /// Records that typing gave `expr` no type, reporting nothing, and why.
    fn give_up(&mut self, expr: ExprId, why: GaveUp) {
        let node = &self.hir().exprs[expr];
        vun_trace::tracing::debug!(at = %node.origin.located(), form = node.kind.name(), why = %why.msg(), "typing gave up");
        self.gave_ups += 1;
        self.finding().gave_up.insert(expr, why);
    }

    /// The type the context of `expr` expects, if it says.
    fn expected(&self, expr: ExprId) -> Option<Ty> {
        self.expected.get(&expr)?.as_ref().ok().copied()
    }

    /// The type `part` of `expr` is expected to have, as `derive` finds it from the type
    /// expected of `expr`; when that is not known here, neither is the part's, for the same
    /// reason.
    fn expected_of(
        &mut self,
        expr: ExprId,
        part: ExprId,
        derive: impl FnOnce(Ty) -> Option<Ty>,
    ) -> Option<Ty> {
        let expected = self.expected.get(&expr).cloned()?;
        self.expecting(part, expected).and_then(derive)
    }

    /// The type `part` is expected to have, as `expected` says; when it is not known here, why
    /// is recorded for `part`.
    fn expecting(&mut self, part: ExprId, expected: Result<Ty, GaveUp>) -> Option<Ty> {
        expected.map_err(|why| self.expected.insert(part, Err(why))).ok()
    }

    /// The type the tail of `block`, the value of `expr`, is expected to have: that of `expr`.
    fn tail_expected(&mut self, expr: ExprId, block: BlockId) -> Option<Ty> {
        match self.hir().blocks[block].tail {
            Some(tail) => self.expected_of(expr, tail, Some),
            None => self.expected(expr),
        }
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
        let annotated = d.ty.map(|t| self.annotation(t));
        // A value whose annotation's type is not known here has no type to be checked against.
        if let (Some(Err(why)), Some(value)) = (&annotated, d.value) {
            self.expected.insert(value, Err(why.clone()));
        }
        let found = d.value.and_then(|v| self.typed_as(v, annotated.clone().and_then(Result::ok)));
        if let (Some(found), Some(Ok(expected)), Some(value)) = (found, annotated.clone(), d.value) {
            self.expect(hir.exprs[value].origin, found, expected, d.ty);
        }
        // A local annotated with a type not known here has no type, not its value's.
        let ty = match (annotated, d.value) {
            (Some(annotated), _) => annotated,
            (None, Some(value)) => self.matched(value, found),
            (None, None) => Err(GaveUp::Form),
        };
        self.held(d, ty.clone().ok(), found);
        self.pattern(d.pat, ty, None);
    }

    /// A form left untyped though nothing under it was reported or gave up is where a gap
    /// starts, and is recorded: with why its expected type is not known, when that is why, left to
    /// the instantiations when it read an unbound `$` parameter, or as its rule giving up.
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        let before = (self.diag.error_count(), self.gave_ups, self.unbound);
        match self.infer(hir, expr) {
            Some(known) => {
                let found = self.finding();
                found.types.insert(expr, known);
                found.gave_up.remove(&expr);
            }
            None if (self.diag.error_count(), self.gave_ups) == (before.0, before.1) => {
                let why = match self.expected.get(&expr) {
                    Some(Err(why)) => why.clone(),
                    _ if self.unbound > before.2 => GaveUp::Unevaluated(Unevaluated::Unbound),
                    _ => GaveUp::Form,
                };
                self.give_up(expr, why);
            }
            None => {}
        }
    }
}
