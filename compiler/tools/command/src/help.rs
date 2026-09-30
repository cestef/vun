//! Help pages generated from the command tree, laid out and styled as cargo's.

use vun_term::{Column, Style, Table, Text};

use crate::parse::{LONG, SHORT};
use crate::{Arity, Detail, HELP_COMMAND, Param, ParamKind, Selected, Values};

/// The two columns of a help section: what to write, and what it does.
const SECTION: [Column; 2] = [Column::left(""), Column::left("")];
/// The title of the usage lines, which the lines below it are aligned past.
const USAGE: &str = "Usage: ";
/// The indentation of a section's rows.
const INDENT: usize = 2;

impl<Cx> Selected<'_, Cx> {
    /// The help page of the last selected command; the long page adds aliases, the values and
    /// environment variables of parameters, and examples.
    #[must_use]
    pub fn help(&self, detail: Detail) -> Text {
        let command = self.last();
        let usage = Text::of(Style::Heading, USAGE);
        let mut page =
            Text::new().plain(command.about).newline().newline().append(usage).append(self.usage());
        if !command.children().is_empty() {
            let also = Text::of(Style::Literal, format!("{} {HELP_COMMAND}", self.invocation()));
            page = page
                .newline()
                .pad(USAGE.len())
                .append(also)
                .plain(" ")
                .styled(Style::Placeholder, "<command>");
        }
        page = page.newline();

        let long = detail == Detail::Long;
        let children = command.children().iter().map(|c| {
            let shown = if long { c.aliases } else { &[] };
            let names =
                std::iter::once(c.name).chain(shown.iter().copied()).map(|n| Text::of(Style::Literal, n));
            [Text::join(names, ", "), Text::from(c.about)]
        });
        let arguments = command.positionals().flat_map(|p| p.rows(p.placeholder(), detail));
        let options = self.sections().into_iter().map(|(title, params)| {
            (title, params.into_iter().flat_map(|p| p.rows(p.spelling(), detail)).collect())
        });
        let examples = command.examples.iter().filter(|_| long).map(|e| {
            [
                Text::of(Style::Literal, format!("{} {}", self.invocation(), e.line)),
                Text::of(Style::Dim, e.about),
            ]
        });
        let sections: Vec<(&str, Vec<[Text; 2]>)> =
            [("Commands", children.collect()), ("Arguments", arguments.collect())]
                .into_iter()
                .chain(options)
                .chain([("Examples", examples.collect())])
                .collect();
        sections.into_iter().filter(|(_, rows)| !rows.is_empty()).fold(page, |page, (title, rows)| {
            let table = Table::new(&SECTION).indent(INDENT).rows(rows);
            page.newline().styled(Style::Heading, format!("{title}:")).newline().append(table.text())
        })
    }

    /// The usage line of the last selected command: `vun build [options] <file>`.
    #[must_use]
    pub fn usage(&self) -> Text {
        let command = self.last();
        let mut line = Text::of(Style::Literal, self.invocation());
        if !command.children().is_empty() {
            line = line.plain(" ").styled(Style::Placeholder, "<command>");
        }
        line = line.plain(" ").styled(Style::Placeholder, "[options]");
        command.positionals().fold(line, |line, p| line.plain(" ").append(p.placeholder()))
    }
}

impl Param {
    /// How it is written in help: its short form, if any, then its long form.
    fn spelling(&self) -> Text {
        let short = self.short_form().map(|s| Text::of(Style::Literal, s).plain(", "));
        let long = Text::of(Style::Literal, self.long_form());
        let value = self.takes_value().then(|| Text::new().plain(" ").append(self.placeholder()));
        short.unwrap_or_default().append(long).append(value.unwrap_or_default())
    }

    /// What stands for its values: `<name>`, `[name]` when optional, `[name]...` when many.
    fn placeholder(&self) -> Text {
        let written = match self.kind {
            ParamKind::Positional(Arity::Optional) => format!("[{}]", self.name),
            ParamKind::Positional(Arity::Many) => format!("[{}]...", self.name),
            _ => self.value_form(),
        };
        Text::of(Style::Placeholder, written)
    }

    /// Its rows in a section: how it is written and what it does, then in the long page a row
    /// under the description for its values and environment variable.
    fn rows(&self, written: Text, detail: Detail) -> Vec<[Text; 2]> {
        let row = [written, Text::from(self.about)];
        let values = match self.values {
            Values::Choice(names) => Some(format!("[values: {}]", names().join(", "))),
            Values::Catalog(catalog) => Some(format!("[values: {}]", catalog.described())),
            Values::Text | Values::Path(_) => None,
        };
        let env = self.env.map(|name| format!("[env: {name}]"));
        let extras: Vec<String> = [values, env].into_iter().flatten().collect();
        if detail == Detail::Short || extras.is_empty() {
            return vec![row];
        }
        vec![row, [Text::new(), Text::of(Style::Dim, extras.join(" "))]]
    }

    /// How messages name it: `--name`, or `<name>` for a positional.
    #[must_use]
    pub fn written(&self) -> String {
        if self.is_positional() { self.value_form() } else { self.long_form() }
    }

    /// How it is written in full: `--name`, and `--name <name>` when it takes a value.
    #[must_use]
    pub fn long(&self) -> String {
        let long = self.long_form();
        if self.takes_value() { [long, self.value_form()].join(" ") } else { long }
    }

    /// `--name`.
    #[must_use]
    pub fn long_form(&self) -> String {
        [LONG, self.name].concat()
    }

    /// `-c`, when it has a short form.
    #[must_use]
    pub fn short_form(&self) -> Option<String> {
        self.short.map(|c| [SHORT, c].iter().collect())
    }

    /// `<name>`: what stands for its value.
    #[must_use]
    pub fn value_form(&self) -> String {
        ["<", self.name, ">"].concat()
    }
}
