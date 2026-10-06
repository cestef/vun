//! Mirrors: URLs read through a scheme, holding an archive in a known format.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_fs::{Edit, Tree};
use vun_uri::{Known, Uri};

use super::{Failure, Format};

/// The suffix of the directory an archive is unpacked in.
const UNPACKING: &str = ".unpacking";

/// A way of reading the URLs of a scheme.
pub trait Scheme: Send + Sync {
    /// The scheme's name, compared without case.
    fn name(&self) -> &str;

    /// Reads the resource `uri` names.
    ///
    /// # Errors
    /// Returns why it cannot be read.
    fn read(&self, uri: &Uri) -> Result<Vec<u8>, Failure>;
}

/// Files on this machine, by their file URIs (RFC 8089).
pub struct File;

impl Scheme for File {
    fn name(&self) -> &str {
        Known::FILE.name
    }

    fn read(&self, uri: &Uri) -> Result<Vec<u8>, Failure> {
        let path = uri.to_file_path().map_err(|e| Failure::Download(Box::new(e)))?;
        fs::read(path).map_err(|e| Failure::Download(Box::new(e)))
    }
}

/// The schemes URLs are read through: `file` always, and those a program installs, such as
/// the network schemes of `vun-http`. Keeping network access out of this crate keeps its TLS
/// stack off the build path of everything that fetches packages.
#[derive(Clone)]
pub struct Schemes {
    list: Vec<Arc<dyn Scheme>>,
}

impl Default for Schemes {
    fn default() -> Self {
        Self { list: vec![Arc::new(File)] }
    }
}

impl Schemes {
    /// Makes `scheme` read the URLs it names, unless a scheme of its name does.
    pub fn install(&mut self, scheme: Arc<dyn Scheme>) {
        if self.list.iter().all(|s| !s.name().eq_ignore_ascii_case(scheme.name())) {
            self.list.push(scheme);
        }
    }

    /// The bytes at `uri`, through the scheme that reads it.
    ///
    /// # Errors
    /// Returns that no scheme reads it, or why reading failed.
    pub fn get(&self, uri: &Uri) -> Result<Vec<u8>, Failure> {
        self.of(uri)?.read(uri)
    }

    /// The names of the schemes, in the order they are tried.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.list.iter().map(|s| s.name().to_owned()).collect()
    }

    /// The scheme that reads `uri`.
    fn of(&self, uri: &Uri) -> Result<Arc<dyn Scheme>, Failure> {
        let found = self.list.iter().find(|s| uri.scheme().is(s.name())).cloned();
        found.ok_or_else(|| Failure::Scheme(self.names()))
    }
}

/// A URL whose scheme and archive format are known.
pub struct Mirror {
    uri: Uri,
    scheme: Arc<dyn Scheme>,
    format: &'static Format,
}

impl Mirror {
    /// # Errors
    /// Returns which part of `uri` no table knows.
    pub fn of(uri: &Uri, schemes: &Schemes) -> Result<Self, Failure> {
        let scheme = schemes.of(uri)?;
        let format = Format::of(uri.path().as_str()).ok_or(Failure::Format)?;
        Ok(Self { uri: uri.clone(), scheme, format })
    }

    /// Where an archive fetched into `dir` is unpacked before it is moved there.
    #[must_use]
    pub fn unpacking(dir: &Path) -> PathBuf {
        let mut name = dir.as_os_str().to_owned();
        name.push(UNPACKING);
        PathBuf::from(name)
    }

    /// Downloads the archive and unpacks it into `dir`, which must not exist; see
    /// [`Format::strip`].
    ///
    /// # Errors
    /// Returns why reading or unpacking failed.
    pub fn fetch(&self, dir: &Path) -> Result<(), Failure> {
        let bytes = self.scheme.read(&self.uri)?;
        let raw = Self::unpacking(dir);
        let unpack = Failure::Unpack;
        raw.remove().map_err(unpack)?;
        self.format.unpack(&bytes, &raw).map_err(unpack)?;
        let entries = raw.entries().map_err(unpack)?;
        let directory = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
        let root = match entries.as_slice() {
            [only] if self.format.strip && directory(only) => only.clone(),
            _ => raw.clone(),
        };
        dir.replace_with(&root).map_err(unpack)?;
        raw.remove().map_err(unpack)
    }
}
