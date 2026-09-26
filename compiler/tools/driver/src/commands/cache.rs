//! `vun cache`: the user's store of fetched packages (spec 12, section 3).

use std::io;

use vun_build::store::Listed;
use vun_command::{Args, Command, Outcome};
use vun_diag::{Diagnostic, Msg, errors};
use vun_term::{Bytes, Column, Noun, Shell, Style, Table, Text};

use super::status::{HashText, REMOVED, Shown};
use crate::Session;

pub(super) const GROUP: &str = "cache";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run(
        "list",
        "list stored packages, their sizes, and whether a project's lockfile keeps them",
        list,
    ),
    Command::run("gc", "remove stored packages no registered lockfile records", gc),
];

/// The columns of a listing of stored packages.
const COLUMNS: [Column; 4] =
    [Column::left("hash"), Column::right("size"), Column::left("kept"), Column::left("package")];
const PACKAGE: Noun = Noun { one: "package", many: "packages" };

impl Session {
    /// Reports that the store cannot be read or changed.
    fn store_failed(&mut self, e: &io::Error) -> Outcome {
        let msg = Msg::new().text("cannot use the store at ").code(self.store.dir().display().to_string());
        self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(e));
        Outcome::Failure
    }
}

/// Stored packages as a table.
trait Listing {
    fn table(&self) -> Table<'static>;
}

impl Listing for [Listed] {
    fn table(&self) -> Table<'static> {
        Table::new(&COLUMNS).rows(self.iter().map(|entry| {
            let kept = if entry.rooted { Text::of(Style::Good, "yes") } else { Text::of(Style::Dim, "no") };
            let label = entry.label.clone().map_or_else(|| Text::of(Style::Dim, "-"), Text::from);
            [entry.hash.text(Shown::Full), Text::from(Bytes(entry.bytes).to_string()), kept, label]
        }))
    }
}

fn list(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    match session.store.list() {
        Ok(listed) => shell.out.print(&listed.table().text()).map(|()| Outcome::Success),
        Err(e) => Ok(session.store_failed(&e)),
    }
}

fn gc(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    match session.store.collect() {
        Ok(collected) => {
            shell.out.print(&collected.removed.table().text())?;
            let freed: u64 = collected.removed.iter().map(|e| e.bytes).sum();
            let what = format!(
                "{}, {}; kept {}",
                PACKAGE.count(collected.removed.len()),
                Bytes(freed),
                collected.kept
            );
            shell.status(REMOVED, Text::from(what));
            Ok(Outcome::Success)
        }
        Err(e) => Ok(session.store_failed(&e)),
    }
}
