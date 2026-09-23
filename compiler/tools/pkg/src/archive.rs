//! The package archive, `.ovpkg`: one zstd stream holding a header, the snapshot of the package's
//! files (their paths and hashes, from which the package hash is computed), then each file's bytes
//! in snapshot order. Archives carry no timestamps, owners or modes, so identical trees give
//! identical archives, and unpacking checks every file against the snapshot as it is written.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use overt_hash::{Hash, Snapshot};

pub struct Archive;

/// How an archive is malformed; carried as the source of an [`io::ErrorKind::InvalidData`].
#[derive(Debug)]
pub enum Malformed {
    /// Not an Overt package archive, or of an unknown version.
    Header,
    Index,
    /// An entry declares a size past the bound for its kind.
    TooLarge,
    Truncated,
    /// A file whose contents do not match its hash in the index, by path.
    Mismatch(String),
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header => f.write_str("not an Overt package archive of a known version"),
            Self::Index => f.write_str("malformed file index"),
            Self::TooLarge => f.write_str("an entry exceeds the size bound"),
            Self::Truncated => f.write_str("truncated archive"),
            Self::Mismatch(path) => write!(f, "`{path}` does not match its hash"),
        }
    }
}

impl std::error::Error for Malformed {}

impl Archive {
    pub const EXTENSION: &str = "ovpkg";
    const MAGIC: &[u8; 8] = b"overtpkg";
    const VERSION: u32 = 1;
    /// zstd level for publishing: archives are written once and read many times, and decoding
    /// speed does not depend on the level.
    const LEVEL: i32 = 19;
    /// log2 of the zstd window, so that content repeated far apart is found.
    const WINDOW_LOG: u32 = 27;
    /// Bounds on what an archive may declare, against a hostile mirror.
    const MAX_INDEX: u64 = 64 << 20;
    const MAX_FILE: u64 = 1 << 30;

    /// The archive of the files under `dir` named by `include`, and their snapshot.
    ///
    /// # Errors
    /// Returns an error if a file cannot be read or compression fails.
    pub fn pack(dir: &Path, include: &[String]) -> io::Result<(Vec<u8>, Snapshot)> {
        let snapshot = Snapshot::of(dir, include);
        let mut out = zstd::stream::write::Encoder::new(Vec::new(), Self::LEVEL)?;
        out.long_distance_matching(true)?;
        out.window_log(Self::WINDOW_LOG)?;
        let index = snapshot.to_string();
        out.write_all(Self::MAGIC)?;
        out.write_all(&Self::VERSION.to_le_bytes())?;
        Self::write_part(&mut out, index.as_bytes())?;
        for (path, _) in snapshot.files() {
            Self::write_part(&mut out, &fs::read(Self::join(dir, path))?)?;
        }
        Ok((out.finish()?, snapshot))
    }

    /// Unpacks `bytes` into `dir`, returning the snapshot every file was checked against.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed, a file does not match its hash, or writing
    /// fails.
    pub fn unpack(bytes: &[u8], dir: &Path) -> io::Result<Snapshot> {
        let mut input = zstd::stream::read::Decoder::new(bytes)?;
        input.window_log_max(Self::WINDOW_LOG)?;
        let mut magic = [0; 8];
        input.read_exact(&mut magic)?;
        let mut version = [0; 4];
        input.read_exact(&mut version)?;
        if &magic != Self::MAGIC || u32::from_le_bytes(version) != Self::VERSION {
            return Err(Self::invalid(Malformed::Header));
        }
        let index = Self::read_part(&mut input, Self::MAX_INDEX)?;
        let snapshot = String::from_utf8(index)
            .ok()
            .and_then(|text| Snapshot::parse(&text))
            .ok_or_else(|| Self::invalid(Malformed::Index))?;
        for (path, expected) in snapshot.files() {
            let contents = Self::read_part(&mut input, Self::MAX_FILE)?;
            if Hash::of(&contents) != expected {
                let path = path.join(&Snapshot::PATH_SEPARATOR.to_string());
                return Err(Self::invalid(Malformed::Mismatch(path)));
            }
            let target = Self::join(dir, path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(target, contents)?;
        }
        Ok(snapshot)
    }

    fn write_part(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
        out.write_all(&(bytes.len() as u64).to_le_bytes())?;
        out.write_all(bytes)
    }

    fn read_part(input: &mut impl Read, max: u64) -> io::Result<Vec<u8>> {
        let mut len = [0; 8];
        input.read_exact(&mut len)?;
        let len = u64::from_le_bytes(len);
        if len > max {
            return Err(Self::invalid(Malformed::TooLarge));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or_default());
        input.take(len).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != len {
            return Err(Self::invalid(Malformed::Truncated));
        }
        Ok(bytes)
    }

    fn join(dir: &Path, components: &[String]) -> PathBuf {
        components.iter().fold(dir.to_path_buf(), |p, c| p.join(c))
    }

    fn invalid(reason: Malformed) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, reason)
    }
}
