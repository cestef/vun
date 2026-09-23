//! Help pages generated from the command tree.

use std::fmt::Write;

use crate::parse::positionals;
use crate::{Action, HELP_COMMAND, Param, ParamKind, Program, Selected};

/// Marks commands that are declared but not implemented.
const PLANNED: &str = "(planned)";

/// The help page of the last selected command.
#[must_use]
pub fn help<Cx>(program: &Program<Cx>, selected: &Selected<'_, Cx>) -> String {
    let command = selected.last();
    let mut page = String::new();
    if selected.is_root() {
        let _ = writeln!(page, "{} {}", command.name, program.version);
    }
    let _ = writeln!(page, "{}\n\nusage: {}", command.about, usage(selected));
    if !command.children().is_empty() {
        let _ = writeln!(page, "       {} {HELP_COMMAND} <command>", selected.invocation());
    }

    let children = command.children().iter().map(|c| {
        let about = match c.action {
            Action::Planned => format!("{} {PLANNED}", c.about),
            Action::Run(_) | Action::Group(_) => c.about.to_owned(),
        };
        (c.name.to_owned(), about)
    });
    section(&mut page, "commands", children);
    let arguments = positionals(command).map(|p| (format!("<{}>", p.name), p.about.to_owned()));
    section(&mut page, "arguments", arguments);
    let options = selected.options().map(|p| (spelling(p), p.about.to_owned()));
    section(&mut page, "options", options);
    page
}

pub(crate) fn usage<Cx>(selected: &Selected<'_, Cx>) -> String {
    let command = selected.last();
    let mut line = selected.invocation();
    if !command.children().is_empty() {
        line.push_str(" <command>");
    }
    line.push_str(" [options]");
    for p in positionals(command) {
        let _ = write!(line, " <{}>", p.name);
    }
    line
}

fn spelling(p: &Param) -> String {
    let short = p.short.map(|c| format!("-{c}, ")).unwrap_or_default();
    let value = if p.kind == ParamKind::Option { format!(" <{}>", p.name) } else { String::new() };
    format!("{short}--{}{value}", p.name)
}

/// A titled two-column table, omitted when empty.
fn section(page: &mut String, title: &str, rows: impl Iterator<Item = (String, String)>) {
    let rows: Vec<_> = rows.collect();
    let Some(width) = rows.iter().map(|(left, _)| left.len()).max() else { return };
    let _ = writeln!(page, "\n{title}:");
    for (left, right) in rows {
        let _ = writeln!(page, "  {left:<width$}  {right}");
    }
}
