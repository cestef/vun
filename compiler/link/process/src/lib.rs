//! External linkers that read the link plan as JSON.

use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_link::{LinkPlan, Linker, LinkerInfo};
use overt_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo {
    id: "process",
    formats: &[ObjectFormat::Elf, ObjectFormat::Coff, ObjectFormat::MachO, ObjectFormat::Wasm],
    regions: true,
};

#[derive(Default)]
pub struct ProcessLinker;

impl Linker for ProcessLinker {
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
