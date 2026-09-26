//! In-process Cranelift code generation: the default for debug builds.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Readiness, Request};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("cranelift"),
    arches: Arches::Only(&[Name("x86_64"), Name("arm64"), Name("riscv64"), Name("s390x")]),
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: true,
        tail_calls: true,
        noalias: false,
        frame_sizes: false,
        int_bits: 128,
        half: Half::Promoted,
        debug_info: true,
    },
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct CraneliftBackend;

impl Backend for CraneliftBackend {
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
