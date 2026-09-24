//! The `vun` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};

use crate::{Registry, Session, VERSION, formats};

pub const PROGRAM: Program<Session> = Program {
    version: VERSION,
    configure: formats::configure,
    root: Command {
        name: "vun",
        about: "The Vun toolchain.",
        params: &[formats::FORMAT],
        action: Action::Group(&[
            Command::planned("new", "create a package"),
            Command::planned("build", "build the package"),
            Command::planned("run", "build and run a program"),
            Command {
                name: "check",
                about: "analyze without generating code",
                params: &[FILE],
                action: Action::Run(check),
            },
            Command::planned("test", "run tests, properties and simulations"),
            Command::planned("bench", "run benchmarks"),
            Command::planned("fuzz", "fuzz a test with coverage guidance"),
            Command::planned("fmt", "format source files"),
            Command::planned("fix", "apply fixes from diagnostics"),
            Command::planned("doc", "generate documentation"),
            explain::EXPLAIN,
            Command::group("dump", "print an intermediate representation with origins", dump::DUMPS),
            Command::group(
                pkg::GROUP,
                "dependencies, releases and API diffs",
                &[
                    Command {
                        name: "add",
                        about: "declare a dependency, fetching an archive to learn its hash",
                        params: &[pkg::SOURCE, pkg::NAME, pkg::VERSION],
                        action: Action::Run(pkg::add),
                    },
                    Command {
                        name: "update",
                        about: "raise dependencies to the newest release their indexes list",
                        params: &[],
                        action: Action::Run(pkg::update),
                    },
                    pkg::FETCH,
                    Command::planned("vendor", "copy dependencies into vendor/"),
                    Command {
                        name: "pack",
                        about: "write the package's archive",
                        params: &[pkg::OUT],
                        action: Action::Run(pkg::pack),
                    },
                    Command::planned("why", "explain why a package is in the graph"),
                    Command::planned("tag", "release workspace members"),
                    Command::planned("api", "print the public API, or diff it against a release"),
                ],
            ),
            Command::group(
                "tool",
                "C bindings, C headers, register packages",
                &[
                    tool::BIND,
                    Command::planned("header", "emit a C header for a library"),
                    Command::planned("svd", "generate a register package from an SVD file"),
                ],
            ),
            Command::group(
                "target",
                "flash, attach to and debug devices",
                &[
                    Command::planned("flash", "write a program to a device"),
                    Command::planned("attach", "stream logs from a device"),
                    Command::planned("debug", "run a GDB server for a device"),
                ],
            ),
            Command::group(
                "toolchain",
                "manage installed toolchains",
                &[
                    Command::planned("list", "list installed toolchains"),
                    Command::planned("install", "install a toolchain"),
                    Command::planned("prune", "remove unused toolchains"),
                ],
            ),
            Command::group(cache::GROUP, "the shared store of fetched packages", cache::COMMANDS),
            Command::planned("lsp", "run the language server"),
            Command {
                name: "components",
                about: "list the backends and linkers in this toolchain",
                params: &[],
                action: Action::Run(components),
            },
        ]),
    },
};

pub(crate) const FILE: Param =
    Param { name: "file", short: None, about: "the source file to read", kind: ParamKind::Positional };

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn check(session: &mut Session, args: &Args, _: &mut dyn Write) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    if let Some(file) = session.load(Path::new(path)) {
        session.check(file);
    }
    Ok(session.outcome())
}

fn components(_: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id).collect();
    writeln!(out, "backends: {}", backends.join(", "))?;
    writeln!(out, "linkers:  {}", linkers.join(", "))?;
    Ok(Outcome::Success)
}

mod cache;
mod dump;
mod explain;
mod pkg;
mod tool;

pub(crate) use pkg::FETCH_PATH;
#[cfg(test)]
mod tests;

impl Session {
    /// How a command that reported its failures as diagnostics ends.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failed() { Outcome::Failure } else { Outcome::Success }
    }
}
