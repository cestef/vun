//! `vun explain <id>`: the explanation page of a diagnostic, an error's or a lint's, offline (spec
//! 10, section 1).

use std::io;

use vun_command::{Args, Command, CommandVerb, Example, Outcome, Param};
use vun_diag::{DiagId, Diagnostic, Msg, errors};
use vun_suggest::closest;
use vun_term::{Document, Format, Shell, Text};

use vun_driver::{Page, Session};

const ID: Param =
    Param::positional("id").about("a diagnostic, such as names.duplicate or lang.errors.names.duplicate");

pub(super) const EXPLAIN: Command<Session> =
    Command::run(CommandVerb::Explain, "explain a diagnostic", explain)
        .params(&[ID])
        .examples(&[Example { line: "int.does_not_fit", about: "the page of a diagnostic, by its id" }]);

fn explain(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let id = args.value(&ID).expect("positional parameters are present when a command runs");
    let pages = session.pages();
    // A compiler error by its id as displayed, `int.does_not_fit`, or any by its binding path.
    let wanted = [DiagId::compiler(id), DiagId::from(id)];
    let Some((_, page)) = pages.iter().find(|(d, _)| wanted.contains(d)) else {
        let msg = Msg::new().text("no diagnostic is named ").code(id);
        let names: Vec<String> = pages.iter().map(|(d, _)| d.to_string()).collect();
        let d = Diagnostic::error(errors::CLI_INVALID_VALUE, msg);
        let d = match closest(id, names.iter().map(String::as_str)) {
            Some(near) => d.help(Msg::did_you_mean(near)),
            None => d.help(Msg::new().text("name an error of ").code("lang.errors").text(", or a lint")),
        };
        session.diag.error(d);
        return Ok(Outcome::Usage);
    };
    let text = match page {
        Page::File(path) => {
            let Some(text) = session.read(path) else { return Ok(Outcome::Failure) };
            text
        }
        Page::Text(text) => text.clone(),
    };
    match shell.format() {
        Format::Human | Format::Styled => shell.page(&Text::markdown(&text))?,
        Format::Json => shell.result(&Document { kind: "explanation", name: id.to_owned(), text })?,
    }
    Ok(Outcome::Success)
}
