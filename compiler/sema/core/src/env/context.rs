//! What the place of a function literal says of its signature (spec 02, section 3), for what the
//! literal leaves out: the type of a parameter written without one, and a result not written.
//! The place is the declaration it is the value of, annotated with a function type (`add: (a:
//! u8, b: u8) -> u8 = (a, b) { ... }`), or the interface a witness it is a member of implements,
//! whose requirement of the same name is `format := ($W: Writer, value, w) { ... }`'s, with the
//! `Self` the impl is for and the `$` parameters the member binds. Where a member leaves its
//! parameters' types out, the requirement is where they are written (`Func::written`), so calls
//! infer its `$` parameters and pass its arguments as the requirement says.

use std::sync::Arc;

use vun_hir::tree::{Entry, ExprId, ExprKind, Param, PatKind};
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Ty, TyKind};

use super::Env;
use crate::callable::Written;

/// The types a place gives the parts of a function literal; why for a part it cannot give here,
/// such as a result that names a member of a `$` parameter an instantiation binds.
pub struct Context {
    /// The number of parameters the place has.
    pub arity: usize,
    pub params: Vec<Result<Ty, Unevaluated>>,
    pub result: Result<Ty, Unevaluated>,
    pub place: Place,
}

/// What gives a function literal its context.
#[derive(Clone, Copy)]
pub enum Place {
    /// The function type its declaration is annotated with.
    Annotation,
    /// The requirement of an interface it implements.
    Requirement,
}

impl Place {
    /// How a report names it.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Annotation => "the function type it is annotated with",
            Self::Requirement => "the requirement it implements",
        }
    }
}

impl Context {
    /// The type of the parameter at `index`, or why it is not known here; none if the place has
    /// no parameter there.
    #[must_use]
    pub fn param(&self, index: usize) -> Option<Result<Ty, Unevaluated>> {
        self.params.get(index).cloned()
    }
}

impl From<Signature> for Context {
    fn from(signature: Signature) -> Self {
        let params: Vec<_> = signature.params.into_iter().map(Ok).collect();
        Self { arity: params.len(), params, result: Ok(signature.result), place: Place::Annotation }
    }
}

impl Env<'_, '_> {
    /// What the place of the function literal `literal` says of its signature, if it has one and
    /// the literal leaves something out: a function type it is annotated with, or the requirement
    /// it implements. Where the place cannot be read here, such as a requirement that names a `$`
    /// parameter of the impl no instantiation binds yet, why. A literal with another number of
    /// parameters than the annotation takes the types of those they have in common, and is a
    /// mismatch with the annotation for the rest.
    #[must_use]
    pub fn context(&self, literal: ExprId) -> Option<Result<Context, Unevaluated>> {
        if !self.leaves_out(literal) {
            return None;
        }
        let (decl, _) = self.declaration_of(literal);
        let annotation = decl.and_then(|decl| self.parsed.hir.decls[decl].ty);
        if let Some(annotation) = annotation {
            let signature = self.ty(annotation).map(|ty| ty.signature().map(Context::from));
            return signature.transpose();
        }
        let asked = self.asked(literal)?;
        Some(asked.and_then(|asked| self.context_from(literal, asked)))
    }

    /// Whether the function literal `literal` leaves out what its place could give: a parameter
    /// written without a type, or its result.
    fn leaves_out(&self, literal: ExprId) -> bool {
        let ExprKind::Fn { returns, .. } = &self.parsed.hir.exprs[literal].kind else { return false };
        returns.is_none() || self.omits_param_types(literal)
    }

    /// Whether the function literal `literal` writes a run-time parameter without a type.
    #[must_use]
    pub fn omits_param_types(&self, literal: ExprId) -> bool {
        let hir = &self.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[literal].kind else { return false };
        params.iter().any(
            |p| matches!(p, Entry::One(Param { pat: Some(pat), ty: None, .. }) if !hir.is_comptime(*pat)),
        )
    }

    /// What the requirement `asked` says of the signature of `literal`, a member implementing it:
    /// its `$` parameters bound to those `literal` binds at their positions. Each part whose
    /// type is not known here is why.
    fn context_from(&self, literal: ExprId, asked: Written<'_, '_>) -> Result<Context, Unevaluated> {
        let hir = &self.parsed.hir;
        let parsed = Arc::clone(&asked.env.parsed);
        let their_hir = &parsed.hir;
        let ExprKind::FnType { params: required, returns, .. } = &their_hir.exprs[asked.expr].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let ExprKind::Fn { params: written, .. } = &hir.exprs[literal].kind else {
            return Err(Unevaluated::Absent);
        };
        let mut bound = Vec::new();
        for (required, written) in required.iter().zip(written) {
            let (Entry::One(Param { pat: Some(required), .. }), Entry::One(Param { pat: Some(written), .. })) =
                (required, written)
            else {
                continue;
            };
            let (&PatKind::Bind(required), &PatKind::Bind(written)) =
                (&their_hir.pats[*required].kind, &hir.pats[*written].kind)
            else {
                continue;
            };
            // A `$` parameter no instantiation binds yet leaves the types that name it unknown.
            if their_hir.binders[required].comptime
                && let Some(value) = self.bound(written).cloned()
            {
                let origin = their_hir.binders[required].origin;
                bound.push(Staging { binder: Node { id: required, origin }, value });
            }
        }
        let env = asked.env.staging(&bound);
        // A member with another number of parameters than its requirement is not an
        // implementation of it, and no position of one is the other's.
        let params = if required.len() == written.len() {
            required.iter().map(|p| env.declared_param(p)).collect()
        } else {
            Vec::new()
        };
        let result = returns.map_or_else(|| Ok(TyKind::unit()), |r| env.ty(r));
        Ok(Context { arity: required.len(), params, result, place: Place::Requirement })
    }
}
