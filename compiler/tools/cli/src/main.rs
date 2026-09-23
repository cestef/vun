//! The `overt` command: a thin shell over `overt-driver`.

use std::process::ExitCode;

use overt_diag::{ChildKind, DiagSink, Diagnostic, Msg, errors};
use overt_driver::{Registry, VERSION, commands};
use overt_origin::OriginTable;
use overt_render_short::ShortRenderer;
use overt_span::SourceFiles;

/// Exit code for command-line usage errors, as in `sysexits.h`.
const EXIT_USAGE: u8 = 64;

fn help() {
    println!("overt {VERSION}\n\nusage: overt <command> [args]\n\ncommands:");
    let width = commands::ALL.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in commands::ALL {
        println!("  {:<width$}  {}", c.name, c.about);
    }
}

fn components() {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id).collect();
    println!("backends: {}", backends.join(", "));
    println!("linkers:  {}", linkers.join(", "));
}

fn report(d: &Diagnostic) {
    ShortRenderer::new(std::io::stderr()).emit(d, &OriginTable::new(), &SourceFiles::new());
}

fn unrecognized(name: &str) -> (Diagnostic, ExitCode) {
    if commands::find(name).is_some() {
        let msg = Msg::new().code(name).text(" is not available yet");
        (Diagnostic::error(errors::CLI_UNAVAILABLE_COMMAND, msg), ExitCode::FAILURE)
    } else {
        let msg = Msg::new().text("unknown command ").code(name);
        let help = Msg::new().text("see ").code("overt help");
        (
            Diagnostic::error(errors::CLI_UNKNOWN_COMMAND, msg).child(ChildKind::Help, help),
            ExitCode::from(EXIT_USAGE),
        )
    }
}

fn main() -> ExitCode {
    overt_trace::init(&overt_trace::Config::from_env());
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => help(),
        Some("--version" | "-V") => println!("overt {VERSION}"),
        Some("components") => components(),
        Some(name) => {
            let (diagnostic, code) = unrecognized(name);
            report(&diagnostic);
            return code;
        }
    }
    ExitCode::SUCCESS
}
