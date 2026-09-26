//! Symbols persist by their text, written once per stream and interned again when read.

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::Symbol;

/// The process's symbols, as the table [`Symbol`] handles are into; the pool itself is global.
#[derive(Default)]
pub struct Symbols;

impl Pooled for Symbol {
    const POOL: &'static str = "symbol";
    type Table = Symbols;

    fn write(self, _: &Symbols, w: &mut Writer<'_>) {
        self.as_str().encode(w);
    }

    fn read(_: &Symbols, r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let len = usize::decode(r)?;
        let text = std::str::from_utf8(r.take(len)?).map_err(|_| Malformed::Text)?;
        Ok(Self::intern(text))
    }
}

vun_codec::pooled!(Symbol);
