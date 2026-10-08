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

use crate::cargo::{self, Flavor};
use crate::pgo::{Pgo, PgoFailed};
use crate::release::Crate;
use crate::toolchain::Stage;

/// The cargo profile `vun` and its components are built with.
pub(crate) const PROFILE: Param = Param::option("profile")
    .about("the cargo profile to build with, `dist` when none (`dev` for the tests)")
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
    let flavor = match Optimized::for_profile(&root, profile, shell) {
        Ok(flavor) => flavor,
        Err(why) => {
            diag.error(why.diagnostic());
            return Ok(Outcome::Failure);
        }
    };
    let binary = match cargo::vun_as(&root, profile, &flavor) {
        Ok(binary) => binary,
        Err(why) => {
            let msg = Msg::new().text("cannot build ").code("vun").text(" from the checkout");
            diag.error(Diagnostic::error(UNBUILT, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let crates = match crates(&root, profile) {
        Ok(crates) => crates,
        Err(why) => {
            let msg = Msg::new().text("cannot build the components of ").code("vun");
            diag.error(Diagnostic::error(UNBUILT, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let staging = Stage::at(&root, toolchains.staging(version));
    let libraries: Vec<PathBuf> = crates.iter().map(|c| c.beside(&binary)).collect();
    let laid = staging
        .lay(&binary, &libraries)
        .and_then(|()| staging.prime(&binary))
        .and_then(|()| staging.keep_release(version, &binary, &crates));
    if let Err(why) = laid {
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

/// The component crates whose libraries `vun` loads, built beside it: of the distributed profile
/// only, the others link them in.
fn crates(root: &Path, profile: &str) -> io::Result<Vec<Crate>> {
    if profile != cargo::DISTRIBUTED {
        return Ok(Vec::new());
    }
    let crates = Crate::all(root)?;
    Crate::build(root, &crates, profile, "build", None).map_err(io::Error::other)?;
    Ok(crates)
}

/// How the installed `vun` is laid out.
struct Optimized;

impl Optimized {
    /// By the profile of its own workloads when it is distributed and `llvm-tools` is installed
    /// (decision 670), as built when not; without, a note says how to.
    fn for_profile(root: &Path, profile: &str, shell: &mut Shell<'_>) -> Result<Flavor, PgoFailed> {
        if profile != cargo::DISTRIBUTED {
            return Ok(Flavor::default());
        }
        match Pgo::of(root).make() {
            Ok(made) => Ok(Flavor { rustflags: Pgo::flags(&made), ..Flavor::default() }),
            Err(PgoFailed::NoMerger) => {
                shell.status(&Unoptimized);
                Ok(Flavor::default())
            }
            Err(why) => Err(why),
        }
    }
}

/// The distributed `vun` built without its profile.
struct Unoptimized;

impl Record for Unoptimized {
    fn kind(&self) -> &'static str {
        "unoptimized"
    }

    fn text(&self) -> Text {
        Text::of(Style::Dim, "without its profile: `rustup component add llvm-tools` makes one")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
    }
}

impl Status for Unoptimized {
    fn verb(&self) -> Verb {
        Verb::warned("Building")
    }
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
pub(crate) fn profile(args: &Args) -> &'static str {
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
