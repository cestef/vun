//! The signatures of primitives: each is declared in `lang` with a function type (decision 66),
//! whose types may name its `$` parameters, so a call is typed with them bound to its arguments.

use std::sync::Arc;

use vun_desugar::MARKERS;
use vun_grammar::TokenKind;
use vun_hir::db::PackageId;
use vun_hir::tree::{Entry, ExprId, ExprKind, Hir, List, Param, PatKind};
use vun_intern::Symbol;
use vun_query::{Cx, Query};

use super::{Env, ItemSite, Unevaluated};
use crate::Primitive;
use crate::eval::Staging;
use crate::item::{DeclRef, Declared, Node, Site};
use crate::trap::Reason;
use crate::ty::{Marker, Signature, Value};

impl Env<'_, '_> {
    /// The declaration of `primitive` in `lang`.
    ///
    /// # Errors
    /// Returns why it is not found here, such as a replaced `lang` that lacks it.
    pub fn primitive_decl(&self, primitive: Primitive) -> Result<DeclRef, Unevaluated> {
        self.cx.get::<PrimitiveDecl>(&(self.file.package, primitive))
    }

    /// The signature of `primitive` for a call whose arguments have the values `args`, where
    /// known: its declared function type, with its `$` parameters bound to their arguments.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value, such as a type naming a `$`
    /// parameter whose argument is not known.
    pub fn primitive_signature(
        &self,
        primitive: Primitive,
        args: &[Option<Value>],
    ) -> Result<Signature, Unevaluated> {
        let comptime = self.cx.get::<PrimitiveComptime>(&(self.file.package, primitive))?;
        let bound = comptime.iter().zip(args).map(|(&c, arg)| arg.clone().filter(|_| c)).collect();
        self.cx.get::<PrimitiveSignature>(&(self.file.package, primitive, bound))
    }

    /// Traps when an argument of `args` is known not to be a value of its parameter's type, as
    /// `-7` for the `$n: Int(0..)` of `lang.mem.Array`.
    ///
    /// # Errors
    /// Returns the trap, [`Reason::Argument`].
    pub fn check_arguments(&self, primitive: Primitive, args: &[Value]) -> Result<(), Unevaluated> {
        let known: Vec<_> = args.iter().cloned().map(Some).collect();
        match self.primitive_signature(primitive, &known) {
            Ok(signature) if signature.refuses(args) => Err(Unevaluated::trap(Reason::Argument)),
            _ => Ok(()),
        }
    }

    /// The markers of `primitive`: its declaration's attributes that are markers, and those its
    /// annotation's prefixes put, as the `unsafe` of `offset: unsafe (...) -> *T = _`.
    #[must_use]
    pub fn primitive_markers(&self, primitive: Primitive) -> Vec<Marker> {
        let Ok(decl) = self.primitive_decl(primitive) else { return Vec::new() };
        let Some(env) = Env::of(self.cx, &decl.file) else { return Vec::new() };
        let hir = &env.parsed.hir;
        let mut markers = env.markers(&hir.decls[decl.decl.id].attrs);
        let mut annotation = hir.decls[decl.decl.id].ty;
        while let Some(ExprKind::Prefix { op, operand, .. }) = annotation.map(|a| &hir.exprs[a].kind) {
            if let Some(marker) = Marker::of_prefix(*op) {
                Marker::insert(&mut markers, marker);
            }
            annotation = Some(*operand);
        }
        markers
    }
}

/// The declaration of a primitive in the `lang` the files of a package see.
pub(crate) struct PrimitiveDecl;

impl Query for PrimitiveDecl {
    const NAME: &'static str = "primitive_decl";
    type Key = (PackageId, Primitive);
    type Value = Result<DeclRef, Unevaluated>;

    fn compute(cx: &Cx<'_>, (package, primitive): &Self::Key) -> Self::Value {
        let Site::Namespace(ns) = cx.get::<ItemSite>(&(*package, primitive.namespace()))? else {
            return Err(Unevaluated::Unsupported);
        };
        match cx.get::<Declared>(&(ns, Symbol::intern(primitive.name()))) {
            Some(Site::Decl(decl)) => Ok(decl),
            _ => Err(Unevaluated::Absent),
        }
    }
}

/// Whether each parameter of a primitive is a `$` one: what its signature depends on.
pub(crate) struct PrimitiveComptime;

impl Query for PrimitiveComptime {
    const NAME: &'static str = "primitive_comptime";
    type Key = (PackageId, Primitive);
    type Value = Result<Arc<[bool]>, Unevaluated>;

    fn compute(cx: &Cx<'_>, key: &Self::Key) -> Self::Value {
        let declared = Declaration::of(cx, key)?;
        let hir = &declared.env.parsed.hir;
        let comptime = declared.params(hir).iter().map(|param| match param {
            Entry::One(Param { pat: Some(pat), .. }) => {
                matches!(hir.pats[*pat].kind, PatKind::Bind(b) if hir.binders[b].comptime)
            }
            _ => false,
        });
        Ok(comptime.collect())
    }
}

/// The signature of a primitive with its `$` parameters bound to the values given, the others
/// `None`.
pub(crate) struct PrimitiveSignature;

impl Query for PrimitiveSignature {
    const NAME: &'static str = "primitive_signature";
    type Key = (PackageId, Primitive, Vec<Option<Value>>);
    type Value = Result<Signature, Unevaluated>;

    fn compute(cx: &Cx<'_>, (package, primitive, args): &Self::Key) -> Self::Value {
        let declared = Declaration::of(cx, &(*package, *primitive))?;
        let parsed = Arc::clone(&declared.env.parsed);
        let hir = &parsed.hir;
        let params = declared.params(hir);
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
        let signature = declared.env.staging(&bound).signature(params, declared.returns)?;
        Ok(declared.markers.iter().copied().fold(signature, Signature::marked))
    }
}

/// A primitive's declaration as its signature is read from: the environment of its file, the
/// prefixes marking its function type, and that type.
struct Declaration<'a, 'db> {
    env: Env<'a, 'db>,
    markers: Vec<TokenKind>,
    annotation: ExprId,
    returns: Option<ExprId>,
}

impl<'a, 'db> Declaration<'a, 'db> {
    fn of(cx: &'a Cx<'db>, key: &(PackageId, Primitive)) -> Result<Self, Unevaluated> {
        let decl = cx.get::<PrimitiveDecl>(key)?;
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

    fn params<'h>(&self, hir: &'h Hir) -> &'h List<Param> {
        let ExprKind::FnType { params, .. } = &hir.exprs[self.annotation].kind else {
            unreachable!("`Declaration::of` found a function type")
        };
        params
    }
}
