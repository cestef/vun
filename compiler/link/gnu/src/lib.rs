//! GNU ld, driven by the target's C compiler (`lang.target.CCompiler`), which passes it the C
//! runtime's start files and libraries on hosted targets; on bare metal the compiler's link flags
//! leave them out. Memory regions and section placement become a linker script.

mod script;

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Edit;
use vun_link::{LinkPlan, Linker, LinkerInfo, OutputKind};
use vun_target::{Name, Readiness, TargetSpec};
use vun_tool::{Arg, Executable, Invocation, Tool, gcc};

pub use script::Script;

pub const INFO: LinkerInfo =
    LinkerInfo { id: Name("gnu"), formats: &[Name("elf")], regions: true, readiness: Readiness::Ready };

/// The extension of a linker script written beside the output.
const SCRIPT: &str = "ld";

#[derive(Default)]
pub struct GnuLinker;

impl Linker for GnuLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        if plan.kind != OutputKind::Executable {
            return Err(INFO.unavailable(diag));
        }
        let compiler = &target.c.compiler;
        let mut args: Vec<Arg> = compiler.link_flags.iter().map(|&f| Arg::Text(f)).collect();
        args.extend([gcc::OUTPUT.into(), Arg::Path(plan.output.clone())]);
        if let Some(entry) = plan.entry {
            args.extend([gcc::ENTRY.into(), Arg::Text(entry)]);
        }
        let mut inputs = plan.inputs.clone();
        if !plan.regions.is_empty() {
            let path = plan.output.with_extension(SCRIPT);
            path.put(Script::of(plan).to_string()).map_err(|e| {
                let msg = Msg::new().text("cannot write ").code(path.display().to_string());
                diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e))
            })?;
            args.extend([gcc::SCRIPT.into(), Arg::Path(path.clone())]);
            inputs.push(path);
        }
        args.extend(plan.inputs.iter().cloned().map(Arg::Path));
        let inv = Invocation { args, inputs, outputs: vec![plan.output.clone()], ..Invocation::default() };
        let driver = compiler.command.as_str();
        Executable::new(driver, driver).check(&inv).map(drop).map_err(|failure| {
            let msg =
                Msg::new().code(driver).text(" could not link ").code(plan.output.display().to_string());
            diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
        })
    }
}

#[cfg(test)]
mod tests;
