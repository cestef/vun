//! The `Backend` interface and artifacts.
//!
//! Nothing above LIR knows about any backend. A backend declares its facts once, in a
//! [`BackendInfo`]; support and capability checks are derived from it (spec 13, section 7).

use std::path::PathBuf;

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_lir::unit::Unit;
use vun_origin::OriginId;
use vun_target::{Arches, Name, Support, TargetSpec, Unsupported};

/// What a backend produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emits {
    Object,
    /// Assembly text, for an assembler tool.
    Assembly,
    /// Source code, for a compiler tool.
    Source,
    /// Code in memory.
    Jit,
}

/// Capabilities a program may require of its backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct BackendCaps {
    pub emits: Emits,
    /// Lowers C ABI aggregates itself; otherwise the driver runs ABI lowering first.
    pub c_abi: bool,
    /// Native vectors; otherwise vectors are lowered to scalars.
    pub vectors: bool,
    pub tail_calls: bool,
    /// Uses the aliasing facts of spec 05, section 8.
    pub noalias: bool,
    /// Reports frame sizes, which `stack.max` requires.
    pub frame_sizes: bool,
    pub debug_info: bool,
}

/// Everything known about a backend without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendInfo {
    pub id: Name,
    pub arches: Arches,
    pub caps: BackendCaps,
}

impl BackendInfo {
    #[must_use]
    pub fn supports(&self, target: &TargetSpec) -> Support {
        if self.arches.contains(target.arch) {
            Support::Yes
        } else {
            Support::No(Unsupported::Arch(target.arch))
        }
    }

    /// Reports that this backend cannot generate code yet.
    pub fn unavailable(&self, diag: &mut DiagCtxt) -> ErrorGuaranteed {
        let msg = Msg::new().text("backend ").code(self.id.0).text(" cannot generate code yet");
        diag.error(Diagnostic::error(errors::BACKEND_UNAVAILABLE, msg))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    /// Relative to the output directory.
    pub name: String,
    pub kind: Emits,
    pub bytes: Vec<u8>,
}

pub trait ArtifactSink {
    fn put(&mut self, artifact: Artifact);
}

impl ArtifactSink for Vec<Artifact> {
    fn put(&mut self, artifact: Artifact) {
        self.push(artifact);
    }
}

/// A line of source, for the line information a backend writes (`#line`, DWARF line tables).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Line {
    pub file: PathBuf,
    /// Counted from 1.
    pub line: u32,
}

/// Where origins are in source (spec 10, section 1).
pub trait Lines {
    fn line(&self, origin: OriginId) -> Option<Line>;
}

/// Code generation from LIR.
pub trait Backend {
    fn info(&self) -> &BackendInfo;

    /// Generates code for one unit.
    ///
    /// # Errors
    /// Returns once an error has been reported to `diag`.
    fn emit(
        &self,
        unit: &Unit,
        target: &TargetSpec,
        lines: &dyn Lines,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed>;
}
