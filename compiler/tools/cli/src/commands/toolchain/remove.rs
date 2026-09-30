//! `vun toolchain remove <version>`: an installed toolchain removed, unless it is the default or
//! the running one.

use std::io;

use vun_command::{Args, Command, Outcome, Param};
use vun_diag::{Diagnostic, Msg, errors};
use vun_span::Version;
use vun_term::Shell;

use vun_driver::Session;

use super::{toolchains, version};

const VERSION: Param = Param::positional("version").about("the toolchain to remove, `major.minor.patch`");

pub(super) const REMOVE: Command<Session> =
    Command::run("remove", "remove an installed toolchain", remove).params(&[VERSION]);

fn remove(session: &mut Session, args: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(toolchains) = toolchains(session) else { return Ok(session.outcome()) };
    let Ok(Some(version)) = version(session, args, &VERSION) else { return Ok(session.outcome()) };
    let named = |why: &'static str| Msg::new().text("the toolchain ").code(version.to_string()).text(why);
    if !toolchains.installed()?.contains(&version) {
        session.diag.error(Diagnostic::error(errors::TOOLCHAIN_NOT_INSTALLED, named(" is not installed")));
    } else if toolchains.default() == Some(version) {
        session.diag.error(Diagnostic::error(errors::TOOLCHAIN_IN_USE, named(" is the default")));
    } else if Version::parse(vun_driver::VERSION) == Some(version) {
        session.diag.error(Diagnostic::error(errors::TOOLCHAIN_IN_USE, named(" is running this command")));
    } else {
        toolchains.remove(version)?;
    }
    Ok(session.outcome())
}
