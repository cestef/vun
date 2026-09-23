//! mold: fast, mature ELF linking.

use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_link::{LinkPlan, Linker, LinkerInfo};
use overt_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo { id: "mold", formats: &[ObjectFormat::Elf], regions: false };

#[derive(Default)]
pub struct MoldLinker;

impl Linker for MoldLinker {
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
