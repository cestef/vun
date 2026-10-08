//! `vun util cache`: the user's store of fetched packages (spec 12, section 3).

use std::io;

use vun_build::store::Listed;
use vun_command::{Args, Command, Outcome};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_term::{Bytes, Column, Record, Row, Shell, Style, Text};

use super::status::{Collected, HashText, Shown};
use vun_driver::Session;

pub(super) const GROUP: &str = "cache";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run(
        "list",
        "list stored packages, their sizes, and whether a project's lockfile keeps them",
        list,
    ),
    Command::run("gc", "remove stored packages no registered lockfile records", gc),
];

trait StoreFailure {
    /// Reports that the store cannot be read or changed.
    fn store_failed(&mut self, e: &io::Error) -> Outcome;
}

impl StoreFailure for Session {
    fn store_failed(&mut self, e: &io::Error) -> Outcome {
        let msg = Msg::new().text("cannot use the store at ").code(self.store.dir().display().to_string());
        self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(e));
        Outcome::Failure
    }
}

/// A stored package, as listings show it.
struct Stored<'a>(&'a Listed);

impl Record for Stored<'_> {
    fn kind(&self) -> &'static str {
        "stored"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let Listed { hash, label, bytes, rooted } = self.0;
        let object = Object::new().with("hash", hash.to_string()).with("bytes", *bytes);
        object.with("kept", *rooted).with("package", label.clone())
    }
}

impl Row for Stored<'_> {
    const COLUMNS: &'static [Column] =
        &[Column::left("hash"), Column::right("size"), Column::left("kept"), Column::left("package")];

    fn cells(&self) -> Vec<Text> {
        let Listed { hash, label, bytes, rooted } = self.0;
        let kept = if *rooted { Text::of(Style::Good, "yes") } else { Text::of(Style::Dim, "no") };
        let label = label.clone().map_or_else(|| Text::of(Style::Dim, "-"), Text::from);
        vec![hash.text(Shown::Full), Text::from(Bytes(*bytes).to_string()), kept, label]
    }
}

fn list(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    match session.store.list() {
        Ok(listed) => shell.table(&listed.iter().map(Stored).collect::<Vec<_>>()).map(|()| Outcome::Success),
        Err(e) => Ok(session.store_failed(&e)),
    }
}

fn gc(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    match session.store.collect() {
        Ok(collected) => {
            shell.table(&collected.removed.iter().map(Stored).collect::<Vec<_>>())?;
            let bytes: u64 = collected.removed.iter().map(|e| e.bytes).sum();
            shell.status(&Collected { removed: collected.removed.len(), bytes, kept: collected.kept });
            Ok(Outcome::Success)
        }
        Err(e) => Ok(session.store_failed(&e)),
    }
}
