//! The encodings of standard types: unsigned integers as LEB128 varints, signed ones zigzagged
//! first, text and sequences prefixed by their length, options and booleans by a tag byte.

use std::sync::Arc;

use crate::{Decode, Encode, Input, Malformed};

/// Bits carried by each varint byte; the high bit says another byte follows.
const VARINT_BITS: u32 = 7;
const VARINT_MORE: u8 = 0x80;

/// LEB128: seven bits per byte, low first.
impl Encode for u128 {
    fn encode(&self, out: &mut Vec<u8>) {
        let mut n = *self;
        loop {
            let low = u8::try_from(n & u128::from(!VARINT_MORE)).expect("masked to seven bits");
            n >>= VARINT_BITS;
            if n == 0 {
                out.push(low);
                return;
            }
            out.push(low | VARINT_MORE);
        }
    }
}

impl Decode for u128 {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        let mut n: u128 = 0;
        for shift in (0..u128::BITS).step_by(VARINT_BITS as usize) {
            let byte = input.front(1)?[0];
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
            fn encode(&self, out: &mut Vec<u8>) {
                u128::from(*self).encode(out);
            }
        }

        impl Decode for $ty {
            fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
                <$ty>::try_from(u128::decode(input)?).map_err(|_| Malformed::Overflow)
            }
        }
    )*};
}
unsigned!(u16, u32, u64);

impl Encode for usize {
    fn encode(&self, out: &mut Vec<u8>) {
        (*self as u128).encode(out);
    }
}

impl Decode for usize {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        Self::try_from(u128::decode(input)?).map_err(|_| Malformed::Overflow)
    }
}

/// Zigzag: small magnitudes of either sign stay short.
impl Encode for i128 {
    fn encode(&self, out: &mut Vec<u8>) {
        ((self << 1) ^ (self >> (i128::BITS - 1))).cast_unsigned().encode(out);
    }
}

impl Decode for i128 {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        let n = u128::decode(input)?;
        Ok((n >> 1).cast_signed() ^ -(n & 1).cast_signed())
    }
}

impl Encode for u8 {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(*self);
    }
}

impl Decode for u8 {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        Ok(input.front(1)?[0])
    }
}

impl Encode for bool {
    fn encode(&self, out: &mut Vec<u8>) {
        u8::from(*self).encode(out);
    }
}

impl Decode for bool {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        match u8::decode(input)? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Malformed::Tag(tag.into())),
        }
    }
}

impl<const N: usize> Encode for [u8; N] {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self);
    }
}

impl<const N: usize> Decode for [u8; N] {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        Ok(input.front(N)?.try_into().expect("taken N bytes"))
    }
}

impl Encode for str {
    fn encode(&self, out: &mut Vec<u8>) {
        self.len().encode(out);
        out.extend_from_slice(self.as_bytes());
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Vec<u8>) {
        self.as_str().encode(out);
    }
}

impl Decode for String {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        let len = usize::decode(input)?;
        let bytes = input.front(len)?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| Malformed::Text)
    }
}

impl<T: Encode> Encode for [T] {
    fn encode(&self, out: &mut Vec<u8>) {
        self.len().encode(out);
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, out: &mut Vec<u8>) {
        self.as_slice().encode(out);
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        let len = usize::decode(input)?;
        // Each item takes at least one byte, so a length past the input is malformed.
        if len > input.len() {
            return Err(Malformed::Truncated);
        }
        (0..len).map(|_| T::decode(input)).collect()
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, out: &mut Vec<u8>) {
        self.is_some().encode(out);
        if let Some(value) = self {
            value.encode(out);
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        if bool::decode(input)? { T::decode(input).map(Some) } else { Ok(None) }
    }
}

impl<T: Encode + ?Sized> Encode for Arc<T> {
    fn encode(&self, out: &mut Vec<u8>) {
        (**self).encode(out);
    }
}

impl<T: Decode> Decode for Arc<T> {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        T::decode(input).map(Self::new)
    }
}

impl Decode for Arc<str> {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        String::decode(input).map(Self::from)
    }
}

macro_rules! tuple {
    ($($name:ident),*) => {
        impl<$($name: Encode),*> Encode for ($($name,)*) {
            #[allow(non_snake_case, reason = "the bindings are named as the type parameters")]
            fn encode(&self, out: &mut Vec<u8>) {
                let ($($name,)*) = self;
                $($name.encode(out);)*
            }
        }

        impl<$($name: Decode),*> Decode for ($($name,)*) {
            fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
                Ok(($($name::decode(input)?,)*))
            }
        }
    };
}
tuple!(A, B);
tuple!(A, B, C);
