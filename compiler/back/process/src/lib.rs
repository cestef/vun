//! External backends: any executable reading LIR text on standard input.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: "process",
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: false,
        debug_info: false,
    },
};

#[derive(Default)]
pub struct ProcessBackend;

impl Backend for ProcessBackend {
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
