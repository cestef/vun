//! wild: fast development links on Linux ELF, aiming at incremental linking. The target's C
//! compiler runs it as its `ld`, from a directory beside the output holding it under that name.

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Edit;
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_link_gnu::Ld;
use vun_target::{Name, Readiness, Run, TargetSpec};
use vun_tool::Executable;

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("wild"),
    formats: &[Name("elf")],
    regions: false,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
    runs: &[Run::Compiler, Run::Program(PROGRAM)],
};

/// The program, as found on the search path.
const PROGRAM: &str = "wild";
/// The name a compiler driver runs its linker by.
const LD: &str = "ld";
/// The extension of the directory beside the output that holds the linker as `ld`.
const PROGRAMS: &str = "linker";

#[derive(Default)]
pub struct WildLinker;

impl Linker for WildLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let Some(program) = Executable::search(PROGRAM) else {
            let msg = Msg::new().text("linker ").code(PROGRAM).text(" is not on the search path");
            return Err(diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        let dir = plan.beside(PROGRAMS);
        let ld = dir.join(LD);
        ld.link_to(&program).map_err(|e| diag.error(Diagnostic::unwritable(&ld, &e)))?;
        Ld::In(&dir).link(&INFO, plan, target, diag)
    }
}
