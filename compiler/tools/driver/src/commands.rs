//! The `overt` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io::{self, Write};
use std::path::Path;

use overt_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};

use crate::{Registry, Session, VERSION, formats};

pub const PROGRAM: Program<Session> = Program {
    version: VERSION,
    configure: formats::configure,
    root: Command {
        name: "overt",
        about: "The Overt toolchain.",
        params: &[formats::FORMAT],
        action: Action::Group(&[
            planned("new", "create a package"),
            planned("build", "build the package"),
            planned("run", "build and run a program"),
            Command {
                name: "check",
                about: "analyze without generating code",
                params: &[FILE],
                action: Action::Run(check),
            },
            planned("test", "run tests, properties and simulations"),
            planned("bench", "run benchmarks"),
            planned("fuzz", "fuzz a test with coverage guidance"),
            planned("fmt", "format source files"),
            planned("fix", "apply fixes from diagnostics"),
            planned("doc", "generate documentation"),
            planned("explain", "explain a diagnostic"),
            group("dump", "print an intermediate representation with origins", DUMPS),
            group(
                "pkg",
                "dependencies, releases and API diffs",
                &[
                    planned("add", "add a dependency"),
                    planned("update", "raise dependency versions"),
                    Command {
                        name: "fetch",
                        about: "download dependencies into the local store",
                        params: &[],
                        action: Action::Run(pkg::fetch),
                    },
                    planned("vendor", "copy dependencies into vendor/"),
                    planned("why", "explain why a package is in the graph"),
                    planned("tag", "release workspace members"),
                    planned("api", "print the public API, or diff it against a release"),
                ],
            ),
            group(
                "tool",
                "C bindings, C headers, register packages",
                &[
                    planned("bind", "generate declarations from a C header"),
                    planned("header", "emit a C header for a library"),
                    planned("svd", "generate a register package from an SVD file"),
                ],
            ),
            group(
                "target",
                "flash, attach to and debug devices",
                &[
                    planned("flash", "write a program to a device"),
                    planned("attach", "stream logs from a device"),
                    planned("debug", "run a GDB server for a device"),
                ],
            ),
            group(
                "toolchain",
                "manage installed toolchains",
                &[
                    planned("list", "list installed toolchains"),
                    planned("install", "install a toolchain"),
                    planned("prune", "remove unused toolchains"),
                ],
            ),
            planned("cache", "manage build caches"),
            planned("lsp", "run the language server"),
            Command {
                name: "components",
                about: "list the backends and linkers in this toolchain",
                params: &[],
                action: Action::Run(components),
            },
        ]),
    },
};

const FILE: Param =
    Param { name: "file", short: None, about: "the source file to read", kind: ParamKind::Positional };

const DUMPS: &[Command<Session>] = &[
    Command {
        name: "tokens",
        about: "tokens and trivia with byte ranges",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Tokens)),
    },
    Command {
        name: "cst",
        about: "the concrete syntax tree, one node per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Cst)),
    },
    Command {
        name: "manifest",
        about: "the package manifest read from a build.ov",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Manifest)),
    },
    Command {
        name: "hir",
        about: "the high-level representation, names resolved, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Hir)),
    },
    planned("mir", "the mid-level representation"),
    planned("lir", "the low-level representation"),
    planned("link", "the link plan"),
];

const fn planned(name: &'static str, about: &'static str) -> Command<Session> {
    Command { name, about, params: &[], action: Action::Planned }
}

const fn group(
    name: &'static str,
    about: &'static str,
    children: &'static [Command<Session>],
) -> Command<Session> {
    Command { name, about, params: &[], action: Action::Group(children) }
}

/// What `overt dump` prints.
#[derive(Clone, Copy)]
enum Stage {
    Tokens,
    Cst,
    Hir,
    Manifest,
}

fn dump(session: &mut Session, args: &Args, out: &mut dyn Write, stage: Stage) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(Path::new(path)) else { return Ok(Outcome::Failure) };
    if let Stage::Manifest = stage {
        if let Some(manifest) = session.manifest(file) {
            writeln!(out, "{manifest:#?}")?;
        }
        return Ok(if session.failed() { Outcome::Failure } else { Outcome::Success });
    }
    let tokens = session.lex(file);
    match stage {
        Stage::Tokens => write!(out, "{}", tokens.dump(session.text(file)))?,
        Stage::Cst => {
            let cst = session.parse(&tokens);
            write!(out, "{}", cst.dump(&tokens, session.text(file)))?;
        }
        Stage::Hir => {
            let cst = session.parse(&tokens);
            let hir = session.lower(&tokens, &cst);
            let resolution = session.resolve(file, &hir);
            let text = overt_hir::dump::dump(
                &hir,
                &resolution,
                &session.interner,
                &session.origins,
                &session.sources,
            );
            write!(out, "{text}")?;
        }
        Stage::Manifest => unreachable!("read above"),
    }
    Ok(if session.failed() { Outcome::Failure } else { Outcome::Success })
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn check(session: &mut Session, args: &Args, _: &mut dyn Write) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    if let Some(file) = session.load(Path::new(path)) {
        session.check(file);
    }
    Ok(if session.failed() { Outcome::Failure } else { Outcome::Success })
}

fn components(_: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id).collect();
    writeln!(out, "backends: {}", backends.join(", "))?;
    writeln!(out, "linkers:  {}", linkers.join(", "))?;
    Ok(Outcome::Success)
}

mod pkg;
#[cfg(test)]
mod tests;
