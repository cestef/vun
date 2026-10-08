//! `vun component`: the backends, linkers and other parts of a toolchain, whether built into its
//! executable or installed from its release (spec 12, section 6).

mod change;
mod list;

use vun_build::toolchains::Toolchains;
use vun_command::{Args, Command, Param};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::{Session, VERSION};
use vun_span::Version;

use super::toolchain::{toolchains, version};

pub(super) const GROUP: &str = "component";

pub(super) const COMMANDS: &[Command<Session>] = &[list::LIST, change::ADD, change::REMOVE];

const TOOLCHAIN: Param =
    Param::option("toolchain").about("the toolchain, `major.minor.patch`; the running one when none");

/// The toolchain asked for, or the running one, and where the toolchains are.
fn asked(session: &mut Session, args: &Args) -> Option<(Toolchains, Version)> {
    let toolchains = toolchains(session)?;
    let asked = version(session, args, &TOOLCHAIN).ok()?;
    Some((toolchains, asked.or_else(|| Version::parse(VERSION))?))
}

/// Like [`asked`], reporting a toolchain that is not installed, whose components cannot change.
fn installed(session: &mut Session, args: &Args) -> Option<(Toolchains, Version)> {
    let (toolchains, version) = asked(session, args)?;
    if toolchains.installed().is_ok_and(|installed| installed.contains(&version)) {
        return Some((toolchains, version));
    }
    let msg = Msg::new().text("the toolchain ").code(version.to_string()).text(" is not installed");
    let help = Msg::new().text("a build that is not installed has the components it was built with");
    session.diag.error(Diagnostic::error(errors::TOOLCHAIN_NOT_INSTALLED, msg).help(help));
    None
}
