//! The value and the type of a namespace member's declaration. A declaration whose value is `_`
//! in `lang` is its primitive (decision 66); elsewhere `_` is provided by the linker or an impl.

use overt_grammar::PATH_SEPARATOR;
use overt_hir::db::Package;
use overt_hir::tree::{DeclId, ExprKind, PatKind};
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::item::DeclRef;
use crate::ty::Value;
use crate::typeck::{Known, Stage, Typer};

pub struct DeclValue;

impl Query for DeclValue {
    const NAME: &'static str = "decl_value";
    type Key = DeclRef;
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, key: &DeclRef) -> Self::Value {
        let env = Env::of(cx, &key.file).ok_or(Unevaluated::Absent)?;
        let hir = &env.parsed.hir;
        let decl = &hir.decls[key.decl];
        let value = decl.value.ok_or(Unevaluated::Runtime)?;
        if !matches!(hir.exprs[value].kind, ExprKind::Hole) {
            return env.eval(value);
        }
        let package = cx.get::<Package>(&key.file.package);
        let path = package.namespace_of(&key.file.path).ok_or(Unevaluated::Provided)?;
        let namespace =
            package.name.into_iter().chain(path).map(Symbol::as_str).collect::<Vec<_>>().join(PATH_SEPARATOR);
        let mut binders = Vec::new();
        hir.binders(decl.pat, &mut binders);
        let name = binders.first().map(|&b| hir.binders[b].name).ok_or(Unevaluated::Provided)?;
        let primitive =
            Primitive::ALL.iter().find(|p| p.namespace() == namespace && p.name() == name.as_str());
        primitive.map(|&p| Value::Primitive(p)).ok_or(Unevaluated::Provided)
    }

    fn cycle(_: &Cx<'_>, _: &DeclRef, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

/// The type of a namespace member's declaration: its annotation, or the type of its value. A
/// `mut` member is a static read at run time; any other is known at compile time.
pub struct DeclType;

impl Query for DeclType {
    const NAME: &'static str = "decl_type";
    type Key = DeclRef;
    type Value = Result<Known, Unevaluated>;

    fn compute(cx: &Cx<'_>, key: &DeclRef) -> Self::Value {
        Env::of(cx, &key.file).ok_or(Unevaluated::Absent)?.decl_type(key.decl)
    }

    fn cycle(_: &Cx<'_>, _: &DeclRef, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

impl Env<'_, '_> {
    /// The type of the declaration `decl` here: its annotation, or the type of its value.
    ///
    /// # Errors
    /// Returns why the type is not known here.
    pub fn decl_type(&self, decl: DeclId) -> Result<Known, Unevaluated> {
        let hir = &self.parsed.hir;
        let decl = &hir.decls[decl];
        let PatKind::Bind(binder) = hir.pats[decl.pat].kind else { return Err(Unevaluated::Unsupported) };
        let stage = if hir.binders[binder].mutable { Stage::Run } else { Stage::Compile };
        let ty = match (decl.ty, decl.value) {
            (Some(annotation), _) => self.ty(annotation)?,
            (None, Some(value)) => {
                Typer::new(self, Stage::Compile).typed(value).ok_or(Unevaluated::Unsupported)?.ty
            }
            (None, None) => return Err(Unevaluated::Unsupported),
        };
        Ok(Known { ty, stage })
    }
}
