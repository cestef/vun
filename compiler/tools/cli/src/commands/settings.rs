//! The options a command takes from the plan of the package it acts on (`b.commands`, spec 12,
//! section 2), where neither the command line nor the environment gives one.

use std::path::Path;

use vun_command::{Args, PathKind, Selected, Setting, Values};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_driver::Session;

use super::PROGRAM;

/// Gives the options a command line and the environment left out the values the plan of the
/// package the command acts on gives them (`b.commands`, spec 12, section 2). Only a command
/// naming a path has a package.
pub(super) fn defaults(
    session: &mut Session,
    selected: &Selected<'_, Session>,
    args: &mut Args,
) -> Result<(), Box<Diagnostic>> {
    let acted = selected.last().positionals().find(|p| matches!(p.values, Values::Path(PathKind::Any)));
    let Some(acted) = acted else { return Ok(()) };
    let path = args.value(acted).map(Path::new);
    let Some((settings, at)) = session.settings(path) else { return Ok(()) };
    for setting in settings.iter() {
        if let Some(unknown) = unknown_setting(selected, setting) {
            return Err(Box::new(unknown.at(at, None)));
        }
    }
    args.defaulted(selected, &settings)
        .map_err(|d| Box::new(d.at(at, None).note("the value is set by the plan, in a call of `b.commands`")))
}

/// The report of `setting` when it names no command, or no option of the command selected.
fn unknown_setting(selected: &Selected<'_, Session>, setting: &Setting) -> Option<Diagnostic> {
    let command = selected.last();
    let near = |given: &str, names: Vec<&str>| {
        let help = vun_suggest::closest(given, names).map(Msg::did_you_mean);
        help.unwrap_or_else(|| Msg::new().text("see ").code("vun --help"))
    };
    match PROGRAM.root.child(&setting.command) {
        None => {
            let names = PROGRAM.root.children().iter().map(|c| c.name).collect();
            let msg = Msg::new().text("the plan sets options of ").code(setting.command.clone());
            let msg = msg.text(", which is no command");
            let d = Diagnostic::error(errors::PLAN_UNKNOWN_SETTING, msg);
            Some(d.child(ChildKind::Help, near(&setting.command, names)))
        }
        Some(named) if named.name == command.name && !selected.accepts(&setting.option) => {
            let options: Vec<&str> = selected.options().map(|p| p.name).collect();
            let msg =
                Msg::new().code(selected.invocation()).text(" has no option ").code(setting.option.clone());
            let d = Diagnostic::error(errors::PLAN_UNKNOWN_SETTING, msg);
            Some(d.child(ChildKind::Help, near(&setting.option, options)))
        }
        Some(_) => None,
    }
}
