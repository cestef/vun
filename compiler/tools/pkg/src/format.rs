//! Archive formats, by the suffix of the URL: Overt's own, and the ones forges and vendors
//! publish.

use std::io::{self, BufReader, Cursor};
use std::path::Path;

use flate2::read::GzDecoder;

use crate::Archive;

pub struct Format {
    pub suffixes: &'static [&'static str],
    /// Unpacks an archive into a directory; entries escaping it are refused by the readers.
    unpack: fn(&[u8], &Path) -> io::Result<()>,
    /// Whether an archive whose only entry is a directory has that directory as root, as the
    /// source archives of forges do.
    pub strip: bool,
}

pub const FORMATS: &[Format] = &[
    Format {
        suffixes: &[".ovpkg"],
        unpack: |bytes, dir| Archive::unpack(bytes, dir).map(drop),
        strip: false,
    },
    Format {
        suffixes: &[".tar.zst", ".tzst"],
        unpack: |bytes, dir| tar::Archive::new(zstd::stream::read::Decoder::new(bytes)?).unpack(dir),
        strip: true,
    },
    Format {
        suffixes: &[".tar.xz", ".txz"],
        unpack: |bytes, dir| {
            let mut tar = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(bytes), &mut tar).map_err(io::Error::other)?;
            tar::Archive::new(tar.as_slice()).unpack(dir)
        },
        strip: true,
    },
    Format {
        suffixes: &[".tar.gz", ".tgz"],
        unpack: |bytes, dir| tar::Archive::new(GzDecoder::new(bytes)).unpack(dir),
        strip: true,
    },
    Format { suffixes: &[".tar"], unpack: |bytes, dir| tar::Archive::new(bytes).unpack(dir), strip: true },
    Format {
        suffixes: &[".zip"],
        unpack: |bytes, dir| {
            zip::ZipArchive::new(Cursor::new(bytes))
                .and_then(|mut z| z.extract(dir))
                .map_err(io::Error::other)
        },
        strip: true,
    },
];

impl Format {
    /// The format whose suffix ends `path`.
    #[must_use]
    pub fn of(path: &str) -> Option<&'static Self> {
        FORMATS.iter().find(|f| f.suffixes.iter().any(|s| path.ends_with(s)))
    }

    pub(crate) fn unpack(&self, bytes: &[u8], dir: &Path) -> io::Result<()> {
        (self.unpack)(bytes, dir)
    }
}
