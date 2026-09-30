//! mold: fast, mature ELF linking, run by the target's C compiler in place of GNU ld.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_link_gnu::Ld;
use vun_target::{Name, Readiness, Run, TargetSpec};
use vun_tool::gcc;

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("mold"),
    formats: &[Name("elf")],
    regions: false,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
    runs: &[Run::Compiler, Run::Program(PROGRAM)],
};

/// The program the C compiler runs as its linker, found on the search path.
const PROGRAM: &str = "mold";

#[derive(Default)]
pub struct MoldLinker;

impl Linker for MoldLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        Ld::Flag(gcc::USE_MOLD).link(&INFO, plan, target, diag)
    }
}
