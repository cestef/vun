//! Attributes: a name from the vocabulary and a value, text, present alone, or a hole.

/// An attribute name. A struct of constants, as [`crate::Tag`] is: [`Name::custom`] covers what
/// the vocabulary does not, such as `data-*` attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name(&'static str);

impl Name {
    #[must_use]
    pub const fn custom(spelled: &'static str) -> Self {
        Self(spelled)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }

    /// This attribute with the text `value`, escaped when written.
    #[must_use]
    pub const fn is(self, text: &str) -> Attr<'_> {
        Attr { name: self, value: Value::Text(text) }
    }

    /// This attribute present alone, as a boolean attribute is: `<input required>`.
    #[must_use]
    pub const fn on(self) -> Attr<'static> {
        Attr { name: self, value: Value::On }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'a> {
    On,
    Text(&'a str),
    /// Filled when a template is rendered.
    Hole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attr<'a> {
    pub name: Name,
    pub value: Value<'a>,
}

/// The vocabulary: a constant per attribute, spelled as the document spells it, `-` written `_`
/// and keywords raw (`r#type`, `r#for`).
macro_rules! names {
    ($($name:ident $(= $spelled:literal)?),* $(,)?) => {
        $(
            pub const $name: Name = Name(names!(@spell $name $($spelled)?));
        )*

        /// Every attribute of the vocabulary.
        pub const ALL: &[Name] = &[$($name),*];
    };
    (@spell $name:ident $spelled:literal) => { $spelled };
    (@spell $name:ident) => { stringify!($name) };
}

#[expect(non_upper_case_globals, reason = "attributes are spelled as the document spells them")]
mod vocabulary {
    use super::Name;

    names! {
        id, class, style, title, lang, dir, hidden, tabindex, role,
        href, src, alt, rel, target, download, media, sizes, srcset, width, height, loading,
        name, content, charset, http_equiv = "http-equiv",
        r#type = "type", value, placeholder, required, disabled, checked, selected, readonly,
        multiple, min, max, step, pattern, autocomplete, autofocus, action, method,
        r#for = "for", form, colspan, rowspan, open, datetime, defer, r#async = "async", integrity,
        crossorigin, aria_label = "aria-label", aria_hidden = "aria-hidden",
        aria_current = "aria-current",
    }
}

pub use vocabulary::*;
