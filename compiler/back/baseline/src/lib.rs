//! The baseline backend (spec 13, section 8.3): native code for debug builds in one pass per
//! function over LIR and its shared analyses, in the design of TPDE, with its own encoder and
//! object writer. x86-64 and ELF first.

pub mod elf;
mod object;
pub mod x86_64;

use std::path::PathBuf;

use vun_backend::{Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Emitted, Lack, Lines};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_index::IndexVec;
use vun_lir::unit::Unit;
use vun_lir::{FuncId, Function};
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
        debug_info: false,
    },
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
        let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let chunk = functions.len().div_ceil(threads).max(1);
        let compiled: Vec<Result<Vec<Compiled>, Lack>> = std::thread::scope(|scope| {
            let runs: Vec<_> = functions
                .chunks(chunk)
                .map(|fs| scope.spawn(move || fs.iter().map(|f| x86_64::func::compile(unit, f)).collect()))
                .collect();
            runs.into_iter().map(|run| run.join().expect("compiling a function does not panic")).collect()
        });
        let mut all = Vec::with_capacity(functions.len());
        for part in compiled {
            all.extend(part?);
        }
        Ok(all.into())
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
