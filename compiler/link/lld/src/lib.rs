//! lld: one program per object format it links, each a flavor with its own command line. The
//! wasm flavor, `wasm-ld`, links the wasm backend's objects (spec 13, section 8.4); ELF, COFF
//! and Mach-O are still to come.

use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::{Name, Readiness, TargetSpec};
use vun_tool::{Arg, Executable, Invocation, Tool};

/// The flavors, by the object format each links, with its program.
const FLAVORS: [(Name, &str); 1] = [(Name("wasm"), "wasm-ld")];

const FORMATS: [Name; FLAVORS.len()] = {
    let mut formats = [Name(""); FLAVORS.len()];
    let mut i = 0;
    while i < FLAVORS.len() {
        formats[i] = FLAVORS[i].0;
        i += 1;
    }
    formats
};

pub const INFO: LinkerInfo = LinkerInfo {
    id: Name("lld"),
    formats: &FORMATS,
    regions: false,
    outputs: &[Output::Executable, Output::Shared],
    readiness: Readiness::Ready,
};

/// `wasm-ld`'s options.
const OUTPUT: &str = "-o";
const ENTRY: &str = "--entry";
const NO_ENTRY: &str = "--no-entry";

#[derive(Default)]
pub struct LldLinker;

impl Linker for LldLinker {
    fn info(&self) -> &LinkerInfo {
        &INFO
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let Some(&(_, program)) = FLAVORS.iter().find(|(format, _)| format.is(target.object_format)) else {
            return Err(INFO.unavailable(diag));
        };
        let Some(path) = Executable::search(program) else {
            let msg = Msg::new().text("linker ").code(program).text(" is not on the search path");
            return Err(diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        let mut args = vec![OUTPUT.into(), Arg::Path(plan.output.clone())];
        match plan.entry {
            Some(entry) => args.extend([ENTRY.into(), Arg::Text(entry)]),
            None => args.push(NO_ENTRY.into()),
        }
        args.extend(plan.inputs.iter().cloned().map(Arg::Path));
        vun_trace::tracing::debug!(program, output = %plan.output.display(), inputs = plan.inputs.len(), "linking");
        let inv = Invocation {
            args,
            inputs: plan.inputs.clone(),
            outputs: vec![plan.output.clone()],
            ..Invocation::default()
        };
        Executable::new(program, path).check(&inv).map(drop).map_err(|failure| {
            let msg =
                Msg::new().code(program).text(" could not link ").code(plan.output.display().to_string());
            diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
        })
    }
}
