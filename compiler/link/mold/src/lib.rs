//! mold: fast, mature ELF linking.

use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{Name, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo { id: Name("mold"), formats: &[Name("elf")], regions: false };

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
