//! wild: fast development links on Linux ELF, aiming at incremental linking.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo { id: "wild", formats: &[ObjectFormat::Elf], regions: false };

#[derive(Default)]
pub struct WildLinker;

impl Linker for WildLinker {
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
