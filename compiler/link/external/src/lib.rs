//! External linkers that read the link plan as JSON.

use vun_link::{Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("external"),
    formats: &[Name("elf"), Name("coff"), Name("macho"), Name("wasm")],
    regions: true,
    outputs: &Output::ALL,
    readiness: Readiness::Planned,
    runs: &[],
};

#[derive(Default)]
pub struct ExternalLinker;

impl Linker for ExternalLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }
}
