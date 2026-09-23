//! In-process Cranelift code generation: the default for debug builds.

use overt_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_lir::unit::Unit;
use overt_target::{Arches, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: "cranelift",
    arches: Arches::Only(&["x86_64", "aarch64", "riscv64", "s390x"]),
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: true,
        tail_calls: true,
        noalias: false,
        frame_sizes: false,
        debug_info: true,
    },
};

#[derive(Default)]
pub struct CraneliftBackend;

impl Backend for CraneliftBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        _unit: &Unit,
        _target: &TargetSpec,
        _out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(INFO.unavailable(diag))
    }
}
