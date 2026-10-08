//! Why a toolchain is not installed.

use std::error::Error;
use std::{fmt, io};

use vun_hash::Hash;
use vun_target::Machine;
use vun_uri::Uri;

/// A fetch that failed, and where it was from.
#[derive(Debug)]
pub struct Fetched {
    pub url: Uri,
    pub why: vun_fetch::Failure,
}

impl Fetched {
    /// The failure of the fetch of `url`, for `map_err`.
    pub fn of(url: &Uri) -> impl FnOnce(vun_fetch::Failure) -> Box<Self> + '_ {
        move |why| Box::new(Self { url: url.clone(), why })
    }
}

impl Uninstalled {
    /// Whether the releases were being read, where `VUN_RELEASES` names another place.
    #[must_use]
    pub fn from_releases(&self) -> bool {
        matches!(self, Self::Manifest(_) | Self::Unsigned(_) | Self::Archive(_))
    }
}

/// Why a toolchain could not be installed.
#[derive(Debug)]
pub enum Uninstalled {
    /// The user has no home to install toolchains in.
    Homeless,
    /// Where releases are found, or their key, does not read.
    Repository(vun_build::release::Misnamed),
    /// The release's manifest could not be read from this address.
    Manifest(Box<Fetched>),
    /// The manifest's signature could not be read from this address.
    Unsigned(Box<Fetched>),
    /// The manifest is not the one the releases' key signed.
    Forged(vun_build::release::Forged),
    /// The manifest is not the one the project's lockfile records.
    Unlocked {
        expected: Hash,
        found: Hash,
    },
    /// The manifest is not JSON.
    Json(vun_data::json::Error),
    /// The manifest is not a release.
    Shape(vun_data::Mismatch),
    /// The release has no toolchain for the running machine.
    Unbuilt(Machine),
    /// The archive could not be fetched or unpacked from this address.
    Archive(Box<Fetched>),
    /// The tree it unpacked to does not hash to the manifest's hash.
    Changed {
        expected: Hash,
        found: Hash,
    },
    /// The release builds no component of this name for the running machine.
    Uncomponented(String),
    Io(io::Error),
}

impl fmt::Display for Uninstalled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Homeless => f.write_str("the user has no home directory to install it in"),
            Self::Repository(_) => f.write_str("where releases are found does not read"),
            Self::Manifest(fetched) => {
                write!(f, "the release's manifest cannot be read from `{}`", fetched.url)
            }
            Self::Unsigned(fetched) => {
                write!(f, "the release's signature cannot be read from `{}`", fetched.url)
            }
            Self::Forged(_) => f.write_str("the release's manifest is not the one its releases signed"),
            Self::Unlocked { expected, found } => {
                write!(
                    f,
                    "the release's manifest hashes to `{found}`, where the lockfile records `{expected}`"
                )
            }
            Self::Json(_) | Self::Shape(_) => f.write_str("the release's manifest is not one"),
            Self::Unbuilt(Machine { arch, os }) => {
                write!(f, "the release has no toolchain for `{arch}` on `{os}`")
            }
            Self::Archive(fetched) => write!(f, "its archive cannot be fetched from `{}`", fetched.url),
            Self::Changed { expected, found } => {
                write!(f, "its archive unpacks to `{found}`, where the release names `{expected}`")
            }
            Self::Uncomponented(name) => {
                write!(f, "the release builds no component `{name}` for this machine")
            }
            Self::Io(_) => f.write_str("the toolchains' directory cannot be written"),
        }
    }
}

impl Error for Uninstalled {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Repository(e) => Some(e),
            Self::Manifest(e) | Self::Unsigned(e) | Self::Archive(e) => Some(&e.why),
            Self::Forged(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Shape(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Homeless
            | Self::Unbuilt(_)
            | Self::Changed { .. }
            | Self::Unlocked { .. }
            | Self::Uncomponented(_) => None,
        }
    }
}
