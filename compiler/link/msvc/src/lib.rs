//! MSVC link.exe for COFF targets.

use vun_link::{Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("msvc"),
    formats: &[Name("coff")],
    regions: false,
    outputs: &[Output::Executable],
    readiness: Readiness::Planned,
};

#[derive(Default)]
pub struct MsvcLinker;

impl Linker for MsvcLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }
}
