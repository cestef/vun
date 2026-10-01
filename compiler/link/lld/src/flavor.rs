//! The flavors of lld, by the object format each links: its program, and the command line it
//! takes for a link plan.

use vun_intern::Symbol;
use vun_link::{LinkPlan, Output};
use vun_target::{Name, TargetSpec};
use vun_tool::Arg;

/// A program of lld and the arguments it links a plan with.
pub(crate) struct Flavor {
    pub(crate) format: Name,
    pub(crate) program: &'static str,
    pub(crate) args: fn(&LinkPlan, &TargetSpec) -> Vec<Arg>,
}

pub(crate) const FLAVORS: [Flavor; 3] = [
    Flavor { format: Name("wasm"), program: "wasm-ld", args: wasm },
    Flavor { format: Name("macho"), program: "ld64.lld", args: macho },
    Flavor { format: Name("coff"), program: "lld-link", args: coff },
];

pub(crate) const FORMATS: [Name; FLAVORS.len()] = {
    let mut formats = [Name(""); FLAVORS.len()];
    let mut i = 0;
    while i < FLAVORS.len() {
        formats[i] = FLAVORS[i].format;
        i += 1;
    }
    formats
};

/// The options both flavors spell alike.
const OUTPUT: &str = "-o";

/// `wasm-ld`'s options.
mod wasm_ld {
    pub(super) const ENTRY: &str = "--entry";
    pub(super) const NO_ENTRY: &str = "--no-entry";
}

/// `ld64.lld`'s options, as Apple's `ld64` spells them.
mod ld64 {
    pub(super) const ARCH: &str = "-arch";
    /// Followed by the platform, the oldest release the program runs on and the SDK's release.
    pub(super) const PLATFORM_VERSION: &str = "-platform_version";
    pub(super) const SYSLIBROOT: &str = "-syslibroot";
    pub(super) const ENTRY: &str = "-e";
    pub(super) const DYLIB: &str = "-dylib";
    /// Joined to the library's name, which `ld64` takes in the same word (`-lSystem`).
    pub(super) const LIBRARY: &str = "-l";
}

/// `lld-link`'s options, as Microsoft's `link` spells them: a name and its value in one word.
mod lld_link {
    pub(super) const OUT: &str = "/out:";
    pub(super) const MACHINE: &str = "/machine:";
    pub(super) const ENTRY: &str = "/entry:";
    pub(super) const NO_ENTRY: &str = "/noentry";
    pub(super) const DLL: &str = "/dll";
    pub(super) const EXPORT: &str = "/export:";
    pub(super) const LIBPATH: &str = "/libpath:";
    pub(super) const CONSOLE: &str = "/subsystem:console";
    /// The system's libraries are named, so that the C runtime's are not found by default.
    pub(super) const NO_DEFAULT_LIBRARIES: &str = "/nodefaultlib";
    /// The extension of an import library, which a library is named by.
    pub(super) const LIBRARY: &str = ".lib";
    /// The machine each architecture links for, as `/machine:` names it.
    pub(super) const MACHINES: [(&str, &str); 3] = [("x86_64", "x64"), ("x86", "x86"), ("arm64", "arm64")];
}

/// One word of the command line from a name that takes its value joined to it.
fn joined(option: &str, value: impl std::fmt::Display) -> Arg {
    Arg::Text(Symbol::intern(&format!("{option}{value}")))
}

/// A COFF link: for the target's machine, a console program started at the target's entry, or a
/// library without one, against the import libraries in its sysroot and no default ones, its
/// libraries named after the objects.
fn coff(plan: &LinkPlan, target: &TargetSpec) -> Vec<Arg> {
    let mut args = vec![joined(lld_link::OUT, plan.output.display())];
    let arch = target.arch.as_str();
    if let Some((_, machine)) = lld_link::MACHINES.iter().find(|(name, _)| *name == arch) {
        args.push(joined(lld_link::MACHINE, machine));
    }
    args.extend([lld_link::CONSOLE.into(), lld_link::NO_DEFAULT_LIBRARIES.into()]);
    match plan.entry {
        Some(entry) => args.push(joined(lld_link::ENTRY, entry)),
        None => args.push(lld_link::NO_ENTRY.into()),
    }
    if plan.kind == Output::Shared {
        args.push(lld_link::DLL.into());
        args.extend(plan.exports.iter().map(|&symbol| joined(lld_link::EXPORT, symbol)));
    }
    let compiler = &target.c.compiler;
    if let Some(root) = compiler.sysroot {
        args.push(joined(lld_link::LIBPATH, root));
    }
    args.extend(plan.inputs.iter().cloned().map(Arg::Path));
    args.extend(
        compiler
            .libraries
            .iter()
            .map(|library| Arg::Text(Symbol::intern(&format!("{library}{}", lld_link::LIBRARY)))),
    );
    args
}

fn wasm(plan: &LinkPlan, _: &TargetSpec) -> Vec<Arg> {
    let mut args = vec![OUTPUT.into(), Arg::Path(plan.output.clone())];
    match plan.entry {
        Some(entry) => args.extend([wasm_ld::ENTRY.into(), Arg::Text(entry)]),
        None => args.push(wasm_ld::NO_ENTRY.into()),
    }
    args.extend(plan.inputs.iter().cloned().map(Arg::Path));
    args
}

/// A Mach-O link: for the target's architecture, on its system from the oldest release it
/// records (the SDK's taken as that one), against its sysroot, and its libraries after the
/// objects.
fn macho(plan: &LinkPlan, target: &TargetSpec) -> Vec<Arg> {
    let mut args =
        vec![ld64::ARCH.into(), Arg::Text(target.arch), OUTPUT.into(), Arg::Path(plan.output.clone())];
    if let Some(version) = target.os_version {
        let version = Symbol::intern(&version.to_string());
        args.extend([
            ld64::PLATFORM_VERSION.into(),
            Arg::Text(target.os),
            Arg::Text(version),
            Arg::Text(version),
        ]);
    }
    let compiler = &target.c.compiler;
    if let Some(root) = compiler.sysroot {
        args.extend([ld64::SYSLIBROOT.into(), Arg::Path(root.as_str().into())]);
    }
    if let Some(entry) = plan.entry {
        args.extend([ld64::ENTRY.into(), Arg::Text(entry)]);
    }
    if plan.kind == Output::Shared {
        args.push(ld64::DYLIB.into());
    }
    args.extend(plan.inputs.iter().cloned().map(Arg::Path));
    args.extend(
        compiler
            .libraries
            .iter()
            .map(|library| Arg::Text(Symbol::intern(&format!("{}{library}", ld64::LIBRARY)))),
    );
    args
}
