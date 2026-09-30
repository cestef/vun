//! A member declaration's type and value, evaluated where its struct was built.

use std::sync::Arc;

use vun_hir::tree::ExprKind;
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Nominal, Ty, TyKind, Value};

use super::{Found, Members};
use crate::decl::TypedDecl;
use crate::env::Env;
use crate::typeck::Known;

/// The type of the declaration `name` among a struct type's members, evaluated where the struct
/// was built: `Eq(u8).eq` is `(&u8, &u8) -> bool`.
pub struct MemberType;

impl Query for MemberType {
    const NAME: &'static str = "member_type";
    type Key = (Ty, Symbol);
    type Value = Result<Known, Unevaluated>;

    fn compute(cx: &Cx<'_>, (ty, name): &Self::Key) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else {
            return Err(Unevaluated::Unsupported);
        };
        let members = cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let env = Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        match members.find(*name) {
            Found::Decl(decl) => env.staging(&decl.staged).decl_type(decl.decl.decl.id),
            Found::Spread(site) => env.site_type(site.clone()),
            _ => Err(Unevaluated::Absent),
        }
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

/// The value of the declaration `name` among a struct type's members, evaluated where the struct
/// was built: a method of `Pair(u8)` is a function value that sees `T` bound to `u8`.
pub struct MemberValue;

impl Query for MemberValue {
    const NAME: &'static str = "member_value";
    type Key = (Ty, Symbol);
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, (ty, name): &Self::Key) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else {
            return Err(Unevaluated::Unsupported);
        };
        let members = cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let env = Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        let decl = match members.find(*name) {
            Found::Decl(decl) => decl,
            Found::Spread(site) => return env.value_of(site.clone()),
            _ => return Err(Unevaluated::Absent),
        };
        let types = cx.get::<TypedDecl>(&(decl.decl.clone(), instance.clone(), decl.staged.clone()));
        let env = env.staging(&decl.staged).typed_by(types.map(|t| Arc::clone(&t.found)));
        let value = env.parsed.hir.decls[decl.decl.decl.id].value.ok_or(Unevaluated::Runtime)?;
        match env.parsed.hir.exprs[value].kind {
            // A requirement of the interface made for a type `Self`, its first argument: what the
            // witness of `Self` provides, as a default of the interface sees it (`Error` in
            // `Writer.write_all`).
            ExprKind::Hole => {
                let target = instance.as_ref().and_then(|i| i.args.first()).and_then(Value::as_type);
                match target.and_then(|t| env.method(t, *name)) {
                    Some(provided) if provided.owner != *ty => {
                        cx.get::<MemberValue>(&(provided.owner, *name))
                    }
                    _ => Err(Unevaluated::Provided),
                }
            }
            _ => env.eval(value),
        }
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
