//! Targets as the compiler reads them: `lang.target.Spec` values of the `targets` package (spec
//! 13, section 7), and the names components declare what they handle by.

use std::path::{Path, PathBuf};

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

mod c;
mod check;
mod floats;
mod machine;
mod name;
mod run;
mod threads;
mod trace;

pub use c::{Atomics, Attributes, BuildsFor, CCompiler, CInt, CTypes, FrameSizes};
pub use check::{Check, Checks};
pub use floats::Floats;
pub use machine::Machine;
pub use name::Name;
pub use run::Run;
pub use threads::Threads;
pub use trace::Detail;

/// `lang.target.Endian`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Endian {
    Little,
    Big,
}

/// `lang.target.Convention`: the calling convention of a target's C functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Convention {
    Sysv,
    Aapcs64,
    DarwinArm64,
    Lp64d,
    AapcsVfp,
    Aapcs,
    Ilp32,
    Wasm32,
}

/// How a build trades compile time for run time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Profile {
    /// Fast to build, easy to debug: the default.
    #[default]
    Debug,
    /// Fast to run.
    Release,
}

impl Profile {
    /// The name its builds are called by: `debug`, `release`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

/// `lang.target.Backends`: the backends a target builds with under each profile, most preferred
/// first.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Backends {
    pub debug: Vec<Symbol>,
    pub release: Vec<Symbol>,
}

impl Backends {
    /// The backends preferred under `profile`, in order.
    #[must_use]
    pub fn of(&self, profile: Profile) -> &[Symbol] {
        match profile {
            Profile::Debug => &self.debug,
            Profile::Release => &self.release,
        }
    }
}

/// `lang.target.Version`: a release of a system, shown as tools spell it (`11.0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// `lang.target.Spec`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct TargetSpec {
    pub arch: Symbol,
    /// The instruction set its code is written in, which inline assembly names.
    pub isa: Symbol,
    /// What its processor has of the instruction set beyond what every processor of it has.
    pub features: Vec<Symbol>,
    pub cpu: Symbol,
    pub os: Symbol,
    pub abi: Symbol,
    pub convention: Convention,
    pub pointer_bits: u32,
    /// The width of a general register: the widest integer the processor computes with.
    pub register_bits: u32,
    pub floats: Floats,
    /// How many threads its programs run.
    pub threads: Threads,
    pub endian: Endian,
    pub object_format: Symbol,
    /// The linker's entry symbol; `None` for libraries (spec 08, rule 2).
    pub entry_symbol: Option<Symbol>,
    /// The oldest release of its system its programs run on, where its binaries record one.
    pub os_version: Option<Version>,
    pub backends: Backends,
    pub linker: Symbol,
    pub c: CTypes,
}

/// Whether a component does its work yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// It generates code, or links.
    Ready,
    /// Registered so that targets can name it, reporting that it is unavailable.
    Planned,
}

/// The names of an open set a component handles, such as architectures or object formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Names {
    Any,
    Only(&'static [Name]),
}

impl Names {
    #[must_use]
    pub fn contains(self, name: Symbol) -> bool {
        match self {
            Self::Any => true,
            Self::Only(list) => list.iter().any(|n| n.is(name)),
        }
    }
}

/// Whether a component supports a target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    No(Unsupported),
}

/// Why a component cannot handle a target or request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Arch(Symbol),
    ObjectFormat(Symbol),
    MemoryRegions,
    Output(Output),
}

/// What a program is built as (`std.build.Output`): an executable a system runs, or a library
/// other programs link, shared (loaded when they run) or static (copied into them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Output {
    #[default]
    Executable,
    Shared,
    Static,
}

impl Output {
    pub const ALL: [Self; 3] = [Self::Executable, Self::Shared, Self::Static];

    /// Its name, as `std.build.Output` and the command line spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::Shared => "shared",
            Self::Static => "static",
        }
    }

    /// Whether it is a library, which has no entry symbol.
    #[must_use]
    pub const fn library(self) -> bool {
        !matches!(self, Self::Executable)
    }

    /// The file in `dir` the program `name` is built into, as the object format `format` names
    /// it: a library with its kind's prefix and extension, an executable as its program.
    #[must_use]
    pub fn file(self, dir: &Path, name: &str, format: Symbol) -> PathBuf {
        let named = LIBRARIES.iter().find(|l| l.format.is(format));
        let file = match (self, named) {
            (Self::Shared, Some(l)) => l.shared.file(name),
            (Self::Static, Some(l)) => l.archive.file(name),
            _ => name.to_owned(),
        };
        dir.join(file)
    }
}

/// How an object format names its libraries, shared and static.
struct Libraries {
    format: Name,
    shared: Naming,
    archive: Naming,
}

/// A prefix before a program's name, and an extension after it.
struct Naming {
    prefix: &'static str,
    extension: &'static str,
}

impl Naming {
    fn file(&self, name: &str) -> String {
        format!("{}{name}.{}", self.prefix, self.extension)
    }
}

const fn named(prefix: &'static str, extension: &'static str) -> Naming {
    Naming { prefix, extension }
}

const LIBRARIES: [Libraries; 4] = [
    Libraries { format: Name("elf"), shared: named("lib", "so"), archive: named("lib", "a") },
    Libraries { format: Name("macho"), shared: named("lib", "dylib"), archive: named("lib", "a") },
    Libraries { format: Name("coff"), shared: named("", "dll"), archive: named("", "lib") },
    Libraries { format: Name("wasm"), shared: named("", "wasm"), archive: named("lib", "a") },
];
