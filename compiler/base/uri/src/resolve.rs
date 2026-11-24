//! Resolving a reference against a base URI (RFC 3986, section 5.2): the URI it names from there.

use std::borrow::Cow;

use crate::compose::{Composer, Pieces};
use crate::error::Invalid;
use crate::path::Path;
use crate::percent::Encoded;
use crate::query::Query;
use crate::uri::{Kind, Reference, Uri, UriOf};

impl Uri {
    /// The URI `reference` names from this one, by the strict algorithm of section 5.2.2.
    #[must_use]
    pub fn resolve<K: Kind>(&self, reference: &UriOf<K>) -> Uri {
        let (base, r) = (self, reference);
        let mut out = Composer::default();
        let base_text = base.as_str();
        let r_text = r.as_str();
        let r_parts = r.parts();
        let (authority, path, query): (Option<Pieces<'_>>, Cow<'_, str>, Option<&str>) =
            if r_parts.scheme.is_some() || r_parts.authority.is_some() {
                let authority = r_parts.authority.as_ref().map(|a| Pieces::of(r_text, a));
                (authority, r.path().without_dots(), r.query().map(Query::as_str))
            } else {
                let authority = base.parts().authority.as_ref().map(|a| Pieces::of(base_text, a));
                if r.path().is_empty() {
                    let query = r.query().or_else(|| base.query()).map(Query::as_str);
                    (authority, Cow::Borrowed(base.path().as_str()), query)
                } else if r.path().is_absolute() {
                    (authority, r.path().without_dots(), r.query().map(Query::as_str))
                } else {
                    let merged = Path::merge(base.path().as_str(), authority.is_some(), r.path().as_str());
                    let path = Cow::Owned(Path::remove_dots(&merged).into_owned());
                    (authority, path, r.query().map(Query::as_str))
                }
            };
        let scheme = r_parts.scheme.map_or(base.scheme().as_str(), |p| p.of(r_text));
        out.scheme(scheme);
        if let Some(pieces) = authority {
            out.authority(pieces);
        }
        out.path(&path);
        out.query(query);
        out.fragment(r.fragment().map(Encoded::as_str));
        out.finish()
    }

    /// The URI the reference `text` names from this one.
    ///
    /// # Errors
    /// Why `text` is not a reference.
    pub fn join(&self, text: &str) -> Result<Uri, Invalid> {
        Ok(self.resolve(&text.parse::<Reference>()?))
    }
}

impl Reference {
    /// The URI this reference names from `base`.
    #[must_use]
    pub fn resolved(&self, base: &Uri) -> Uri {
        base.resolve(self)
    }
}
