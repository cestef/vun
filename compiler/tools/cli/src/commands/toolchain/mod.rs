//! `vun toolchain`: this toolchain, a release of `vun` and its library (spec 12, section 6).

mod show;

use vun_command::Command;
use vun_driver::Session;

pub(super) const GROUP: &str = "toolchain";

pub(super) const COMMANDS: &[Command<Session>] = &[show::SHOW];
