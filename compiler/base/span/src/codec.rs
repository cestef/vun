//! A file persists by its key, the path shown and its text, written once per stream and added
//! again when read: diagnostics read from other processes still show their source.

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::{FileId, SourceFiles, SourceMap};

impl Pooled for FileId {
    const POOL: &'static str = "file";

    fn write(self, w: &mut Writer<'_>) {
        let files = SourceFiles::global();
        let file = files.file(self);
        (files.key(self), file.path.as_str(), file.text.as_str()).encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (key, path, text) = <(String, String, String)>::decode(r)?;
        Ok(SourceFiles::global().add_as(&key, path, text))
    }
}

vun_codec::pooled!(FileId);
