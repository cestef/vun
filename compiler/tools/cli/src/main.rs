//! The `overt` command: a thin shell over `overt-driver`.

use std::path::Path;
use std::process::ExitCode;

use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_driver::commands::{self, Command};
use overt_driver::{Registry, Session, VERSION};
use overt_render_short::ShortRenderer;

/// How the process ends.
enum Exit {
    Success,
    Failure,
    /// Command-line misuse, reported as `EX_USAGE` from `sysexits.h`.
    Usage,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        const EX_USAGE: u8 = 64;
        match exit {
            Exit::Success => ExitCode::SUCCESS,
            Exit::Failure => ExitCode::FAILURE,
            Exit::Usage => ExitCode::from(EX_USAGE),
        }
    }
}

fn print_table(title: &str, entries: &[Command]) {
    println!("{title}:");
    let width = entries.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in entries {
        println!("  {:<width$}  {}", c.name, c.about);
    }
}

fn help() -> Exit {
    println!("overt {VERSION}\n\nusage: overt <command> [args]\n");
    print_table("commands", commands::ALL);
    Exit::Success
}

fn components() -> Exit {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id).collect();
    println!("backends: {}", backends.join(", "));
    println!("linkers:  {}", linkers.join(", "));
    Exit::Success
}

fn usage(session: &mut Session, d: Diagnostic) -> Exit {
    session.diag.error(d);
    Exit::Usage
}

fn dump(session: &mut Session, args: &[String]) -> Exit {
    let [what, path] = args else {
        let names = commands::DUMPS.iter().map(|c| c.name);
        let msg = Msg::new().text("expected ").code("overt dump <what> <file>");
        let help = Msg::new().text("<what> is one of ").code_list(names);
        return usage(
            session,
            Diagnostic::error(errors::CLI_MISSING_ARGUMENT, msg).child(ChildKind::Help, help),
        );
    };
    if commands::DUMPS.iter().all(|c| c.name != what) {
        return unknown(session, what, commands::DUMPS);
    }
    let Some(file) = session.load(Path::new(path)) else { return Exit::Failure };
    let tokens = session.lex(file);
    print!("{}", tokens.dump(session.text(file)));
    if session.failed() { Exit::Failure } else { Exit::Success }
}

fn unknown(session: &mut Session, name: &str, known: &[Command]) -> Exit {
    let msg = Msg::new().text("unknown command ").code(name);
    let help = Msg::new().text("expected one of ").code_list(known.iter().map(|c| c.name));
    usage(session, Diagnostic::error(errors::CLI_UNKNOWN_COMMAND, msg).child(ChildKind::Help, help))
}

fn run(session: &mut Session, args: &[String]) -> Exit {
    let Some((name, rest)) = args.split_first() else { return help() };
    match name.as_str() {
        "help" | "--help" | "-h" => help(),
        "--version" | "-V" => {
            println!("overt {VERSION}");
            Exit::Success
        }
        "components" => components(),
        "dump" => dump(session, rest),
        name if commands::find(name).is_some() => {
            let msg = Msg::new().code(name).text(" is not available yet");
            session.diag.error(Diagnostic::error(errors::CLI_UNAVAILABLE_COMMAND, msg));
            Exit::Failure
        }
        name => unknown(session, name, commands::ALL),
    }
}

fn main() -> ExitCode {
    overt_trace::init(&overt_trace::Config::from_env());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut session = Session::new();
    let exit = run(&mut session, &args);
    session.emit(&mut ShortRenderer::new(std::io::stderr()));
    exit.into()
}
