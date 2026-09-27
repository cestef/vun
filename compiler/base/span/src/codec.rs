//! A file persists by its key, the path shown and its text, written once per stream and added
//! again when read: diagnostics read from other processes still show their source.

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::{ByteRange, FileId, FileRef, SourceFiles, SourceMap, Span};

impl Pooled for FileId {
    const POOL: &'static str = "file";

    fn write(self, w: &mut Writer<'_>) {
        let files = SourceFiles::global();
        let file = files.file(self);
        (files.key(self), file.path.as_str(), file.text.as_str()).encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (key, path, text) = <(FileRef, String, String)>::decode(r)?;
        Ok(SourceFiles::global().add_as(key, path, text))
    }
}

vun_codec::pooled!(FileId);

impl Decode for ByteRange {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (lo, hi) = <(u32, u32)>::decode(r)?;
        (lo <= hi).then_some(Self { lo, hi }).ok_or(Malformed::Invariant)
    }
}

/// A span read lies within its file's text, on character boundaries.
impl Decode for Span {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (file, range) = <(FileId, ByteRange)>::decode(r)?;
        let text = &SourceFiles::global().file(file).text;
        let within = [range.lo, range.hi]
            .iter()
            .all(|&at| usize::try_from(at).is_ok_and(|at| text.is_char_boundary(at)));
        within.then_some(Self { file, range }).ok_or(Malformed::Invariant)
    }
}
