//! Installing toolchains (spec 12, section 6): a release's manifest read through the session's
//! schemes and its signature checked, the archive of the running machine's toolchain fetched and
//! unpacked beside the installed ones, its tree hashed and checked against the manifest's hash,
//! then moved in with the manifest, which a lockfile pinning the release records the hash of.

mod pin;
mod uninstalled;

use vun_build::release::{Published, Release, Releases, Wanted};
use vun_build::toolchains::Toolchains;
use vun_data::FromData;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fetch::Mirror;
use vun_fs::Edit;
use vun_span::Version;
use vun_target::Machine;
use vun_trace::tracing;

pub use pin::{ENV, LOCAL};
pub use uninstalled::Uninstalled;

use crate::Session;

/// What installing did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Installed {
    /// It fetched and moved the toolchain in.
    Fresh,
    /// The toolchain was there already, whole.
    Already,
}

impl Session {
    /// The manifest of the release `wanted`, once its signature is checked.
    ///
    /// # Errors
    /// Returns why it cannot be read, or is not the one the releases' key signed.
    pub fn toolchain_release(&self, wanted: Wanted) -> Result<Published, Uninstalled> {
        let releases = Releases::user().map_err(Uninstalled::Repository)?;
        let url = releases.manifest(wanted);
        tracing::debug!(%url, key = %releases.key, "release manifest");
        let manifest = self.schemes.get(&url).map_err(Uninstalled::Manifest)?;
        let signature = self.schemes.get(&releases.signature(wanted)).map_err(Uninstalled::Unsigned)?;
        releases.key.verify(&manifest, &signature).map_err(Uninstalled::Forged)?;
        let text = String::from_utf8_lossy(&manifest);
        let value = vun_data::json::parse(&text).map_err(Uninstalled::Json)?;
        let release = Release::from_data(&value).map_err(Uninstalled::Shape)?;
        Ok(Published { release, manifest })
    }

    /// Installs the running machine's toolchain of `published` among `toolchains`, unless it is
    /// there whole, keeping its manifest.
    ///
    /// # Errors
    /// Returns why it cannot be; nothing is kept then.
    pub fn install(&self, toolchains: &Toolchains, published: &Published) -> Result<Installed, Uninstalled> {
        let release = &published.release;
        let build = release.build_for(Machine::RUNNING).ok_or(Uninstalled::Unbuilt(Machine::RUNNING))?;
        let dir = toolchains.dir(release.version);
        if dir.exists() {
            if Toolchains::hash(&dir).is_ok_and(|hash| hash == build.hash) {
                dir.join(Release::FILE).put(&published.manifest).map_err(Uninstalled::Io)?;
                return Ok(Installed::Already);
            }
            // Changed since it was installed: installed again.
            toolchains.remove(release.version).map_err(Uninstalled::Io)?;
        }
        let url = Releases::user().map_err(Uninstalled::Repository)?.archive(release, build);
        tracing::info!(%url, version = %release.version, "fetching a toolchain");
        let staging = toolchains.staging(release.version);
        Mirror::of(&url, &self.schemes).and_then(|m| m.fetch(&staging)).map_err(Uninstalled::Archive)?;
        let found = Toolchains::hash(&staging).map_err(Uninstalled::Io)?;
        if found != build.hash {
            staging.remove().map_err(Uninstalled::Io)?;
            return Err(Uninstalled::Changed { expected: build.hash, found });
        }
        staging.join(Release::FILE).put(&published.manifest).map_err(Uninstalled::Io)?;
        toolchains.install(release.version, &staging).map_err(Uninstalled::Io)?;
        Ok(Installed::Fresh)
    }

    /// Reports that the toolchain of `version`, or the latest, cannot be installed.
    pub fn uninstalled(&mut self, version: Option<Version>, why: &Uninstalled) -> ErrorGuaranteed {
        let msg = Msg::new().text("cannot install the toolchain");
        let msg = match version {
            Some(version) => msg.text(" ").code(version.to_string()),
            None => msg.text(" of the latest release"),
        };
        self.diag.error(Diagnostic::error(errors::TOOLCHAIN_INSTALL_FAILED, msg).caused_by(why))
    }
}
