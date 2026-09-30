//! Pinned toolchains (spec 12, section 6): a project's `build.vun` names the toolchain that builds
//! it (`.toolchain`), and any `vun` loading the project installs that one when missing and hands
//! the command to it, so no launcher stands apart from the toolchains.

use std::path::Path;
use std::process::Command;

use vun_build::release::Wanted;
use vun_build::toolchains::Toolchains;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_span::Version;
use vun_trace::tracing;

use super::Uninstalled;
use crate::{Session, VERSION};

/// The variable that, set to [`LOCAL`], keeps the running toolchain whatever a project pins.
pub const ENV: &str = "VUN_TOOLCHAIN";
pub const LOCAL: &str = "local";

impl Session {
    /// Hands the command over to the toolchain the project around `path` pins, installed first
    /// when missing, unless it pins none, pins this one, or [`ENV`] is [`LOCAL`]. Returns when this
    /// toolchain is the one to run.
    ///
    /// # Errors
    /// Returns once why the pinned toolchain cannot run is reported.
    pub fn hand_off(&mut self, path: &Path) -> Result<(), ErrorGuaranteed> {
        if std::env::var(ENV).is_ok_and(|v| v == LOCAL) {
            return Ok(());
        }
        let Some((_, key)) = path.parent().and_then(|dir| self.package_around(dir)) else { return Ok(()) };
        let Some(pinned) = self.packages.list[&key].toolchain else { return Ok(()) };
        if Version::parse(VERSION) == Some(pinned) {
            return Ok(());
        }
        let Some(toolchains) = Toolchains::user() else {
            return Err(self.uninstalled(Some(pinned), &Uninstalled::Homeless));
        };
        if !toolchains.installed().is_ok_and(|installed| installed.contains(&pinned)) {
            let release = self
                .toolchain_release(Wanted::Version(pinned))
                .map_err(|why| self.uninstalled(Some(pinned), &why))?;
            self.install(&toolchains, &release).map_err(|why| self.uninstalled(Some(pinned), &why))?;
        }
        tracing::info!(%pinned, "handing the command to the pinned toolchain");
        let mut command = Command::new(toolchains.exe(pinned));
        // It runs as the toolchain it is, whatever the project pins.
        command.args(std::env::args_os().skip(1)).env(ENV, LOCAL);
        let failed = Self::replace_with(&mut command);
        let msg = Msg::new().text("cannot run the pinned toolchain ").code(pinned.to_string());
        Err(self.diag.error(Diagnostic::error(errors::TOOLCHAIN_INSTALL_FAILED, msg).caused_by(&failed)))
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
