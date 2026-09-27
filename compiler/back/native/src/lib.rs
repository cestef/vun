//! The native backend (spec 13, section 8.3): machine code for debug builds, in one pass per
//! function over LIR and its shared analyses, in the design of TPDE. The framework is
//! `vun-codegen`, the assemblers `vun-asm-*`, the object writer `vun-object`; this crate holds
//! what is particular to each architecture: calling conventions, snippets and instruction
//! compilers. x86-64 and arm64, in ELF objects, and arm64 in Mach-O objects.

pub mod arm64;
mod listing;
mod object;
pub mod width;
pub mod x86_64;

#[cfg(test)]
mod tests;

use std::fmt::Display;
use std::path::PathBuf;

use vun_asm::Isa;
use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Emitted, Feature, Lack, Missing,
    Readiness, Request,
};
use vun_codegen::{Compiled, Isel};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_index::IndexVec;
use vun_lir::analysis::Reached;
use vun_lir::legalize::Half;
use vun_lir::unit::Unit;
use vun_lir::{Address, FuncId, Function};
use vun_object::{Format, Machine};

use object::Assembly;
use vun_par::Parallel;
use vun_target::{Arches, Name, TargetSpec};

pub use listing::Listing;

/// An architecture the backend generates code for: its name among targets', the machine of its
/// objects, and the compilation of a unit into one.
struct Arch {
    name: Name,
    machine: Machine,
    object: fn(&Request<'_>, Machine, Format) -> Result<Emitted, Lack>,
}

/// An architecture's instruction compilers, with the calling convention a target follows.
pub trait Native: Isel {
    /// How its call frames are described to unwinders.
    const CALL_FRAME: vun_dwarf::CallFrame;

    fn convention(target: &TargetSpec) -> Self::Convention;
}

/// The object formats the backend writes, by their names among targets', and the machines it
/// writes each for.
const FORMATS: [(Name, Format, &[Machine]); 2] = [
    (Name("elf"), Format::Elf, &[Machine::X86_64, Machine::Aarch64]),
    (Name("macho"), Format::MachO, &[Machine::Aarch64]),
];

const ARCHES: [Arch; 2] = [
    Arch { name: Name("x86_64"), machine: Machine::X86_64, object: NativeBackend::object::<x86_64::X86> },
    Arch { name: Name("arm64"), machine: Machine::Aarch64, object: NativeBackend::object::<arm64::Arm> },
];

const NAMES: [Name; ARCHES.len()] = {
    let mut names = [Name(""); ARCHES.len()];
    let mut i = 0;
    while i < ARCHES.len() {
        names[i] = ARCHES[i].name;
        i += 1;
    }
    names
};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("native"),
    arches: Arches::Only(&NAMES),
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[Feature::CAbi, Feature::FrameSizes, Feature::DebugInfo],
        int_bits: 64,
        half: Half::Promoted,
    },
    readiness: Readiness::Ready,
};

/// The object's name among the build's artifacts, and its listing's.
const OBJECT: &str = "program.o";
const LISTING: &str = "program.lst";

/// A function compiled for the architecture `A`.
pub type Code<A> = Compiled<<<A as vun_codegen::Arch>::Isa as Isa>::Inst<Address>>;

/// Bytes a call pushes above a frame: the return address and the saved frame pointer.
const LINKAGE: u64 = 16;

#[derive(Default)]
pub struct NativeBackend;

impl NativeBackend {
    /// The machine code of every function of `unit` for `A`, compiled in parallel, listed when
    /// `listed`.
    ///
    /// # Errors
    /// Returns the first thing a function needs that the backend cannot compile yet.
    pub fn compile<A: Isel>(
        unit: &Unit,
        convention: A::Convention,
        listed: bool,
    ) -> Result<IndexVec<FuncId, Code<A>>, Lack>
    where
        Code<A>: Send,
    {
        // A function inlined everywhere it was called is left to nothing: it has no code.
        let reached = Reached::of(unit);
        let functions: Vec<(FuncId, &Function)> = unit.functions.iter().collect();
        let compiled = functions.par_map(|&(id, f)| {
            if !reached.contains(id) {
                return Ok(Compiled::unreached());
            }
            let _span = vun_trace::tracing::debug_span!("native", f = %f.linkage).entered();
            let compiled = vun_codegen::compile::<A>(unit, f, convention, listed);
            if let Ok(c) = &compiled {
                vun_trace::tracing::debug!(bytes = c.code.bytes.len(), frame = c.frame, "compiled");
            }
            compiled
        });
        Ok(compiled.into_iter().collect::<Result<Vec<_>, _>>()?.into())
    }

    /// The relocatable object of `request`'s unit for `A`, each function's frame size (with the
    /// return address and saved frame pointer, as C compilers count them), and its listing if
    /// asked.
    ///
    /// # Errors
    /// As [`Self::compile`].
    pub fn object<A: Native>(request: &Request<'_>, machine: Machine, format: Format) -> Result<Emitted, Lack>
    where
        Code<A>: Send,
        <A::Isa as Isa>::Inst<Address>: Display,
    {
        let code = Self::compile::<A>(request.unit, A::convention(request.target), request.listing)?;
        let bytes =
            Assembly { unit: request.unit, code: &code, lines: request.lines, frames: &A::CALL_FRAME }
                .object((machine, format))
                .write();
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

    /// The format of the objects of `request`'s target, if the backend writes it for `machine`.
    fn format(request: &Request<'_>, machine: Machine) -> Option<Format> {
        let found = FORMATS
            .iter()
            .find(|(name, _, machines)| name.is(request.target.object_format) && machines.contains(&machine));
        found.map(|&(_, format, _)| format)
    }

    /// The architecture of `request`'s target.
    ///
    /// # Panics
    /// For an architecture [`INFO`] does not list, which no build chooses the backend for.
    fn arch(request: &Request<'_>) -> &'static Arch {
        ARCHES
            .iter()
            .find(|a| a.name.is(request.target.arch))
            .expect("the backend is chosen only for the architectures it lists")
    }

    /// The object of `request`'s unit for its target's architecture.
    ///
    /// # Errors
    /// As [`Self::compile`].
    ///
    /// # Panics
    /// Panics for an architecture [`INFO`] does not list, which no build chooses the backend for.
    fn emitted(request: &Request<'_>) -> Result<Emitted, Lack> {
        let arch = Self::arch(request);
        let format = Self::format(request, arch.machine).expect("checked before emitting");
        (arch.object)(request, arch.machine, format)
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
        if Self::format(request, Self::arch(request).machine).is_none() {
            return Err(diag.error(Missing::Format(request.target.object_format).diagnostic(INFO.id)));
        }
        let emitted = Self::emitted(request).map_err(|lack| diag.error(lack.diagnostic(INFO.id)))?;
        for artifact in emitted.artifacts {
            out.put(artifact);
        }
        out.frames(emitted.frames);
        Ok(())
    }
}
