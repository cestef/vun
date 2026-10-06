//! Typed bodies: what typing a body finds.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::DiagCtxt;
use vun_hir::tree::{BinderId, ExprId, PatId};
use vun_intern::Symbol;
use vun_sema_ty::call::Staging;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use super::{Iteration, Known, Operation, Operator};
use crate::asm::Block;

/// What typing finds in a body, or in one `$for` step of it, apart from its locals and reports:
/// what a step keeps apart from the rest of its body. A typed body holds it and reads as it; it
/// is shared, so that evaluating at compile time while a body is typed reads what typing found
/// so far without copying it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Found {
    pub types: BTreeMap<ExprId, Known>,
    /// Member expressions naming a declaration through a value: `p.norm` in `p.norm()`, whose
    /// call passes `p` first (spec 01, section 4); each with the struct declaring it, the type's
    /// own or a witness, and its name.
    pub methods: BTreeMap<ExprId, (Ty, Symbol)>,
    /// The type each `.name` is looked up in, the one its context expects (spec 01, section 2).
    pub contexts: BTreeMap<ExprId, Ty>,
    /// The function each operator calls (spec 02, section 4).
    pub operators: BTreeMap<Operation, Operator>,
    /// The body of each step of a `$for`, typed with its binder bound (spec 03, section 1).
    pub steps: BTreeMap<Step, Arc<TypedBody>>,
    /// How each `for p in e` drives `e`, by `e`.
    pub iterations: BTreeMap<ExprId, Iteration>,
    /// The `$` arguments each call leaves out, by parameter, as inferred.
    pub inferred: BTreeMap<ExprId, Vec<(usize, Value)>>,
    /// Where the arguments each call gathers into its callee's pack start, among the written
    /// ones (spec 01, section 4): a pack takes those from there on.
    pub packs: BTreeMap<ExprId, usize>,
    /// The `$` evaluations that surely run, each a `$e`: run once the body is typed
    /// ([`Reported`](super::Reported)).
    pub evaluations: Vec<ExprId>,
    /// The `$for` bindings a step is typed under; none for a whole body.
    pub staged: Vec<Staging>,
    /// Each block of instructions, checked against its architecture's table.
    pub asm: BTreeMap<ExprId, Block>,
    /// The forms typing gave no type though it typed their parts and reported nothing, where a
    /// silent gap starts, with why.
    pub gave_up: BTreeMap<ExprId, GaveUp>,
}

/// Why typing gave a form no type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum GaveUp {
    /// The rule of the form's kind gave up, its parts typed.
    Form,
    /// The type of what it names has no value here.
    Unevaluated(Unevaluated),
    /// It names a member of this type, whose members are not all known here.
    Members(Ty),
    /// It depends on a form typing reported an error for.
    Reported,
}

impl Found {
    /// The typed body of what was found, its locals of the types `binders`.
    #[must_use]
    pub fn body(self: Arc<Self>, binders: BTreeMap<BinderId, Ty>, diag: DiagCtxt, exprs: usize) -> TypedBody {
        TypedBody { found: self, binders, diag, exprs }
    }
}

/// A typed function body: what typing found, the type each local holds, what typing reports, and
/// how many expressions the body holds, nested functions and quotes apart.
#[derive(Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct TypedBody {
    pub found: Arc<Found>,
    /// The type each local of the body holds, steps' included.
    pub binders: BTreeMap<BinderId, Ty>,
    pub diag: DiagCtxt,
    pub exprs: usize,
}

impl Deref for TypedBody {
    type Target = Found;

    fn deref(&self) -> &Found {
        &self.found
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct Step {
    pub at: PatId,
    pub index: usize,
}
