//! The toolchains installed side by side (spec 12, section 6): a directory per version in the
//! user's data directory, each a toolchain's tree (decision 364), and the default one, which the
//! user's `vun` links to.

use std::io;
use std::path::{Path, PathBuf};

use vun_fs::{Edit, Tree};
use vun_hash::{Hash, Snapshot};
use vun_span::Version;

/// The toolchain's name: its executable's, its directory's in the user's, its archives'.
pub const NAME: &str = "vun";
/// What a toolchain's tree holds, as snapshots name it: what a release archive holds and the
/// release's hash covers.
pub const CONTENTS: [&str; 3] = [BIN, "lib", "LICENSE.md"];
/// The directory of a toolchain's executable.
pub const BIN: &str = "bin";

/// The toolchains of a user.
pub struct Toolchains {
    root: PathBuf,
    /// The user's `vun`, a link to the default toolchain's.
    link: PathBuf,
}

impl Toolchains {
    /// The directory of the toolchains in the user's `vun` data directory.
    pub const DIR: &str = "toolchains";
    /// The user's directory of executables, below their home, where the default `vun` is linked.
    pub const BINARIES: [&str; 2] = [".local", "bin"];

    /// The toolchains in `root`, the default linked at `link`.
    #[must_use]
    pub fn at(root: &Path, link: &Path) -> Self {
        Self { root: root.to_path_buf(), link: link.to_path_buf() }
    }

    /// The user's: `vun/toolchains` in the data directory (XDG on Unix, including macOS), the
    /// default linked as `~/.local/bin/vun`; `None` for a user without a home.
    #[must_use]
    pub fn user() -> Option<Self> {
        use etcetera::BaseStrategy;
        let base = etcetera::choose_base_strategy().ok()?;
        let link = Self::BINARIES.iter().fold(base.home_dir().to_path_buf(), |dir, d| dir.join(d));
        Some(Self::at(&base.data_dir().join(NAME).join(Self::DIR), &link.join(Self::executable())))
    }

    /// The directory of the toolchain of `version`.
    #[must_use]
    pub fn dir(&self, version: Version) -> PathBuf {
        self.root.join(version.to_string())
    }

    /// The executable of the toolchain of `version`.
    #[must_use]
    pub fn exe(&self, version: Version) -> PathBuf {
        self.dir(version).join(BIN).join(Self::executable())
    }

    /// Where the default toolchain's executable is linked.
    #[must_use]
    pub fn link(&self) -> &Path {
        &self.link
    }

    /// The versions installed, oldest first.
    ///
    /// # Errors
    /// Returns an error if the directory exists and cannot be read.
    pub fn installed(&self) -> io::Result<Vec<Version>> {
        let entries = match self.root.entries() {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            entries => entries?,
        };
        let mut versions: Vec<Version> = entries
            .iter()
            .filter(|dir| dir.join(BIN).is_dir())
            .filter_map(|dir| Version::parse(dir.file_name()?.to_str()?))
            .collect();
        versions.sort_unstable();
        Ok(versions)
    }

    /// The version the user's `vun` links to, if it links to one of these.
    #[must_use]
    pub fn default(&self) -> Option<Version> {
        let target = std::fs::read_link(&self.link).ok()?;
        let dir = target.strip_prefix(&self.root).ok()?.components().next()?;
        Version::parse(dir.as_os_str().to_str()?)
    }

    /// Links the user's `vun` to the toolchain of `version`.
    ///
    /// # Errors
    /// Returns an error if the link cannot be written, or the system has no symbolic links.
    pub fn make_default(&self, version: Version) -> io::Result<()> {
        self.link.link_to(&self.exe(version))
    }

    /// Moves the toolchain tree at `from` in as the one of `version`, unless one is there.
    ///
    /// # Errors
    /// Returns an error if it can be neither moved in nor removed.
    pub fn install(&self, version: Version, from: &Path) -> io::Result<()> {
        self.dir(version).install(from)
    }

    /// Removes the toolchain of `version`.
    ///
    /// # Errors
    /// Returns an error if something of it cannot be removed.
    pub fn remove(&self, version: Version) -> io::Result<()> {
        self.dir(version).remove()
    }

    /// Where a toolchain of `version` is unpacked before it is checked and moved in.
    #[must_use]
    pub fn staging(&self, version: Version) -> PathBuf {
        use vun_fs::Part;
        self.dir(version).part()
    }

    /// The hash of the toolchain tree in `dir`, as a release names it.
    ///
    /// # Errors
    /// As [`Snapshot::of`].
    pub fn hash(dir: &Path) -> io::Result<Hash> {
        let contents: Vec<String> = CONTENTS.iter().map(|&c| c.to_owned()).collect();
        Snapshot::of(dir, &contents).map(|s| s.root())
    }

    /// The file name of a toolchain's executable on this system.
    fn executable() -> String {
        [NAME, std::env::consts::EXE_SUFFIX].concat()
    }
}

#[cfg(test)]
mod tests;
