//! The encodings of standard types: unsigned integers as LEB128 varints, signed ones zigzagged
//! first, text and sequences prefixed by their length, options and booleans by a tag byte.

mod containers;

use crate::{Decode, Encode, Malformed, Reader, Writer};

/// Bits carried by each varint byte; the high bit says another byte follows.
const VARINT_BITS: u32 = 7;
const VARINT_MORE: u8 = 0x80;

/// LEB128: seven bits per byte, low first.
impl Encode for u128 {
    fn encode(&self, w: &mut Writer<'_>) {
        let mut n = *self;
        loop {
            let low = u8::try_from(n & u128::from(!VARINT_MORE)).expect("masked to seven bits");
            n >>= VARINT_BITS;
            if n == 0 {
                w.byte(low);
                return;
            }
            w.byte(low | VARINT_MORE);
        }
    }
}

impl Decode for u128 {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let mut n: u128 = 0;
        for shift in (0..u128::BITS).step_by(VARINT_BITS as usize) {
            let byte = r.byte()?;
            let bits = u128::from(byte & !VARINT_MORE);
            n |= bits
                .checked_shl(shift)
                .filter(|shifted| shifted >> shift == bits)
                .ok_or(Malformed::Overflow)?;
            if byte & VARINT_MORE == 0 {
                return Ok(n);
            }
        }
        Err(Malformed::Overflow)
    }
}

macro_rules! unsigned {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            fn encode(&self, w: &mut Writer<'_>) {
                u128::from(*self).encode(w);
            }
        }

        impl Decode for $ty {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                <$ty>::try_from(u128::decode(r)?).map_err(|_| Malformed::Overflow)
            }
        }
    )*};
}
unsigned!(u16, u32, u64);

impl Encode for usize {
    fn encode(&self, w: &mut Writer<'_>) {
        (*self as u128).encode(w);
    }
}

impl Decode for usize {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Self::try_from(u128::decode(r)?).map_err(|_| Malformed::Overflow)
    }
}

/// Zigzag: small magnitudes of either sign stay short.
impl Encode for i128 {
    fn encode(&self, w: &mut Writer<'_>) {
        ((self << 1) ^ (self >> (i128::BITS - 1))).cast_unsigned().encode(w);
    }
}

impl Decode for i128 {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let n = u128::decode(r)?;
        Ok((n >> 1).cast_signed() ^ -(n & 1).cast_signed())
    }
}

macro_rules! signed {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            fn encode(&self, w: &mut Writer<'_>) {
                i128::from(*self).encode(w);
            }
        }

        impl Decode for $ty {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                <$ty>::try_from(i128::decode(r)?).map_err(|_| Malformed::Overflow)
            }
        }
    )*};
}
signed!(i8, i16, i32, i64);

impl Encode for u8 {
    fn encode(&self, w: &mut Writer<'_>) {
        w.byte(*self);
    }
}

impl Decode for u8 {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        r.byte()
    }
}

impl Encode for bool {
    fn encode(&self, w: &mut Writer<'_>) {
        u8::from(*self).encode(w);
    }
}

impl Decode for bool {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        match u8::decode(r)? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Malformed::Tag(tag.into())),
        }
    }
}

impl<const N: usize> Encode for [u8; N] {
    fn encode(&self, w: &mut Writer<'_>) {
        w.write(self);
    }
}

impl<const N: usize> Decode for [u8; N] {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(r.take(N)?.try_into().expect("taken N bytes"))
    }
}
