//! `vun toolchain`: what this toolchain is made of.

use std::io;

use vun_command::{Args, Command, Outcome};
use vun_json::Object;
use vun_term::{Column, Record, Row, Shell, Style, Text};

use vun_target::{Arches, Name, Readiness};

use crate::{Registry, Session};

pub(super) const GROUP: &str = "toolchain";

pub(super) const COMMANDS: &[Command<Session>] =
    &[Command::run("components", "list the backends and linkers in this toolchain", components)];

/// A component of the toolchain.
struct Component {
    kind: &'static str,
    name: &'static str,
    readiness: Readiness,
    /// The architectures a backend generates code for, or the object formats a linker links;
    /// `None` for any.
    handles: Option<Vec<&'static str>>,
    /// What a backend emits, or what a linker builds programs as.
    outputs: Vec<&'static str>,
}

impl Component {
    fn status(&self) -> &'static str {
        match self.readiness {
            Readiness::Ready => "ready",
            Readiness::Planned => "planned",
        }
    }

    fn handles(&self) -> String {
        self.handles.as_ref().map_or_else(|| "any".to_owned(), |names| names.join(", "))
    }
}

impl Record for Component {
    fn kind(&self) -> &'static str {
        "component"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("kind", self.kind)
            .with("name", self.name)
            .with("status", self.status())
            .with("handles", self.handles.clone())
            .with("outputs", self.outputs.clone())
    }
}

impl Row for Component {
    const COLUMNS: &'static [Column] = &[
        Column::left("kind"),
        Column::left("name"),
        Column::left("status"),
        Column::left("handles"),
        Column::left("outputs"),
    ];

    fn cells(&self) -> Vec<Text> {
        let status = match self.readiness {
            Readiness::Ready => Style::Good,
            Readiness::Planned => Style::Dim,
        };
        vec![
            Text::of(Style::Dim, self.kind),
            Text::of(Style::Literal, self.name),
            Text::of(status, self.status()),
            Text::of(Style::Dim, self.handles()),
            Text::of(Style::Dim, self.outputs.join(", ")),
        ]
    }
}

fn components(_: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let names = |list: &'static [Name]| list.iter().map(|n| n.0).collect();
    let backends = r.backends().map(vun_backend::Backend::info).map(|i| Component {
        kind: "backend",
        name: i.id.0,
        readiness: i.readiness,
        handles: match i.arches {
            Arches::Any => None,
            Arches::Only(list) => Some(names(list)),
        },
        outputs: vec![i.caps.emits.name()],
    });
    let linkers = r.linkers().map(vun_link::Linker::info).map(|i| Component {
        kind: "linker",
        name: i.id.0,
        readiness: i.readiness,
        handles: Some(names(i.formats)),
        outputs: i.outputs.iter().map(|o| o.name()).collect(),
    });
    shell.table(&backends.chain(linkers).collect::<Vec<_>>())?;
    Ok(Outcome::Success)
}
