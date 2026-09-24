//! GNU ld, driven by the target's C compiler (`lang.target.CCompiler`), which passes it the C
//! runtime's start files and libraries: how hosted programs link, and vendor toolchains' linker.

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_link::{LinkPlan, Linker, LinkerInfo, OutputKind};
use vun_target::{Name, TargetSpec};
use vun_tool::{Executable, Invocation, Tool};

pub const INFO: LinkerInfo = LinkerInfo { id: Name("gnu"), formats: &[Name("elf")], regions: true };

#[derive(Default)]
pub struct GnuLinker;

impl Linker for GnuLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        if plan.kind != OutputKind::Executable || !plan.regions.is_empty() {
            return Err(INFO.unavailable(diag));
        }
        let driver = target.c.compiler.command.as_str();
        let mut args = vec!["-o".to_owned(), plan.output.display().to_string()];
        if let Some(entry) = plan.entry {
            args.extend(["-e".to_owned(), entry.as_str().to_owned()]);
        }
        args.extend(plan.inputs.iter().map(|p| p.display().to_string()));
        let inv = Invocation {
            args,
            inputs: plan.inputs.clone(),
            outputs: vec![plan.output.clone()],
            ..Invocation::default()
        };
        Executable::new(driver, driver).check(&inv).map(drop).map_err(|failure| {
            let msg =
                Msg::new().code(driver).text(" could not link ").code(plan.output.display().to_string());
            diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
        })
    }
}
