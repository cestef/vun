//! The records ELF lays out, each declared once with its fields' order in each class
//! ([`record!`]), from which it is written, read, and measured.

use vun_bytes::Out;

use crate::Class;
use crate::input::{Input, Malformed};

/// A field of a record: its width in a class, and its bytes there, little-endian.
pub(crate) trait Field: Sized {
    fn width(class: Class) -> u64;
    fn put(self, class: Class, out: &mut Out);
    fn take(class: Class, input: &mut Input<'_>) -> Result<Self, Malformed>;
}

/// An address, an offset or a size: 32 bits in `ELFCLASS32`, 64 in `ELFCLASS64`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Word(pub(crate) u64);

/// A signed word: a relocation's addend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SignedWord(pub(crate) i64);

macro_rules! fixed {
    ($($ty:ty),*) => {$(
        impl Field for $ty {
            fn width(_: Class) -> u64 {
                size_of::<$ty>() as u64
            }

            fn put(self, _: Class, out: &mut Out) {
                out.bytes(&self.to_le_bytes());
            }

            fn take(_: Class, input: &mut Input<'_>) -> Result<Self, Malformed> {
                Ok(<$ty>::from_le_bytes(input.array()?))
            }
        }
    )*};
}

fixed!(u8, u16, u32);

impl<const N: usize> Field for [u8; N] {
    fn width(_: Class) -> u64 {
        N as u64
    }

    fn put(self, _: Class, out: &mut Out) {
        out.bytes(&self);
    }

    fn take(_: Class, input: &mut Input<'_>) -> Result<Self, Malformed> {
        input.array()
    }
}

impl Field for Word {
    fn width(class: Class) -> u64 {
        class.word()
    }

    /// # Panics
    /// Panics on a value past 32 bits in `ELFCLASS32`.
    fn put(self, class: Class, out: &mut Out) {
        match class {
            Class::Elf32 => out.u32(u32::try_from(self.0).expect("a 32-bit word")),
            Class::Elf64 => out.u64(self.0),
        }
    }

    fn take(class: Class, input: &mut Input<'_>) -> Result<Self, Malformed> {
        Ok(Self(match class {
            Class::Elf32 => u64::from(u32::take(class, input)?),
            Class::Elf64 => u64::from_le_bytes(input.array()?),
        }))
    }
}

impl Field for SignedWord {
    fn width(class: Class) -> u64 {
        class.word()
    }

    /// # Panics
    /// Panics on a value past 32 bits in `ELFCLASS32`.
    fn put(self, class: Class, out: &mut Out) {
        match class {
            Class::Elf32 => out.bytes(&i32::try_from(self.0).expect("a 32-bit word").to_le_bytes()),
            Class::Elf64 => out.bytes(&self.0.to_le_bytes()),
        }
    }

    fn take(class: Class, input: &mut Input<'_>) -> Result<Self, Malformed> {
        Ok(Self(match class {
            Class::Elf32 => i64::from(i32::from_le_bytes(input.array()?)),
            Class::Elf64 => i64::from_le_bytes(input.array()?),
        }))
    }
}

/// Declares a record: its fields, then their order in each class where the classes differ.
macro_rules! record {
    (
        $(#[$doc:meta])*
        struct $name:ident { $($field:ident: $ty:ty,)* }
    ) => {
        record! {
            $(#[$doc])*
            struct $name { $($field: $ty,)* }
            elf32 [$($field),*]
            elf64 [$($field),*]
        }
    };
    (
        $(#[$doc:meta])*
        struct $name:ident { $($field:ident: $ty:ty,)* }
        elf32 [$($in32:ident),*]
        elf64 [$($in64:ident),*]
    ) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub(crate) struct $name { $(pub(crate) $field: $ty,)* }

        impl $name {
            /// Its size in `class`.
            pub(crate) fn size(class: $crate::Class) -> u64 {
                0 $(+ <$ty as $crate::record::Field>::width(class))*
            }

            /// Appends it as `class` lays it out.
            pub(crate) fn put(self, class: $crate::Class, out: &mut vun_bytes::Out) {
                use $crate::record::Field;
                match class {
                    $crate::Class::Elf32 => { $(self.$in32.put(class, out);)* }
                    $crate::Class::Elf64 => { $(self.$in64.put(class, out);)* }
                }
            }

            /// Reads it as `class` lays it out.
            pub(crate) fn take(
                class: $crate::Class,
                input: &mut $crate::input::Input<'_>,
            ) -> Result<Self, $crate::input::Malformed> {
                use $crate::record::Field;
                let mut record = Self::default();
                match class {
                    $crate::Class::Elf32 => { $(record.$in32 = Field::take(class, input)?;)* }
                    $crate::Class::Elf64 => { $(record.$in64 = Field::take(class, input)?;)* }
                }
                Ok(record)
            }
        }
    };
}
