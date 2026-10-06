//! The native backend (spec 13, section 8.3): machine code for debug builds, in one pass per
//! function over LIR and its shared analyses, in the design of TPDE. The framework is
//! `vun-codegen` and `vun-native`; each architecture's calling conventions, snippets and
//! instruction compilers are a crate of their own (`vun-native-x86_64`, `vun-native-arm64`,
//! `vun-native-thumb`), listed here with the object formats written for them: each in ELF
//! objects, and arm64 in Mach-O objects.

#[cfg(test)]
mod tests;

use vun_backend::{
    ArtifactSink, Assist, Assisted, Backend, BackendCaps, BackendInfo, Emits, Emitted, Feature, HeldFloats,
    Lack, Missing, Readiness, Request, Widest,
};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::FuncId;
use vun_lir_passes::legalize::{Division, Half};
use vun_native::Native;
#[cfg(feature = "arm64")]
use vun_native_arm64::Arm;
#[cfg(feature = "thumb")]
use vun_native_thumb::Thumb2;
#[cfg(feature = "x86_64")]
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
    assist: fn(&Request<'_>, &[FuncId]) -> Assisted,
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

/// The architectures compiled in, each by the feature that brings its crate: its name among
/// targets' and the compilation of a unit into an object.
macro_rules! arches {
    ($($feature:literal => $name:literal, $arch:ty;)*) => {
        const ARCHES: &[Arch] = &[$(
            #[cfg(feature = $feature)]
            Arch {
                name: Name($name),
                machine: <$arch>::machine,
                follows: |t| <$arch>::convention(t).is_some(),
                object: <$arch>::object,
                assist: <$arch>::assist,
                division: <$arch>::division,
            },
        )*];

        const NAMES: &[Name] = &[$(
            #[cfg(feature = $feature)]
            Name($name),
        )*];
    };
}

arches! {
    "x86_64" => "x86_64", X86;
    "arm64" => "arm64", Arm;
    "thumb" => "arm", Thumb2;
}

const FORMAT_NAMES: [Name; FORMATS.len()] = {
    let mut names = [Name(""); FORMATS.len()];
    let mut i = 0;
    while i < FORMATS.len() {
        names[i] = FORMATS[i].0;
        i += 1;
    }
    names
};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("native"),
    arches: Names::Only(NAMES),
    formats: Names::Only(&FORMAT_NAMES),
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[Feature::CAbi, Feature::FrameSizes, Feature::DebugInfo, Feature::Assist],
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

impl Assist for NativeBackend {
    fn assists(&self, target: &TargetSpec) -> bool {
        Names::Only(NAMES).contains(target.arch) && (Self::arch(target).follows)(target)
    }

    fn compile(&self, request: &Request<'_>, ids: &[FuncId]) -> Assisted {
        (Self::arch(request.target).assist)(request, ids)
    }
}

impl Backend for NativeBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn assist(&self) -> Option<&dyn Assist> {
        Some(self)
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
