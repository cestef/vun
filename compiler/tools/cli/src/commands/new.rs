//! `vun new <path> [--template name]`: starts a project from a template of the library.

use std::io;
use std::path::Path;

use vun_command::{Args, Command, Example, Outcome, Param, PathKind, Values};
use vun_driver::Session;
use vun_driver::packages::Library;
use vun_driver::subject::Subject;
use vun_intern::Symbol;
use vun_term::Shell;

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
    Command::run("new", "start a project from a template", new).params(&[PATH, TEMPLATE]).examples(&[
        Example { line: "hello", about: "a hosted program in the directory `hello`" },
        Example { line: "blink --template rp2350", about: "firmware for a Raspberry Pi Pico 2" },
    ]);

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn new(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let dir = Path::new(args.value(&PATH).expect("positional parameters are present when a command runs"));
    let template = args.value(&TEMPLATE).unwrap_or(Library::DEFAULT_TEMPLATE);
    shell.status(&Started { step: Step::New, subject: Subject::relative(dir) });
    let _ = session.new_project(dir, template);
    session.emit(shell);
    Ok(session.outcome())
}
