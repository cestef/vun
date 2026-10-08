//! A package's plan (spec 12, section 2): what its `build` records, run at compile time.

use std::ffi::OsStr;
use std::path::Path;

use vun_build::layout::MANIFEST;
use vun_build::manifest::PACKAGE;
use vun_diag::{Diagnostic, Msg, errors};
use vun_sema_plan::{Plan, Planned, Unplanned};
use vun_sema_ty::trap::Unevaluated;
use vun_span::{FileId, SourceMap};

use crate::Session;

impl Session {
    /// Runs the plan of `file` when it is a package's `build.vun`: checking the package's
    /// manifest runs its `build`, as building it would.
    pub fn check_plan(&mut self, file: FileId) {
        if self.is_package(file) {
            self.plan(file);
        }
    }

    /// Whether `file` is a package's `build.vun`: one declaring `package` (spec 12, section 2).
    pub fn is_package(&mut self, file: FileId) -> bool {
        let manifest = Path::new(&self.sources.file(file).path).file_name() == Some(OsStr::new(MANIFEST));
        manifest && self.declarations(file, |decls, _| decls.iter().any(|d| d.name == PACKAGE))
    }

    /// The plan of the `build.vun` loaded as `file`, reporting why it has none.
    pub fn plan(&mut self, file: FileId) -> Option<Plan> {
        let key = self.file_ref(file);
        let name = self.sources.file(file).path.clone();
        let d = match self.db.get::<Planned>(&key)? {
            Ok(plan) => return Some(plan),
            Err(Unplanned::Missing { name: required, at }) => {
                let msg = Msg::new()
                    .code(name)
                    .text(" does not declare ")
                    .code(required.as_str())
                    .text(", which ")
                    .code(vun_items::std::build::Package.to_string())
                    .text(" requires");
                let help = Msg::new()
                    .text("declare it: ")
                    .code([required.as_str(), " := (b: &mut std.build.Build) {};"].concat());
                Diagnostic::error(errors::PLAN_MISSING_BUILD, msg).at(at, None).help(help)
            }
            // What the plan reads of the target is unknown, which is reported as not chosen.
            Err(Unplanned::Stopped { .. }) if self.untargeted() => return None,
            Err(Unplanned::Stopped { why: Unevaluated::Trapped(trap), at }) => trap.report(at),
            Err(Unplanned::Stopped { why, at }) => {
                let msg = Msg::new().text("the plan of ").code(name).text(" could not run: ").text(why.why());
                Diagnostic::error(errors::PLAN_STOPPED, msg).at(at, None)
            }
        };
        self.diag.error(d);
        None
    }
}
