//! Element names, and what a browser does with each one's content.

/// How an element's content is treated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Content is markup, and the element has an end tag.
    Normal,
    /// No content and no end tag.
    Void,
    /// Content is not markup, so it is written as it is and never escaped: only
    /// [`crate::Raw`] belongs in one.
    Verbatim,
}

/// An element name. A struct of constants rather than an enum, so that [`Tag::custom`] leaves room
/// for custom elements the vocabulary below does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag {
    name: &'static str,
    kind: Kind,
}

impl Tag {
    /// A custom element, which has content and an end tag.
    #[must_use]
    pub const fn custom(name: &'static str) -> Self {
        Self { name, kind: Kind::Normal }
    }

    /// The name as written in the document.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn kind(self) -> Kind {
        self.kind
    }
}

/// The vocabulary: a constant per element, spelled as the document spells it.
macro_rules! tags {
    ($($kind:ident: $($name:ident)*;)*) => {
        $($(
            #[doc = concat!("`<", stringify!($name), ">`.")]
            pub const $name: Tag = Tag { name: stringify!($name), kind: Kind::$kind };
        )*)*

        /// Every element of the vocabulary.
        pub const ALL: &[Tag] = &[$($($name,)*)*];
    };
}

#[expect(non_upper_case_globals, reason = "elements are spelled as the document spells them")]
mod vocabulary {
    use super::{Kind, Tag};

    tags! {
        Normal: html head title body header footer nav main section article aside h1 h2 h3 h4 h5 h6
            p div span a em strong small code pre kbd samp var sub sup b i u s q blockquote ul ol li
            dl dt dd table thead tbody tfoot tr th td caption figure figcaption details summary
            form label button select option textarea fieldset legend time mark abbr cite dfn
            del ins output progress meter template slot svg picture video audio canvas iframe object;
        Void: area base br col embed hr img input link meta source track wbr;
        Verbatim: script style;
    }
}

pub use vocabulary::*;
