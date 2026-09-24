//! Portable C99 through the configured C toolchain: reaches every target with a C compiler.

use vun_backend::{ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Lines};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, Name, TargetSpec};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("c"),
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
        _lines: &dyn Lines,
        _out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(INFO.unavailable(diag))
    }
}
