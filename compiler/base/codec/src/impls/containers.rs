//! Text and sequences, prefixed by their length; options, results and tuples, by their parts.

use std::sync::Arc;

use crate::{Decode, Encode, Malformed, Reader, Writer};

impl Encode for str {
    fn encode(&self, w: &mut Writer<'_>) {
        self.len().encode(w);
        w.write(self.as_bytes());
    }
}

impl Encode for String {
    fn encode(&self, w: &mut Writer<'_>) {
        self.as_str().encode(w);
    }
}

impl Decode for String {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let len = usize::decode(r)?;
        let bytes = r.take(len)?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| Malformed::Text)
    }
}

impl<T: Encode> Encode for [T] {
    fn encode(&self, w: &mut Writer<'_>) {
        self.len().encode(w);
        for item in self {
            item.encode(w);
        }
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, w: &mut Writer<'_>) {
        self.as_slice().encode(w);
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let len = usize::decode(r)?;
        // Every encoding takes at least one byte, so a length past the input is malformed.
        if len > r.remaining() {
            return Err(Malformed::Truncated);
        }
        (0..len).map(|_| T::decode(r)).collect()
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, w: &mut Writer<'_>) {
        self.is_some().encode(w);
        if let Some(value) = self {
            value.encode(w);
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        if bool::decode(r)? { T::decode(r).map(Some) } else { Ok(None) }
    }
}

impl<T: Encode, E: Encode> Encode for Result<T, E> {
    fn encode(&self, w: &mut Writer<'_>) {
        self.is_ok().encode(w);
        match self {
            Ok(value) => value.encode(w),
            Err(error) => error.encode(w),
        }
    }
}

impl<T: Decode, E: Decode> Decode for Result<T, E> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(if bool::decode(r)? { Ok(T::decode(r)?) } else { Err(E::decode(r)?) })
    }
}

/// References and owning pointers encode as what they point to.
impl<T: Encode + ?Sized> Encode for &T {
    fn encode(&self, w: &mut Writer<'_>) {
        (**self).encode(w);
    }
}

macro_rules! pointer {
    ($($ptr:ident),*) => {$(
        impl<T: Encode + ?Sized> Encode for $ptr<T> {
            fn encode(&self, w: &mut Writer<'_>) {
                (**self).encode(w);
            }
        }

        impl<T: Decode> Decode for $ptr<T> {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                T::decode(r).map(Self::new)
            }
        }

        impl<T: Decode> Decode for $ptr<[T]> {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                Vec::decode(r).map(Self::from)
            }
        }

        impl Decode for $ptr<str> {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                String::decode(r).map(Self::from)
            }
        }
    )*};
}
pointer!(Arc, Box);

impl<T: ?Sized> Encode for std::marker::PhantomData<T> {
    fn encode(&self, _: &mut Writer<'_>) {}
}

impl<T: ?Sized> Decode for std::marker::PhantomData<T> {
    fn decode(_: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(Self)
    }
}

macro_rules! tuple {
    ($($name:ident),*) => {
        impl<$($name: Encode),*> Encode for ($($name,)*) {
            #[allow(non_snake_case, reason = "the bindings are named as the type parameters")]
            fn encode(&self, w: &mut Writer<'_>) {
                let ($($name,)*) = self;
                $($name.encode(w);)*
            }
        }

        impl<$($name: Decode),*> Decode for ($($name,)*) {
            fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
                Ok(($($name::decode(r)?,)*))
            }
        }
    };
}
tuple!(A);
tuple!(A, B);
tuple!(A, B, C);
tuple!(A, B, C, D);
