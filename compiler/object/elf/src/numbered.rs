//! Values ELF numbers, each declared with its number once.

/// Declares an enum of values ELF numbers, each value's number written once, and `Other` for a
/// number it does not name: `number` gives a value's number, `of` a number's value.
macro_rules! numbered {
    ($(#[$attr:meta])* $vis:vis enum $name:ident: $int:ty {
        $($(#[$doc:meta])* $variant:ident = $number:literal,)*
    }) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($(#[$doc])* $variant,)*
            Other($int),
        }

        impl $name {
            #[must_use]
            pub const fn number(self) -> $int {
                match self {
                    $(Self::$variant => $number,)*
                    Self::Other(number) => number,
                }
            }

            #[must_use]
            pub const fn of(number: $int) -> Self {
                match number {
                    $($number => Self::$variant,)*
                    number => Self::Other(number),
                }
            }
        }
    };
}
