//! A release of the toolchain (spec 12, section 6): the manifest each release of the repository
//! carries, naming for each machine the archive of its toolchain and the hash of the tree the
//! archive unpacks to, which installing checks before it keeps anything. A release is tagged with
//! its version as it is (`0.4.2`), so that its files are found from the version alone. The manifest
//! is signed ([`Key`]), so that it and every archive it names are the ones the project released.

mod key;

use std::error::Error;
use std::fmt;

use vun_data::{FromData, ToData};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::Machine;
use vun_uri::{Invalid, Uri};

pub use key::{Forged, Key};

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

/// A release's manifest as it was published: read, and its bytes, which the signature signs and
/// a lockfile pinning the release records the hash of.
#[derive(Clone, Debug)]
pub struct Published {
    pub release: Release,
    pub manifest: Vec<u8>,
}

impl Published {
    /// The hash of the manifest's bytes, which names every machine's toolchain through theirs.
    #[must_use]
    pub fn hash(&self) -> Hash {
        Hash::of(&self.manifest)
    }
}

/// Which release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    Latest,
    Version(Version),
}

/// Where releases are found: the repository the workspace declares, or the base
/// [`Releases::ENV`] names (a mirror, or a directory as a `file:` URI), laid out as a forge lays
/// them out; their manifests signed with [`Key::releases`], or the key [`Key::ENV`] names.
pub struct Releases {
    base: Uri,
    pub key: Key,
}

/// Why the releases named are not found.
#[derive(Debug)]
pub enum Misnamed {
    /// The base named, or the repository, is not a URI.
    Base(Invalid),
    /// The key named is not an Ed25519 key.
    Key,
}

impl Releases {
    /// Names the base releases are found under instead of the repository.
    pub const ENV: &str = "VUN_RELEASES";

    /// The releases under `base`, signed with `key`.
    #[must_use]
    pub fn at(base: Uri, key: Key) -> Self {
        Self { base, key }
    }

    /// The user's.
    ///
    /// # Errors
    /// Returns why the base or the key named do not read.
    pub fn user() -> Result<Self, Misnamed> {
        let named = std::env::var(Self::ENV).ok();
        let base = Uri::parse(named.unwrap_or_else(|| REPOSITORY.to_owned())).map_err(Misnamed::Base)?;
        let key = match std::env::var(Key::ENV) {
            Ok(text) => Key::parse(&text).ok_or(Misnamed::Key)?,
            Err(_) => Key::releases(),
        };
        Ok(Self { base, key })
    }

    /// Where the manifest of the release `wanted` is.
    #[must_use]
    pub fn manifest(&self, wanted: Wanted) -> Uri {
        self.of(wanted, Release::FILE)
    }

    /// Where the signature of the manifest of the release `wanted` is.
    #[must_use]
    pub fn signature(&self, wanted: Wanted) -> Uri {
        self.of(wanted, Release::SIGNATURE)
    }

    fn of(&self, wanted: Wanted, name: &str) -> Uri {
        match wanted {
            Wanted::Latest => self.base.with_segments(LATEST.iter().copied().chain([name])),
            Wanted::Version(version) => self.file(version, name),
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
    /// The name of the manifest's signature: the raw Ed25519 signature of its bytes.
    pub const SIGNATURE: &str = "release.json.sig";

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

impl fmt::Display for Misnamed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(_) => write!(f, "where releases are found is not a URI"),
            Self::Key => write!(f, "`{}` is not an Ed25519 key, `ed25519:<base32>`", Key::ENV),
        }
    }
}

impl Error for Misnamed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Base(e) => Some(e),
            Self::Key => None,
        }
    }
}

#[cfg(test)]
mod tests;
