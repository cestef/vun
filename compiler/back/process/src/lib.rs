//! External backends: any executable reading LIR text on standard input.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Lines, Readiness};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, Name, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("process"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: false,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: false,
        int_bits: 64,
        debug_info: false,
    },
    readiness: Readiness::Planned,
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
        _lines: &dyn Lines,
        _out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(INFO.unavailable(diag))
    }
}
