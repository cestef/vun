//! The native backend (spec 13, section 8.3): machine code for debug builds, in one pass per
//! function over LIR and its shared analyses, in the design of TPDE. The framework is
//! `vun-codegen`, the assemblers `vun-asm-*`, the object writer `vun-object`; this crate holds
//! what is particular to each architecture: calling conventions, snippets and instruction
//! compilers. x86-64 and ELF first.

mod listing;
mod object;
pub mod x86_64;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Emitted, Lack, Readiness, Request,
};
use vun_codegen::Compiled;
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_index::IndexVec;
use vun_lir::legalize::Half;
use vun_lir::unit::Unit;
use vun_lir::{Address, FuncId, Function};
use vun_par::Parallel;
use vun_target::{Arches, Name};

pub use listing::Listing;

pub const INFO: BackendInfo = BackendInfo {
    id: Name("native"),
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
    readiness: Readiness::Ready,
};

/// The object's name among the build's artifacts, and its listing's.
const OBJECT: &str = "program.o";
const LISTING: &str = "program.lst";

/// A function compiled for x86-64.
pub type Code = Compiled<vun_asm_x86_64::Inst<Address>>;

/// Bytes a call pushes above a frame: the return address and the saved frame pointer.
const LINKAGE: u64 = 16;

#[derive(Default)]
pub struct NativeBackend;

impl NativeBackend {
    /// The machine code of every function of `unit`, compiled in parallel, listed when `listed`.
    ///
    /// # Errors
    /// Returns the first thing a function needs that the backend cannot compile yet.
    pub fn compile(unit: &Unit, listed: bool) -> Result<IndexVec<FuncId, Code>, Lack> {
        let functions: Vec<&Function> = unit.functions.iter().map(|(_, f)| f).collect();
        let compiled = functions.par_map(|f| {
            let _span = vun_trace::tracing::debug_span!("native", f = %f.linkage).entered();
            let compiled = vun_codegen::compile::<x86_64::X86>(unit, f, listed);
            if let Ok(c) = &compiled {
                vun_trace::tracing::debug!(bytes = c.code.bytes.len(), frame = c.frame, "compiled");
            }
            compiled
        });
        Ok(compiled.into_iter().collect::<Result<Vec<_>, _>>()?.into())
    }

    /// The relocatable object of `request`'s unit, each function's frame size (with the return
    /// address and saved frame pointer, as C compilers count them), and its listing if asked.
    ///
    /// # Errors
    /// As [`Self::compile`].
    pub fn object(request: &Request<'_>) -> Result<Emitted, Lack> {
        let code = Self::compile(request.unit, request.listing)?;
        let bytes = object::assemble(request.unit, &code).write();
        let mut artifacts = vec![Artifact { name: PathBuf::from(OBJECT), kind: Emits::Object, bytes }];
        if request.listing {
            let text = Listing { unit: request.unit, code: &code, lines: request.lines }.to_string();
            artifacts.push(Artifact {
                name: PathBuf::from(LISTING),
                kind: Emits::Listing,
                bytes: text.into_bytes(),
            });
        }
        let frames = code.iter().map(|(id, c)| (id, c.frame + LINKAGE)).collect();
        Ok(Emitted { artifacts, frames })
    }
}

impl Backend for NativeBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let emitted = Self::object(request).map_err(|lack| diag.error(lack.diagnostic(INFO.id.0)))?;
        for artifact in emitted.artifacts {
            out.put(artifact);
        }
        out.frames(emitted.frames);
        Ok(())
    }
}
