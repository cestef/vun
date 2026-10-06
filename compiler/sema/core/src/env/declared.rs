//! The signatures of declarations whose value is `_`, which their annotation, a function type,
//! alone gives: a `lang` primitive (decision 66) and a link symbol (spec 08, section 1). The types
//! may name `$` parameters, so a call is typed with them bound to its arguments.

use std::sync::Arc;

use vun_desugar::MARKERS;
use vun_grammar::TokenKind;
use vun_hir::tree::{Entry, ExprId, ExprKind, Hir, List, Param, PatKind};
use vun_query::{Cx, Query};
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::{DeclRef, Node};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Value};

use super::Env;
use crate::callable::Written;

impl Env<'_, '_> {
    /// The signature of the declaration `decl` for a call whose arguments have the values `args`,
    /// where known: its annotation, with its `$` parameters bound to their arguments.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value, such as a type naming a `$`
    /// parameter whose argument is not known.
    pub fn declared_signature(
        &self,
        decl: &DeclRef,
        args: &[Option<Value>],
    ) -> Result<Signature, Unevaluated> {
        let comptime = self.cx.get::<DeclComptime>(decl)?;
        let bound = comptime.iter().zip(args).map(|(&c, arg)| arg.clone().filter(|_| c)).collect();
        self.cx.get::<DeclSignature>(&(decl.clone(), bound))
    }
}

impl<'a, 'db> Env<'a, 'db> {
    /// Where the parameters of the declaration `decl` are written: its annotation.
    #[must_use]
    pub fn declared_written(&self, decl: &DeclRef) -> Option<Written<'a, 'db>> {
        let annotated = Annotated::of(self.cx, decl).ok()?;
        let expr = annotated.annotation();
        Some(Written { env: annotated.env, expr })
    }
}

/// Whether each parameter of a declaration is a `$` one: what its signature depends on.
pub(crate) struct DeclComptime;

impl Query for DeclComptime {
    const NAME: &'static str = "decl_comptime";
    type Key = DeclRef;
    type Value = Result<Arc<[bool]>, Unevaluated>;

    fn compute(cx: &Cx<'_>, key: &DeclRef) -> Self::Value {
        let annotated = Annotated::of(cx, key)?;
        let hir = &annotated.env.parsed.hir;
        let comptime = annotated.params(hir).iter().map(|param| match param {
            Entry::One(Param { pat: Some(pat), .. }) => {
                matches!(hir.pats[*pat].kind, PatKind::Bind(b) if hir.binders[b].comptime)
            }
            _ => false,
        });
        Ok(comptime.collect())
    }
}

/// The signature of a declaration with its `$` parameters bound to the values given, the others
/// `None`.
pub(crate) struct DeclSignature;

impl Query for DeclSignature {
    const NAME: &'static str = "decl_signature";
    type Key = (DeclRef, Vec<Option<Value>>);
    type Value = Result<Signature, Unevaluated>;

    fn compute(cx: &Cx<'_>, (decl, args): &Self::Key) -> Self::Value {
        let annotated = Annotated::of(cx, decl)?;
        let parsed = Arc::clone(&annotated.env.parsed);
        let hir = &parsed.hir;
        let params = annotated.params(hir);
        let bound: Vec<_> = params
            .iter()
            .zip(args)
            .filter_map(|(param, arg)| {
                let Entry::One(Param { pat: Some(pat), .. }) = param else { return None };
                let PatKind::Bind(binder) = hir.pats[*pat].kind else { return None };
                let node = Node { id: binder, origin: hir.binders[binder].origin };
                Some(Staging { binder: node, value: arg.clone()? })
            })
            .collect();
        // The attributes mark the function type as well: `#[abi.c]`.
        let attributes = annotated.env.markers(&hir.decls[decl.decl.id].attrs);
        let signature = annotated.env.staging(&bound).signature(params, annotated.returns)?;
        let marked = annotated.markers.iter().copied().fold(signature, Signature::marked);
        Ok(attributes.into_iter().fold(marked, Signature::with))
    }
}

/// A declaration as its signature is read from: the environment of its file, the prefixes marking
/// its function type, and that type.
pub(super) struct Annotated<'a, 'db> {
    pub(super) env: Env<'a, 'db>,
    pub(super) markers: Vec<TokenKind>,
    annotation: ExprId,
    returns: Option<ExprId>,
}

impl<'a, 'db> Annotated<'a, 'db> {
    pub(super) fn of(cx: &'a Cx<'db>, decl: &DeclRef) -> Result<Self, Unevaluated> {
        let env = Env::of(cx, &decl.file).ok_or(Unevaluated::Absent)?;
        let hir = &env.parsed.hir;
        let mut annotation = hir.decls[decl.decl.id].ty.ok_or(Unevaluated::Unsupported)?;
        // `unsafe (...) -> T`: the prefixes mark the signature (the table `markers`).
        let mut markers = Vec::new();
        while let ExprKind::Prefix { op, operand, .. } = &hir.exprs[annotation].kind
            && MARKERS.iter().any(|row| row.token == *op)
        {
            markers.push(*op);
            annotation = *operand;
        }
        let ExprKind::FnType { returns, .. } = &hir.exprs[annotation].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let returns = *returns;
        Ok(Self { env, markers, annotation, returns })
    }

    /// The expression of the function type, past its markers.
    pub(super) fn annotation(&self) -> ExprId {
        self.annotation
    }

    fn params<'h>(&self, hir: &'h Hir) -> &'h List<Param> {
        let ExprKind::FnType { params, .. } = &hir.exprs[self.annotation].kind else {
            unreachable!("`Annotated::of` found a function type")
        };
        params
    }
}
