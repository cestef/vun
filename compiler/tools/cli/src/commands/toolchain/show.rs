//! `vun toolchain show`: the running toolchain's version, where it and its library are, and how
//! many components it has.

use std::io;

use vun_command::{Args, Command, Outcome};
use vun_data::Object;
use vun_term::{Column, Record, Row, Shell, Style, Text};

use vun_build::toolchains::Toolchains;
use vun_driver::install::Has;
use vun_driver::packages::{Found, Library};
use vun_driver::{Session, VERSION};
use vun_span::Version;

pub(super) const SHOW: Command<Session> = Command::run(
    "show",
    "show the running toolchain: its version, binary and library, and its components",
    show,
);

fn show(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let library = Library::find();
    let binary = vun_fs::EXECUTABLE.clone().ok_or(io::ErrorKind::NotFound)?;
    let facts = [
        Fact { about: About::Version, value: VERSION.to_owned(), why: None },
        Fact { about: About::Binary, value: binary.display().to_string(), why: None },
        Fact {
            about: About::Library,
            value: library.dir.display().to_string(),
            why: Some(Why::Library(library.found)),
        },
        Fact { about: About::Components, value: components(session), why: Some(Why::See(SEE_COMPONENTS)) },
    ];
    shell.table(&facts)?;
    Ok(Outcome::Success)
}

/// The command listing the components, which a count of them points to.
const SEE_COMPONENTS: &str = "vun component list";

/// How many components the running toolchain has, by how it has them.
fn components(session: &Session) -> String {
    let Some(version) = Version::parse(VERSION) else { return String::new() };
    let listed = session.components_of(Toolchains::user().as_ref(), version);
    let count = |is: fn(&Has) -> bool| listed.iter().filter(|c| is(&c.has)).count();
    let counts = [
        ("built in", count(|h| matches!(h, Has::BuiltIn))),
        ("loaded", count(|h| matches!(h, Has::Loaded(_)))),
        ("installed", count(|h| matches!(h, Has::Installed))),
        ("available", count(|h| matches!(h, Has::Available))),
    ];
    let shown: Vec<String> =
        counts.into_iter().filter(|&(_, n)| n > 0).map(|(what, n)| format!("{n} {what}")).collect();
    shown.join(", ")
}

/// What a fact of the toolchain is about.
#[derive(Clone, Copy)]
enum About {
    Version,
    Binary,
    Library,
    Components,
}

impl About {
    const fn name(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Binary => "binary",
            Self::Library => "library",
            Self::Components => "components",
        }
    }
}

/// Why a fact is what it is.
#[derive(Clone, Copy)]
enum Why {
    /// How the library was found.
    Library(Found),
    /// A command that says more.
    See(&'static str),
}

/// A fact of the toolchain, with why it is so, or where to see more.
struct Fact {
    about: About,
    value: String,
    why: Option<Why>,
}

impl Fact {
    fn why(&self) -> Option<Text> {
        let dim = |text| Text::of(Style::Dim, text);
        self.why.map(|why| match why {
            Why::Library(Found::Named) => dim("named by ").append(Text::of(Style::Literal, Library::ENV)),
            Why::Library(Found::Installed) => dim("beside the binary"),
            Why::Library(Found::Checkout) => dim("the checkout it was built from"),
            Why::See(command) => dim("see ").append(Text::of(Style::Literal, command)),
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
