//! A URI persists as its text and is parsed again when read.

use vun_codec::{Decode, Encode, Malformed, Reader, Writer};

use crate::{Kind, UriOf};

impl<K: Kind> Encode for UriOf<K> {
    fn encode(&self, w: &mut Writer<'_>) {
        self.as_str().encode(w);
    }
}

impl<K: Kind> Decode for UriOf<K> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        String::decode(r)?.parse().map_err(|_| Malformed::Invariant)
    }
}
