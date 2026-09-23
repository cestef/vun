//! Archive formats, by the suffix of the URL or file: Overt's own, and the ones forges and vendors
//! publish. Each reads and writes, so that `overt pkg pack` can publish in any of them.

use std::io::{self, BufReader, Write};
use std::path::Path;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use overt_hash::Snapshot;

use crate::{Archive, Contents, Tar, Zip, Zstd};

pub struct Format {
    /// The first is what `overt pkg pack` names the archives it writes.
    pub suffixes: &'static [&'static str],
    /// Unpacks an archive into a directory; entries escaping it are refused by the readers.
    pub(crate) unpack: fn(&[u8], &Path) -> io::Result<()>,
    pub(crate) pack: fn(&Contents<'_>) -> io::Result<Vec<u8>>,
    /// Whether an archive whose only entry is a directory has that directory as root, as the
    /// source archives of forges do, and as packing writes them.
    pub strip: bool,
}

pub const FORMATS: &[Format] = &[
    Archive::FORMAT,
    Format {
        suffixes: &[".tar.zst", ".tzst"],
        unpack: |bytes, dir| Tar::unpack(Zstd::reader(bytes)?, dir),
        pack: |contents| Ok(Zstd::compress(&Tar::pack(contents)?)),
        strip: true,
    },
    Format {
        suffixes: &[".tar.xz", ".txz"],
        unpack: |bytes, dir| {
            let mut tar = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(bytes), &mut tar).map_err(io::Error::other)?;
            Tar::unpack(tar.as_slice(), dir)
        },
        pack: |contents| {
            let mut xz = Vec::new();
            lzma_rs::xz_compress(&mut Tar::pack(contents)?.as_slice(), &mut xz)?;
            Ok(xz)
        },
        strip: true,
    },
    Format {
        suffixes: &[".tar.gz", ".tgz"],
        unpack: |bytes, dir| Tar::unpack(GzDecoder::new(bytes), dir),
        pack: |contents| {
            // No file name and a zero timestamp in the header, so that the bytes follow the tree.
            let mut gz = GzEncoder::new(Vec::new(), Compression::best());
            gz.write_all(&Tar::pack(contents)?)?;
            gz.finish()
        },
        strip: true,
    },
    Format {
        suffixes: &[".tar"],
        unpack: |bytes, dir| Tar::unpack(bytes, dir),
        pack: Tar::pack,
        strip: true,
    },
    Format { suffixes: &[".zip"], unpack: Zip::unpack, pack: Zip::pack, strip: true },
];

impl Format {
    /// The format whose suffix ends `path`.
    #[must_use]
    pub fn of(path: &str) -> Option<&'static Self> {
        FORMATS.iter().find(|f| f.suffixes.iter().any(|s| path.ends_with(s)))
    }

    /// The suffix of the archives it writes.
    #[must_use]
    pub fn suffix(&self) -> &'static str {
        self.suffixes[0]
    }

    pub(crate) fn unpack(&self, bytes: &[u8], dir: &Path) -> io::Result<()> {
        (self.unpack)(bytes, dir)
    }

    /// The archive of the files under `dir` named by `include`, under the directory `top` in
    /// formats that wrap a tree in one, and the snapshot the files were read with.
    ///
    /// # Errors
    /// Returns an error if a file cannot be read.
    pub fn pack(&self, dir: &Path, include: &[String], top: &str) -> io::Result<(Vec<u8>, Snapshot)> {
        let snapshot = Snapshot::of(dir, include);
        let bytes = (self.pack)(&Contents { dir, snapshot: &snapshot, top })?;
        Ok((bytes, snapshot))
    }
}
