//! Help pages generated from the command tree.

use std::fmt::Write;

use crate::parse::LONG;
use crate::{Action, HELP_COMMAND, Param, ParamKind, Program, Selected};

/// Marks commands that are declared but not implemented.
const PLANNED: &str = "(planned)";

impl<Cx> Selected<'_, Cx> {
    /// The help page of the last selected command.
    #[must_use]
    pub fn help(&self, program: &Program<Cx>) -> String {
        let command = self.last();
        let mut page = Page(String::new());
        if self.is_root() {
            let _ = writeln!(page.0, "{} {}", command.name, program.version);
        }
        let _ = writeln!(page.0, "{}\n\nusage: {}", command.about, self.usage());
        if !command.children().is_empty() {
            let _ = writeln!(page.0, "       {} {HELP_COMMAND} <command>", self.invocation());
        }

        let children = command.children().iter().map(|c| {
            let about = match c.action {
                Action::Planned => format!("{} {PLANNED}", c.about),
                Action::Run(_) | Action::Group(_) => c.about.to_owned(),
            };
            (c.name.to_owned(), about)
        });
        page.section("commands", children);
        let arguments = command.positionals().map(|p| (format!("<{}>", p.name), p.about.to_owned()));
        page.section("arguments", arguments);
        let options = self.options().map(|p| (p.spelling(), p.about.to_owned()));
        page.section("options", options);
        page.0
    }

    /// The usage line of the last selected command.
    pub(crate) fn usage(&self) -> String {
        let command = self.last();
        let mut line = self.invocation();
        if !command.children().is_empty() {
            line.push_str(" <command>");
        }
        line.push_str(" [options]");
        for p in command.positionals() {
            let _ = write!(line, " <{}>", p.name);
        }
        line
    }
}

impl Param {
    /// How it is written in help: its short form, if any, then its long form.
    fn spelling(&self) -> String {
        let short = self.short.map(|c| format!("-{c}, ")).unwrap_or_default();
        format!("{short}{}", self.long())
    }

    /// How it is written in full: `--name`, and `--name <name>` when it takes a value.
    #[must_use]
    pub fn long(&self) -> String {
        let value = if self.kind == ParamKind::Option { format!(" <{}>", self.name) } else { String::new() };
        format!("{LONG}{}{value}", self.name)
    }
}

/// A help page being written.
struct Page(String);

impl Page {
    /// A titled two-column table, omitted when empty.
    fn section(&mut self, title: &str, rows: impl Iterator<Item = (String, String)>) {
        let rows: Vec<_> = rows.collect();
        let Some(width) = rows.iter().map(|(left, _)| left.len()).max() else { return };
        let _ = writeln!(self.0, "\n{title}:");
        for (left, right) in rows {
            let _ = writeln!(self.0, "  {left:<width$}  {right}");
        }
    }
}
