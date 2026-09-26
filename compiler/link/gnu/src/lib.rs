//! GNU ld, driven by the target's C compiler (`lang.target.CCompiler`), which passes it the C
//! runtime's start files and libraries on hosted targets; on bare metal the compiler's link flags
//! leave them out. Memory regions and section placement become a linker script. The driving is
//! shared with the linkers that stand in for GNU ld (`Ld`).

mod script;

use std::path::Path;

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
        Ld::Own.link(&INFO, plan, target, diag)
    }
}

/// The linker a GCC-style compiler driver runs: its own, one a flag names, or the `ld` of a
/// directory it looks in first. Linkers that stand in for GNU ld (mold, wild) are driven so, the
/// driver passing the C runtime's start files and libraries as it does to GNU ld.
#[derive(Clone, Copy, Debug)]
pub enum Ld<'a> {
    Own,
    Flag(&'static str),
    In(&'a Path),
}

impl Ld<'_> {
    /// Links `plan` through the target's C compiler, running this linker; memory regions and
    /// section placement become a linker script.
    ///
    /// # Errors
    /// Returns once an error has been reported to `diag`: an output other than an executable,
    /// a script not written, or a link that failed.
    pub fn link(
        self,
        info: &LinkerInfo,
        plan: &LinkPlan,
        target: &TargetSpec,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        if plan.kind != OutputKind::Executable {
            return Err(info.unavailable(diag));
        }
        let compiler = &target.c.compiler;
        let mut args: Vec<Arg> = compiler.link_flags.iter().map(|&f| Arg::Text(f)).collect();
        match self {
            Self::Own => {}
            Self::Flag(flag) => args.push(flag.into()),
            Self::In(dir) => args.extend([gcc::PROGRAMS.into(), Arg::Path(dir.to_path_buf())]),
        }
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
