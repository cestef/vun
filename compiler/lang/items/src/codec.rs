//! A handle persists as its path of symbols, and reads back as an equal handle: the path is interned, so
//! reading it again stores nothing more.

use vun_codec::{Decode, Encode, Malformed, Reader, Writer};
use vun_intern::{Pool, Symbol};

use crate::Item;

/// The paths of handles read back.
static PATHS: Pool<Vec<&'static str>> = Pool::new();

impl Encode for Item {
    fn encode(&self, w: &mut Writer<'_>) {
        let path: Vec<Symbol> = self.path.iter().copied().map(Symbol::intern).collect();
        path.encode(w);
    }
}

impl Decode for Item {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let path = Vec::<Symbol>::decode(r)?.into_iter().map(Symbol::as_str).collect();
        Ok(Self::new(PATHS.intern(path).get()))
    }
}
