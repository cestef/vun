//! LLVM code generation: optimized builds and the widest native reach.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Readiness, Request};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("llvm"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: true,
        tail_calls: true,
        noalias: true,
        frame_sizes: true,
        int_bits: 128,
        half: Half::Computed,
        debug_info: true,
    },
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct LlvmBackend;

impl Backend for LlvmBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        _request: &Request<'_>,
        _out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(INFO.unavailable(diag))
    }
}
