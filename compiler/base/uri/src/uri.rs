//! A URI or a URI reference: its text as written and where each component lies in it. Whether a
//! scheme is required is part of the type: `Uri` (`Uri`) has one, a `Reference`
//! (`Reference`) may be relative.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use crate::authority::{Authority, Host};
use crate::error::{Invalid, Why};
use crate::parts::Parts;
use crate::path::Path;
use crate::percent::Encoded;
use crate::query::Query;
use crate::scheme::Scheme;

mod sealed {
    pub trait Sealed {}
}

/// Which references a `Uri` holds.
pub trait Kind: sealed::Sealed + 'static {
    const SCHEME: bool;
}

/// URIs, with a scheme.
#[derive(Debug)]
pub enum Absolute {}

/// Any reference, relative ones included.
#[derive(Debug)]
pub enum Any {}

impl sealed::Sealed for Absolute {}
impl sealed::Sealed for Any {}

impl Kind for Absolute {
    const SCHEME: bool = true;
}

impl Kind for Any {
    const SCHEME: bool = false;
}

/// A URI (RFC 3986, section 3) with `Absolute`, a URI reference (section 4.1) with `Any`.
pub struct UriOf<K: Kind> {
    text: String,
    parts: Parts,
    kind: PhantomData<fn() -> K>,
}

/// A URI: a reference with a scheme.
pub type Uri = UriOf<Absolute>;

/// A URI reference: a URI, or a reference relative to one.
pub type Reference = UriOf<Any>;

impl<K: Kind> UriOf<K> {
    pub(crate) fn from_parts(text: String, parts: Parts) -> Self {
        Self { text, parts, kind: PhantomData }
    }

    pub(crate) const fn parts(&self) -> &Parts {
        &self.parts
    }

    /// # Errors
    /// Why `text` is not a reference, or, for a `Uri`, that it is relative.
    pub fn parse(text: impl Into<String>) -> Result<Self, Invalid> {
        let text = text.into();
        let parts = Parts::parse(&text)?;
        if K::SCHEME && parts.scheme.is_none() {
            return Err(Invalid::new(Why::Relative, 0));
        }
        Ok(Self::from_parts(text, parts))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.text
    }

    #[must_use]
    pub fn authority(&self) -> Option<Authority<'_>> {
        self.parts.authority.map(|parts| Authority::new(&self.text, parts))
    }

    #[must_use]
    pub fn host(&self) -> Option<Host<'_>> {
        self.authority().map(|a| a.host())
    }

    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.authority()?.port()
    }

    #[must_use]
    pub fn path(&self) -> Path<'_> {
        Path(Encoded(self.parts.path.of(&self.text)))
    }

    #[must_use]
    pub fn query(&self) -> Option<Query<'_>> {
        self.parts.query.map(|p| Query(Encoded(p.of(&self.text))))
    }

    #[must_use]
    pub fn fragment(&self) -> Option<Encoded<'_>> {
        self.parts.fragment.map(|p| Encoded(p.of(&self.text)))
    }

    /// The scheme, when there is one.
    fn scheme_part(&self) -> Option<Scheme<'_>> {
        self.parts.scheme.map(|p| Scheme(p.of(&self.text)))
    }

    /// This reference without its fragment, as a request sends it.
    #[must_use]
    pub fn without_fragment(&self) -> Self {
        let Some(fragment) = self.parts.fragment else { return self.clone() };
        // The `#` before the fragment goes with it.
        let text = self.text[..fragment.start() - 1].to_owned();
        Self::from_parts(text, Parts { fragment: None, ..self.parts })
    }

    /// Checks that the recorded parts are those the text parses into.
    ///
    /// # Errors
    /// Why the text does not parse, or that its parts differ.
    pub fn verify(&self) -> Result<(), Invalid> {
        let parts = Parts::parse(&self.text)?;
        if parts != self.parts || (K::SCHEME && parts.scheme.is_none()) {
            return Err(Invalid::new(Why::Parts, 0));
        }
        Ok(())
    }

    /// This URI as a reference.
    #[must_use]
    pub fn into_reference(self) -> Reference {
        UriOf::from_parts(self.text, self.parts)
    }
}

impl Uri {
    #[must_use]
    pub fn scheme(&self) -> Scheme<'_> {
        // Parsing and composing a `Uri` record a scheme always.
        Scheme(self.parts.scheme.unwrap_or_default().of(&self.text))
    }
}

impl Reference {
    #[must_use]
    pub fn scheme(&self) -> Option<Scheme<'_>> {
        self.scheme_part()
    }

    /// Whether it has no scheme, and so names nothing until resolved against a base.
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        self.parts.scheme.is_none()
    }
}

impl TryFrom<Reference> for Uri {
    type Error = Invalid;

    /// The reference as a URI, when it has a scheme.
    fn try_from(reference: Reference) -> Result<Self, Invalid> {
        if reference.is_relative() {
            return Err(Invalid::new(Why::Relative, 0));
        }
        Ok(UriOf::from_parts(reference.text, reference.parts))
    }
}

impl From<Uri> for Reference {
    fn from(uri: Uri) -> Self {
        uri.into_reference()
    }
}

impl<K: Kind> Clone for UriOf<K> {
    fn clone(&self) -> Self {
        Self::from_parts(self.text.clone(), self.parts)
    }
}

/// References compare as written; `normalized` makes equivalent ones equal.
impl<K: Kind> PartialEq for UriOf<K> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl<K: Kind> Eq for UriOf<K> {}

impl<K: Kind> Hash for UriOf<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl<K: Kind> PartialOrd for UriOf<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Kind> Ord for UriOf<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.text.cmp(&other.text)
    }
}

impl<K: Kind> fmt::Debug for UriOf<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.text, f)
    }
}

impl<K: Kind> fmt::Display for UriOf<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl<K: Kind> AsRef<str> for UriOf<K> {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl<K: Kind> FromStr for UriOf<K> {
    type Err = Invalid;

    fn from_str(text: &str) -> Result<Self, Invalid> {
        Self::parse(text)
    }
}

impl<K: Kind> TryFrom<&str> for UriOf<K> {
    type Error = Invalid;

    fn try_from(text: &str) -> Result<Self, Invalid> {
        Self::parse(text)
    }
}

impl<K: Kind> TryFrom<String> for UriOf<K> {
    type Error = Invalid;

    fn try_from(text: String) -> Result<Self, Invalid> {
        Self::parse(text)
    }
}
