//! A package's plan (spec 12, section 2): what its `build.vun` records when `std.build.plan`
//! runs its `build` at compile time on a plan without steps. The compiler knows neither the name
//! `build` nor the shape of a step: the declarations run are those `std.build.Package` requires,
//! and the steps are the values `std.build.plan` gives, which the driver reads by reflection.

mod action;

pub use action::{Action, Options, Region, Use};

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_intern::Symbol;
use vun_items::Item;
use vun_origin::OriginId;
use vun_query::{Cx, Query};

use crate::decl::DeclValue;
use crate::env::{Env, Unevaluated};
use crate::eval::{Call, Instance, MeteredCall};
use crate::item::DeclRef;
use crate::member::Members;
use crate::target::{Read, Unread};
use crate::ty::{TyKind, Value};

/// The plan of the `build.vun` a key names, in the order its steps were recorded.
pub struct Planned;

/// The steps of a plan: each a value of `std.build.Action`, and where the `build` that recorded
/// them is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Plan {
    pub actions: Vec<Value>,
    pub at: OriginId,
}

impl Plan {
    /// What each step does.
    ///
    /// # Errors
    /// Returns the step at which a value is not an action as `std.build` declares it, and why.
    pub fn steps(&self) -> Result<Vec<Action>, (usize, Unread)> {
        self.actions.iter().enumerate().map(|(at, a)| Action::read(a).map_err(|why| (at, why))).collect()
    }
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

impl Env<'_, '_> {
    /// The names the interface `interface` requires, which are alike for every type it is
    /// applied to.
    fn requirements(&self, interface: Item) -> Result<Vec<Symbol>, Unevaluated> {
        let Value::Interface(f) = self.item(interface)? else { return Err(Unevaluated::Unsupported) };
        let call = Instance { f, args: vec![Value::Type(TyKind::of_types())] };
        let requirements = self.cx.get::<Call>(&call)?.as_type().ok_or(Unevaluated::Unsupported)?;
        let members = self.cx.get::<Members>(&requirements).ok_or(Unevaluated::Unsupported)?;
        Ok(members.decls.iter().filter(|d| d.decl.required(self.cx)).map(|d| d.name).collect())
    }
}
