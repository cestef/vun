//! Pinned toolchains (spec 12, section 6): a project's `build.vun` names the toolchain that builds
//! it (`.toolchain`), and any `vun` loading the project installs that one when missing and hands
//! the command to it, so no launcher stands apart from the toolchains. The project's lockfile
//! records the hash of the release's manifest, and a toolchain installed from another manifest is
//! not handed to.

use std::path::Path;
use std::process::Command;

use vun_build::release::{Releases, Wanted};
use vun_build::toolchains::Toolchains;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_span::Version;
use vun_target::Machine;
use vun_trace::tracing;

use super::{Held, Uninstalled};
use crate::{Session, VERSION};

/// The variable that, set to [`LOCAL`], keeps the running toolchain whatever a project pins.
pub const ENV: &str = "VUN_TOOLCHAIN";
pub const LOCAL: &str = "local";

impl Session {
    /// Hands the command over to the toolchain the project around `path` pins, installed first
    /// when missing or installed from another manifest than the lockfile records, unless it pins
    /// none, pins this one, or [`ENV`] is [`LOCAL`]. Returns when this toolchain is the one to run.
    ///
    /// # Errors
    /// Returns once why the pinned toolchain cannot run is reported.
    pub fn hand_off(&mut self, path: &Path) -> Result<(), ErrorGuaranteed> {
        if std::env::var(ENV).is_ok_and(|v| v == LOCAL) {
            return Ok(());
        }
        let Some((dir, key)) = path.parent().and_then(|dir| self.package_around(dir)) else { return Ok(()) };
        // A workspace's root pins the toolchain of its members.
        let (_, key) = self.project_of(&dir, key);
        let Some(pinned) = self.packages.list[&key].toolchain else { return Ok(()) };
        if VERSION.parse::<Version>().ok() == Some(pinned) {
            return Ok(());
        }
        let Some(toolchains) = Toolchains::user() else {
            return Err(self.uninstalled(Some(pinned), &Uninstalled::Homeless));
        };
        let locked = self.recorded_lock(key).and_then(|l| l.toolchain()).filter(|t| t.version == pinned);
        let installed = toolchains.installed().is_ok_and(|installed| installed.contains(&pinned));
        let components = self.packages.list[&key].components.clone();
        let stale = locked.is_some_and(|t| toolchains.manifest(pinned) != Some(t.hash));
        if !installed || stale || !Self::holds(&toolchains, pinned, &components) {
            let published = self
                .toolchain_release(Wanted::Version(pinned))
                .map_err(|why| self.uninstalled(Some(pinned), &why))?;
            if let Some(locked) = locked
                && published.hash() != locked.hash
            {
                let why = Uninstalled::Unlocked { expected: locked.hash, found: published.hash() };
                return Err(self.uninstalled(Some(pinned), &why));
            }
            if !installed || stale {
                self.install(&toolchains, &published).map_err(|why| self.uninstalled(Some(pinned), &why))?;
            }
            let releases = Releases::user()
                .map_err(|why| self.uninstalled(Some(pinned), &Uninstalled::Repository(why)))?;
            self.install_components(&toolchains, &releases, &published.release, &components)
                .map_err(|why| self.uninstalled(Some(pinned), &why))?;
        }
        tracing::info!(%pinned, "handing the command to the pinned toolchain");
        let mut command = Command::new(toolchains.exe(pinned));
        // It runs as the toolchain it is, whatever the project pins.
        command.args(std::env::args_os().skip(1)).env(ENV, LOCAL);
        let failed = Self::replace_with(&mut command);
        let msg = Msg::new().text("cannot run the pinned toolchain ").code(pinned.to_string());
        Err(self.diag.error(Diagnostic::error(errors::TOOLCHAIN_INSTALL_FAILED, msg).caused_by(&failed)))
    }

    /// Whether the installed toolchain of `version` holds the components `names`, whole, as its
    /// kept manifest names them.
    fn holds(toolchains: &Toolchains, version: Version, names: &[String]) -> bool {
        let Some(release) = toolchains.release(version) else { return names.is_empty() };
        let Some(build) = release.build_for(Machine::RUNNING) else { return names.is_empty() };
        let held = Held::of(toolchains, version, build);
        names.iter().all(|name| held.iter().any(|h| h.piece.name == *name && h.installed))
    }

    /// Replaces this process with `command`, returning only why it could not.
    #[cfg(unix)]
    fn replace_with(command: &mut Command) -> std::io::Error {
        use std::os::unix::process::CommandExt;
        command.exec()
    }

    /// Runs `command` and exits with its status, returning only why it could not run.
    #[cfg(not(unix))]
    fn replace_with(command: &mut Command) -> std::io::Error {
        match command.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(e) => e,
        }
    }
}
