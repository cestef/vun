//! A package's plan (spec 12, section 2): what its `build.vun` records when `std.build.plan`
//! runs its `build` at compile time on a plan without steps. The compiler knows neither the name
//! `build` nor the shape of a step: the declarations run are those `std.build.Package` requires,
//! and the steps are the values `std.build.plan` gives, which the driver reads by reflection.

use vun_hir::db::FileRef;
use vun_hir::tree::{Hir, Member, PatKind, each_entry};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_query::{Cx, Query};

use crate::decl::DeclValue;
use crate::env::{Env, Unevaluated};
use crate::eval::{Call, Instance, MeteredCall};
use crate::item::DeclRef;
use crate::member::Members;
use crate::ty::{TyKind, Value};

/// The plan of the `build.vun` a key names, in the order its steps were recorded.
pub struct Planned;

/// The steps of a plan: each a value of `std.build.Action`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Plan {
    pub actions: Vec<Value>,
}

/// Why a file has no plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
        let required = Required::of(env).map_err(|why| stopped(why, whole))?;
        // The package's plan runs the one declaration its interface requires.
        let [name] = required.as_slice() else { return Err(stopped(Unevaluated::Unsupported, whole)) };
        let Some(decl) = declared(hir, *name) else {
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
            Value::Array(actions) => Ok(Plan { actions }),
            _ => Err(stopped(Unevaluated::Unsupported, at)),
        }
    }
}

/// The names `std.build.Package` requires of a `build.vun`.
struct Required;

impl Required {
    fn of(env: &Env<'_, '_>) -> Result<Vec<Symbol>, Unevaluated> {
        let Value::Interface(package) = env.item(vun_items::std::build::Package)? else {
            return Err(Unevaluated::Unsupported);
        };
        // The requirements are named alike for every type: any stands for the package.
        let call = Instance { f: package, args: vec![Value::Type(TyKind::of_types())] };
        let requirements = env.cx.get::<Call>(&call)?.as_type().ok_or(Unevaluated::Unsupported)?;
        let members = env.cx.get::<Members>(&requirements).ok_or(Unevaluated::Unsupported)?;
        Ok(members.decls.iter().filter(|d| d.decl.required(env.cx)).map(|d| d.name).collect())
    }
}

/// The top-level declaration of `hir` binding `name`.
fn declared(hir: &Hir, name: Symbol) -> Option<vun_hir::tree::DeclId> {
    let mut found = None;
    each_entry(&hir.namespaces[hir.root?].members, &mut |m| {
        if let Member::Decl(d) = m
            && let PatKind::Bind(b) = hir.pats[hir.decls[*d].pat].kind
            && hir.binders[b].name == name
        {
            found = found.or(Some(*d));
        }
    });
    found
}
