//! lld: ELF, COFF, Mach-O and wasm with linker scripts; the default for embedded targets.

use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_link::{LinkPlan, Linker, LinkerInfo};
use overt_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo {
    id: "lld",
    formats: &[ObjectFormat::Elf, ObjectFormat::Coff, ObjectFormat::MachO, ObjectFormat::Wasm],
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
