//! `vun toolchain default <version>`: an installed toolchain made the default, the one the user's
//! `vun` links to.

use std::io;

use vun_command::{Args, Command, Outcome, Param};
use vun_diag::{Diagnostic, Msg, errors};
use vun_term::Shell;

use vun_driver::Session;

use super::{toolchains, version};

const VERSION: Param =
    Param::positional("version").about("the toolchain to make the default, `major.minor.patch`");

pub(super) const DEFAULT: Command<Session> =
    Command::run("default", "make an installed toolchain the default", default).params(&[VERSION]);

fn default(session: &mut Session, args: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(toolchains) = toolchains(session) else { return Ok(session.outcome()) };
    let Ok(Some(version)) = version(session, args, &VERSION) else { return Ok(session.outcome()) };
    if !toolchains.installed()?.contains(&version) {
        let msg = Msg::new().text("the toolchain ").code(version.to_string()).text(" is not installed");
        let help = Msg::new().text("install it with ").code("vun toolchain install");
        session.diag.error(Diagnostic::error(errors::TOOLCHAIN_NOT_INSTALLED, msg).help(help));
    } else if let Err(e) = toolchains.make_default(version) {
        let msg = Msg::new()
            .text("cannot link the default toolchain at ")
            .code(toolchains.link().display().to_string());
        session.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
    }
    Ok(session.outcome())
}
