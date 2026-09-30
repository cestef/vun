//! A package's plan (spec 12, section 2): what its `build.vun` records when `std.build.plan`
//! runs its `build` at compile time on a plan without steps. The compiler knows neither the name
//! `build` nor the shape of a step: the declarations run are those `std.build.Package` requires,
//! and the steps are the values `std.build.plan` gives, which `vun-sema-plan` reads as steps and
//! lints as their levels.

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instance;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::trap::Unevaluated;

use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::MeteredCall;

/// The plan of the `build.vun` a key names, in the order its steps were recorded.
pub struct Planned;

/// The steps of a plan: each a value of `std.build.Action`, and where the `build` that recorded
/// them is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Plan {
    pub actions: Vec<Value>,
    pub at: OriginId,
}

/// Why a file has no plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unplanned {
    /// The file does not declare `name`, which `std.build.Package` requires; `at` is the file.
    Missing { name: Symbol, at: OriginId },
    /// Running the plan stopped: why, and the value of the declaration run, where it is reported.
    Stopped { why: Unevaluated, at: OriginId },
}

impl Query for Planned {
    const NAME: &'static str = "planned";
    type Key = FileRef;
    /// `None` for a file the driver did not read.
    type Value = Option<Result<Plan, Unplanned>>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let env = Env::of(cx, file)?;
        let whole = env.parsed.hir.namespaces[env.parsed.hir.root?].origin;
        Some(Self::run(&env, whole))
    }
}

impl Planned {
    /// Runs the plan of the file of `env`, whose root namespace is at `whole`.
    fn run(env: &Env<'_, '_>, whole: OriginId) -> Result<Plan, Unplanned> {
        let (cx, file, hir) = (env.cx, &env.file, &env.parsed.hir);
        let stopped = |why, at| Unplanned::Stopped { why, at };
        let required = env.requirements(vun_items::std::build::Package).map_err(|why| stopped(why, whole))?;
        // The package's plan runs the one declaration its interface requires.
        let [name] = required.as_slice() else { return Err(stopped(Unevaluated::Unsupported, whole)) };
        let Some(decl) = hir.root.and_then(|root| hir.declaring(root, *name)) else {
            return Err(Unplanned::Missing { name: *name, at: whole });
        };
        let decl = DeclRef::new(file.clone(), hir, decl);
        let at = hir.decls[decl.decl.id].value.map_or(decl.decl.origin, |v| hir.exprs[v].origin);
        let build = cx.get::<DeclValue>(&decl).map_err(|why| stopped(why, at))?;
        let Value::Fn(plan) = env.item(vun_items::std::build::plan).map_err(|why| stopped(why, at))? else {
            return Err(stopped(Unevaluated::Unsupported, at));
        };
        let called = cx.get::<MeteredCall>(&Instance { f: plan, args: vec![build] });
        match called.value.map_err(|why| stopped(why, at))? {
            Value::Array(actions) => Ok(Plan { actions, at }),
            _ => Err(stopped(Unevaluated::Unsupported, at)),
        }
    }
}
