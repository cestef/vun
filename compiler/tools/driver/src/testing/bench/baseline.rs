//! Baselines (spec 11, section 5): the statistics of a run, kept in `.vun/bench/<name>.json` in
//! the package, which a later run compares with.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use vun_data::json::{self, Json};
use vun_data::{FromData, Mismatch, ToData};
use vun_fs::{Edit, WORK};

use super::{Measure, Statistics};

/// The directory of a package's baselines, in its working directory.
const DIRECTORY: &str = "bench";
/// What a baseline's file name ends with.
const EXTENSION: &str = "json";

/// The statistics of each benchmark, in the measure they were taken in.
#[derive(Clone, Debug, PartialEq, ToData, FromData)]
pub struct Baseline {
    pub measure: Measure,
    pub benchmarks: BTreeMap<String, Statistics>,
}

/// Why a baseline is not read or written.
#[derive(Debug)]
pub enum BaselineError {
    /// The name is no file name: it holds a separator or is empty.
    Name(String),
    File(PathBuf, io::Error),
    Json(PathBuf, json::Error),
    Shape(PathBuf, Mismatch),
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => write!(f, "`{name}` is no baseline name: write a name that is a file name"),
            Self::File(path, _) => write!(f, "the baseline `{}` is not read or written", path.display()),
            Self::Json(path, _) | Self::Shape(path, _) => {
                write!(f, "the baseline `{}` is no baseline", path.display())
            }
        }
    }
}

impl Error for BaselineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Name(_) => None,
            Self::File(_, source) => Some(source),
            Self::Json(_, source) => Some(source),
            Self::Shape(_, source) => Some(source),
        }
    }
}

impl Baseline {
    /// Where the baseline `name` of the package at `root` is kept.
    ///
    /// # Errors
    /// Fails when `name` is no file name.
    pub fn path(root: &Path, name: &str) -> Result<PathBuf, BaselineError> {
        let mut parts = Path::new(name).components();
        match (parts.next(), parts.next()) {
            (Some(Component::Normal(_)), None) => {}
            _ => return Err(BaselineError::Name(name.to_owned())),
        }
        Ok(root.join(WORK).join(DIRECTORY).join(name).with_extension(EXTENSION))
    }

    /// The baseline `name` of the package at `root`; `None` when there is none yet.
    ///
    /// # Errors
    /// Fails when the file is unreadable or holds no baseline.
    pub fn read(root: &Path, name: &str) -> Result<Option<Self>, BaselineError> {
        let path = Self::path(root, name)?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(BaselineError::File(path, e)),
        };
        let value = json::parse(&text).map_err(|e| BaselineError::Json(path.clone(), e))?;
        Self::from_data(&value).map(Some).map_err(|e| BaselineError::Shape(path, e))
    }

    /// Keeps these as the baseline `name` of the package at `root`, the benchmarks it kept that
    /// these do not measure again with them.
    ///
    /// # Errors
    /// Fails when the file cannot be read or written.
    pub fn write(mut self, root: &Path, name: &str) -> Result<PathBuf, BaselineError> {
        let path = Self::path(root, name)?;
        if let Some(kept) = Self::read(root, name)?.filter(|kept| kept.measure == self.measure) {
            for (benchmark, statistics) in kept.benchmarks {
                self.benchmarks.entry(benchmark).or_insert(statistics);
            }
        }
        let text = format!("{:#}\n", Json(&self.to_data()));
        path.put(text).map_err(|e| BaselineError::File(path.clone(), e))?;
        Ok(path)
    }
}
