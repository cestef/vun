//! `vun toolchain`: what this toolchain is made of.

use std::io;

use vun_command::{Args, Command, Outcome};
use vun_term::{Column, Shell, Style, Table, Text};

use crate::{Registry, Session};

pub(super) const GROUP: &str = "toolchain";

pub(super) const COMMANDS: &[Command<Session>] =
    &[Command::run("components", "list the backends and linkers in this toolchain", components)];

const COLUMNS: [Column; 2] = [Column::left("kind"), Column::left("name")];

fn components(_: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let backends =
        r.backends().map(|b| [Text::of(Style::Dim, "backend"), Text::of(Style::Literal, b.info().id.0)]);
    let linkers =
        r.linkers().map(|l| [Text::of(Style::Dim, "linker"), Text::of(Style::Literal, l.info().id.0)]);
    shell.out.print(&Table::new(&COLUMNS).rows(backends.chain(linkers)).text())?;
    Ok(Outcome::Success)
}
