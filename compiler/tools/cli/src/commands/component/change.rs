//! `vun component add` and `remove`: libraries of a toolchain's release installed beside it, and
//! taken away again. The release is the manifest kept when the toolchain was installed, so
//! naming a component needs no network.

use std::io;

use vun_build::release::{Build, Release, Releases};
use vun_build::toolchains::Toolchains;
use vun_command::{Args, Arity, Command, CommandVerb, Example, Outcome, Param};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::install::{Installed, Uninstalled};
use vun_span::Version;
use vun_target::Machine;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use super::{TOOLCHAIN, installed};

const NAMES: Param = Param::positional("names")
    .arity(Arity::Many)
    .about("the components, by name: `backend-cranelift`, or `cranelift` when only one has it");

pub(super) const ADD: Command<Session> = Command::run(
    CommandVerb::Add,
    "install components from the toolchain's release, and what they need",
    add,
)
.params(&[NAMES, TOOLCHAIN])
.examples(&[Example {
    line: "backend-cranelift",
    about: "add the Cranelift backend to the running toolchain",
}]);

pub(super) const REMOVE: Command<Session> =
    Command::run(CommandVerb::Remove, "remove installed components", remove)
        .params(&[NAMES, TOOLCHAIN])
        .examples(&[Example { line: "cranelift", about: "remove the Cranelift backend" }]);

/// A toolchain whose release builds components, and the words naming some of them.
struct Chosen<'a> {
    toolchains: Toolchains,
    version: Version,
    release: Release,
    words: Vec<&'a str>,
}

impl Chosen<'_> {
    fn build(&self) -> Option<&Build> {
        self.release.build_for(Machine::RUNNING)
    }
}

/// What `args` choose, once a toolchain is installed, has a release and the components are named;
/// `None` once why not is reported.
fn chosen<'a>(session: &mut Session, args: &'a Args) -> Option<Chosen<'a>> {
    let (toolchains, version) = installed(session, args)?;
    let words: Vec<&str> = args.values(&NAMES).collect();
    if words.is_empty() {
        let msg = Msg::new().text("name the components");
        let help = Msg::new().text("the toolchain's are listed by ").code("vun component list");
        session.diag.error(Diagnostic::error(errors::CLI_MISSING_ARGUMENT, msg).help(help));
        return None;
    }
    let Some(release) = toolchains.release(version) else {
        let msg = Msg::new()
            .text("the toolchain ")
            .code(version.to_string())
            .text(" has no release to take components from");
        let help = Msg::new()
            .text("it was not installed from one; install a release with ")
            .code("vun toolchain install");
        session.diag.error(Diagnostic::error(errors::TOOLCHAIN_NO_COMPONENT, msg).help(help));
        return None;
    };
    Some(Chosen { toolchains, version, release, words })
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn add(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(chosen) = chosen(session, args) else { return Ok(session.outcome()) };
    let Some(build) = chosen.build() else {
        session.uninstalled(Some(chosen.version), &Uninstalled::Unbuilt(Machine::RUNNING));
        return Ok(session.outcome());
    };
    let Some(names) = session.pieces_named(build, &chosen.words) else { return Ok(session.outcome()) };
    let releases = match Releases::user() {
        Ok(releases) => releases,
        Err(why) => {
            session.uninstalled(Some(chosen.version), &Uninstalled::Repository(why));
            return Ok(session.outcome());
        }
    };
    match session.install_components(&chosen.toolchains, &releases, &chosen.release, &names) {
        Ok(installed) => {
            for (name, installed) in installed {
                shell.status(&Added { name, installed });
            }
        }
        Err(why) => {
            session.uninstalled(Some(chosen.version), &why);
        }
    }
    Ok(session.outcome())
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn remove(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(chosen) = chosen(session, args) else { return Ok(session.outcome()) };
    let Some(build) = chosen.build() else {
        session.uninstalled(Some(chosen.version), &Uninstalled::Unbuilt(Machine::RUNNING));
        return Ok(session.outcome());
    };
    let Some(names) = session.pieces_named(build, &chosen.words) else { return Ok(session.outcome()) };
    if let Ok(removed) = session.uninstall_components(&chosen.toolchains, chosen.version, build, &names) {
        for (name, was_installed) in removed {
            shell.status(&Removed { name, was_installed });
        }
    }
    Ok(session.outcome())
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

/// A component removed, or found not to be installed.
struct Removed {
    name: String,
    was_installed: bool,
}

impl Record for Removed {
    fn kind(&self) -> &'static str {
        "component_removed"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.name.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.name.clone()).with("was_installed", self.was_installed)
    }
}

impl Status for Removed {
    fn verb(&self) -> Verb {
        if self.was_installed { Verb::good("Removed") } else { Verb::warned("Absent") }
    }
}
