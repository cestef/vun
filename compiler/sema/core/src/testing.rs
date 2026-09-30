//! A namespace's tests (spec 11, section 1): the program `std.testing.runner` gives for it, and
//! the tests `std.meta.collect` finds in it for editors. The compiler knows neither tests nor
//! their kinds: both are library code, called as `std.build.plan` is.

use vun_hir::db::{FileRef, Namespace};
use vun_intern::Symbol;
use vun_items::lang::meta_Decl;
use vun_items::std::meta_Collected;
use vun_query::{Cx, Query};
use vun_sema_ty::call::Instance;
use vun_sema_ty::item::{DeclRef, Declared, Site};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use crate::env::Env;
use crate::eval::Call;

/// The type of the program running the tests of a namespace, called from a file of its package.
pub struct Runner;

impl Query for Runner {
    const NAME: &'static str = "runner";
    type Key = (FileRef, Namespace);
    type Value = Result<Ty, Unevaluated>;

    fn compute(cx: &Cx<'_>, (file, ns): &Self::Key) -> Self::Value {
        let env = Env::of(cx, file).ok_or(Unevaluated::Absent)?;
        let Value::Fn(runner) = env.item(vun_items::std::testing::runner)? else {
            return Err(Unevaluated::Unsupported);
        };
        match cx.get::<Call>(&Instance { f: runner, args: vec![Value::Namespace(ns.clone())] })? {
            Value::Type(ty) => Ok(ty),
            _ => Err(Unevaluated::Unsupported),
        }
    }
}

/// The tests of a namespace and those nested in it, as the runner finds them
/// (`std.meta.collect(ns, std.testing.test)`), called from a file of its package: each by its
/// name and declaration.
pub struct Tests;

impl Query for Tests {
    const NAME: &'static str = "tests";
    type Key = (FileRef, Namespace);
    type Value = Result<Vec<(Symbol, DeclRef)>, Unevaluated>;

    fn compute(cx: &Cx<'_>, (file, ns): &Self::Key) -> Self::Value {
        let env = Env::of(cx, file).ok_or(Unevaluated::Absent)?;
        let Value::Fn(collect) = env.item(vun_items::std::meta::collect)? else {
            return Err(Unevaluated::Unsupported);
        };
        let test = env.item(vun_items::std::testing::test)?;
        let found =
            cx.get::<Call>(&Instance { f: collect, args: vec![Value::Namespace(ns.clone()), test] })?;
        let found = found.items().ok_or(Unevaluated::Unsupported)?;
        found
            .iter()
            .map(|collected| {
                let decl = collected.field(meta_Collected::decl);
                let (Some(Value::Namespace(ns)), Some(name)) = (
                    collected.field(meta_Collected::ns),
                    decl.and_then(|d| d.field(meta_Decl::name)?.text()),
                ) else {
                    return Err(Unevaluated::Unsupported);
                };
                let name = Symbol::intern(&name);
                match cx.get::<Declared>(&(ns.clone(), name)) {
                    Some(Site::Decl(decl)) => Ok((name, decl)),
                    _ => Err(Unevaluated::Unsupported),
                }
            })
            .collect()
    }
}
