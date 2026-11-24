//! `vun component list`: every component of a toolchain, whether built in, installed from its
//! release or only available there.

use std::io;

use vun_command::{Args, Command, CommandVerb, Example, Outcome};
use vun_data::Object;
use vun_driver::Session;
use vun_driver::install::{Component, Has};
use vun_target::{Names, Readiness};
use vun_term::{Column, Record, Row, Shell, Style, Text};

use super::{TOOLCHAIN, asked};

pub(super) const LIST: Command<Session> = Command::run(
    CommandVerb::List,
    "list the components of a toolchain: the backends, linkers and other parts, and which it has",
    list,
)
.params(&[TOOLCHAIN])
.examples(&[
    Example { line: "", about: "the running toolchain's components" },
    Example { line: "--toolchain 0.4.2", about: "those an installed toolchain has, or its release builds" },
]);

fn list(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some((toolchains, version)) = asked(session, args) else { return Ok(session.outcome()) };
    let rows: Vec<Listed> =
        session.components_of(Some(&toolchains), version).into_iter().map(Listed).collect();
    shell.table(&rows)?;
    Ok(session.outcome())
}

/// What a toolchain has of a component, as one word.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    BuiltIn,
    Loaded,
    Installed,
    Available,
    /// Declared, not yet implemented.
    Planned,
}

impl State {
    const fn name(self) -> &'static str {
        match self {
            Self::BuiltIn => "built in",
            Self::Loaded => "loaded",
            Self::Installed => "installed",
            Self::Available => "available",
            Self::Planned => "planned",
        }
    }

    const fn style(self) -> Style {
        match self {
            Self::BuiltIn | Self::Loaded | Self::Installed => Style::Good,
            Self::Available | Self::Planned => Style::Dim,
        }
    }
}

struct Listed(Component);

impl Listed {
    fn state(&self) -> State {
        match (&self.0.has, self.0.readiness) {
            (_, Readiness::Planned) => State::Planned,
            (Has::BuiltIn, _) => State::BuiltIn,
            (Has::Loaded(_), _) => State::Loaded,
            (Has::Installed, _) => State::Installed,
            (Has::Available, _) => State::Available,
        }
    }

    fn handles(&self) -> Option<Vec<&'static str>> {
        match self.0.handles {
            Names::Any => None,
            Names::Only(names) => Some(names.iter().map(|n| n.0).collect()),
        }
    }
}

impl Record for Listed {
    fn kind(&self) -> &'static str {
        "component"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let library = match &self.0.has {
            Has::Loaded(path) => Some(path.display().to_string()),
            Has::BuiltIn | Has::Installed | Has::Available => None,
        };
        Object::new()
            .with("kind", self.0.kind.map(vun_driver::install::Kind::name))
            .with("name", self.0.name.clone())
            .with("state", self.state().name())
            .with("library", library)
            .with("profile", self.0.profile.map(vun_build::release::Profile::name))
            .with("handles", self.handles())
            .with("outputs", self.0.outputs.clone())
    }
}

impl Row for Listed {
    const COLUMNS: &'static [Column] = &[
        Column::left("kind"),
        Column::left("name"),
        Column::left("state"),
        Column::left("profile"),
        Column::left("handles"),
        Column::left("outputs"),
    ];

    fn cells(&self) -> Vec<Text> {
        let state = self.state();
        let handles = self.handles().map_or_else(|| "any".to_owned(), |names| names.join(", "));
        let profile = self.0.profile.map_or("", vun_build::release::Profile::name);
        vec![
            Text::of(Style::Dim, self.0.kind.map_or("", vun_driver::install::Kind::name)),
            Text::of(Style::Literal, self.0.name.clone()),
            Text::of(state.style(), state.name()),
            Text::of(Style::Dim, profile),
            Text::of(Style::Dim, handles),
            Text::of(Style::Dim, self.0.outputs.join(", ")),
        ]
    }
}
