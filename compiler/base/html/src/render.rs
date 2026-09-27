//! Rendering at run time: anything that writes itself into a document.

use crate::escape::Escape;

/// Anything that can be written into a document. Text escapes itself; markup written as it is
/// is spelled [`crate::Raw`].
pub trait Render {
    fn render(&self, out: &mut String);

    /// This alone, as a string.
    fn html(&self) -> String {
        let mut out = String::new();
        self.render(&mut out);
        out
    }
}

impl Render for () {
    fn render(&self, _: &mut String) {}
}

impl Render for str {
    fn render(&self, out: &mut String) {
        Escape::Text.write(out, self);
    }
}

impl Render for String {
    fn render(&self, out: &mut String) {
        self.as_str().render(out);
    }
}

impl Render for char {
    fn render(&self, out: &mut String) {
        self.encode_utf8(&mut [0; 4]).render(out);
    }
}

impl<T: Render + ?Sized> Render for &T {
    fn render(&self, out: &mut String) {
        (**self).render(out);
    }
}

impl<T: Render + ?Sized> Render for Box<T> {
    fn render(&self, out: &mut String) {
        (**self).render(out);
    }
}

impl<T: Render> Render for Option<T> {
    fn render(&self, out: &mut String) {
        if let Some(inner) = self {
            inner.render(out);
        }
    }
}

impl<T: Render> Render for [T] {
    fn render(&self, out: &mut String) {
        for item in self {
            item.render(out);
        }
    }
}

impl<T: Render, const N: usize> Render for [T; N] {
    fn render(&self, out: &mut String) {
        self.as_slice().render(out);
    }
}

impl<T: Render> Render for Vec<T> {
    fn render(&self, out: &mut String) {
        self.as_slice().render(out);
    }
}

/// Every item of an iterator, in order. The iterator is cloned on each render, since a
/// document renders from a shared reference and may render more than once.
#[derive(Clone, Copy, Debug)]
pub struct Each<I>(pub I);

impl<T: Render, I: Iterator<Item = T> + Clone> Render for Each<I> {
    fn render(&self, out: &mut String) {
        for item in self.0.clone() {
            item.render(out);
        }
    }
}

/// Integers render as decimal, which needs no escaping.
macro_rules! integers {
    ($($int:ty)*) => {$(
        impl Render for $int {
            fn render(&self, out: &mut String) {
                use std::fmt::Write as _;
                let _ = write!(out, "{self}");
            }
        }
    )*};
}

integers!(u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize);

/// Siblings render in order, which is how an element takes several children without a
/// container around them.
macro_rules! siblings {
    ($(($($name:ident)+))*) => {$(
        impl<$($name: Render),+> Render for ($($name,)+) {
            fn render(&self, out: &mut String) {
                #[expect(non_snake_case, reason = "each sibling is named as its type")]
                let ($($name,)+) = self;
                $($name.render(out);)+
            }
        }
    )*};
}

siblings!((A) (A B) (A B C) (A B C D) (A B C D E) (A B C D E F) (A B C D E F G) (A B C D E F G H)
    (A B C D E F G H I) (A B C D E F G H I J) (A B C D E F G H I J K) (A B C D E F G H I J K L));
