//! lld: ELF, COFF, Mach-O and wasm with linker scripts; the default for embedded targets.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{Name, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("lld"),
    formats: &[Name("elf"), Name("coff"), Name("macho"), Name("wasm")],
    regions: true,
};

#[derive(Default)]
pub struct LldLinker;

impl Linker for LldLinker {
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
