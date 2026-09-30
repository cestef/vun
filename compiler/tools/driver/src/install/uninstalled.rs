//! Why a toolchain is not installed.

use std::error::Error;
use std::{fmt, io};

use vun_hash::Hash;
use vun_target::Machine;

/// Why a toolchain could not be installed.
#[derive(Debug)]
pub enum Uninstalled {
    /// The user has no home to install toolchains in.
    Homeless,
    /// The base releases are found under is not a URI.
    Repository(vun_uri::Invalid),
    /// The release's manifest could not be read.
    Manifest(vun_fetch::Failure),
    /// The manifest is not JSON.
    Json(vun_data::json::Error),
    /// The manifest is not a release.
    Shape(vun_data::Mismatch),
    /// The release has no toolchain for the running machine.
    Unbuilt(Machine),
    /// The archive could not be fetched or unpacked.
    Archive(vun_fetch::Failure),
    /// The tree it unpacked to does not hash to the manifest's hash.
    Changed {
        expected: Hash,
        found: Hash,
    },
    Io(io::Error),
}

impl fmt::Display for Uninstalled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Homeless => f.write_str("the user has no home directory to install it in"),
            Self::Repository(_) => f.write_str("where releases are found is not a URI"),
            Self::Manifest(_) => f.write_str("the release's manifest cannot be read"),
            Self::Json(_) | Self::Shape(_) => f.write_str("the release's manifest is not one"),
            Self::Unbuilt(Machine { arch, os }) => {
                write!(f, "the release has no toolchain for `{arch}` on `{os}`")
            }
            Self::Archive(_) => f.write_str("its archive cannot be fetched or unpacked"),
            Self::Changed { expected, found } => {
                write!(f, "its archive unpacks to `{found}`, where the release names `{expected}`")
            }
            Self::Io(_) => f.write_str("the toolchains' directory cannot be written"),
        }
    }
}

impl Error for Uninstalled {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Repository(e) => Some(e),
            Self::Manifest(e) | Self::Archive(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Shape(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Homeless | Self::Unbuilt(_) | Self::Changed { .. } => None,
        }
    }
}
