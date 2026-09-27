//! lld: ELF, COFF, Mach-O and wasm with linker scripts; the default for embedded targets.

use vun_link::{Linker, LinkerInfo};
use vun_target::{Name, Readiness};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("lld"),
    formats: &[Name("elf"), Name("coff"), Name("macho"), Name("wasm")],
    regions: true,
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct LldLinker;

impl Linker for LldLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }
}
