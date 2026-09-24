//! LLVM code generation: optimized builds and the widest native reach.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: "llvm",
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: true,
        tail_calls: true,
        noalias: true,
        frame_sizes: true,
        debug_info: true,
    },
};

#[derive(Default)]
pub struct LlvmBackend;

impl Backend for LlvmBackend {
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
