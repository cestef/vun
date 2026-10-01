//! What commands report, shown for people or for programs. A command never chooses between
//! the two: it reports records, and the shell's [`Format`] decides how each is shown.

use std::io;

use vun_data::{Object, Value};

use crate::{Column, Shell, Table, Text, Verb, Verbosity};

/// How output is shown: for people, or as JSON lines for programs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Human,
    /// One JSON object per line on standard output, each naming its `type`: results, status
    /// events and diagnostics alike.
    Json,
    /// What people read, on the streams they read it on, each line a JSON object of its pieces
    /// and their styles: for tools that draw vun's output themselves, as the brand's videos do.
    Styled,
}

impl Format {
    pub const ALL: [Self; 3] = [Self::Human, Self::Json, Self::Styled];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Styled => "styled",
        }
    }
}

/// Something a command reports.
pub trait Record {
    /// Its `type` in JSON, such as `finished` or `package`.
    fn kind(&self) -> &'static str;
    /// How people read it.
    fn text(&self) -> Text;
    /// Its fields for programs, without its `type`.
    fn fields(&self) -> Object<'static>;

    /// The JSON line: its `type`, then its fields.
    fn json(&self) -> Value<'static> {
        let head = Object::new().with("type", self.kind());
        Value::Object(
            self.fields().iter().fold(head, |o, (name, value)| o.with(name.to_owned(), value.clone())),
        )
    }
}

/// A named text a command produces: a dump, a page, a script. People read the text as is.
pub struct Document {
    pub kind: &'static str,
    pub name: String,
    pub text: String,
}

impl Record for Document {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn text(&self) -> Text {
        Text::from(self.text.trim_end_matches('\n').to_owned())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.name.clone()).with("text", self.text.clone())
    }
}

/// A record reported as progress: a status line for people.
pub trait Status: Record {
    fn verb(&self) -> Verb;
}

/// A record shown as a row of a table for people.
pub trait Row: Record {
    const COLUMNS: &'static [Column];
    fn cells(&self) -> Vec<Text>;
}

impl Shell<'_> {
    /// Reports a result: its text on standard output, or its JSON line. A record with nothing
    /// to say to people, such as a check that passed, shows only as JSON.
    ///
    /// # Errors
    /// Fails when the output does.
    pub fn result(&mut self, record: &dyn Record) -> io::Result<()> {
        match self.format() {
            Format::Json => self.out.line(&Text::from(vun_data::json::Json(&record.json()).to_string())),
            Format::Human | Format::Styled if record.text().width() == 0 => Ok(()),
            Format::Human | Format::Styled => self.out.line(&record.text()),
        }
    }

    /// Reports results as a table, or as one JSON line each.
    ///
    /// # Errors
    /// Fails when the output does.
    pub fn table<R: Row>(&mut self, rows: &[R]) -> io::Result<()> {
        match self.format() {
            Format::Human | Format::Styled => {
                self.out.print(&Table::new(R::COLUMNS).rows(rows.iter().map(Row::cells)).text())
            }
            Format::Json => rows.iter().try_for_each(|r| self.result(r)),
        }
    }

    /// Reports what `write` writes as the document `kind` named `name`, and passes on what it
    /// returns.
    ///
    /// # Errors
    /// Fails when `write` or the output does.
    pub fn document<T>(
        &mut self,
        kind: &'static str,
        name: String,
        write: impl FnOnce(&mut dyn io::Write) -> io::Result<T>,
    ) -> io::Result<T> {
        let mut bytes = Vec::new();
        let done = write(&mut bytes)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        self.result(&Document { kind, name, text })?;
        Ok(done)
    }

    /// Reports progress, unless quiet: a status line on standard error, or a JSON line on
    /// standard output. A failure is a result, not progress, and is reported even when quiet.
    /// Best-effort: a closed stream does not fail the command.
    pub fn status(&mut self, status: &dyn Status) {
        if self.verbosity == Verbosity::Quiet && !status.verb().is_failure() {
            return;
        }
        let _ = match self.format() {
            Format::Human | Format::Styled => self.err.line(&status.verb().line(status.text())),
            Format::Json => self.result(status),
        };
    }
}
