//! GNU ld, driven by the target's C compiler (`lang.target.CCompiler`), which passes it the C
//! runtime's start files and libraries on hosted targets; on bare metal the compiler's link flags
//! leave them out. Memory regions and section placement become a linker script. The driving is
//! shared with the linkers that stand in for GNU ld (`Ld`).

mod script;

use std::path::Path;

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed};
use vun_fs::Edit;
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness, Run, TargetSpec};
use vun_tool::{Arg, Executable, Invocation, Task, Tool, gcc};

pub use script::{Script, Unscripted};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("gnu"),
    formats: &[Name("elf")],
    regions: true,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
    runs: &[Run::Compiler],
};

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
        let compiler = &target.c.compiler;
        let mut args: Vec<Arg> = compiler.link_flags.iter().map(|&f| Arg::Text(f)).collect();
        args.extend(
            compiler.sysroot.iter().flat_map(|root| [gcc::SYSROOT.into(), Arg::Path(root.as_str().into())]),
        );
        match self {
            Self::Own => {}
            Self::Flag(flag) => args.push(flag.into()),
            Self::In(dir) => args.extend([gcc::PROGRAMS.into(), Arg::Path(dir.to_path_buf())]),
        }
        if plan.kind == Output::Shared {
            args.push(gcc::SHARED.into());
        }
        args.extend([gcc::OUTPUT.into(), Arg::Path(plan.output.clone())]);
        if let Some(entry) = plan.entry {
            args.extend([gcc::ENTRY.into(), Arg::Text(entry)]);
        }
        let mut inputs = plan.inputs.clone();
        if plan.places() {
            let script = Script::of(plan).map_err(|why| diag.error(why.report(info)))?;
            let path = plan.beside(SCRIPT);
            path.put(script.to_string()).map_err(|e| diag.error(Diagnostic::unwritable(&path, &e)))?;
            args.extend([gcc::SCRIPT.into(), Arg::Path(path.clone())]);
            inputs.push(path);
        }
        args.extend(plan.inputs.iter().cloned().map(Arg::Path));
        args.extend(compiler.libraries.iter().flat_map(|&library| [gcc::LIBRARY.into(), Arg::Text(library)]));
        let inv = Invocation { args, inputs, outputs: vec![plan.output.clone()], ..Invocation::default() };
        let driver = compiler.command.as_str();
        Executable::new(driver, driver)
            .check(&inv)
            .map(drop)
            .map_err(|failure| diag.error(failure.report(driver, Task::Link, &plan.output)))
    }
}

#[cfg(test)]
mod tests;
