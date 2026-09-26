//! An origin persists by what it records, written once per stream with the origins it names, and
//! recorded again when read: its anchored identity, and so its hash, is the same in both processes.

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::{Origin, OriginId, OriginTable};

impl Pooled for OriginId {
    const POOL: &'static str = "origin";

    fn write(self, w: &mut Writer<'_>) {
        OriginTable::global().get(self).encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(OriginTable::global().push(Origin::decode(r)?))
    }
}

vun_codec::pooled!(OriginId);
