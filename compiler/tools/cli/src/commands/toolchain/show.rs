//! `vun toolchain show`: the running toolchain's version, where it and its library are, and the
//! backends and linkers compiled into it.

use std::io;

use vun_command::{Args, Command, Outcome};
use vun_data::Object;
use vun_term::{Column, Record, Row, Shell, Style, Text};

use vun_driver::packages::{Found, Library};
use vun_driver::{Registry, Session, VERSION};
use vun_target::{Name, Names, Readiness};

pub(super) const SHOW: Command<Session> =
    Command::run("show", "show this toolchain's version, library, backends and linkers", show);

fn show(_: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let library = Library::find();
    let binary = std::env::current_exe()?;
    let facts = [
        Fact { about: About::Version, value: VERSION.to_owned(), why: None },
        Fact { about: About::Binary, value: binary.display().to_string(), why: None },
        Fact { about: About::Library, value: library.dir.display().to_string(), why: Some(library.found) },
    ];
    shell.table(&facts)?;
    let r = Registry::with_defaults();
    let backends = r.backends().map(vun_backend::Backend::info).map(|i| Component {
        kind: Kind::Backend,
        name: i.id,
        readiness: i.readiness,
        handles: i.arches,
        outputs: vec![i.caps.emits.name()],
    });
    let linkers = r.linkers().map(vun_link::Linker::info).map(|i| Component {
        kind: Kind::Linker,
        name: i.id,
        readiness: i.readiness,
        handles: Names::Only(i.formats),
        outputs: i.outputs.iter().map(|o| o.name()).collect(),
    });
    shell.table(&backends.chain(linkers).collect::<Vec<_>>())?;
    Ok(Outcome::Success)
}

/// What a fact of the toolchain is about.
#[derive(Clone, Copy)]
enum About {
    Version,
    Binary,
    Library,
}

impl About {
    fn name(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Binary => "binary",
            Self::Library => "library",
        }
    }
}

/// A fact of the toolchain, with why, for the library, it is where it is.
struct Fact {
    about: About,
    value: String,
    why: Option<Found>,
}

impl Fact {
    fn why(&self) -> Option<Text> {
        let dim = |text| Text::of(Style::Dim, text);
        self.why.map(|found| match found {
            Found::Named => dim("named by ").append(Text::of(Style::Literal, Library::ENV)),
            Found::Installed => dim("beside the binary"),
            Found::Checkout => dim("the checkout it was built from"),
        })
    }
}

impl Record for Fact {
    fn kind(&self) -> &'static str {
        "fact"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let why = self.why().map(|w| w.to_string());
        Object::new().with("fact", self.about.name()).with("value", self.value.clone()).with("why", why)
    }
}

impl Row for Fact {
    const COLUMNS: &'static [Column] =
        &[Column::left("toolchain"), Column::left("value"), Column::left("why")];

    fn cells(&self) -> Vec<Text> {
        let value = Text::of(Style::Literal, self.value.clone());
        vec![Text::of(Style::Key, self.about.name()), value, self.why().unwrap_or_default()]
    }
}

/// What a component is.
#[derive(Clone, Copy)]
enum Kind {
    Backend,
    Linker,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Linker => "linker",
        }
    }
}

/// A component of the toolchain.
struct Component {
    kind: Kind,
    name: Name,
    readiness: Readiness,
    /// The architectures a backend generates code for, or the object formats a linker links.
    handles: Names,
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

    fn handles(&self) -> Option<Vec<&'static str>> {
        match self.handles {
            Names::Any => None,
            Names::Only(names) => Some(names.iter().map(|n| n.0).collect()),
        }
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
            .with("kind", self.kind.name())
            .with("name", self.name.0)
            .with("status", self.status())
            .with("handles", self.handles())
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
        let handles = self.handles().map_or_else(|| "any".to_owned(), |names| names.join(", "));
        vec![
            Text::of(Style::Dim, self.kind.name()),
            Text::of(Style::Literal, self.name.0),
            Text::of(status, self.status()),
            Text::of(Style::Dim, handles),
            Text::of(Style::Dim, self.outputs.join(", ")),
        ]
    }
}
