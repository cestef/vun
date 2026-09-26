//! Symbols and interned values persist by content, written once per stream and interned again
//! when read.

use std::hash::Hash;

use vun_codec::{Decode, Encode, Malformed, Pooled, Reader, Writer};

use crate::{Interned, Pool, Symbol};

impl Pooled for Symbol {
    const POOL: &'static str = "symbol";

    fn write(self, w: &mut Writer<'_>) {
        self.as_str().encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let len = usize::decode(r)?;
        let text = std::str::from_utf8(r.take(len)?).map_err(|_| Malformed::Text)?;
        Ok(Self::intern(text))
    }
}

vun_codec::pooled!(Symbol);

/// A type whose values are interned in one pool of the process, found by the type.
pub trait Internable: Hash + Eq + Send + Sync + Encode + Decode + 'static {
    /// Names the pool of its values in a stream.
    const POOL: &'static str;

    fn pool() -> &'static Pool<Self>;
}

/// An interned value persists by its content, written once per stream and interned again when
/// read.
impl<T: Internable> Pooled for Interned<T> {
    const POOL: &'static str = T::POOL;

    fn write(self, w: &mut Writer<'_>) {
        (*self).encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(T::pool().intern(T::decode(r)?))
    }
}

impl<T: Internable> Encode for Interned<T> {
    fn encode(&self, w: &mut Writer<'_>) {
        w.pooled(*self);
    }
}

impl<T: Internable> Decode for Interned<T> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        r.pooled()
    }
}
