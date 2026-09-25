//! `vun toolchain`: installed toolchains, and what this one is made of.

use std::io::{self, Write};

use vun_command::{Action, Args, Command, Outcome};

use crate::{Registry, Session};

pub(super) const GROUP: &str = "toolchain";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::planned("list", "list installed toolchains"),
    Command::planned("install", "install a toolchain"),
    Command::planned("prune", "remove unused toolchains"),
    Command {
        name: "components",
        about: "list the backends and linkers in this toolchain",
        params: &[],
        action: Action::Run(components),
    },
];

fn components(_: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id.0).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id.0).collect();
    writeln!(out, "backends: {}", backends.join(", "))?;
    writeln!(out, "linkers:  {}", linkers.join(", "))?;
    Ok(Outcome::Success)
}
