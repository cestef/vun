//! `vun toolchain list`: the toolchains installed, the running one and the default marked.

use std::io;

use vun_command::{Args, Command, CommandVerb, Outcome};
use vun_data::Object;
use vun_span::Version;
use vun_term::{Column, Record, Row, Shell, Status, Style, Text, Verb};

use vun_driver::{Session, VERSION};

use super::toolchains;

pub(super) const LIST: Command<Session> = Command::run(
    CommandVerb::List,
    "list the toolchains installed, the running one and the default marked",
    list,
);

fn list(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(toolchains) = toolchains(session) else { return Ok(session.outcome()) };
    let running = VERSION.parse::<Version>().ok();
    let default = toolchains.default();
    let rows: Vec<Listed> = toolchains
        .installed()?
        .into_iter()
        .map(|version| Listed {
            version,
            running: Some(version) == running,
            default: Some(version) == default,
            dir: toolchains.dir(version).display().to_string(),
        })
        .collect();
    if rows.is_empty() {
        shell.status(&Bare);
    } else {
        shell.table(&rows)?;
    }
    Ok(session.outcome())
}

/// No toolchain is installed: the running one is a build run in place.
struct Bare;

impl Record for Bare {
    fn kind(&self) -> &'static str {
        "no_toolchain"
    }

    fn text(&self) -> Text {
        Text::of(Style::Dim, "none is installed, this one runs in place; ")
            .append(Text::of(Style::Literal, "vun toolchain install"))
            .append(Text::of(Style::Dim, " adds a release"))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
    }
}

impl Status for Bare {
    fn verb(&self) -> Verb {
        Verb::warned("Empty")
    }
}

/// An installed toolchain.
struct Listed {
    version: Version,
    running: bool,
    default: bool,
    dir: String,
}

impl Listed {
    fn marks(&self) -> Text {
        let marks: Vec<&str> = [(self.default, "default"), (self.running, "running")]
            .iter()
            .filter(|m| m.0)
            .map(|m| m.1)
            .collect();
        Text::of(Style::Good, marks.join(", "))
    }
}

impl Record for Listed {
    fn kind(&self) -> &'static str {
        "toolchain"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("version", self.version.to_string())
            .with("default", self.default)
            .with("running", self.running)
            .with("dir", self.dir.clone())
    }
}

impl Row for Listed {
    const COLUMNS: &'static [Column] = &[Column::left("version"), Column::left(""), Column::left("dir")];

    fn cells(&self) -> Vec<Text> {
        vec![
            Text::of(Style::Literal, self.version.to_string()),
            self.marks(),
            Text::of(Style::Dim, self.dir.clone()),
        ]
    }
}
