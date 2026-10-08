//! Installing components (spec 12, section 6): the libraries of a release's toolchain, each its
//! own archive, fetched, checked against the manifest's hash and put in the toolchain's
//! `components` directory one by one.

use std::fs;

use vun_build::release::{Build, Piece, Profile, Release, Releases};
use vun_build::toolchains::{COMPONENTS, Toolchains};
use vun_fetch::Mirror;
use vun_fs::Edit;
use vun_target::Machine;
use vun_trace::tracing;

use super::{Fetched, Installed, Uninstalled};
use crate::Session;

impl Session {
    /// The components `profile` holds in `release` for the running machine.
    ///
    /// # Errors
    /// Returns [`Uninstalled::Unbuilt`] when the release has no build for the machine.
    pub fn profile_components(release: &Release, profile: Profile) -> Result<Vec<String>, Uninstalled> {
        let build = release.build_for(Machine::RUNNING).ok_or(Uninstalled::Unbuilt(Machine::RUNNING))?;
        Ok(build.in_profile(profile).map(|piece| piece.name.clone()).collect())
    }

    /// Installs the components `names` of `release`, and what they need, for the running machine into the installed
    /// toolchain of its version, each unless it is there whole.
    ///
    /// # Errors
    /// Returns why one cannot be; those before it are installed.
    pub fn install_components(
        &self,
        toolchains: &Toolchains,
        releases: &Releases,
        release: &Release,
        names: &[String],
    ) -> Result<Vec<(String, Installed)>, Uninstalled> {
        let build = release.build_for(Machine::RUNNING).ok_or(Uninstalled::Unbuilt(Machine::RUNNING))?;
        let mut installed = Vec::new();
        let pieces = build.with_needs(names).map_err(|name| Uninstalled::Uncomponented(name.to_owned()))?;
        for piece in pieces {
            installed.push((piece.name.clone(), self.install_piece(toolchains, releases, release, piece)?));
        }
        Ok(installed)
    }

    fn install_piece(
        &self,
        toolchains: &Toolchains,
        releases: &Releases,
        release: &Release,
        piece: &Piece,
    ) -> Result<Installed, Uninstalled> {
        let version = release.version;
        if toolchains.component_hash(version, piece).is_ok_and(|hash| hash == piece.hash) {
            return Ok(Installed::Already);
        }
        let url = releases.component(release, piece);
        tracing::info!(%url, component = %piece.name, "fetching a component");
        let staging = toolchains.staging(version);
        Mirror::of(&url, &self.registry.schemes())
            .and_then(|m| m.fetch(&staging))
            .map_err(Fetched::of(&url))
            .map_err(Uninstalled::Archive)?;
        let found = Toolchains::hash_of(&staging, &[COMPONENTS]).map_err(Uninstalled::Io)?;
        if found != piece.hash {
            staging.remove().map_err(Uninstalled::Io)?;
            return Err(Uninstalled::Changed { expected: piece.hash, found });
        }
        let library = fs::read(staging.join(COMPONENTS).join(&piece.file)).map_err(Uninstalled::Io)?;
        toolchains.components(version).join(&piece.file).put(library).map_err(Uninstalled::Io)?;
        staging.remove().map_err(Uninstalled::Io)?;
        Ok(Installed::Fresh)
    }
}

/// What a toolchain has of the components its release builds for the running machine.
pub struct Held<'a> {
    pub piece: &'a Piece,
    /// Whether its library is installed, whole.
    pub installed: bool,
}

impl Held<'_> {
    /// What of `build`'s components the toolchain of `version` has.
    #[must_use]
    pub fn of<'a>(toolchains: &Toolchains, version: vun_span::Version, build: &'a Build) -> Vec<Held<'a>> {
        let whole = |piece: &Piece| toolchains.component_hash(version, piece).is_ok_and(|h| h == piece.hash);
        build.components.iter().map(|piece| Held { piece, installed: whole(piece) }).collect()
    }
}
