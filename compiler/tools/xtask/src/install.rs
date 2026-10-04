//! `cargo xtask install`: builds `vun` from this checkout and installs it as a toolchain beside
//! the others, the one the user's `vun` links to (spec 12, section 6). The toolchain of the
//! workspace's version is replaced, so installing again picks up the new build.

use std::io;
use std::path::{Path, PathBuf};

use vun_build::toolchains::Toolchains;
use vun_command::{Args, Outcome, Param, Shell, Values};
use vun_data::Object;
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Edit;
use vun_harness::repo_root;
use vun_span::Version;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::Executable;

use crate::cargo;
use crate::toolchain::Stage;

/// The cargo profile the `vun` to install is built with.
pub(crate) const PROFILE: Param = Param::option("profile")
    .about("the cargo profile to build `vun` with")
    .values(Values::Choice(|| PROFILES.to_vec()));

/// The profiles `vun` is built with; `dist` is what a release ships.
const PROFILES: &[&str] = &["dist", "release", "dev"];
const DEFAULT: &str = "dist";

/// What the installed executable is called.
const NAME: &str = "vun";

const UNBUILT: DiagId = DiagId::new("xtask.install.unbuilt");
const HOMELESS: DiagId = DiagId::new("xtask.install.homeless");
const UNSTAGED: DiagId = DiagId::new("xtask.install.unstaged");
const SHADOWED: DiagId = DiagId::new("xtask.install.shadowed");
const UNINSTALLED: DiagId = DiagId::new("xtask.install.uninstalled");

pub(crate) fn install(diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(toolchains) = Toolchains::user() else {
        let msg = Msg::new().text("this user has no home directory to install a toolchain in");
        diag.error(Diagnostic::error(HOMELESS, msg));
        return Ok(Outcome::Failure);
    };
    let version = Version::parse(env!("CARGO_PKG_VERSION")).expect("the workspace's version is one");
    let profile = profile(args);
    let root = repo_root();
    let binary = match cargo::vun(&root, profile) {
        Ok(binary) => binary,
        Err(why) => {
            let msg = Msg::new().text("cannot build ").code("vun").text(" from the checkout");
            diag.error(Diagnostic::error(UNBUILT, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let staging = Stage::at(&root, toolchains.staging(version));
    if let Err(why) = staging.lay(&binary) {
        staging.dir().remove()?;
        let msg = Msg::new().text("cannot put the toolchain ").code(version.to_string()).text(" together");
        diag.error(Diagnostic::error(UNSTAGED, msg).caused_by(&why));
        return Ok(Outcome::Failure);
    }
    // A toolchain of the same version is replaced, so the new build is the one installed.
    if toolchains.dir(version).exists() {
        toolchains.remove(version)?;
    }
    let settle = toolchains.install(version, staging.dir()).and_then(|()| toolchains.make_default(version));
    if let Err(why) = settle {
        let msg = Msg::new().text("cannot install the toolchain ").code(version.to_string());
        diag.error(Diagnostic::error(UNINSTALLED, msg).caused_by(&why));
        return Ok(Outcome::Failure);
    }
    shell.status(&Installed { version, profile });
    if let Some(first) = Shadow::first(toolchains.link()) {
        let msg = Msg::new()
            .text("another ")
            .code("vun")
            .text(" comes first on the search path: ")
            .code(first.display().to_string());
        let help = Msg::new()
            .text("remove it, or put ")
            .code(toolchains.link().display().to_string())
            .text(" before it");
        diag.report(Diagnostic::warning(SHADOWED, msg).help(help));
    }
    Ok(Outcome::Success)
}

/// A `vun` on the search path that is not the one installed.
struct Shadow;

impl Shadow {
    /// The first `vun` on the search path when it is not the file `link` resolves to.
    fn first(link: &Path) -> Option<PathBuf> {
        let first = Executable::search(NAME)?;
        let same = first.canonicalize().ok().zip(link.canonicalize().ok()).is_some_and(|(a, b)| a == b);
        (!same).then_some(first)
    }
}

/// The profile `args` names, one of [`PROFILES`], `dist` when it names none.
fn profile(args: &Args) -> &'static str {
    let named = args.value(&PROFILE).unwrap_or(DEFAULT);
    PROFILES.iter().copied().find(|p| *p == named).unwrap_or(DEFAULT)
}

/// The toolchain installed from the checkout.
struct Installed {
    version: Version,
    profile: &'static str,
}

impl Record for Installed {
    fn kind(&self) -> &'static str {
        "installed"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.version.to_string())
            .append(Text::of(Style::Dim, format!(", built {}, the default", self.profile)))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("version", self.version.to_string()).with("profile", self.profile)
    }
}

impl Status for Installed {
    fn verb(&self) -> Verb {
        Verb::good("Installed")
    }
}
