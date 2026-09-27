//! Templates: a document baked by the Rust compiler into one string, with the places of its
//! holes. Rendering copies the text between holes and renders each fill, so its cost follows the
//! number of holes, not the size of the page. The number of holes is part of the type, so a
//! fill of another length does not compile.

use crate::escape::Escape;
use crate::node::{Cursor, Cut, Doc};
use crate::render::Render;

/// A document baked into buffers of exactly its size. Built by [`crate::template!`], which sizes
/// it from the document.
#[doc(hidden)]
pub struct Baked<const N: usize, const H: usize> {
    pub bytes: [u8; N],
    pub cuts: [Cut; H],
}

impl<const N: usize, const H: usize> Baked<N, H> {
    /// # Panics
    /// At compile time, if the document is not `N` bytes long with `H` holes.
    #[must_use]
    pub const fn of(doc: &Doc<'_>) -> Self {
        let mut bytes = [0; N];
        let mut cuts = [Cut { at: 0, within: Escape::Text }; H];
        let mut cursor = Cursor { bytes: &mut bytes, cuts: &mut cuts, len: 0, holes: 0 };
        cursor.nodes(doc.0);
        assert!(cursor.len == N && cursor.holes == H, "a template is baked at the size measured");
        Self { bytes, cuts }
    }
}

/// A baked document with `H` holes.
pub struct Template<const H: usize> {
    text: &'static str,
    cuts: [Cut; H],
}

impl<const H: usize> Template<H> {
    /// # Panics
    /// At compile time, if `bytes` is not UTF-8, which a document of `&str` always is.
    #[must_use]
    pub const fn new(bytes: &'static [u8], cuts: [Cut; H]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self { text, cuts },
            Err(_) => panic!("a baked document is UTF-8"),
        }
    }

    /// The baked text, holes closed up.
    #[must_use]
    pub const fn text(&self) -> &'static str {
        self.text
    }

    /// Appends the document to `out`, its holes filled in document order by the items of
    /// `fill`: text escaped as text, and in an attribute value as the value.
    pub fn render(&self, out: &mut String, fill: &impl Fill<H>) {
        let mut from = 0;
        for (nth, cut) in self.cuts.iter().enumerate() {
            out.push_str(&self.text[from..cut.at]);
            let item = fill.nth(nth);
            match cut.within {
                Escape::Text => item.render(out),
                // Rendering escapes as text; a value also needs its quotes escaped.
                Escape::Attribute => Escape::Attribute.widen(Escape::Text, out, &item.html()),
            }
            from = cut.at;
        }
        out.push_str(&self.text[from..]);
    }

    /// The document with its holes filled, as a string.
    #[must_use]
    pub fn html(&self, fill: &impl Fill<H>) -> String {
        let mut out = String::with_capacity(self.text.len());
        self.render(&mut out, fill);
        out
    }
}

/// What fills a template's `H` holes: a tuple of as many items, each rendering itself.
pub trait Fill<const H: usize> {
    /// The item filling hole `nth`.
    fn nth(&self, nth: usize) -> &dyn Render;
}

/// A tuple of `n` renderable items fills `n` holes.
macro_rules! fills {
    ($($count:literal: ($($name:ident $at:tt)*))*) => {$(
        impl<$($name: Render),*> Fill<$count> for ($($name,)*) {
            fn nth(&self, nth: usize) -> &dyn Render {
                let items: [&dyn Render; $count] = [$(&self.$at),*];
                items[nth]
            }
        }
    )*};
}

fills! {
    1: (A 0)
    2: (A 0 B 1)
    3: (A 0 B 1 C 2)
    4: (A 0 B 1 C 2 D 3)
    5: (A 0 B 1 C 2 D 3 E 4)
    6: (A 0 B 1 C 2 D 3 E 4 F 5)
    7: (A 0 B 1 C 2 D 3 E 4 F 5 G 6)
    8: (A 0 B 1 C 2 D 3 E 4 F 5 G 6 H 7)
}

impl<const H: usize> Template<H> {}

impl Render for Template<0> {
    /// A template without holes is its text.
    fn render(&self, out: &mut String) {
        out.push_str(self.text);
    }
}
