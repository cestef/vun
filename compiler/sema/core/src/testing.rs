//! The program running a namespace's tests (spec 11, section 1): the type `std.testing.runner`
//! gives for the namespace, among whose members the program's exports are. The compiler knows
//! neither tests nor their kinds: the runner is library code, called as `std.build.plan` is.

use vun_hir::db::{FileRef, Namespace};
use vun_query::{Cx, Query};
use vun_sema_ty::call::Instance;
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
