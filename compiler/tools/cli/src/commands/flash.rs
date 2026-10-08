//! `vun flash [path]`: builds what the plan records and writes each program its plan flashes to
//! its device.

use std::io;

use vun_command::{Args, Command, Example, Outcome};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::subject::Subject;
use vun_term::Shell;

use super::inputs::Inputs;
use super::status::{Started, Step};
use super::{COMPILE, PATH};

pub(super) const FLASH: Command<Session> = Command::run(
    "flash",
    "build the programs a package's plan records, and write those it flashes to their devices",
    flash,
)
.params(&[PATH, vun_driver::codegen::BACKEND, vun_driver::codegen::LINKER, vun_driver::program::OUTPUT])
.sets(&COMPILE)
.examples(&[Example {
    line: "",
    about: "write the program the plan of the package around this directory flashes to its device",
}]);

fn flash(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(built) = session.built(args, shell)? else { return Ok(session.outcome()) };
    let flashings: Vec<_> = built.iter().flat_map(|program| &program.flashing).collect();
    if flashings.is_empty() {
        let msg = Msg::new().text("the plan flashes no program");
        let help = Msg::new().text("record one with ").code("b.flash").text(" after the step that builds it");
        session.diag.error(Diagnostic::error(errors::PLAN_NOTHING_TO_FLASH, msg).help(help));
    }
    for flashing in flashings {
        shell.status(&Started { step: Step::Flash, subject: Subject::relative(&flashing.file) });
        if session.deliver(&flashing.file, &flashing.ways).is_err() {
            break;
        }
    }
    session.emit(shell);
    Ok(session.outcome())
}
