//! Primitives: each is declared in `lang` with a function type (decision 66), which gives its
//! signature (`Env::declared_signature`).

use vun_hir::db::PackageId;
use vun_hir::tree::ExprKind;
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_query::{Cx, Query};
use vun_sema_ty::item::{DeclRef, Declared, Site};
use vun_sema_ty::trap::{Reason, Unevaluated};
use vun_sema_ty::{Marker, Signature, Value};

use super::{Env, ItemSite};

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
        self.declared_signature(&self.primitive_decl(primitive)?, args)
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
