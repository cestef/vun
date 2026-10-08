//! `vun toolchain install [version]` and `vun toolchain update`: a release's toolchain fetched,
//! checked against its manifest and installed beside the others; `update` makes the latest the
//! default, the one the user's `vun` links to.

use std::io;

use vun_build::release::{Profile, Releases, Wanted};
use vun_command::{Args, Arity, Command, Outcome, Param, Values};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_span::Version;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use vun_driver::Session;
use vun_driver::install::{Installed, Uninstalled};

use super::super::status::{Started, Step};
use super::{toolchains, version};

const VERSION: Param = Param::positional("version")
    .arity(Arity::Optional)
    .about("the release to install, `major.minor.patch`; the latest when none");
const PROFILE: Param = Param::option("profile")
    .about("how many of the release's components to install with it")
    .values(Values::Choice(|| Profile::ALL.iter().map(|p| p.name()).collect()));
/// The profile installed when none is named.
const DEFAULT_PROFILE: Profile = Profile::Default;
const DEFAULT: Param = Param::flag("default").about("make it the default, the one the user's `vun` links to");

pub(super) const INSTALL: Command<Session> =
    Command::run("install", "install a release's toolchain beside the others", install)
        .params(&[VERSION, DEFAULT, PROFILE]);
pub(super) const UPDATE: Command<Session> =
    Command::run("update", "install the latest release's toolchain and make it the default", update)
        .params(&[PROFILE]);

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn install(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Ok(version) = version(session, args, &VERSION) else { return Ok(session.outcome()) };
    let wanted = version.map_or(Wanted::Latest, Wanted::Version);
    let profile = args.value(&PROFILE).and_then(Profile::named).unwrap_or(DEFAULT_PROFILE);
    Ok(settle(session, shell, wanted, args.flag(&DEFAULT), profile))
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn update(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let profile = args.value(&PROFILE).and_then(Profile::named).unwrap_or(DEFAULT_PROFILE);
    Ok(settle(session, shell, Wanted::Latest, true, profile))
}

/// Installs the release `wanted` with the components of its profile `profile`, and makes it the
/// default if `default`.
fn settle(
    session: &mut Session,
    shell: &mut Shell<'_>,
    wanted: Wanted,
    default: bool,
    profile: Profile,
) -> Outcome {
    let Some(toolchains) = toolchains(session) else { return session.outcome() };
    let asked = match wanted {
        Wanted::Latest => None,
        Wanted::Version(version) => Some(version),
    };
    let published = match session.toolchain_release(wanted) {
        Ok(published) => published,
        Err(why) => {
            session.uninstalled(asked, &why);
            return session.outcome();
        }
    };
    let release = &published.release;
    shell.status(&Started { step: Step::Install, subject: release.version.to_string() });
    let installed = match session.install(&toolchains, &published) {
        Ok(installed) => installed,
        Err(why) => {
            session.uninstalled(Some(release.version), &why);
            return session.outcome();
        }
    };
    let components = Releases::user().map_err(Uninstalled::Repository).and_then(|releases| {
        let names = Session::profile_components(&published.release, profile)?;
        session.install_components(&toolchains, &releases, &published.release, &names)
    });
    if let Err(why) = components {
        session.uninstalled(Some(release.version), &why);
        return session.outcome();
    }
    if default && let Err(e) = toolchains.make_default(release.version) {
        let at = toolchains.link().display().to_string();
        let msg = Msg::new().text("cannot link the default toolchain at ").code(at);
        session.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
        return session.outcome();
    }
    shell.status(&Settled { version: release.version, installed, default });
    session.outcome()
}

/// How installing a toolchain ended.
struct Settled {
    version: Version,
    installed: Installed,
    /// Whether it is now the default.
    default: bool,
}

impl Record for Settled {
    fn kind(&self) -> &'static str {
        "installed"
    }

    fn text(&self) -> Text {
        let text = Text::of(Style::Strong, self.version.to_string());
        if self.default { text.append(Text::of(Style::Dim, ", the default")) } else { text }
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("version", self.version.to_string())
            .with("fresh", self.installed == Installed::Fresh)
            .with("default", self.default)
    }
}

impl Status for Settled {
    fn verb(&self) -> Verb {
        match self.installed {
            Installed::Fresh => Verb::good("Installed"),
            Installed::Already => Verb::good("Present"),
        }
    }
}
