//! `vun cache`: the user's store of fetched packages (spec 12, section 3).

use std::io::{self, Write};

use vun_command::{Action, Args, Command, Outcome};
use vun_diag::{Diagnostic, Msg, errors};

use crate::Session;

pub(super) const GROUP: &str = "cache";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command {
        name: "list",
        about: "list stored packages, their sizes, and whether a project's lockfile keeps them",
        params: &[],
        action: Action::Run(list),
    },
    Command {
        name: "gc",
        about: "remove stored packages no registered lockfile records",
        params: &[],
        action: Action::Run(gc),
    },
];

impl Session {
    /// Reports that the store cannot be read or changed.
    fn store_failed(&mut self, e: &io::Error) -> Outcome {
        let msg = Msg::new().text("cannot use the store at ").code(self.store.dir().display().to_string());
        self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(e));
        Outcome::Failure
    }
}

fn list(session: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    match session.store.list() {
        Ok(listed) => {
            for entry in &listed {
                writeln!(out, "{entry}")?;
            }
            Ok(Outcome::Success)
        }
        Err(e) => Ok(session.store_failed(&e)),
    }
}

fn gc(session: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    match session.store.collect() {
        Ok(collected) => {
            for entry in &collected.removed {
                writeln!(out, "{entry}")?;
            }
            let freed: u64 = collected.removed.iter().map(|e| e.bytes).sum();
            let removed = collected.removed.len();
            writeln!(out, "removed {removed} packages ({freed} bytes), kept {}", collected.kept)?;
            Ok(Outcome::Success)
        }
        Err(e) => Ok(session.store_failed(&e)),
    }
}
