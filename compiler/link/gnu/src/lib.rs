//! GNU ld: compatibility with vendor toolchains, with linker scripts.

use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_link::{LinkPlan, Linker, LinkerInfo};
use overt_target::{ObjectFormat, TargetSpec};

pub const INFO: LinkerInfo = LinkerInfo { id: "gnu", formats: &[ObjectFormat::Elf], regions: true };

#[derive(Default)]
pub struct GnuLinker;

impl Linker for GnuLinker {
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
