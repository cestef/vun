//! QBE as a subprocess on IL text: small and simple, no vectors, no 32-bit targets.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Readiness, Request};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("qbe"),
    arches: Arches::Only(&[Name("x86_64"), Name("arm64"), Name("riscv64")]),
    caps: BackendCaps {
        emits: Emits::Assembly,
        c_abi: true,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: false,
        int_bits: 64,
        half: Half::Promoted,
        debug_info: false,
    },
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct QbeBackend;

impl Backend for QbeBackend {
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
