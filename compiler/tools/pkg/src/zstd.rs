//! zstd streams in pure Rust (ruzstd): read with any window up to its bound, written at the
//! fastest level ruzstd implements, about zstd's level 1. Contents are verified by their BLAKE3
//! hashes, so the frame checksum is not checked.

use std::io::{self, Read};

use ruzstd::decoding::StreamingDecoder;
use ruzstd::encoding::{CompressionLevel, compress_to_vec};

pub struct Zstd;

impl Zstd {
    /// The decompressed stream of the frame `bytes` starts with.
    ///
    /// # Errors
    /// Returns an error if the frame header does not read.
    pub fn reader(bytes: &[u8]) -> io::Result<impl Read + '_> {
        StreamingDecoder::new(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    #[must_use]
    pub fn compress(bytes: &[u8]) -> Vec<u8> {
        compress_to_vec(bytes, CompressionLevel::Fastest)
    }
}
