//! Static libraries (spec 08, section 2): the target's archiver (`lang.target.CCompiler.archiver`,
//! `ar` or one its toolchain names) bundles a program's objects, with an index of their symbols
//! for the linkers that read the library, in every object format those archivers write.

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness, Run, TargetSpec};
use vun_tool::{Arg, Executable, Invocation, Task, Tool};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("ar"),
    formats: &[Name("elf"), Name("macho"), Name("coff"), Name("wasm")],
    regions: false,
    outputs: &[Output::Static],
    readiness: Readiness::Ready,
    runs: &[Run::Archiver],
};

/// Replaces or inserts the members, creating the archive, and writes its symbol index.
const CREATE: &str = "rcs";

#[derive(Default)]
pub struct ArLinker;

impl Linker for ArLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let archiver = target.c.compiler.archiver.as_str();
        // An archiver adds to the archive it finds: the library is made anew.
        if plan.output.exists() {
            std::fs::remove_file(&plan.output)
                .map_err(|e| diag.error(Diagnostic::unwritable(&plan.output, &e)))?;
        }
        let mut args = vec![CREATE.into(), Arg::Path(plan.output.clone())];
        args.extend(plan.inputs.iter().cloned().map(Arg::Path));
        vun_trace::tracing::debug!(archiver, output = %plan.output.display(), inputs = plan.inputs.len(), "archiving");
        let inv = Invocation {
            args,
            inputs: plan.inputs.clone(),
            outputs: vec![plan.output.clone()],
            ..Invocation::default()
        };
        Executable::new(archiver, archiver)
            .check(&inv)
            .map(drop)
            .map_err(|failure| diag.error(failure.report(archiver, Task::Archive, &plan.output)))
    }
}
