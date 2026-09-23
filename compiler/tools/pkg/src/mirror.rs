//! Mirrors: URLs read through a scheme, holding an archive in a known format.

use std::fs;
use std::path::Path;

use overt_fs::{Edit, Tree};

use super::{Failure, Format};

/// The largest archive read, against a mirror serving without end.
const MAX_ARCHIVE: u64 = 4 << 30;

pub struct Scheme {
    pub prefix: &'static str,
    /// Reads the resource: given the whole URL and what follows the prefix.
    read: fn(&str, &str) -> Result<Vec<u8>, Failure>,
}

pub const SCHEMES: &[Scheme] = &[
    Scheme { prefix: "https://", read: http },
    Scheme { prefix: "http://", read: http },
    Scheme { prefix: "file://", read: |_, path| fs::read(path).map_err(|e| Failure::Download(Box::new(e))) },
];

fn http(url: &str, _: &str) -> Result<Vec<u8>, Failure> {
    let failed = |e: ureq::Error| Failure::Download(Box::new(e));
    let mut response = ureq::get(url).call().map_err(failed)?;
    response.body_mut().with_config().limit(MAX_ARCHIVE).read_to_vec().map_err(failed)
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
    pub fn parse(url: &'u str) -> Result<Self, Failure> {
        let scheme = SCHEMES.iter().find(|s| url.starts_with(s.prefix)).ok_or(Failure::Scheme)?;
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
