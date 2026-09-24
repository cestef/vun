//! The value and the type of a namespace member's declaration. A declaration whose value is `_`
//! in `lang` is its primitive (decision 66); elsewhere `_` is provided by the linker or an impl.

use std::sync::Arc;

use overt_grammar::PATH_SEPARATOR;
use overt_hir::db::Package;
use overt_hir::tree::{DeclId, ExprKind, PatKind};
use overt_hir::visit::Visitor;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::eval::Instance;
use crate::item::DeclRef;
use crate::target::Target;
use crate::ty::Value;
use crate::typeck::{Known, Stage, TypedBody, Typer};

pub struct DeclValue;

impl Query for DeclValue {
    const NAME: &'static str = "decl_value";
    type Key = DeclRef;
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, key: &DeclRef) -> Self::Value {
        let env = Env::of(cx, &key.file).ok_or(Unevaluated::Absent)?;
        let hir = &env.parsed.hir;
        let decl = &hir.decls[key.decl.id];
        let value = decl.value.ok_or(Unevaluated::Runtime)?;
        if !matches!(hir.exprs[value].kind, ExprKind::Hole) {
            let types = cx.get::<TypedDecl>(&(key.clone(), None));
            return env.typed_by(types).eval(value);
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
        let primitive = *primitive.ok_or(Unevaluated::Provided)?;
        Ok(primitive.constant(&cx.db().resource::<Target>()).unwrap_or(Value::Primitive(primitive)))
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
        Env::of(cx, &key.file).ok_or(Unevaluated::Absent)?.decl_type(key.decl.id)
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
    pub fn decl_type(&self, decl_id: DeclId) -> Result<Known, Unevaluated> {
        let hir = &self.parsed.hir;
        let decl = &hir.decls[decl_id];
        let PatKind::Bind(binder) = hir.pats[decl.pat].kind else { return Err(Unevaluated::Unsupported) };
        let stage = if hir.binders[binder].mutable { Stage::Run } else { Stage::Compile };
        let ty = match (decl.ty, decl.value) {
            (Some(annotation), _) => self.ty(annotation)?,
            (None, Some(value)) => {
                let key = (DeclRef::new(self.file.clone(), hir, decl_id), self.instance.clone());
                let typed = self.cx.get::<TypedDecl>(&key).ok_or(Unevaluated::Unsupported)?;
                typed.types.get(&value).ok_or(Unevaluated::Unsupported)?.ty
            }
            (None, None) => return Err(Unevaluated::Unsupported),
        };
        Ok(Known { ty, stage })
    }
}

/// The typed value of a namespace member's declaration, in the compile-time call its namespace
/// was built in, if any: the types compile-time evaluation reads for forms typed by context,
/// such as `.{ }` and `.name`, and what typing it reports.
pub struct TypedDecl;

impl Query for TypedDecl {
    const NAME: &'static str = "typed_decl";
    type Key = (DeclRef, Option<Instance>);
    type Value = Option<Arc<TypedBody>>;

    fn compute(cx: &Cx<'_>, (decl, instance): &Self::Key) -> Self::Value {
        let env = match instance {
            Some(instance) => Env::within(cx, instance)?,
            None => Env::of(cx, &decl.file)?,
        };
        let mut typer = Typer::new(&env, Stage::Compile);
        typer.decl(&env.parsed.hir, decl.decl.id);
        Some(Arc::new(typer.finish(0)))
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        None
    }
}
