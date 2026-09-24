//! Mirrors: URLs read through a scheme, holding an archive in a known format.

use std::fs;
use std::path::Path;

use vun_fs::{Edit, Tree};

use super::{Failure, Format};

/// A way of reading URLs that start with `prefix`.
pub struct Scheme {
    pub prefix: &'static str,
    /// Reads the resource: given the whole URL and what follows the prefix.
    pub read: fn(&str, &str) -> Result<Vec<u8>, Failure>,
}

impl Scheme {
    /// Files on this machine, by absolute path.
    pub const FILE: Self = Self {
        prefix: "file://",
        read: |_, path| fs::read(path).map_err(|e| Failure::Download(Box::new(e))),
    };

    /// The URL of the local file `path`.
    #[must_use]
    pub fn file_url(path: &Path) -> String {
        format!("{}{}", Self::FILE.prefix, path.display())
    }
}

/// The schemes URLs are read through: `file://` always, and those a program installs, such as
/// the network schemes of `vun-http`. Keeping network access out of this crate keeps its TLS
/// stack off the build path of everything that fetches packages.
pub struct Schemes {
    list: Vec<&'static Scheme>,
}

impl Default for Schemes {
    fn default() -> Self {
        Self { list: vec![&Scheme::FILE] }
    }
}

impl Schemes {
    /// Makes `schemes` read the URLs they start.
    pub fn install(&mut self, schemes: &'static [Scheme]) {
        self.list.extend(schemes);
    }

    /// The bytes at `url`, through the scheme that reads it.
    ///
    /// # Errors
    /// Returns that no scheme reads the URL, or why reading failed.
    pub fn get(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let scheme = self.of(url)?;
        (scheme.read)(url, &url[scheme.prefix.len()..])
    }

    /// The scheme that reads `url`.
    fn of(&self, url: &str) -> Result<&'static Scheme, Failure> {
        let found = self.list.iter().find(|s| url.starts_with(s.prefix)).copied();
        found.ok_or_else(|| Failure::Scheme(self.list.iter().map(|s| s.prefix).collect()))
    }
}

/// A URL whose scheme and archive format are known.
pub struct Mirror<'u> {
    url: &'u str,
    scheme: &'static Scheme,
    format: &'static Format,
}

impl<'u> Mirror<'u> {
    /// # Errors
    /// Returns which part of the URL no table knows.
    pub fn parse(url: &'u str, schemes: &Schemes) -> Result<Self, Failure> {
        let scheme = schemes.of(url)?;
        let path = url.split(['?', '#']).next().unwrap_or(url);
        let format = Format::of(path).ok_or(Failure::Format)?;
        Ok(Self { url, scheme, format })
    }

    /// Downloads the archive and unpacks it into `dir`, which must not exist; see
    /// [`Format::strip`].
    ///
    /// # Errors
    /// Returns why reading or unpacking failed.
    pub fn fetch(&self, dir: &Path) -> Result<(), Failure> {
        let bytes = (self.scheme.read)(self.url, &self.url[self.scheme.prefix.len()..])?;
        let raw = dir.with_extension("unpacking");
        let unpack = Failure::Unpack;
        raw.remove().map_err(unpack)?;
        self.format.unpack(&bytes, &raw).map_err(unpack)?;
        let entries = raw.entries().map_err(unpack)?;
        let root = match entries.as_slice() {
            [only] if self.format.strip && only.is_dir() => only.clone(),
            _ => raw.clone(),
        };
        dir.replace_with(&root).map_err(unpack)?;
        raw.remove().map_err(unpack)
    }
}
