//! The `Linker` interface and the target-independent link plan.
//!
//! Memory regions and sections declared in the build go into a [`LinkPlan`]. Only adapters know
//! linker syntax: script-capable ones turn a plan into a linker script, others into flags. Each
//! adapter declares its facts once, in a [`LinkerInfo`] (spec 13, section 7).

use std::path::PathBuf;

use overt_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use overt_origin::OriginId;
use overt_target::{ObjectFormat, Support, TargetSpec, Unsupported};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Executable,
    SharedLibrary,
    /// For foreign build systems.
    StaticLibrary,
}

/// A region of target memory, such as flash or RAM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRegion {
    pub name: String,
    pub origin: u64,
    pub len: u64,
    /// Where the build declared it.
    pub declared: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionPlacement {
    pub name: String,
    /// Region holding the contents at run time.
    pub region: String,
    /// Region holding the initial contents, when different (`.data` loaded from flash).
    pub load_region: Option<String>,
    /// Keep even if unreferenced.
    pub keep: bool,
}

/// Everything a linker needs, independent of any linker's syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPlan {
    /// Objects and archives, in order.
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub kind: OutputKind,
    /// From the target (spec 08, rule 2).
    pub entry: Option<String>,
    pub regions: Vec<MemoryRegion>,
    pub sections: Vec<SectionPlacement>,
}

/// Everything known about a linker adapter without running it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkerInfo {
    pub id: &'static str,
    pub formats: &'static [ObjectFormat],
    /// Whether memory regions and section placement can be expressed (linker scripts).
    pub regions: bool,
}

impl LinkerInfo {
    #[must_use]
    pub fn supports(&self, target: &TargetSpec, plan: &LinkPlan) -> Support {
        if !self.formats.contains(&target.object_format) {
            return Support::No(Unsupported::ObjectFormat(target.object_format));
        }
        if !self.regions && !plan.regions.is_empty() {
            return Support::No(Unsupported::MemoryRegions);
        }
        Support::Yes
    }

    /// Reports that this adapter cannot link yet.
    pub fn unavailable(&self, diag: &mut DiagCtxt) -> ErrorGuaranteed {
        let msg = Msg::new().text("linker ").code(self.id).text(" cannot link yet");
        diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg))
    }
}

pub trait Linker {
    fn info(&self) -> &LinkerInfo;

    /// Links.
    ///
    /// # Errors
    /// Returns once an error has been reported to `diag`.
    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed>;
}
