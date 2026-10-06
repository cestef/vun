//! A message: a value with the handles it holds, written by content so that either side of the
//! boundary reads it in its own tables.

use vun_codec::{Context, Decode, Encode, Malformed, Pools, Writer};

#[derive(Encode, Decode)]
pub struct Wire {
    bytes: Vec<u8>,
    pools: Pools,
}

impl Wire {
    /// The message of `value`.
    #[must_use]
    pub fn seal<T: Encode + ?Sized>(value: &T) -> Vec<u8> {
        let mut writer = Writer::new();
        let bytes = writer.encode(value);
        Self { bytes, pools: writer.into_pools() }.encoded()
    }

    /// The value of `message`, its handles recorded in this process's tables.
    ///
    /// # Errors
    /// Returns why the bytes are not a message of a `T`.
    pub fn open<T: Decode>(message: &[u8]) -> Result<T, Malformed> {
        let Self { bytes, pools } = Self::decoded(message)?;
        Context::new(pools).read(&bytes)
    }
}
