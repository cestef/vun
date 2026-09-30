//! The wasm linker (spec 13, section 8.4): relocatable objects in the tool conventions' format,
//! the wasm backend's or clang's, linked in process into a program, with no `wasm-ld`. Symbols
//! resolve across the objects (`resolve`), strong over weak and the first COMDAT group over
//! later ones; a function none defines stays an import of the library its object names; what the
//! entry, the exports and the symbols kept do not reach is left out (`live`); the stack comes
//! first in memory, the sections after it (`layout`); the functions whose addresses are taken
//! fill the indirect function table as relocations are applied (`patch`).

mod layout;
mod live;
mod patch;
mod program;
mod provided;
mod resolve;
mod section;
mod unlinked;
mod write;

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Edit;
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness, TargetSpec};
use vun_wasm::Object;

pub use layout::STACK;
pub use unlinked::Unlinked;

use program::Program;

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("wasm"),
    formats: &[Name("wasm")],
    regions: false,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
    runs: &[],
};

#[derive(Default)]
pub struct WasmLinker;

impl WasmLinker {
    /// The program `objects` make, entered at `entry` if it has one.
    ///
    /// # Errors
    /// Returns why they do not link.
    pub fn program(objects: &[Vec<u8>], entry: Option<&str>) -> Result<Vec<u8>, Unlinked> {
        let read = objects.iter().enumerate();
        let read =
            read.map(|(input, bytes)| Object::read(bytes).map_err(|why| Unlinked::Malformed { input, why }));
        let objects = read.collect::<Result<Vec<_>, _>>()?;
        Program::of(&objects, entry)?.write()
    }
}

impl Linker for WasmLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, _: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let read = plan
            .inputs
            .iter()
            .map(|path| std::fs::read(path).map_err(|e| diag.error(Diagnostic::unreadable(path, &e))));
        let objects = read.collect::<Result<Vec<_>, _>>()?;
        vun_trace::tracing::debug!(output = %plan.output.display(), inputs = objects.len(), "linking");
        let entry = plan.entry.map(vun_intern::Symbol::as_str);
        let program = Self::program(&objects, entry).map_err(|unlinked| {
            let id = match unlinked {
                Unlinked::Malformed { .. } | Unlinked::Relocation { .. } => errors::LINK_MALFORMED,
                Unlinked::Undefined(_) => errors::LINK_UNDEFINED,
                Unlinked::Duplicate(_) | Unlinked::Clash { .. } => errors::LINK_DUPLICATE,
            };
            let msg = match &unlinked {
                Unlinked::Malformed { input, .. } => {
                    Msg::new().text("cannot read the object ").code(plan.inputs[*input].display().to_string())
                }
                _ => Msg::new().text("cannot link ").code(plan.output.display().to_string()),
            };
            diag.error(Diagnostic::error(id, msg).caused_by(&unlinked))
        })?;
        plan.output.put(program).map_err(|e| diag.error(Diagnostic::unwritable(&plan.output, &e)))
    }
}
