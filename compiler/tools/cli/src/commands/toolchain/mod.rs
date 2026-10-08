//! `vun toolchain`: this toolchain, a release of `vun` and its library, and the others installed
//! beside it (spec 12, section 6).

mod default;
mod install;
mod list;
mod remove;
mod show;

use vun_build::toolchains::Toolchains;
use vun_command::{Args, Command, Param};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::install::Uninstalled;
use vun_span::Version;

pub(super) const GROUP: &str = "toolchain";

pub(super) const COMMANDS: &[Command<Session>] =
    &[show::SHOW, list::LIST, install::INSTALL, install::UPDATE, default::DEFAULT, remove::REMOVE];

/// The user's toolchains; their absence, for a user without a home, is reported.
pub(in crate::commands) fn toolchains(session: &mut Session) -> Option<Toolchains> {
    let toolchains = Toolchains::user();
    if toolchains.is_none() {
        session.uninstalled(None, &Uninstalled::Homeless);
    }
    toolchains
}

/// The version `param` names, if it names one; one that is not a version is reported.
pub(in crate::commands) fn version(
    session: &mut Session,
    args: &Args,
    param: &Param,
) -> Result<Option<Version>, ()> {
    let Some(text) = args.value(param) else { return Ok(None) };
    let version = Version::parse(text);
    if version.is_none() {
        let msg = Msg::new().code(text).text(" is not a version, `major.minor.patch`");
        session.diag.error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg));
    }
    version.map(Some).ok_or(())
}
