//! lld: one program per object format it links, each a flavor with its own command line. The
//! wasm flavor, `wasm-ld`, links the wasm backend's objects (spec 13, section 8.4); the Mach-O
//! flavor, `ld64.lld`, links macOS programs against the system's libraries in the target's
//! sysroot, an SDK; the COFF flavor, `lld-link`, links Windows programs against the import
//! libraries in the sysroot. ELF is still to come.

mod flavor;

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::{Readiness, Run, TargetSpec};
use vun_tool::{Executable, Invocation, Task, Tool};

use flavor::{FLAVORS, FORMATS};

pub const INFO: LinkerInfo = LinkerInfo {
    id: vun_target::Name("lld"),
    formats: &FORMATS,
    regions: false,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
    runs: &[],
};

#[derive(Default)]
pub struct LldLinker;

impl Linker for LldLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn runs(&self, target: &TargetSpec) -> Vec<Run> {
        FLAVORS
            .iter()
            .filter(|f| f.format.is(target.object_format))
            .map(|f| Run::Program(f.program))
            .collect()
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let Some(flavor) = FLAVORS.iter().find(|f| f.format.is(target.object_format)) else {
            return Err(INFO.unavailable(diag));
        };
        let program = flavor.program;
        let Some(path) = Executable::search(program) else {
            let msg = Msg::new().text("linker ").code(program).text(" is not on the search path");
            return Err(diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        let args = (flavor.args)(plan, target);
        vun_trace::tracing::debug!(program, output = %plan.output.display(), inputs = plan.inputs.len(), "linking");
        let inv = Invocation {
            args,
            inputs: plan.inputs.clone(),
            outputs: vec![plan.output.clone()],
            ..Invocation::default()
        };
        Executable::new(program, path)
            .check(&inv)
            .map(drop)
            .map_err(|failure| diag.error(failure.report(program, Task::Link, &plan.output)))
    }
}
