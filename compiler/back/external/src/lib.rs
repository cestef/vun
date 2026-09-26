//! External backends: any executable reading LIR text on standard input.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Readiness, Request};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("external"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
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
pub struct ExternalBackend;

impl Backend for ExternalBackend {
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
