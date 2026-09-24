//! QBE as a subprocess on IL text: small and simple, no vectors, no 32-bit targets.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: "qbe",
    arches: Arches::Only(&["x86_64", "aarch64", "riscv64"]),
    caps: BackendCaps {
        emits: Emits::Assembly,
        c_abi: true,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: false,
        debug_info: false,
    },
};

#[derive(Default)]
pub struct QbeBackend;

impl Backend for QbeBackend {
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
