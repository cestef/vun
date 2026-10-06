//! `vun toolchain component`: the components of a toolchain, the libraries of its release's
//! backends and linkers, listed, added from the release and removed (spec 12, section 6).

use std::io;

use vun_build::release::{Releases, Wanted};
use vun_build::toolchains::Toolchains;
use vun_command::{Args, Arity, Command, Outcome, Param};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::install::{Held, Installed, Uninstalled};
use vun_driver::{Session, VERSION};
use vun_span::Version;
use vun_target::Machine;
use vun_term::{Column, Record, Row, Shell, Status, Style, Text, Verb};

use super::{toolchains, version};

const NAMES: Param = Param::positional("name").arity(Arity::Many).about("the components, by name");
const TOOLCHAIN: Param =
    Param::option("toolchain").about("the toolchain, `major.minor.patch`; the running one when none");

pub(super) const GROUP: Command<Session> = Command::group(
    "component",
    "the libraries of a toolchain's backends and linkers",
    &[
        Command::run("list", "list the components its release builds, and which are installed", list)
            .params(&[TOOLCHAIN]),
        Command::run("add", "install components from the toolchain's release", add)
            .params(&[NAMES, TOOLCHAIN]),
        Command::run("remove", "remove installed components", remove).params(&[NAMES, TOOLCHAIN]),
    ],
);

/// The toolchain asked for, and where it is, when it is installed.
fn target(session: &mut Session, args: &Args) -> Option<(Toolchains, Version)> {
    let toolchains = toolchains(session)?;
    let asked = version(session, args, &TOOLCHAIN).ok()?;
    let version = asked.or_else(|| Version::parse(VERSION))?;
    if toolchains.installed().is_ok_and(|installed| installed.contains(&version)) {
        return Some((toolchains, version));
    }
    let msg = Msg::new().text("the toolchain ").code(version.to_string()).text(" is not installed");
    session.diag.error(Diagnostic::error(errors::TOOLCHAIN_NOT_INSTALLED, msg));
    None
}

fn list(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some((toolchains, version)) = target(session, args) else { return Ok(session.outcome()) };
    let release = toolchains.release(version);
    let build = release.as_ref().and_then(|r| r.build_for(Machine::RUNNING));
    let rows: Vec<Listed> = build
        .map(|build| Held::of(&toolchains, version, build))
        .unwrap_or_default()
        .into_iter()
        .map(|held| Listed {
            name: held.piece.name.clone(),
            file: held.piece.file.clone(),
            installed: held.installed,
        })
        .collect();
    shell.table(&rows)?;
    Ok(session.outcome())
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn add(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some((toolchains, version)) = target(session, args) else { return Ok(session.outcome()) };
    let names: Vec<String> = args.values(&NAMES).map(str::to_owned).collect();
    let published = match session.toolchain_release(Wanted::Version(version)) {
        Ok(published) if toolchains.manifest(version).is_some_and(|kept| kept != published.hash()) => {
            let why = Uninstalled::Unlocked {
                expected: toolchains.manifest(version).unwrap_or(published.hash()),
                found: published.hash(),
            };
            session.uninstalled(Some(version), &why);
            return Ok(session.outcome());
        }
        Ok(published) => published,
        Err(why) => {
            session.uninstalled(Some(version), &why);
            return Ok(session.outcome());
        }
    };
    let releases = match Releases::user() {
        Ok(releases) => releases,
        Err(why) => {
            session.uninstalled(Some(version), &Uninstalled::Repository(why));
            return Ok(session.outcome());
        }
    };
    match session.install_components(&toolchains, &releases, &published, &names) {
        Ok(installed) => {
            for (name, installed) in installed {
                shell.status(&Added { name, installed });
            }
        }
        Err(why) => {
            session.uninstalled(Some(version), &why);
        }
    }
    Ok(session.outcome())
}

fn remove(session: &mut Session, args: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some((toolchains, version)) = target(session, args) else { return Ok(session.outcome()) };
    let release = toolchains.release(version);
    let build = release.as_ref().and_then(|r| r.build_for(Machine::RUNNING));
    for name in args.values(&NAMES) {
        if let Some(piece) = build.and_then(|build| build.piece(name)) {
            toolchains.remove_component(version, piece)?;
        } else {
            let msg = Msg::new().text("the toolchain has no component ").code(name.to_owned());
            session.diag.error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg));
        }
    }
    Ok(session.outcome())
}

/// A component of a toolchain's release.
struct Listed {
    name: String,
    file: String,
    installed: bool,
}

impl Record for Listed {
    fn kind(&self) -> &'static str {
        "component"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.name.clone())
            .with("file", self.file.clone())
            .with("installed", self.installed)
    }
}

impl Row for Listed {
    const COLUMNS: &'static [Column] = &[Column::left("name"), Column::left("state"), Column::left("file")];

    fn cells(&self) -> Vec<Text> {
        let state = if self.installed {
            Text::of(Style::Good, "installed")
        } else {
            Text::of(Style::Dim, "available")
        };
        vec![Text::of(Style::Literal, self.name.clone()), state, Text::of(Style::Dim, self.file.clone())]
    }
}

/// A component added.
struct Added {
    name: String,
    installed: Installed,
}

impl Record for Added {
    fn kind(&self) -> &'static str {
        "component_added"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.name.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.name.clone()).with("fresh", self.installed == Installed::Fresh)
    }
}

impl Status for Added {
    fn verb(&self) -> Verb {
        match self.installed {
            Installed::Fresh => Verb::good("Added"),
            Installed::Already => Verb::good("Present"),
        }
    }
}
