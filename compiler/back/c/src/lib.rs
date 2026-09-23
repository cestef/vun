//! Portable C99 through the configured C toolchain: reaches every target with a C compiler.

use overt_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits};
use overt_diag::{DiagCtxt, ErrorGuaranteed};
use overt_lir::unit::Unit;
use overt_target::{Arches, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: "c",
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Source,
        c_abi: true,
        vectors: true,
        tail_calls: false,
        noalias: true,
        frame_sizes: true,
        debug_info: true,
    },
};

#[derive(Default)]
pub struct CBackend;

impl Backend for CBackend {
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
