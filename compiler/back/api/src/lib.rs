//! The `Backend` interface and artifacts.
//!
//! Nothing above LIR knows about any backend. A backend declares its facts once, in a
//! [`BackendInfo`]; support and capability checks are derived from it (spec 13, section 7).

mod assist;
mod compiled;
mod lack;

pub use assist::{Assist, Assisted, Unassisted};
pub use compiled::Compiled;
pub use lack::{AsmOperand, Lack, Missing};

mod caps;
mod trace;

pub use trace::Traced;

pub use caps::{BackendCaps, Feature, HeldFloats, Widest};

use std::path::PathBuf;
use vun_codec::{Decode, Encode};

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_intern::Symbol;
use vun_lir::FuncId;
use vun_lir::unit::Unit;
use vun_lir_passes::legalize::Division;
use vun_origin::OriginId;
pub use vun_target::Readiness;
use vun_target::{Name, Names, Profile, Run, Support, TargetSpec, Unsupported};

/// What a backend produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Emits {
    Object,
    /// Assembly text, for an assembler tool.
    Assembly,
    /// Source code, for a compiler tool.
    Source,
    /// Source other sources include, compiled only through them.
    Header,
    /// Code in memory.
    Jit,
    /// The code as text for people, with where each part comes from (`vun debug dump code`).
    Listing,
}

impl Emits {
    /// Its name, as `vun component list` lists it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Assembly => "assembly",
            Self::Source => "source",
            Self::Header => "header",
            Self::Jit => "jit",
            Self::Listing => "listing",
        }
    }

    /// The extension of a file of this kind.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Object => "o",
            Self::Assembly => "s",
            Self::Source => "c",
            Self::Header => "h",
            Self::Jit => "bin",
            Self::Listing => "lst",
        }
    }
}

/// Which part of a program an artifact holds: all of it, or, for a program a backend splits, what
/// the other parts share, its data, or one shard of its functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Part {
    Whole,
    Shared,
    Data,
    Shard(u32),
}

impl Part {
    /// The file holding this part as `kind`: every artifact is named here, by what it is.
    #[must_use]
    pub fn file(self, kind: Emits) -> PathBuf {
        let stem = match self {
            Self::Whole | Self::Shared => "program".to_owned(),
            Self::Data => "data".to_owned(),
            Self::Shard(n) => format!("shard{n}"),
        };
        PathBuf::from(stem).with_extension(kind.extension())
    }
}

/// Everything known about a backend without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct BackendInfo {
    pub id: Name,
    pub arches: Names,
    /// The object formats it writes; any for a backend emitting source.
    pub formats: Names,
    pub caps: BackendCaps,
    pub readiness: Readiness,
    /// The programs it runs.
    pub runs: &'static [Run],
}

impl BackendInfo {
    #[must_use]
    pub fn supports(&self, target: &TargetSpec) -> Support {
        if !self.arches.contains(target.arch) {
            Support::No(Unsupported::Arch(target.arch))
        } else if !self.formats.contains(target.object_format) {
            Support::No(Unsupported::ObjectFormat(target.object_format))
        } else {
            Support::Yes
        }
    }

    /// Reports that this backend cannot generate code yet.
    pub fn unavailable(&self, diag: &mut DiagCtxt) -> ErrorGuaranteed {
        let msg = Msg::new().text("backend ").code(self.id.0).text(" cannot generate code yet");
        diag.error(Diagnostic::error(errors::BACKEND_UNAVAILABLE, msg))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Artifact {
    pub part: Part,
    pub kind: Emits,
    pub bytes: Vec<u8>,
}

impl Artifact {
    /// Its file, relative to the output directory.
    #[must_use]
    pub fn file(&self) -> PathBuf {
        self.part.file(self.kind)
    }
}

/// What a backend emits into: its artifacts and, from a backend that knows them while
/// generating code, each function's frame size in bytes.
pub trait ArtifactSink {
    fn put(&mut self, artifact: Artifact);

    fn frames(&mut self, sizes: Vec<(FuncId, u64)>);
}

/// What a backend emitted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Emitted {
    pub artifacts: Vec<Artifact>,
    /// Each function's frame size in bytes, when the backend reports them itself.
    pub frames: Vec<(FuncId, u64)>,
}

impl ArtifactSink for Emitted {
    fn put(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
    }

    fn frames(&mut self, sizes: Vec<(FuncId, u64)>) {
        self.frames = sizes;
    }
}

/// A line of source, for the line information a backend writes (`#line`, DWARF line tables).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Line {
    pub file: PathBuf,
    /// Counted from 1.
    pub line: u32,
    /// Counted from 1, in characters.
    pub column: u32,
}

/// Where origins are in source (spec 10, section 1).
pub trait Lines {
    fn line(&self, origin: OriginId) -> Option<Line>;

    /// The declaration `origin` is in, and those around it, outermost first: what names an
    /// internal function's symbol for debuggers and profilers.
    fn declaration(&self, origin: OriginId) -> Vec<Symbol> {
        let _ = origin;
        Vec::new()
    }

    /// The code `origin` is a copy of, and the function declared at the second origin it is
    /// inlined into, where it is inlined code.
    fn inlined(&self, origin: OriginId) -> Option<(OriginId, OriginId)> {
        let _ = origin;
        None
    }

    /// The name debuggers show for an internal definition at `origin`: the path of its
    /// declaration, if it has one.
    fn name(&self, origin: OriginId) -> Option<String> {
        let path = self.declaration(origin);
        (!path.is_empty()).then(|| path.iter().map(|name| name.as_str()).collect::<Vec<_>>().join("."))
    }
}

/// What a backend is asked to generate code for.
pub struct Request<'a> {
    /// The program, legalized for the backend.
    pub unit: &'a Unit,
    pub target: &'a TargetSpec,
    pub lines: &'a dyn Lines,
    /// What compiles the functions the backend cannot express.
    pub assist: &'a dyn Assist,
    pub profile: Profile,
    /// Whether to add a listing of the code to the artifacts ([`Emits::Listing`]).
    pub listing: bool,
}

/// A backend declared, which cannot generate code yet: its facts, for choosing and reporting.
#[derive(Clone, Copy, Debug)]
pub struct Planned(pub &'static BackendInfo);

impl Backend for Planned {
    fn info(&self) -> &BackendInfo {
        self.0
    }

    fn emit(
        &self,
        _: &Request<'_>,
        _: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Err(self.0.unavailable(diag))
    }
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
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed>;

    /// How it divides integers no wider than its widest on `target`: by an instruction, unless
    /// it emits the processor's own and the processor has none (`Legalize`).
    /// What compiles functions for another backend, if it does (`Feature::Assist`).
    fn assist(&self) -> Option<&dyn Assist> {
        None
    }

    fn division(&self, target: &TargetSpec) -> Division {
        let _ = target;
        Division::Computed
    }

    /// The frame size of each function of `unit`, in bytes, from what the tools building its
    /// artifacts reported (`report`); none for a backend that reads no report, such as one
    /// reporting them while it emits ([`ArtifactSink::frames`]).
    fn frame_sizes(&self, unit: &Unit, report: &[u8]) -> Vec<(FuncId, u64)> {
        let _ = (unit, report);
        Vec::new()
    }
}
