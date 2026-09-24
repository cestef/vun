//! The value and the type of a namespace member's declaration. A declaration whose value is `_`
//! in `lang` is its primitive (decision 66); elsewhere it is a link symbol the linker provides.

use std::sync::Arc;

use vun_hir::db::Package;
use vun_hir::tree::{DeclId, ExprKind, Hir, PatKind};
use vun_hir::visit::Visitor;
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::eval::{Instance, Staging};
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
            let types = cx.get::<TypedDecl>(&(key.clone(), None, Vec::new()));
            return env.typed_by(types).eval(value);
        }
        // Outside `lang`, a declaration provided from outside is a link symbol (spec 08, section 3).
        match Self::primitive(cx, key, hir) {
            Some(primitive) => {
                Ok(primitive.constant(&cx.db().resource::<Target>()).unwrap_or(Value::Primitive(primitive)))
            }
            None => Ok(Value::Linked(key.clone())),
        }
    }

    fn cycle(_: &Cx<'_>, _: &DeclRef, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

impl DeclValue {
    /// The primitive the hole declaration `key` of `hir` is, if it is one: by the path of its
    /// namespace and its name.
    fn primitive(cx: &Cx<'_>, key: &DeclRef, hir: &Hir) -> Option<Primitive> {
        let package = cx.get::<Package>(&key.file.package);
        let path = package.namespace_of(&key.file.path)?;
        let mut binders = Vec::new();
        hir.binders(hir.decls[key.decl.id].pat, &mut binders);
        let name = hir.binders[*binders.first()?].name;
        let at: Vec<&str> = package.name.into_iter().chain(path).chain([name]).map(Symbol::as_str).collect();
        Primitive::ALL.iter().copied().find(|p| p.item().path() == at.as_slice())
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
            // A declaration's markers mark the function type it is annotated with, as in
            // `#[abi.c] puts: unsafe (s: *u8) -> i32 = _`.
            (Some(annotation), _) => {
                let ty = Value::Type(self.ty(annotation)?);
                self.marked_by(ty, &decl.attrs).as_type().ok_or(Unevaluated::Unsupported)?
            }
            (None, Some(value)) => {
                let key = (
                    DeclRef::new(self.file.clone(), hir, decl_id),
                    self.instance.clone(),
                    self.staged().to_vec(),
                );
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
    type Key = (DeclRef, Option<Instance>, Vec<Staging>);
    type Value = Option<Arc<TypedBody>>;

    fn compute(cx: &Cx<'_>, (decl, instance, staged): &Self::Key) -> Self::Value {
        let env = match instance {
            Some(instance) => Env::within(cx, instance)?,
            None => Env::of(cx, &decl.file)?,
        };
        let env = env.staging(staged);
        let mut typer = Typer::new(&env, Stage::Compile);
        typer.decl(&env.parsed.hir, decl.decl.id);
        Some(Arc::new(typer.finish(0)))
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        None
    }
}
