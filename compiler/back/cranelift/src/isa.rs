//! The processor Cranelift generates code for: chosen from the target's architecture, with the
//! settings every function is compiled under.

use std::sync::Arc;

use cranelift_codegen::isa::{self, TargetIsa};
use cranelift_codegen::settings::{self, Configurable};
use target_lexicon::{
    Aarch64Architecture, Architecture, BinaryFormat, Environment, OperatingSystem, Triple, Vendor,
};
use vun_abi::Rules;
use vun_backend::Missing;
use vun_intern::Symbol;
use vun_lir::Signature;
use vun_object::{Format, Machine};
use vun_target::{Convention, Name, Profile, TargetSpec};

use crate::plan::Placed;

/// An architecture the backend generates code for, by the feature that brings its Cranelift
/// support: its name among targets', the machine of its objects and Cranelift's name for it.
struct Arch {
    name: Name,
    machine: Machine,
    architecture: Architecture,
    place: Place,
}

/// How a signature's records are placed under a convention of the architecture.
type Place = fn(&Signature, Convention) -> Result<Placed, Missing>;

macro_rules! arches {
    ($($feature:literal => $name:literal, $machine:expr, $architecture:expr, $place:expr;)*) => {
        const ARCHES: &[Arch] = &[$(
            #[cfg(feature = $feature)]
            Arch { name: Name($name), machine: $machine, architecture: $architecture, place: $place },
        )*];

        /// The architectures compiled in, as targets name them.
        pub(crate) const NAMES: &[Name] = &[$(
            #[cfg(feature = $feature)]
            Name($name),
        )*];
    };
}

arches! {
    "x86_64" => "x86_64", Machine::X86_64, Architecture::X86_64,
        |s, c| Placed::by(s, vun_asm_x86_64::conventions::CONVENTIONS, c, true);
    // Cranelift has no struct arguments on arm64: a large record is copied and passed by address.
    "arm64" => "arm64", Machine::Aarch64, Architecture::Aarch64(Aarch64Architecture::Aarch64),
        |s, c| Placed::by(s, vun_asm_arm64::conventions::CONVENTIONS, c, false);
}

/// An object format the backend writes: its name among targets', Cranelift's name for it, the
/// object writer's, and how code reaches storage per thread in it.
struct Written {
    name: Name,
    binary: BinaryFormat,
    writer: Format,
    tls_model: &'static str,
}

const FORMATS: [Written; 1] =
    [Written { name: Name("elf"), binary: BinaryFormat::Elf, writer: Format::Elf, tls_model: "elf_gd" }];

/// The names of the formats, as `INFO` lists them.
pub(crate) const FORMAT_NAMES: [Name; FORMATS.len()] = {
    let mut names = [Name(""); FORMATS.len()];
    let mut i = 0;
    while i < FORMATS.len() {
        names[i] = FORMATS[i].name;
        i += 1;
    }
    names
};

/// The operating systems targets name that Cranelift tells apart; any other is none.
const SYSTEMS: &[(Name, OperatingSystem)] = &[(Name("linux"), OperatingSystem::Linux)];

/// What every function is compiled under, besides the optimization level.
const SETTINGS: &[(&str, &str)] = &[
    // Position-independent, so that programs link as executables of either kind.
    ("is_pic", "true"),
    // A frame pointer in every function: the chain a backtrace walks (spec 07, section 3).
    ("preserve_frame_pointers", "true"),
    ("unwind_info", "false"),
    // No runtime provides a function stack probes could call.
    ("probestack_strategy", "inline"),
];

/// The optimization level of a profile's builds.
const fn optimization(profile: Profile) -> &'static str {
    match profile {
        Profile::Debug => "none",
        Profile::Release => "speed",
    }
}

/// A processor and the machine its objects are for.
pub(crate) struct Isa {
    pub(crate) target: Arc<dyn TargetIsa>,
    pub(crate) machine: Machine,
    pub(crate) format: Format,
    /// How the target's C convention passes values, where this crate declares it.
    pub(crate) rules: Option<&'static Rules>,
    convention: Convention,
    place: Place,
}

impl Isa {
    /// The processor of `target`, its code optimized as `profile` asks.
    ///
    /// # Errors
    /// Returns the object format it does not write.
    ///
    /// # Panics
    /// For an architecture the backend does not list, which no build chooses it for.
    pub(crate) fn of(target: &TargetSpec, profile: Profile) -> Result<Self, Missing> {
        let (arch, os, format) = (target.arch, target.os, target.object_format);
        Self::build(arch, os, format, target.convention, profile)
    }

    /// The processor of `arch` and `os`, with objects of `format`.
    ///
    /// # Errors
    /// Returns the object format it does not write.
    ///
    /// # Panics
    /// For an architecture the backend does not list.
    pub(crate) fn build(
        arch: Symbol,
        os: Symbol,
        format: Symbol,
        convention: Convention,
        profile: Profile,
    ) -> Result<Self, Missing> {
        let written = FORMATS.iter().find(|w| w.name.is(format)).ok_or(Missing::Format(format))?;
        let arch = ARCHES
            .iter()
            .find(|a| a.name.is(arch))
            .expect("the backend is chosen only for the architectures it lists");
        let operating_system = SYSTEMS
            .iter()
            .find(|(name, _)| name.is(os))
            .map_or(OperatingSystem::None_, |&(_, system)| system);
        let triple = Triple {
            architecture: arch.architecture,
            vendor: Vendor::Unknown,
            operating_system,
            environment: Environment::Unknown,
            binary_format: written.binary,
        };
        let mut flags = settings::builder();
        for (name, value) in SETTINGS
            .iter()
            .copied()
            .chain([("opt_level", optimization(profile)), ("tls_model", written.tls_model)])
        {
            flags.set(name, value).expect("the settings are Cranelift's own");
        }
        let target = isa::lookup(triple)
            .expect("the architecture's support is compiled in")
            .finish(settings::Flags::new(flags))
            .expect("the settings suit the architecture");
        let rules = Rules::of(convention);
        Ok(Self {
            target,
            machine: arch.machine,
            format: written.writer,
            rules,
            convention,
            place: arch.place,
        })
    }

    /// Where a call of the C convention `signature` places its records.
    pub(crate) fn place(&self, signature: &Signature) -> Result<Placed, Missing> {
        (self.place)(signature, self.convention)
    }
}
