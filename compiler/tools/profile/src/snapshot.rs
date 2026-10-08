//! Entries kept in a file between runs, which a later run is compared with.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

use vun_data::json::{self, Json};
use vun_data::{FromData, Mismatch, ToData};
use vun_fs::{Edit, WORK};

use crate::entry::{Entry, Key};

/// The directory of snapshots in the working directory.
const DIRECTORY: &str = "profile";
const EXTENSION: &str = "json";

/// The entries of a measurement.
#[derive(Clone, Debug, Default, PartialEq, ToData, FromData)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
}

#[derive(Debug)]
pub enum SnapshotError {
    /// The name is no file name.
    Name(String),
    File(PathBuf, io::Error),
    Json(PathBuf, json::Error),
    Shape(PathBuf, Mismatch),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => write!(f, "`{name}` is no snapshot name: write a name that is a file name"),
            Self::File(path, _) => write!(f, "the snapshot `{}` is not read or written", path.display()),
            Self::Json(path, _) | Self::Shape(path, _) => {
                write!(f, "the snapshot `{}` is no snapshot", path.display())
            }
        }
    }
}

impl Error for SnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Name(_) => None,
            Self::File(_, source) => Some(source),
            Self::Json(_, source) => Some(source),
            Self::Shape(_, source) => Some(source),
        }
    }
}

impl Snapshot {
    /// Where the snapshot `name` of the repository at `root` is kept.
    ///
    /// # Errors
    /// Fails when `name` is no file name.
    pub fn path(root: &Path, name: &str) -> Result<PathBuf, SnapshotError> {
        let mut parts = Path::new(name).components();
        match (parts.next(), parts.next()) {
            (Some(Component::Normal(_)), None) => {}
            _ => return Err(SnapshotError::Name(name.to_owned())),
        }
        Ok(root.join(WORK).join(DIRECTORY).join(name).with_extension(EXTENSION))
    }

    /// The snapshot `name`; `None` when there is none yet.
    ///
    /// # Errors
    /// Fails when the file is unreadable or holds no snapshot.
    pub fn read(root: &Path, name: &str) -> Result<Option<Self>, SnapshotError> {
        let path = Self::path(root, name)?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(SnapshotError::File(path, e)),
        };
        let value = json::parse(&text).map_err(|e| SnapshotError::Json(path.clone(), e))?;
        Self::from_data(&value).map(Some).map_err(|e| SnapshotError::Shape(path, e))
    }

    /// The entry of `key`.
    #[must_use]
    pub fn find(&self, key: &Key) -> Option<&Entry> {
        self.entries.iter().find(|e| &e.key == key)
    }

    /// Keeps `entries` as the snapshot `name`: they replace those of the same keys, and the
    /// entries of other keys that the snapshot held stay.
    ///
    /// # Errors
    /// Fails when the file cannot be read or written.
    pub fn write(entries: Vec<Entry>, root: &Path, name: &str) -> Result<PathBuf, SnapshotError> {
        let path = Self::path(root, name)?;
        let mut kept = Self::read(root, name)?.unwrap_or_default();
        kept.entries.retain(|old| !entries.iter().any(|new| new.key == old.key));
        kept.entries.extend(entries);
        let text = format!("{:#}\n", Json(&kept.to_data()));
        path.put(text).map_err(|e| SnapshotError::File(path.clone(), e))?;
        Ok(path)
    }
}
