//! `vun new <path> [--template name]`: starts a project from a template of the library.

use std::io;
use std::path::{Path, PathBuf};

use vun_command::{Args, Command, CommandVerb, Example, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_driver::Session;
use vun_driver::packages::Library;
use vun_driver::subject::Subject;
use vun_intern::Symbol;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use super::status::{Started, Step};

const PATH: Param = Param::positional("path")
    .about("the directory of the project, made if it is not there; its name is the package's")
    .values(Values::Path(PathKind::Dir));

const TEMPLATE: Param = Param::option("template")
    .about("the project to start from, one of the library's templates; `hello` when none")
    .values(Values::Choice(|| {
        Library::templates(&Library::find().dir).iter().map(|name| Symbol::intern(name).as_str()).collect()
    }));

pub(super) const NEW: Command<Session> =
    Command::run(CommandVerb::New, "start a project from a template", new)
        .params(&[PATH, TEMPLATE])
        .examples(&[
            Example { line: "hello", about: "a hosted program in the directory `hello`" },
            Example { line: "blink --template rp2350", about: "firmware for a Raspberry Pi Pico 2" },
        ]);

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn new(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let dir = Path::new(args.value(&PATH).expect("positional parameters are present when a command runs"));
    let template = args.value(&TEMPLATE).unwrap_or(Library::DEFAULT_TEMPLATE);
    shell.status(&Started { step: Step::New, subject: Subject::relative(dir) });
    let made = session.new_project(dir, template);
    session.emit(shell);
    if let Ok(files) = made {
        shell.status(&Created { dir: Subject::relative(dir), files });
    }
    Ok(session.outcome())
}

/// A project started: where, and the files the template made.
struct Created {
    dir: String,
    files: Vec<PathBuf>,
}

impl Record for Created {
    fn kind(&self) -> &'static str {
        "created"
    }

    fn text(&self) -> Text {
        let files: Vec<String> = self.files.iter().map(|f| f.display().to_string()).collect();
        Text::of(Style::Strong, self.dir.clone())
            .append(Text::of(Style::Dim, format!(" with {}", files.join(", "))))
    }

    fn fields(&self) -> Object<'static> {
        let files: Vec<String> = self.files.iter().map(|f| f.display().to_string()).collect();
        Object::new().with("dir", self.dir.clone()).with("files", files)
    }
}

impl Status for Created {
    fn verb(&self) -> Verb {
        Verb::good("Created")
    }
}
