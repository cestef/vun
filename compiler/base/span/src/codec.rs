//! A file persists by its key, the path shown and its text, written once per stream and added
//! again when read: diagnostics read from other processes still show their source.

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::{FileId, SourceFiles, SourceMap};

impl Pooled for FileId {
    const POOL: &'static str = "file";
    type Table = SourceFiles;

    fn write(self, files: &SourceFiles, w: &mut Writer<'_>) {
        let file = files.file(self);
        (files.key(self), file.path.as_str(), file.text.as_str()).encode(w);
    }

    fn read(files: &SourceFiles, r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (key, path, text) = <(String, String, String)>::decode(r)?;
        Ok(files.add_as(&key, path, text))
    }
}

vun_codec::pooled!(FileId);
