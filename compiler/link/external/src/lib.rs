//! External linkers that read the link plan as JSON.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{Name, Readiness, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("external"),
    formats: &[Name("elf"), Name("coff"), Name("macho"), Name("wasm")],
    regions: true,
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct ExternalLinker;

impl Linker for ExternalLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(
        &self,
        _plan: &LinkPlan,
        _target: &TargetSpec,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(INFO.unavailable(diag))
    }
}
