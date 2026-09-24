//! In-process Cranelift code generation: the default for debug builds.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, TargetSpec};

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
