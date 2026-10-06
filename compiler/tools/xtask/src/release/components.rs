//! The components of a release (spec 12, section 6): each library a component crate builds, packed
//! as an archive of its own, named as the component calls itself and placed in the smallest
//! profile its crate's `[package.metadata.vun-component]` names.

use std::io;
use std::path::{Path, PathBuf};

use vun_build::release::{Build, Piece, Profile};
use vun_build::toolchains::{COMPONENTS, NAME, Toolchains};
use vun_driver::Components;
use vun_fetch::Format;
use vun_fs::Edit;
use vun_target::Machine;

use crate::cargo;

/// A component crate: the library it builds and the profile it is in.
pub(crate) struct Crate {
    package: String,
    library: String,
    profile: Profile,
    needs: Vec<String>,
}

impl Crate {
    /// The component crates of the workspace at `root`: those declaring `package.metadata.vun-component`.
    ///
    /// # Errors
    /// Returns why cargo reports no metadata, or a crate names no profile.
    pub(crate) fn all(root: &Path) -> io::Result<Vec<Self>> {
        let metadata = cargo::metadata(root).map_err(io::Error::other)?;
        let metadata = vun_data::json::parse(&metadata).map_err(io::Error::other)?;
        let mut crates = Vec::new();
        for package in metadata["packages"].items() {
            let Some(name) = package["name"].as_str() else { continue };
            let declared = &package["metadata"]["vun-component"];
            if declared.is_null() {
                continue;
            }
            let profile = declared["profile"].as_str();
            let profile = profile.and_then(Profile::named).ok_or_else(|| {
                io::Error::other(format!("{name} names no profile in its `package.metadata.vun-component`"))
            })?;
            let needs = declared["needs"].items().filter_map(|n| n.as_str().map(str::to_owned)).collect();
            crates.push(Self {
                package: name.to_owned(),
                library: Components::library(name),
                profile,
                needs,
            });
        }
        Ok(crates)
    }

    /// Where `built` the executable of a build, the libraries of the same build are: beside it.
    pub(crate) fn beside(&self, binary: &Path) -> PathBuf {
        binary.parent().unwrap_or(Path::new("")).join(&self.library)
    }

    /// Builds the libraries of `crates` in `profile`.
    ///
    /// # Errors
    /// Returns why cargo cannot run or fails.
    pub(crate) fn build(
        root: &Path,
        crates: &[Self],
        profile: &str,
        tool: &str,
        target: Option<&str>,
    ) -> Result<(), cargo::CargoFailed> {
        let packages: Vec<String> = crates.iter().map(|c| c.package.clone()).collect();
        cargo::components(root, profile, tool, target, &packages)
    }

    /// Packs the library built beside `binary` as the archive of its component for `machine`,
    /// written in `out`.
    ///
    /// # Errors
    /// Returns why the library is not there, is not a component, or the archive is not written.
    pub(crate) fn pack(&self, binary: &Path, out: &Path, machine: Machine) -> io::Result<Piece> {
        let library = self.beside(binary);
        let name = Components::named(&library).map_err(io::Error::other)?;
        let name = name.as_str();
        let stage = out.join(format!(".stage-{name}"));
        stage.fresh()?;
        stage.join(COMPONENTS).join(&self.library).put(std::fs::read(&library)?)?;
        let format = Format::of(Build::SUFFIX).expect("component archives are of a known format");
        let (bytes, snapshot) = format.pack(&stage, &[COMPONENTS.to_owned()], NAME)?;
        let hashed = Toolchains::hash_of(&stage, &[COMPONENTS])?;
        stage.remove()?;
        if hashed != snapshot.root() {
            return Err(io::Error::other(format!(
                "the staged component {name} hashes apart from its snapshot"
            )));
        }
        let archive = Piece::archive_name(name, machine);
        out.join(&archive).put(bytes)?;
        Ok(Piece {
            name: name.to_owned(),
            archive,
            file: self.library.clone(),
            hash: snapshot.root(),
            profile: self.profile,
            needs: self.needs.clone(),
        })
    }
}
