//! Strict C99 through the target's C compiler: reaches every target with one (spec 13, section
//! 8.2). The backend builds C as data (`ast`) from LIR (`emit`) and prints it once.

pub mod ast;
mod emit;
mod lack;

pub use lack::{Lack, Missing};

use vun_backend::{Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Lines};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::unit::Unit;
use vun_target::{Arches, Name, TargetSpec};

use emit::Emitter;

pub const INFO: BackendInfo = BackendInfo {
    id: Name("c"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Source,
        c_abi: true,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: false,
        debug_info: true,
    },
};

/// The name of the C file a unit becomes.
pub const SOURCE: &str = "program.c";

#[derive(Default)]
pub struct CBackend;

impl CBackend {
    /// The C source of `unit` for `target`.
    ///
    /// # Errors
    /// Returns what the unit needs that C99 cannot express.
    pub fn source(unit: &Unit, target: &TargetSpec, lines: &dyn Lines) -> Result<String, Lack> {
        Ok(Emitter::file(unit, target, lines)?.to_string())
    }
}

impl Backend for CBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        unit: &Unit,
        target: &TargetSpec,
        lines: &dyn Lines,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let source =
            Self::source(unit, target, lines).map_err(|lack| diag.error(lack.diagnostic(INFO.id.0)))?;
        out.put(Artifact { name: SOURCE.to_owned(), kind: Emits::Source, bytes: source.into_bytes() });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
