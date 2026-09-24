//! MSVC link.exe for COFF targets.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo { id: "msvc", formats: &[ObjectFormat::Coff], regions: false };

#[derive(Default)]
pub struct MsvcLinker;

impl Linker for MsvcLinker {
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
