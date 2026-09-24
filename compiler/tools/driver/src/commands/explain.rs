//! `vun explain <id>`: the explanation page of a diagnostic, offline (spec 10, section 1).

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Action, Args, Command, Outcome, Param, ParamKind};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_suggest::closest;

use crate::Session;
use crate::packages::LIBRARY;

const ID: Param = Param {
    name: "id",
    short: None,
    about: "a diagnostic, such as names.duplicate or lang.errors.names.duplicate",
    kind: ParamKind::Positional,
};

pub(super) const EXPLAIN: Command<Session> =
    Command { name: "explain", about: "explain a diagnostic", params: &[ID], action: Action::Run(explain) };

fn explain(session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let id = args.value(&ID).expect("positional parameters are present when a command runs");
    let found = errors::EXPLANATIONS.iter().find(|(d, _)| d.to_string() == id || d.0 == id);
    let Some((_, page)) = found else {
        let msg = Msg::new().text("no diagnostic is named ").code(id);
        let names: Vec<String> = errors::EXPLANATIONS.iter().map(|(d, _)| d.to_string()).collect();
        let d = Diagnostic::error(errors::CLI_INVALID_VALUE, msg);
        let d = match closest(id, names.iter().map(String::as_str)) {
            Some(near) => d.child(ChildKind::Help, Msg::did_you_mean(near)),
            None => d,
        };
        session.diag.error(d);
        return Ok(Outcome::Usage);
    };
    let Some(text) = session.read(&Path::new(LIBRARY).join(page)) else { return Ok(Outcome::Failure) };
    out.write_all(text.as_bytes())?;
    Ok(Outcome::Success)
}
