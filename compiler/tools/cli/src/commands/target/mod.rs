//! `vun target`: the targets a program may be built for, and what a build for one is made with
//! and where each part comes from (spec 12, section 6).

mod list;
mod show;

use std::path::Path;

use vun_command::{Args, Arity, Command, Param, PathKind, Values};
use vun_span::FileId;
use vun_term::{Shell, Style, Text};

use vun_driver::Session;
use vun_driver::setup::Found;

use super::{COMPILE, Inputs};

pub(super) const GROUP: &str = "target";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run("list", "list the targets, what builds for each, and what the machine lacks", list::list)
        .params(&[PATH])
        .sets(&COMPILE),
    Command::run("show", "show what a build for the target uses and where each part comes from", show::show)
        .params(&[PATH])
        .sets(&COMPILE),
];

const PATH: Param = Param::positional("file")
    .arity(Arity::Optional)
    .about("the program's root file; a program anywhere when none")
    .values(Values::Path(PathKind::File));

/// The root file `args` names, or a file holding nothing.
fn root(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> Option<FileId> {
    match args.value(&PATH) {
        Some(path) => session.root_at(Path::new(path), None, shell),
        None => Some(session.empty_file()),
    }
}

/// How the commands of `vun target` show whether the machine has a program.
trait Shows {
    fn text(&self) -> Text;
    /// Where it is, if the machine has it.
    fn at(&self) -> Option<String>;
}

impl Shows for Found {
    fn text(&self) -> Text {
        match self {
            Self::At(path) => Text::of(Style::Dim, path.display().to_string()),
            Self::Host(path) => Text::of(Style::Warning, path.display().to_string())
                .append(Text::of(Style::Dim, ", builds for this machine")),
            Self::Missing => Text::of(Style::Error, "missing"),
        }
    }

    fn at(&self) -> Option<String> {
        match self {
            Self::At(path) | Self::Host(path) => Some(path.display().to_string()),
            Self::Missing => None,
        }
    }
}
