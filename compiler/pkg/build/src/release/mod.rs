//! A release of the toolchain (spec 12, section 6): the manifest each release of the repository
//! carries, naming for each machine the archive of its toolchain and the hash of the tree the
//! archive unpacks to, which installing checks before it keeps anything. A release is tagged with
//! its version as it is (`0.4.2`), so that its files are found from the version alone.

use vun_data::{FromData, ToData};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::Machine;
use vun_uri::{Invalid, Uri};

/// The repository whose releases hold the toolchain's, as the workspace declares it.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The path of a forge's releases in a repository, and of their files in one.
const RELEASES: [&str; 2] = ["releases", "download"];
/// The path of the files of the latest release, which forges redirect to it.
const LATEST: [&str; 3] = ["releases", "latest", "download"];

/// The manifest of a release.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Release {
    pub version: Version,
    pub builds: Vec<Build>,
}

/// The toolchain a release has for one machine.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Build {
    pub machine: Machine,
    /// The archive's file name among the release's.
    pub archive: String,
    /// The hash of the toolchain's tree ([`crate::toolchains::CONTENTS`]).
    pub hash: Hash,
}

/// Which release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    Latest,
    Version(Version),
}

/// Where releases are found: the repository the workspace declares, or the base
/// [`Releases::ENV`] names (a mirror, or a directory as a `file:` URI), laid out as a forge lays
/// them out.
pub struct Releases {
    base: Uri,
}

impl Releases {
    /// Names the base releases are found under instead of the repository.
    pub const ENV: &str = "VUN_RELEASES";

    /// The releases under `base`.
    #[must_use]
    pub fn at(base: Uri) -> Self {
        Self { base }
    }

    /// The user's.
    ///
    /// # Errors
    /// Returns why the base named, or the repository, is not a URI.
    pub fn user() -> Result<Self, Invalid> {
        let named = std::env::var(Self::ENV).ok();
        Ok(Self { base: Uri::parse(named.unwrap_or_else(|| REPOSITORY.to_owned()))? })
    }

    /// Where the manifest of the release `wanted` is.
    #[must_use]
    pub fn manifest(&self, wanted: Wanted) -> Uri {
        match wanted {
            Wanted::Latest => self.base.with_segments(LATEST.iter().chain(&[Release::FILE])),
            Wanted::Version(version) => self.file(version, Release::FILE),
        }
    }

    /// Where the archive of `build` of `release` is.
    #[must_use]
    pub fn archive(&self, release: &Release, build: &Build) -> Uri {
        self.file(release.version, &build.archive)
    }

    fn file(&self, version: Version, name: &str) -> Uri {
        let tag = version.to_string();
        self.base.with_segments(RELEASES.iter().copied().chain([tag.as_str(), name]))
    }
}

impl Release {
    /// The name of the manifest among a release's files.
    pub const FILE: &str = "release.json";

    /// The build for `machine`.
    #[must_use]
    pub fn build_for(&self, machine: Machine) -> Option<&Build> {
        self.builds.iter().find(|b| b.machine == machine)
    }
}

impl Build {
    /// The suffix of toolchain archives: gzip-compressed tar, which every system unpacks
    /// without `vun`, as a first installation does.
    pub const SUFFIX: &str = ".tar.gz";

    /// The file name of the archive of `machine`'s toolchain: the same in every release, so that
    /// the latest one is found without its version (`vun-x86_64-linux.tar.gz`).
    #[must_use]
    pub fn archive_name(machine: Machine) -> String {
        [crate::toolchains::NAME, "-", machine.arch.0, "-", machine.os.0, Self::SUFFIX].concat()
    }
}

#[cfg(test)]
mod tests;
