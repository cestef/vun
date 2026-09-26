//! The baseline backend (spec 13, section 8.3): native code for debug builds in one pass per
//! function over LIR and its shared analyses, in the design of TPDE, with its own encoder and
//! object writer. x86-64 and ELF first.

pub mod elf;
mod object;
pub mod x86_64;

use std::path::PathBuf;

use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Emitted, Lack, Lines, Readiness,
};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_index::IndexVec;
use vun_lir::legalize::Half;
use vun_lir::unit::Unit;
use vun_lir::{FuncId, Function};
use vun_par::Parallel;
use vun_target::{Arches, Name, TargetSpec};

use x86_64::func::Compiled;

pub const INFO: BackendInfo = BackendInfo {
    id: Name("baseline"),
    arches: Arches::Only(&[Name("x86_64")]),
    caps: BackendCaps {
        emits: Emits::Object,
        c_abi: true,
        vectors: false,
        tail_calls: false,
        noalias: false,
        frame_sizes: true,
        int_bits: 64,
        half: Half::Promoted,
        debug_info: false,
    },
    readiness: Readiness::Generates,
};

/// The object's name among the build's artifacts.
const OBJECT: &str = "program.o";

#[derive(Default)]
pub struct BaselineBackend;

impl BaselineBackend {
    /// The machine code of every function of `unit`, compiled in parallel.
    ///
    /// # Errors
    /// Returns the first thing a function needs that the backend cannot compile yet.
    ///
    /// # Panics
    /// Never: compiling a function does not panic.
    pub fn compile(unit: &Unit) -> Result<IndexVec<FuncId, Compiled>, Lack> {
        let functions: Vec<&Function> = unit.functions.iter().map(|(_, f)| f).collect();
        let compiled = functions.par_map(|f| x86_64::func::compile(unit, f));
        Ok(compiled.into_iter().collect::<Result<Vec<_>, _>>()?.into())
    }

    /// The relocatable object of `unit`, with each function's frame size in bytes.
    ///
    /// # Errors
    /// As [`Self::compile`].
    pub fn object(unit: &Unit) -> Result<Emitted, Lack> {
        let code = Self::compile(unit)?;
        let bytes = object::assemble(unit, &code).write();
        Ok(Emitted {
            artifacts: vec![Artifact { name: PathBuf::from(OBJECT), kind: Emits::Object, bytes }],
            frames: code.iter().map(|(id, c)| (id, c.stack())).collect(),
        })
    }
}

impl Backend for BaselineBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        unit: &Unit,
        _target: &TargetSpec,
        _lines: &dyn Lines,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let emitted = Self::object(unit).map_err(|lack| diag.error(lack.diagnostic(INFO.id.0)))?;
        for artifact in emitted.artifacts {
            out.put(artifact);
        }
        out.frames(emitted.frames);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
