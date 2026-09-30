//! The native backend (spec 13, section 8.3): machine code for debug builds, in one pass per
//! function over LIR and its shared analyses, in the design of TPDE. The framework is
//! `vun-codegen` and `vun-native`; each architecture's calling conventions, snippets and
//! instruction compilers are a crate of their own (`vun-native-x86_64`, `vun-native-arm64`,
//! `vun-native-thumb`), listed here with the object formats written for them: each in ELF
//! objects, and arm64 in Mach-O objects.

#[cfg(test)]
mod tests;

use vun_backend::{
    ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Emitted, Feature, HeldFloats, Lack, Missing,
    Readiness, Request, Widest,
};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir_passes::legalize::{Division, Half};
use vun_native::Native;
use vun_native_arm64::Arm;
use vun_native_thumb::Thumb2;
use vun_native_x86_64::X86;
use vun_object::{Float, Format, Machine};
use vun_target::{Name, Names, TargetSpec};

/// An architecture the backend generates code for: its name among targets', the machine of a
/// target's objects, and the compilation of a unit into one.
struct Arch {
    name: Name,
    machine: fn(&TargetSpec) -> Machine,
    /// Whether it follows a target's calling convention.
    follows: fn(&TargetSpec) -> bool,
    object: fn(&Request<'_>, Machine, Format) -> Result<Emitted, Lack>,
    division: fn(&TargetSpec) -> Division,
}

/// The object formats the backend writes, by their names among targets', and the machines it
/// writes each for.
const FORMATS: [(Name, Format, &[Machine]); 2] = [
    (
        Name("elf"),
        Format::Elf,
        &[Machine::X86_64, Machine::Aarch64, Machine::Arm(Float::Soft), Machine::Arm(Float::Hard)],
    ),
    (Name("macho"), Format::MachO, &[Machine::Aarch64]),
];

const ARCHES: [Arch; 3] = [
    Arch {
        name: Name("x86_64"),
        machine: X86::machine,
        follows: |t| X86::convention(t).is_some(),
        object: X86::object,
        division: X86::division,
    },
    Arch {
        name: Name("arm64"),
        machine: Arm::machine,
        follows: |t| Arm::convention(t).is_some(),
        object: Arm::object,
        division: Arm::division,
    },
    Arch {
        name: Name("arm"),
        machine: Thumb2::machine,
        follows: |t| Thumb2::convention(t).is_some(),
        object: Thumb2::object,
        division: Thumb2::division,
    },
];

const FORMAT_NAMES: [Name; FORMATS.len()] = {
    let mut names = [Name(""); FORMATS.len()];
    let mut i = 0;
    while i < FORMATS.len() {
        names[i] = FORMATS[i].0;
        i += 1;
    }
    names
};

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
    arches: Names::Only(&NAMES),
    formats: Names::Only(&FORMAT_NAMES),
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[Feature::CAbi, Feature::FrameSizes, Feature::DebugInfo],
        widest: Widest::Register,
        half: Half::Promoted,
        floats: HeldFloats::Computed,
    },
    readiness: Readiness::Ready,
    runs: &[],
};

#[derive(Default)]
pub struct NativeBackend;

impl NativeBackend {
    /// The format of the objects of `request`'s target, if the backend writes it for `machine`.
    fn format(request: &Request<'_>, machine: Machine) -> Option<Format> {
        let found = FORMATS
            .iter()
            .find(|(name, _, machines)| name.is(request.target.object_format) && machines.contains(&machine));
        found.map(|&(_, format, _)| format)
    }

    /// The architecture of `target`.
    ///
    /// # Panics
    /// For an architecture [`INFO`] does not list, which no build chooses the backend for.
    fn arch(target: &TargetSpec) -> &'static Arch {
        ARCHES
            .iter()
            .find(|a| a.name.is(target.arch))
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
        let arch = Self::arch(request.target);
        let machine = (arch.machine)(request.target);
        let format = Self::format(request, machine).expect("checked before emitting");
        (arch.object)(request, machine, format)
    }
}

impl Backend for NativeBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn division(&self, target: &TargetSpec) -> Division {
        (Self::arch(target).division)(target)
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let arch = Self::arch(request.target);
        if !(arch.follows)(request.target) {
            return Err(diag.error(Missing::Convention.diagnostic(INFO.id)));
        }
        if Self::format(request, (arch.machine)(request.target)).is_none() {
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
