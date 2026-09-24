//! The signatures of primitives: each is declared in `lang` with a function type (decision 66),
//! whose types may name its `$` parameters, so a call is typed with them bound to its arguments.

use vun_desugar::MARKERS;
use vun_hir::tree::{Entry, ExprKind, Param, PatKind};
use vun_intern::Symbol;

use super::{Env, Unevaluated};
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
        let Site::Namespace(ns) = self.site(primitive.namespace())? else {
            return Err(Unevaluated::Unsupported);
        };
        match self.cx.get::<Declared>(&(ns, Symbol::intern(primitive.name()))) {
            Some(Site::Decl(decl)) => Ok(decl),
            _ => Err(Unevaluated::Absent),
        }
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
        let decl = self.primitive_decl(primitive)?;
        let env = Env::of(self.cx, &decl.file).ok_or(Unevaluated::Absent)?;
        let parsed = std::sync::Arc::clone(&env.parsed);
        let hir = &parsed.hir;
        let mut annotation = hir.decls[decl.decl.id].ty.ok_or(Unevaluated::Unsupported)?;
        // `unsafe (...) -> T`: the prefixes mark the signature (the table `markers`).
        let mut markers = Vec::new();
        while let ExprKind::Prefix { op, operand, .. } = &hir.exprs[annotation].kind
            && MARKERS.iter().any(|(t, _)| t == op)
        {
            markers.push(*op);
            annotation = *operand;
        }
        let ExprKind::FnType { params, returns } = &hir.exprs[annotation].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let bound: Vec<_> = params
            .iter()
            .zip(args)
            .filter_map(|(param, arg)| {
                let Entry::One(Param { pat: Some(pat), .. }) = param else { return None };
                let PatKind::Bind(binder) = hir.pats[*pat].kind else { return None };
                let node = Node { id: binder, origin: hir.binders[binder].origin };
                hir.binders[binder].comptime.then(|| Some(Staging { binder: node, value: arg.clone()? }))?
            })
            .collect();
        let signature = env.staging(&bound).signature(params, *returns)?;
        Ok(markers.into_iter().fold(signature, Signature::marked))
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
