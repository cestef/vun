//! The `Linker` interface and the target-independent link plan.
//!
//! Memory regions and sections declared in the build go into a [`LinkPlan`]. Only adapters know
//! linker syntax: script-capable ones turn a plan into a linker script, others into flags. Each
//! adapter declares its facts once, in a [`LinkerInfo`] (spec 13, section 7).

mod bound;

use std::ffi::OsString;
use std::path::PathBuf;
use vun_codec::{Decode, Encode};

pub use bound::Bound;
use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_intern::Symbol;
use vun_origin::OriginId;
pub use vun_target::Output;
use vun_target::{Name, Readiness, Run, Support, TargetSpec, Unsupported};

/// A region of target memory, such as flash or RAM.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct MemoryRegion {
    pub name: Symbol,
    pub origin: u64,
    pub len: u64,
    /// Where the build declared it.
    pub declared: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct SectionPlacement {
    pub name: Symbol,
    /// Region holding the contents at run time.
    pub region: Symbol,
    /// Region holding the initial contents, when different (`.data` loaded from flash).
    pub load_region: Option<Symbol>,
    /// Keep even if unreferenced.
    pub keep: bool,
}

/// Everything a linker needs, independent of any linker's syntax.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct LinkPlan {
    /// Objects and archives, in order.
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub kind: Output,
    /// From the target (spec 08, rule 2).
    pub entry: Option<Symbol>,
    /// The link symbols a library makes available to what loads it: its interface. A program has
    /// none, the symbols its runtime exports being its own.
    pub exports: Vec<Symbol>,
    pub regions: Vec<MemoryRegion>,
    pub sections: Vec<SectionPlacement>,
}

impl LinkPlan {
    /// Whether it places anything in memory: regions, or sections in them.
    #[must_use]
    pub fn places(&self) -> bool {
        !self.regions.is_empty() || !self.sections.is_empty()
    }

    /// A file written beside the output for the link: its name, then `.` and `extension`.
    #[must_use]
    pub fn beside(&self, extension: &str) -> PathBuf {
        let mut name = self.output.file_name().map(OsString::from).unwrap_or_default();
        name.push(".");
        name.push(extension);
        self.output.with_file_name(name)
    }
}

/// Everything known about a linker adapter without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LinkerInfo {
    pub id: Name,
    /// The object formats it links, as targets name them.
    pub formats: &'static [Name],
    /// Whether memory regions and section placement can be expressed (linker scripts).
    pub regions: bool,
    /// What it builds programs as.
    pub outputs: &'static [Output],
    pub readiness: Readiness,
    /// The programs it runs, whatever the target; see [`Linker::runs`].
    pub runs: &'static [Run],
}

impl LinkerInfo {
    #[must_use]
    pub fn supports(&self, target: &TargetSpec, plan: &LinkPlan) -> Support {
        if !self.formats.iter().any(|f| f.is(target.object_format)) {
            return Support::No(Unsupported::ObjectFormat(target.object_format));
        }
        if !self.regions && plan.places() {
            return Support::No(Unsupported::MemoryRegions);
        }
        if !self.outputs.contains(&plan.kind) {
            return Support::No(Unsupported::Output(plan.kind));
        }
        Support::Yes
    }

    /// Reports that this adapter cannot link yet.
    pub fn unavailable(&self, diag: &mut DiagCtxt) -> ErrorGuaranteed {
        let msg = Msg::new().text("linker ").code(self.id.0).text(" cannot link yet");
        diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg))
    }
}

pub trait Linker {
    fn info(&self) -> &LinkerInfo;

    /// The programs it runs to link for `target`.
    fn runs(&self, target: &TargetSpec) -> Vec<Run> {
        let _ = target;
        self.info().runs.to_vec()
    }

    /// Links; an adapter that is only planned reports that it cannot yet.
    ///
    /// # Errors
    /// Returns once an error has been reported to `diag`.
    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let _ = (plan, target);
        Err(self.info().unavailable(diag))
    }
}
